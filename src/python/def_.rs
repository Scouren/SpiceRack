//! The definition API in Python (`spicerack.def_`): `DialectDef`, whose
//! methods are the Rust calls 1:1 (same names, same arguments, each
//! returning the definition for chaining), `register`, `Lang`, and the
//! write hooks' `Card` and `Answer` (`DIALECT-PLUGIN.md` §2, §4, §6.3). A
//! Python hook is an external hook of the definition: the writer returns
//! a request per hooked card (`render::render_pending`), [`render`] calls
//! each hook once per deck with its cards and hands the answers to
//! `render::splice`. The vocabulary is `def_enums`; the `Dialect` base
//! class and `register_dialect` are Python (`spicerack/dialects.py`).
//!
//! Cites: none.
//!
//! Contents:
//! - `pub(super) struct PyDialectDef`, `pub(super) struct PyLang`, `pub(super) struct PyCard`,
//!   `pub(super) struct PyAnswer`, `From<PyLang> for Lang`
//! - `pub(super) fn create_def_module(m: &Bound<'_, PyModule>) -> PyResult<()>`
//! - `pub(super) fn plugin(name: &str) -> Option<Lang>`, `pub(super) fn parse_lang(name: &str) -> Result<Lang, String>`,
//!   `pub(super) fn builtin_of(lang: Lang) -> Dialect`
//! - `pub(super) fn render(src: &Source, lang: Lang) -> Result<String, String>`

use std::sync::Mutex;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use super::def_enums::*;
use crate::syntax::def::{Answer, DialectDef, register as def_register};
use crate::syntax::render::{self, Pending, Source};
use crate::syntax::{Dialect, Lang};

#[pyclass(name = "DialectDef", module = "spicerack.def_")]
pub(super) struct PyDialectDef {
    def: DialectDef,
    /// Its name (`new`, `renamed`): a registration of a held name makes
    /// no new row.
    name: String,
    /// Its Python hooks by head, in `hook` order: held by the hook table
    /// only once registered as a new row.
    hooks: Vec<(String, Py<PyAny>)>,
}

#[pyclass(name = "Lang", module = "spicerack.def_", eq, frozen)]
#[derive(Clone, Copy, PartialEq)]
pub(super) struct PyLang(pub(super) Lang);

#[pyclass(name = "Card", module = "spicerack.def_", get_all, frozen)]
pub(super) struct PyCard {
    head: String,
    words: Vec<String>,
    params: Vec<(String, String)>,
    text: String,
}

#[pyclass(name = "Answer", module = "spicerack.def_", frozen)]
#[derive(Clone)]
pub(super) struct PyAnswer {
    text: Option<String>,
    refuse: Option<String>,
}

/// The Python hooks, by the id their definition's `external_hook` holds.
/// A hook is added when its definition registers a new row and never
/// freed, as the row is never freed: one leak per registration with a
/// hook, bounded by `LANGS_MAX` rows.
static HOOKS: Mutex<Vec<Py<PyAny>>> = Mutex::new(Vec::new());

/// Generates `PyDialectDef`'s methods: each Rust call of the same name,
/// its arguments converted (`From<Py<Enum>>`), the definition returned.
macro_rules! calls {
    ($($name:ident($($a:ident: $t:ty),*);)* @extra { $($extra:tt)* }) => {
        #[pymethods]
        impl PyDialectDef {
            $(
                fn $name<'py>(mut slf: PyRefMut<'py, Self>, $($a: $t),*) -> PyRefMut<'py, Self> {
                    let d = std::mem::replace(&mut slf.def, DialectDef::new(""));
                    slf.def = d.$name($($a.into()),*);
                    slf
                }
            )*
            $($extra)*
        }
    };
}

/// Adds the `def_` submodule (and `spicerack.def_` in `sys.modules`).
pub(super) fn create_def_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    let d = PyModule::new(py, "def_")?;
    add_enums(&d)?;
    d.add_class::<PyDialectDef>()?;
    d.add_class::<PyLang>()?;
    d.add_class::<PyCard>()?;
    d.add_class::<PyAnswer>()?;
    d.add_function(wrap_pyfunction!(register, &d)?)?;
    d.add("Keyword", py.get_type::<PyTag>())?;
    m.add_submodule(&d)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("spicerack.def_", &d)
}

/// The plugin registered as `name` (a language past the built-ins).
pub(super) fn plugin(name: &str) -> Option<Lang> {
    Lang::from_name(name).filter(|l| l.index() as usize >= Lang::BUILTIN_COUNT)
}

