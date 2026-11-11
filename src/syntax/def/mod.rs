//! The dialect-definition API: a [`DialectDef`] is the calls that declare
//! a language (its lexicon rows, its spelling, a capability cell per
//! construct, its behaviours), and [`register`] checks one and freezes it
//! into the row behind a new [`Lang`]. Every built-in is declared with
//! these calls (`syntax::dialects`); a plugin makes the same ones, from
//! Rust or through the Python binding (`tests/fixtures/transpiler/DIALECT-PLUGIN.md`).
//!
//! A definition either starts empty ([`DialectDef::new`]) or copies a
//! registered language ([`DialectDef::extends`]); every later call sets a
//! field or adds a row. A row whose key a table holds replaces it in
//! place; a new one is added last, so a table keeps its declaration order
//! (the first holding a word wins, the first suffix of a scale is written).
//! A shared table (`keywords`, `operators`, `devices`) adds only the rows
//! whose keys the definition does not hold.
//!
//! Cites: none (the tables' own files cite their manuals).
//!
//! Contents:
//! - `pub struct DialectDef`, `pub enum Keywords`, `pub enum Operators`, `pub enum Devices`
//! - `DialectDef::new(name: &str) -> DialectDef`, `DialectDef::renamed(self, name: &str) -> DialectDef`, `DialectDef::extends(self, base: Lang) -> DialectDef`
//! - the lexicon calls (`lex.rs`, with `set!`, the scalar setter): `title`, `fold_case`, `comment_line`, `comment_col1`, `blank_led_comment`,
//!   `cont_skip`, `comment_tail`, `block_comment`, `long_strings`, `dollar`, `plus`, `backslash`,
//!   `end`, `stop`, `join`, `quote`, `string`, `string_expr`, `name_escape`, `ident_start`,
//!   `ident_continue`, `directive`, `head_words`, `head_after`, `operators`, `operator`,
//!   `expr_braces`, `caret`, `infix`, `no_infix`, `right`, `nonassoc`, `ternary_nesting`, `prefix`,
//!   `prefix_bp`, `probe`, `index`, `lib_sections`, `exponent`, `scale`, `no_scale`, `tail`,
//!   `arithmetic`, `keywords`, `keyword`, `named`,
//!   `devices`, `device`, `master`, `analysis`, `region`
//! - the spelling calls: `comment`, `path_quote`, `expr`, `func`, `subckt_params`,
//!   `ngspice_numbers`, `write_lexicon`
//! - the capability calls: `native`, `spell`, `lift`, `refuse`, `omit`, `copy`
//! - the behaviour calls: `writer`, `text_family`, `caret_power`, `hier_sep`, `number_reading`,
//!   `model_semantics`, `source_rules`, `card_rules`, `named_rules`, `options_form`, `load_form`,
//!   `control_block`, `analysis_form`, `end_rule`, `value_quoting`, `coupling`, `param_form`,
//!   `named_params`, `analysis_masters`, `expr_eval`, `parasitics`, `netlist_rules`,
//!   `subckt_multiplicity`, `spectre_analysis`, `temp_name`, `builtin_constant`, `builtin_device`,
//!   `vacask_analysis`, `option_to`, `option_spell`, `option_method`, `option_cosmetic`, `model_master`,
//!   `spice_model_type`, `kept_word`, `dot_rule`
//! - the plugin calls (`plugin`): `code`, `refuse_code`, `omit_code`, `write_head`, `write_master`,
//!   `option`, `hook`, `external_hook`, `runs_on`, `regions_registered`; `pub type WriteHook`,
//!   `pub enum Hook`, `pub struct Card`, `pub enum Answer`
//! - `pub fn register(def: DialectDef) -> Result<Lang, RegisterError>`, `pub enum RegisterError`

mod copy;
mod dump;
mod freeze;
#[macro_use]
mod lex;
mod plugin;
mod tables;

use crate::syntax::behaviour::{
    AnalysisForm, Behaviours, CardRules, ControlBlock, Coupling, EndRule, ExprEval, IncludeScope,
    LoadForm, ModelSemantics, NamedParams, NamedRules, NetlistRules, NumberReading, OptionsForm,
    ParamForm, Parasitics, SourceRules, ValueQuoting, Writer,
};
use crate::syntax::device::Device;
use crate::syntax::diag::{Code, Kind};
use crate::syntax::lang::CONSTRUCT_COUNT;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, End, Join, Lexicon, Nesting, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Action, Alt, Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::statement::{Analysis, Statement};
use crate::syntax::token::Tag;
use crate::syntax::{Dialect, Lang};

