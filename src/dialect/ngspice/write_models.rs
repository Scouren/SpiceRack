//! Model cards: a `Model` row → `.model name word [level=n] [pchan] [version=v] k=v …`, the
//! type word and level by `families::card` (the inverse of the reader's dispatch), a code
//! model's or module's name as the word, the version under its family's keyword, the parameters
//! in IR order: a number as `INPevaluate` reads it, a string quoted, anything else
//! `expr_write::value` (bare name or `{…}`). A bin's name is already `base.<digits>`.
//!
//! The version is always written, also when it equals ngspice's default (one spelling, no
//! omission: a silent card reads as its device's default and is written out; fixtures
//! `read/models/bsim_version` (`p4`: silent → "4.8.3", b4set.c:205) and `write/models/families`
//! (`q`: "4.8.3" → `version=4.8.3`)).
//!
//! A family ngspice has no device for is a `Feature::Family` its `FEATURES` lack (R32).
//! `admits` (R21) refuses what ngspice cannot run as the row says: a version that picks no device or a version on a family ngspice runs one way, a
//! parameter the device's tables lack (ngspice would ignore it: a wrong number with no crash),
//! a bin of a family ngspice does not bin or not named `base.<digits>`, a value it reads back
//! otherwise or that reads the run.
//!
//! Cites: ngspice-45 `inpdomod.c`, `inpgmod.c` (`create_model`, `INPgetModBin`), `inpcom.c`
//! `inp_vdmos_model`; `rewrite/DIALECT-NGSPICE.md` §4; `rewrite/RULINGS.md` R21.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, ir::ScopeId, &mut ir::Diags)`,
//! `pub(super) fn model(w: &mut Out<'_>, item: ir::ItemRef)`

use super::codes::Row;
use super::expr_write;
use super::families;
use super::number::{self, Grammar};
use super::read_models;
use super::write::Out;
use crate::ir::{self, Code, Info, Kind, ModelFamily as F};

/// What this area spells: exactly the reader's image (law L5).
pub(super) const FEATURES: ir::FeatureSet = read_models::FEATURES;

/// This area's codes, `W1200–W1299`.
pub(super) const CODES: &[Row] = &[
    refusal(
        VERSION,
        "a model version ngspice has no device for, or none on a versioned family",
        "ngspice-45 inpdomod.c INPfindVer",
        "BSIM3, BSIM4 and HiSIM-HV pick their device by the version's prefix; a family with one device and no version parameter runs one version; a family with a version parameter needs the row's version (none would be ngspice's default, which the row does not state).",
    ),
    refusal(
        TYPE_WORD,
        "a code model or module name ngspice reads otherwise",
        "ngspice-45 inpdomod.c default branch, inptyplk.c",
        "the type word of a code model must be one of the pinned build's code models (devhelp), and a module's must be no type word, built-in device or code model: ngspice looks those up first, and an unknown word is \"Unknown model type - ignored\" (run).",
    ),
    refusal(
        PARAM,
        "a model parameter ngspice's device lacks",
        "ngspice-45 inpgmod.c create_model",
        "ngspice ignores a name no parameter table of the device has (unrecognized parameter): the number would be silently wrong.",
    ),
    refusal(
        BIN,
        "a bin ngspice does not bin",
        "ngspice-45 inpgmod.c INPgetModBin",
        "ngspice bins BSIM3, BSIM4, HiSIM2 and HiSIM-HV models named base.<digits> only.",
    ),
    refusal(
        VALUE,
        "a model value ngspice reads otherwise",
        "ngspice-45 inpcom.c inp_fix_param_values",
        "a literal ngspice spells in another form, a string or version it cannot quote, or a value that reads the temperature or the circuit.",
    ),
];

/// A version ngspice has no device for, or a version on a family ngspice runs one way.
const VERSION: Code = Code::new(b'W', 1202);
/// A code model ngspice's build lacks, or a module whose name ngspice reads as something else.
const TYPE_WORD: Code = Code::new(b'W', 1206);
/// A parameter the device's tables lack (or `level`, a field).
const PARAM: Code = Code::new(b'W', 1203);
/// A bin ngspice does not bin.
const BIN: Code = Code::new(b'W', 1204);
/// A value ngspice reads back as another value, or one that reads the run.
const VALUE: Code = Code::new(b'W', 1205);