/// A dialect name: a built-in's (aliases too), else a registered plugin's.
///
/// # Errors
/// No language is registered as `name`.
pub(super) fn parse_lang(name: &str) -> Result<Lang, String> {
    match name.parse::<Dialect>() {
        Ok(d) => Ok(d.into()),
        Err(e) => plugin(name).ok_or(e),
    }
}

/// The built-in in `lang`'s `extends` chain (ngspice for a root plugin):
/// whose library paths and analysis forms a Python deck takes.
pub(super) fn builtin_of(lang: Lang) -> Dialect {
    let mut chain = std::iter::successors(Some(lang), |l| l.base());
    let own = |l: Lang| Dialect::ALL.into_iter().find(|&d| Lang::from(d) == l);
    chain.find_map(own).unwrap_or(Dialect::Ngspice)
}

/// `src` written for `lang`, its Python hooks called (once per hook per
/// deck), or the refusal's messages.
///
/// # Errors
/// The deck is refused; a hook raises, answers a different number of
/// cards than it was given, or refuses with a code `lang` did not declare.
pub(super) fn render(src: &Source, lang: Lang) -> Result<String, String> {
    let messages = |e: render::Diags| e.messages.trim_end().to_string();
    let p = render::render_pending(src, lang).map_err(messages)?;
    let answers = match p.requests.is_empty() {
        true => Vec::new(),
        false => Python::with_gil(|py| answers(py, &p, lang))?,
    };
    render::splice(src, p, &answers)
        .map(|w| w.text)
        .map_err(messages)
}

impl From<PyLang> for Lang {
    fn from(p: PyLang) -> Lang {
        p.0
    }
}

// ---- private ----