pub(crate) use dump::dump;
pub use freeze::{RegisterError, register};
pub use plugin::{Answer, Card, Hook, WriteHook};

/// A language being declared.
#[derive(Clone)]
pub struct DialectDef {
    name: String,
    /// Its own lexicon rows; `None` while it reads `shared` (or nothing).
    lex: Option<Lex>,
    /// The lexicon it shares with the language it extends.
    shared: Option<&'static Lexicon>,
    sp: Sp,
    cells: [Option<Action>; CONSTRUCT_COUNT],
    b: Behaviours,
    /// `None`: the language itself.
    text_family: Option<Lang>,
    /// Its own codes: id, kind, message (`code`).
    codes: Vec<(String, Kind, String)>,
    /// Per construct: the own code a `W0201`/`W0202` cell raises.
    code_ids: [Option<String>; CONSTRUCT_COUNT],
    /// The post-pass's maps and the hooks (`plugin`).
    heads: Vec<(Tag, String)>,
    write_masters: Vec<(String, String)>,
    options: Vec<(String, String)>,
    hooks: Vec<(String, Hook)>,
    runs_on: Option<Dialect>,
    /// The language it extends.
    base: Option<Lang>,
}

/// A shared table of keyword rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keywords {
    /// The dot cards every SPICE dialect shares, with their aliases.
    Spice,
}

/// A shared table of operator rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operators {
    /// The SPICE comparison and logic operators.
    Spice,
}

/// A shared device table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Devices {
    /// The SPICE element letters.
    Spice,
    /// ngspice's own letters.
    NgspiceOwn,
    /// The `O` lossy line (Xyce, LTspice).
    LossyLine,
    /// Xyce's `P` port.
    XycePort,
    /// Spectre's masters.
    Spectre,
    /// VACASK's built-in modules.
    Vacask,
}

/// The lexicon rows, owned while being declared.
#[derive(Clone, Default)]
struct Lex {
    title: bool,
    fold_case: bool,
    comment_lines: Vec<String>,
    comment_col1: Vec<u8>,
    blank_led_comment: bool,
    cont_skip: Vec<String>,
    comment_tails: Vec<String>,
    block_comment: bool,
    long_strings: bool,
    dollar: Option<Dollar>,
    plus: bool,
    backslash: Option<Backslash>,
    end: Option<End>,
    stop: Option<u8>,
    join: Option<Join>,
    quote: Option<Quote>,
    string: Option<Quoting>,
    string_expr: bool,
    name_escape: bool,
    ident_start: Vec<u8>,
    ident_continue: Vec<u8>,
    directive: Option<u8>,
    head_words: bool,
    heads_after: Vec<Tag>,
    operators: Vec<(String, Tag)>,
    expr_braces: bool,
    caret: Option<Caret>,
    infix: Vec<(Tag, u8)>,
    right: Vec<Tag>,
    nonassoc: Vec<Tag>,
    ternary: Nesting,
    prefix: Vec<Tag>,
    prefix_bp: u8,
    probes: Vec<String>,
    index: bool,
    lib_sections: bool,
    exponent: Vec<u8>,
    scales: Vec<(String, Scale)>,
    tail: Option<Tail>,
    arithmetic: Option<Arithmetic>,
    keywords: Vec<(String, Tag)>,
    named: bool,
    devices: Vec<(String, Device)>,
    masters: Vec<(String, Statement)>,
    analyses: Vec<(String, Analysis)>,
    regions: Vec<(String, Lang)>,
    regions_registered: bool,
}

/// The spelling, owned while being declared.
#[derive(Clone)]
struct Sp {
    comment: String,
    path_quote: String,
    expr: (String, String),
    func: FuncForm,
    subckt_params: Option<String>,
    ngspice_numbers: bool,
    /// The language whose lexicon the writer spells letters and suffixes
    /// from; `None`: its own.
    write_lexicon: Option<Lang>,
}

