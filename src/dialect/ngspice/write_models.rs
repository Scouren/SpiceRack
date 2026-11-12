//! Model cards: a `Model` row → `.model name word [level=n] [pchan] [version=v] k=v …`, the
//! type word and level by `families::card` (the inverse of the reader's dispatch), a code
//! model's or module's name as the word, the version under its family's keyword, the parameters
//! in IR order: a number as `INPevaluate` reads it, a string quoted, anything else
//! `expr_write::value` (bare name or `{…}`). A bin's name is already `base.<digits>`.
//!
//! `admits` (R21) refuses what ngspice cannot run as the row says: a family it has no device
//! for, a version that picks no device or a version on a family ngspice runs one way, a
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
use super::tables::Eval;
use super::write::Out;
use crate::ir::{self, Code, Info, Kind, ModelFamily as F};

/// What this area spells: exactly the reader's image (law L5).
pub(super) const FEATURES: ir::FeatureSet = read_models::FEATURES;

/// This area's codes, `W1200–W1299`.
pub(super) const CODES: &[Row] = &[
    refusal(
        FAMILY,
        "a model family ngspice has no device for",
        "ngspice-45 inpdomod.c INPdomodel",
        "no type word and level of ngspice runs this family's equations.",
    ),
    refusal(
        VERSION,
        "a model version ngspice has no device for",
        "ngspice-45 inpdomod.c INPfindVer",
        "BSIM3, BSIM4 and HiSIM-HV pick their device by the version's prefix; a family with one device and no version parameter runs one version.",
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

/// A family ngspice has no device for.
const FAMILY: Code = Code::new(b'W', 1201);
/// A version ngspice has no device for, or a version on a family ngspice runs one way.
const VERSION: Code = Code::new(b'W', 1202);
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
    let span = design.span_of(item);
    let mut refuse = |code| diags.push(code, span, 0);
    let named = matches!(m.family, F::CodeModel | F::Module);
    if !named && families::card(m.family, m.polarity).is_none() {
        refuse(FAMILY);
        return;
    }
    let version = (!m.version.is_none()).then(|| version_text(design, m.version));
    let device = if named {
        None
    } else {
        let has = families::version(m.family).is_some();
        let picked = version
            .clone()
            .flatten()
            .filter(|_| families::dispatches(m.family));
        let ok = match &version {
            None => true,
            Some(v) => has && v.as_deref().is_some_and(bare),
        };
        match families::device(m.family, picked.as_deref()) {
            Some(d) if ok => Some(d),
            _ => {
                refuse(VERSION);
                None
            }
        }
    };
    let params = design.params(m.params);
    for &(name, value) in params {
        let name = design.str(name);
        if let Some(dev) = device
            && (name == "level" || !families::accepts(dev, name))
        {
            refuse(PARAM);
        }
        if !value_spells(design, value) {
            refuse(VALUE);
        }
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

/// A version's text: a string as is, a number as `INPevaluate` reads it; `None` for any other
/// expression.
fn version_text(d: &ir::Design, e: ir::ExprId) -> Option<String> {
    match d.expr(e) {
        ir::Expr::Str(s) => Some(d.str(s).to_string()),
        ir::Expr::Num(n) => {
            let mut out = String::new();
            number::spell(d.num(n), Grammar::Inp, &mut out);
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
        ir::Expr::Num(n) => number::spell(d.num(n), Grammar::Inp, out),
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
            ir::expr_class(d, e) < ir::ExprClass::Temp
                && expr_write::spells_in(d, e, Eval::Numparam)
        }
    }
}