calls! {
    title(v: bool);
    fold_case(v: bool);
    comment_line(s: &str);
    comment_col1(b: u8);
    blank_led_comment(v: bool);
    cont_skip(s: &str);
    comment_tail(s: &str);
    block_comment(v: bool);
    long_strings(v: bool);
    dollar(v: PyDollar);
    plus(v: bool);
    backslash(v: PyBackslash);
    stop(v: Option<u8>);
    join(v: PyJoin);
    quote(v: PyQuote);
    string(v: PyQuoting);
    string_expr(v: bool);
    name_escape(v: bool);
    ident_start(bytes: &str);
    ident_continue(bytes: &str);
    directive(v: Option<u8>);
    head_words(v: bool);
    head_after(t: PyTag);
    operators(table: PyOperators);
    operator(text: &str, t: PyTag);
    expr_braces(v: bool);
    caret(v: PyCaret);
    infix(t: PyTag, bp: u8);
    right(t: PyTag);
    prefix(t: PyTag);
    prefix_bp(v: u8);
    probe(name: &str);
    index(v: bool);
    lib_sections(v: bool);
    exponent(letters: &str);
    scale(text: &str, s: PyScale);
    tail(v: PyTail);
    arithmetic(v: PyArithmetic);
    keywords(table: PyKeywords);
    keyword(word: &str, t: PyTag);
    named(v: bool);
    devices(table: PyDevices);
    master(word: &str, s: PyStatement);
    analysis(word: &str, a: PyAnalysis);
    region(name: &str, lang: PyLang);
    comment(s: &str);
    path_quote(s: &str);
    expr(open: &str, close: &str);
    func(v: PyFuncForm);
    subckt_params(word: Option<&str>);
    ngspice_numbers(v: bool);
    write_lexicon(lang: PyLang);
    native(c: PyConstruct);
    spell(c: PyConstruct, alt: PyAlt);
    lift(c: PyConstruct, rule: PyRule);
    refuse(c: PyConstruct, code: PyCode);
    omit(c: PyConstruct, code: PyCode);
    copy(c: PyConstruct, code: PyCode);
    writer(v: PyWriter);
    caret_power(v: bool);
    hier_sep(v: char);
    number_reading(v: PyNumberReading);
    model_semantics(v: PyModelSemantics);
    source_rules(v: PySourceRules);
    card_rules(v: PyCardRules);
    named_rules(v: PyNamedRules);
    options_form(v: PyOptionsForm);
    load_form(v: PyLoadForm);
    control_block(v: PyControlBlock);
    analysis_form(v: PyAnalysisForm);
    end_rule(v: PyEndRule);
    value_quoting(v: PyValueQuoting);
    coupling(v: PyCoupling);
    param_form(v: PyParamForm);
    include_scope(v: PyIncludeScope);
    named_params(v: PyNamedParams);
    analysis_masters(lang: PyLang);
    expr_eval(v: PyExprEval);
    parasitics(v: PyParasitics);
    netlist_rules(v: PyNetlistRules);
    subckt_multiplicity(v: bool);
    text_family(lang: PyLang);
    spectre_analysis(kind: &str);
    code(id: &str, kind: PyKind, message: &str);
    refuse_code(c: PyConstruct, id: &str);
    omit_code(c: PyConstruct, id: &str);
    write_head(t: PyTag, text: &str);
    write_master(word: &str, text: &str);
    option(spice: &str, own: &str);
    runs_on(sim: PySimulator);
    regions_registered(v: bool);
    @extra {
        #[new]
        fn new(name: &str) -> Self {
            Self { def: DialectDef::new(name), name: name.into(), hooks: Vec::new() }
        }

        /// `DialectDef::renamed`.
        fn renamed<'py>(mut slf: PyRefMut<'py, Self>, name: &str) -> PyRefMut<'py, Self> {
            let d = std::mem::replace(&mut slf.def, DialectDef::new(""));
            slf.def = d.renamed(name);
            slf.name = name.into();
            slf
        }

        /// `DialectDef::extends`: the base's tables, its hooks among them,
        /// replace every earlier call's, this definition's hooks too.
        fn extends<'py>(mut slf: PyRefMut<'py, Self>, base: PyLang) -> PyRefMut<'py, Self> {
            let d = std::mem::replace(&mut slf.def, DialectDef::new(""));
            slf.def = d.extends(base.into());
            slf.hooks.clear();
            slf
        }

        /// `DialectDef::end`: `End(col1, then_blank, stops)` or `None`.
        fn end<'py>(mut slf: PyRefMut<'py, Self>, v: Option<PyEnd>) -> PyRefMut<'py, Self> {
            let d = std::mem::replace(&mut slf.def, DialectDef::new(""));
            slf.def = d.end(v.map(|e| e.0));
            slf
        }

        /// `DialectDef::device`: `word` names the element `like` names.
        ///
        /// # Errors
        /// No device row is spelled `like`.
        fn device<'py>(mut slf: PyRefMut<'py, Self>, word: &str, like: &str) -> PyResult<PyRefMut<'py, Self>> {
            let d = std::mem::replace(&mut slf.def, DialectDef::new(""));
            let held = d.clone();
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || d.device(word, like))) {
                Ok(d) => slf.def = d,
                Err(_) => {
                    slf.def = held;
                    return Err(PyValueError::new_err(format!("device: no row spelled '{like}'")));
                }
            }
            Ok(slf)
        }

        /// `DialectDef::hook`: the cards headed `head` are answered by
        /// `f(cards) -> [Answer]`, called once per deck. Its table row is
        /// held now (an id is given at `register`).
        fn hook<'py>(mut slf: PyRefMut<'py, Self>, head: &str, f: Py<PyAny>) -> PyRefMut<'py, Self> {
            let d = std::mem::replace(&mut slf.def, DialectDef::new(""));
            slf.def = d.external_hook(head, u16::MAX);
            match slf.hooks.iter_mut().find(|h| h.0 == head) {
                Some(h) => h.1 = f,
                None => slf.hooks.push((head.into(), f)),
            }
            slf
        }
    }
}

#[pyfunction]
/// Registers `d` (a copy: `d` stays usable): its `Lang`. Its hooks are
/// held from here, once, when it makes a new row.
///
/// # Errors
/// `ValueError` with `RegisterError`'s message.
fn register(py: Python<'_>, d: PyRef<'_, PyDialectDef>) -> PyResult<PyLang> {
    let mut held = HOOKS.lock().unwrap_or_else(|p| p.into_inner());
    let mut def = d.def.clone();
    for (k, (head, _)) in d.hooks.iter().enumerate() {
        let id = u16::try_from(held.len() + k).expect("fewer than 65535 hooks");
        assert!(id < u16::MAX, "u16::MAX is the unregistered hook");
        def = def.external_hook(head, id);
    }
    let fresh = Lang::from_name(&d.name).is_none();
    let lang = def_register(def).map_err(|e| PyValueError::new_err(e.to_string()))?;
    if fresh {
        held.extend(d.hooks.iter().map(|h| h.1.clone_ref(py)));
    }
    Ok(PyLang(lang))
}

#[pymethods]
impl PyLang {
    #[classattr]
    const NGSPICE: PyLang = PyLang(Lang::NGSPICE);
    #[classattr]
    const ESPICE: PyLang = PyLang(Lang::ESPICE);
    #[classattr]
    const HSPICE: PyLang = PyLang(Lang::HSPICE);
    #[classattr]
    const XYCE: PyLang = PyLang(Lang::XYCE);
    #[classattr]
    const LTSPICE: PyLang = PyLang(Lang::LTSPICE);
    #[classattr]
    const SPECTRE: PyLang = PyLang(Lang::SPECTRE);
    #[classattr]
    const VACASK: PyLang = PyLang(Lang::VACASK);
    #[classattr]
    const CDL: PyLang = PyLang(Lang::CDL);
    #[classattr]
    const VERILOG: PyLang = PyLang(Lang::VERILOG);
    #[classattr]
    const NGSPICE_GIVEN: PyLang = PyLang(Lang::NGSPICE_GIVEN);