impl DialectDef {
    /// An empty definition named `name`: no lexicon (a writer only, until
    /// a lexicon call), every cell unanswered, the default behaviours.
    pub fn new(name: &str) -> DialectDef {
        DialectDef {
            name: name.to_string(),
            lex: None,
            shared: None,
            sp: Sp {
                comment: "*".into(),
                path_quote: String::new(),
                expr: ("{".into(), "}".into()),
                func: FuncForm::Func,
                subckt_params: Some("PARAMS:".into()),
                ngspice_numbers: false,
                write_lexicon: None,
            },
            cells: [None; CONSTRUCT_COUNT],
            b: Behaviours::DEFAULT,
            text_family: None,
            codes: Vec::new(),
            code_ids: std::array::from_fn(|_| None),
            heads: Vec::new(),
            write_masters: Vec::new(),
            options: Vec::new(),
            hooks: Vec::new(),
            runs_on: None,
            base: None,
        }
    }

    /// The same definition under `name`: a built-in's calls registered
    /// again as a language of its own (`tests/dialect_defs.rs`).
    pub fn renamed(mut self, name: &str) -> DialectDef {
        self.name = name.to_string();
        self
    }

    /// Copies registered language `base`: its lexicon (shared until a
    /// lexicon call changes it), spelling, cells, behaviours, codes, maps,
    /// hooks and `runs_on` (a built-in's own simulator is not copied: a
    /// plugin runs only where it says, `runs_on`). A `Lang` exists only
    /// once registered, so a chain cannot hold a cycle.
    ///
    /// # Panics
    /// When `base` is not registered.
    pub fn extends(mut self, base: Lang) -> DialectDef {
        let r = base.row();
        let own = |s: &&str| s.to_string();
        self.codes = r
            .codes
            .iter()
            .map(|c| (c.0.into(), c.1, c.2.into()))
            .collect();
        self.code_ids = std::array::from_fn(|i| match r.cells[i] {
            Action::Refuse(Code::W0201) | Action::Spell(Alt::Omit(Code::W0202)) => {
                let d = usize::from(r.details[i] - r.first_code);
                Some(r.codes[d].0.to_string())
            }
            _ => None,
        });
        self.heads = r.heads.iter().map(|h| (h.0, own(&h.1))).collect();
        self.write_masters = r
            .write_masters
            .iter()
            .map(|m| (own(&m.0), own(&m.1)))
            .collect();
        self.options = r.options.iter().map(|m| (own(&m.0), own(&m.1))).collect();
        self.hooks = r.hooks.iter().map(|h| (own(&h.0), h.1)).collect();
        self.runs_on = r.runs_on;
        self.base = Some(base);
        let sp = base.spelling();
        self.shared = base.lexicon();
        self.lex = None;
        self.sp = Sp {
            comment: sp.comment.into(),
            path_quote: sp.path_quote.into(),
            expr: (sp.expr.0.into(), sp.expr.1.into()),
            func: sp.func,
            subckt_params: sp.subckt_params.map(String::from),
            ngspice_numbers: sp.ngspice_numbers,
            write_lexicon: (base.lexicon().map(|l| l.lang) != Some(sp.lexicon.lang))
                .then_some(sp.lexicon.lang),
        };
        self.cells = base.cells().map(Some);
        self.b = *base.behaviours();
        let family = self.b.text_family;
        self.text_family = (family != base).then_some(family);
        self
    }

    // ---- the spelling ----

    /// What starts a comment line in a written deck.
    pub fn comment(mut self, s: &str) -> Self {
        self.sp.comment = s.into();
        self
    }

    /// The quote around a written file path.
    pub fn path_quote(mut self, s: &str) -> Self {
        self.sp.path_quote = s.into();
        self
    }

    /// The delimiters around a written expression.
    pub fn expr(mut self, open: &str, close: &str) -> Self {
        self.sp.expr = (open.into(), close.into());
        self
    }

    /// How a function is written.
    pub fn func(mut self, v: FuncForm) -> Self {
        self.sp.func = v;
        self
    }

    /// The word before a subcircuit's parameter defaults.
    pub fn subckt_params(mut self, word: Option<&str>) -> Self {
        self.sp.subckt_params = word.map(String::from);
        self
    }

    /// The target reads numbers with ngspice's arithmetic.
    pub fn ngspice_numbers(mut self, v: bool) -> Self {
        self.sp.ngspice_numbers = v;
        self
    }