/// Appends a refusal for a `Model` ngspice cannot run as the row says (R21).
pub(super) fn admits(
    design: &ir::Design,
    item: ir::ItemRef,
    _scope: ir::ScopeId,
    diags: &mut ir::Diags,
) {
    let m: ir::Model = design.row(item.index);
    let mut refuse = |code| diags.push_at(code, design, item, 0);
    let named = matches!(m.family, F::CodeModel | F::Module);
    // the type word names a code model or a module only by what ngspice has loaded: a word
    // the reader would read as the other kind (or as a built-in) has no spelling here
    let word_ok = match m.family {
        F::CodeModel => families::code_model(design.str(m.word)),
        F::Module => families::module_word(design.str(m.module.name)),
        _ => true,
    };
    if !word_ok {
        refuse(TYPE_WORD);
    }
    if !named && families::card(m.family, m.polarity).is_none() {
        // a family ngspice lacks is a feature FEATURES lacks: `check_legal` refused it
        return;
    }
    let version = (!m.version.is_none()).then(|| version_text(design, m.version));
    let device = if named {
        // a module's or code model's version has no field on ngspice: the reader keeps a
        // `version=` there as one of its parameters (l4 seed 49)
        if version.is_some() {
            refuse(VERSION);
        }
        None
    } else {
        let has = families::version(m.family).is_some();
        let picked = version
            .clone()
            .flatten()
            .filter(|_| families::dispatches(m.family));
        // `check` refuses a versioned family's model without a version (round 7); a version
        // is written as the vocabulary spells it, so it reads back as itself
        let ok = match &version {
            None => true,
            Some(v) => has && v.as_deref().is_some_and(bare),
        };
        match families::device(m.family, picked.as_deref()) {
            Some(d)
                if ok
                    && version.as_ref().is_none_or(|v| {
                        v.as_deref()
                            .and_then(|t| families::standard(m.family, d, t))
                            == v.as_deref()
                    }) =>
            {
                Some(d)
            }
            _ => {
                refuse(VERSION);
                None
            }
        }
    };
    let params = design.params(m.params);
    for &(name, value) in params {
        let name = design.str(name);
        // `level` is the family's (MESA's own, a parameter of the device)
        let level = name == "level" && m.family != F::Mesa;
        if let Some(dev) = device
            && (level || !families::accepts(dev, name))
        {
            refuse(PARAM);
        }
        if !value_spells(design, value) {
            refuse(VALUE);
        }
    }
    if m.family == F::Mesa && !mesa_level(design, m.params) {
        // without its level 2-4 the card reads back as a MES (level 1) model
        refuse(PARAM);
    }
    if m.bin_of != ir::NameId::NONE {
        let name = design.str(m.name);
        let base = design.str(m.bin_of);
        let digits = name
            .strip_prefix(base)
            .and_then(|r| r.strip_prefix('.'))
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()));
        if !digits || !families::binnable(m.family) {
            refuse(BIN);
        }
    }
}

/// `.model name word [level=n] [pchan] [version=v] k=v …`.
pub(super) fn model(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let m: ir::Model = d.row(item.index);
    let mut s = format!(".model {} ", d.str(m.name));
    match m.family {
        F::CodeModel => s.push_str(d.str(m.word)),
        F::Module => s.push_str(d.str(m.module.name)),
        f => {
            let (word, level) = families::card(f, m.polarity).expect("admits refused the rest");
            s.push_str(word);
            if let Some(level) = level {
                s.push_str(&format!(" level={level}"));
            }
            if f == F::Vdmos && m.polarity == ir::Polarity::P {
                s.push_str(" pchan");
            }
        }
    }
    if !m.version.is_none() {
        let (key, _) = families::version(m.family).expect("admits refused a version");
        let text = version_text(d, m.version).expect("admits refused another value");
        s.push_str(&format!(" {key}={text}"));
    }
    for &(name, value) in d.params(m.params) {
        s.push(' ');
        s.push_str(d.str(name));
        s.push('=');
        spell(d, value, &mut s);
    }
    w.line(&s);
}

// ---- private ----

/// A refusal row.
const fn refusal(
    code: Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
    (
        code,
        Info {
            title,
            cite,
            explain,
            kind: Kind::Refusal,
        },
    )
}

/// Whether a MESA row's params hold `level` as the number 2, 3 or 4.
fn mesa_level(d: &ir::Design, params: ir::ParamList) -> bool {
    d.params(params).iter().any(|&(name, v)| {
        d.str(name) == "level"
            && matches!(d.expr(v), ir::Expr::Num(n) if [2.0, 3.0, 4.0].contains(&d.literal_value(d.num(n))))
    })
}

/// A version's text: a string as is, a number as `INPevaluate` reads it; `None` for any other
/// expression.
fn version_text(d: &ir::Design, e: ir::ExprId) -> Option<String> {
    match d.expr(e) {
        ir::Expr::Str(s) => Some(d.str(s).to_string()),
        ir::Expr::Num(n) => {
            let mut out = String::new();
            number::spell(d, d.num(n), Grammar::Inp, &mut out);
            Some(out)
        }
        _ => None,
    }
}

/// Whether a version text is one token ngspice reads whole (`INPfindVer` takes `%s`).
fn bare(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|c| c.is_ascii_graphic() && !b"(),=\"'{}".contains(&c))
}

/// Appends a parameter value: a number as `INPevaluate` reads it, a string quoted, else
/// `expr_write::value`.
fn spell(d: &ir::Design, e: ir::ExprId, out: &mut String) {
    match d.expr(e) {
        ir::Expr::Num(n) => number::spell(d, d.num(n), Grammar::Inp, out),
        ir::Expr::Str(s) => {
            out.push('"');
            out.push_str(d.str(s));
            out.push('"');
        }
        _ => expr_write::value(d, e, out),
    }
}

/// Whether [`spell`] writes `e` so that the reader reads it back as itself: every literal
/// (R28), a string with no quote or line break, an expression numparam spells that reads
/// neither the temperature nor the circuit.
fn value_spells(d: &ir::Design, e: ir::ExprId) -> bool {
    match d.expr(e) {
        ir::Expr::Num(_) => true,
        ir::Expr::Str(s) => !d.str(s).contains(['"', '\n', '\r']),
        _ => {
            ir::expr_class(d, e) < ir::ExprClass::Circuit
                && expr_write::spells_in(d, e, expr_write::eval_of(d, e))
        }
    }
}