    /// The language registered as `name`, or `None`.
    #[staticmethod]
    fn from_name(name: &str) -> Option<PyLang> {
        Lang::from_name(name).map(PyLang)
    }

    fn name(&self) -> &'static str {
        self.0.name()
    }

    /// `Lang::dump`: the row as canonical text.
    fn dump(&self) -> String {
        self.0.dump()
    }

    /// The simulator that reads its text, `None` for netlist only.
    fn runs_on(&self) -> Option<PySimulator> {
        self.0.runs_on().map(PySimulator)
    }

    fn __repr__(&self) -> String {
        format!("Lang({:?})", self.0.name())
    }
}

#[pymethods]
impl PyAnswer {
    /// The card's text instead of the writer's.
    #[staticmethod]
    #[pyo3(name = "Text")]
    fn text(s: String) -> Self {
        Self {
            text: Some(s),
            refuse: None,
        }
    }

    /// The deck refused with the definition's code `id` (`W0201`).
    #[staticmethod]
    #[pyo3(name = "Refuse")]
    fn refuse(id: String) -> Self {
        Self {
            text: None,
            refuse: Some(id),
        }
    }

    fn __repr__(&self) -> String {
        match (&self.text, &self.refuse) {
            (Some(t), _) => format!("Answer.Text({t:?})"),
            (_, Some(id)) => format!("Answer.Refuse({id:?})"),
            _ => unreachable!("an answer is one of the two"),
        }
    }
}

/// `p`'s answers: each hook called once with its requests' cards.
fn answers(py: Python<'_>, p: &Pending, lang: Lang) -> Result<Vec<Answer>, String> {
    let mut out: Vec<Option<Answer>> = vec![None; p.requests.len()];
    let mut ids: Vec<u16> = p.requests.iter().map(|r| r.hook).collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let mine: Vec<usize> = (0..p.requests.len())
            .filter(|&i| p.requests[i].hook == id)
            .collect();
        let cards = mine.iter().map(|&i| {
            let r = &p.requests[i];
            card(&p.text[r.span.start as usize..r.span.end as usize])
        });
        let cards: Vec<PyCard> = cards.collect();
        let f = HOOKS.lock().unwrap_or_else(|e| e.into_inner())[usize::from(id)].clone_ref(py);
        let head = &cards[0].head.clone();
        let got = f
            .call1(py, (cards,))
            .map_err(|e| format!("hook '{head}': {e}"))?;
        let got: Vec<PyAnswer> = got.extract(py).map_err(|e| format!("hook '{head}': {e}"))?;
        if got.len() != mine.len() {
            return Err(format!(
                "hook '{head}': {} answers for {} cards",
                got.len(),
                mine.len()
            ));
        }
        for (&i, a) in mine.iter().zip(got) {
            out[i] = Some(answer(a, lang)?);
        }
    }
    Ok(out
        .into_iter()
        .map(|a| a.expect("every request answered"))
        .collect())
}

/// A written card's view: continuation lines joined by one space.
fn card(text: &str) -> PyCard {
    let lines = text.lines().map(|l| {
        let l = l.trim();
        l.strip_prefix('+')
            .unwrap_or(l)
            .trim_end_matches('\\')
            .trim()
    });
    let joined = lines.collect::<Vec<_>>().join(" ");
    let (head, words, params) = render::card_fields(&joined);
    PyCard {
        head: head.into(),
        words: words.into_iter().map(String::from).collect(),
        params: params
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
        text: joined.clone(),
    }
}

/// `a` as the writer's answer: a refusal names a code `lang` declared.
fn answer(a: PyAnswer, lang: Lang) -> Result<Answer, String> {
    match (a.text, a.refuse) {
        (Some(t), _) => Ok(Answer::Text(t)),
        (_, Some(id)) => {
            let codes = lang.row().codes.iter();
            let held = codes.map(|c| c.0).find(|&c| c == id);
            held.map(Answer::Refuse).ok_or_else(|| {
                format!(
                    "a hook refuses with '{id}', a code {} did not declare",
                    lang.name()
                )
            })
        }
        _ => unreachable!("an answer is one of the two"),
    }
}