    /// The writer spells element letters and suffixes from `lang`'s
    /// lexicon (a writer with no lexicon of its own: CDL).
    pub fn write_lexicon(mut self, lang: Lang) -> Self {
        self.sp.write_lexicon = Some(lang);
        self
    }

    // ---- the capability column ----

    /// `c` is written in the target's own form.
    pub fn native(self, c: Construct) -> Self {
        self.cell(c, Action::Native)
    }

    /// `c` is written as alternative `alt`.
    pub fn spell(self, c: Construct, alt: Alt) -> Self {
        self.cell(c, Action::Spell(alt))
    }

    /// `c` is rewritten by rule `rule`.
    pub fn lift(self, c: Construct, rule: Rule) -> Self {
        self.cell(c, Action::Lift(rule))
    }

    /// `c` refuses the deck with `code`.
    pub fn refuse(self, c: Construct, code: Code) -> Self {
        self.cell(c, Action::Refuse(code))
    }

    /// `c` is left out, noted `code`.
    pub fn omit(self, c: Construct, code: Code) -> Self {
        self.cell(c, Action::Spell(Alt::Omit(code)))
    }

    /// `c` is written as is, noted `code`.
    pub fn copy(self, c: Construct, code: Code) -> Self {
        self.cell(c, Action::Spell(Alt::Copy(code)))
    }

    // ---- the behaviours ----

    set!(writer, b.writer: Writer);
    set!(caret_power, b.caret_power: bool);
    set!(hier_sep, b.hier_sep: char);
    set!(number_reading, b.numbers: NumberReading);
    set!(model_semantics, b.model_semantics: ModelSemantics);
    set!(source_rules, b.source_rules: SourceRules);
    set!(card_rules, b.card_rules: CardRules);
    set!(named_rules, b.named_rules: NamedRules);
    set!(options_form, b.options_form: OptionsForm);
    set!(load_form, b.load_form: LoadForm);
    set!(control_block, b.control_block: ControlBlock);
    set!(analysis_form, b.analysis_form: AnalysisForm);
    set!(end_rule, b.end_rule: EndRule);
    set!(value_quoting, b.value_quoting: ValueQuoting);
    set!(coupling, b.coupling: Coupling);
    set!(param_form, b.param_form: ParamForm);
    set!(include_scope, b.include_scope: IncludeScope);
    set!(named_params, b.named_params: NamedParams);

    /// A named instance whose master is one of `lang`'s analysis words is
    /// read as that analysis.
    pub fn analysis_masters(mut self, lang: Lang) -> Self {
        self.b.analysis_masters = Some(lang);
        self
    }
    set!(expr_eval, b.expr_eval: ExprEval);
    set!(parasitics, b.parasitics: Parasitics);
    set!(netlist_rules, b.netlist_rules: NetlistRules);
    set!(subckt_multiplicity, b.subckt_multiplicity: bool);

    /// A region kept as `lang`'s text is written as written here.
    pub fn text_family(mut self, lang: Lang) -> Self {
        self.text_family = Some(lang);
        self
    }
}

// ---- private ----

impl DialectDef {
    /// The own lexicon rows, copied from the shared lexicon on first use.
    fn lex(&mut self) -> &mut Lex {
        if self.lex.is_none() {
            self.lex = Some(self.shared.take().map_or_else(Lex::default, Lex::of));
        }
        self.lex.as_mut().expect("just set")
    }

    fn b(&mut self) -> &mut Behaviours {
        &mut self.b
    }

    fn cell(mut self, c: Construct, a: Action) -> Self {
        self.cells[c as usize] = Some(a);
        self.code_ids[c as usize] = None;
        self
    }
}

/// Adds `s` to `v` unless it holds it.
fn push_new(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

/// Adds `key`'s row to `v` unless `v` holds the key (a shared table's
/// row yields to the definition's own).
fn add<K: PartialEq, V>(v: &mut Vec<(K, V)>, key: K, val: V) {
    if !v.iter().any(|r| r.0 == key) {
        v.push((key, val));
    }
}

/// Sets `key`'s row in `v`: in place where `v` holds the key, else last.
fn put<K: PartialEq, V>(v: &mut Vec<(K, V)>, key: K, val: V) {
    match v.iter_mut().find(|r| r.0 == key) {
        Some(r) => r.1 = val,
        None => v.push((key, val)),
    }
}

/// `s` for the life of the program: a registered row's text.
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}
