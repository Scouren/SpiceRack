//! `Model` rows → `model name module params`, the inverse of `read_models`: the module word
//! from the family (`families::default_word`; a user module's own name), `type=1` or `type=-1`
//! for a model with a polarity (stated even for VACASK's default 1: run t1; a deck that omits
//! `type` then prints it as given after a round trip, spelling only), `version=` from the model's
//! version, each family parameter under the module's name for it (`model_<p>`).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cir-masters; R21 (`admits`).
//!
//! Contents: `UNSPELLED`, `LIBRARIES`, `UNPROVEN`, `VERSION`, `LACKED` (their rows are in `read_elements::CODES`), `admit`, `model`, `word` (crate-private)

use super::expr_write::write_expr;
use super::families::{self, Name};
use super::name::{self, Place};
use super::write::Writer;
use crate::ir;

/// A name no quoting spells (VACASK's quoted names hold no white space: VD input-identifiers).
pub(super) const UNSPELLED: ir::Code = ir::Code::new(b'W', 2101);
/// Two device libraries of one family naming files of different modules.
pub(super) const LIBRARIES: ir::Code = ir::Code::new(b'W', 2103);
/// A model of a family no run proved a VACASK module implements.
pub(super) const UNPROVEN: ir::Code = ir::Code::new(b'W', 2107);
/// A model version VACASK's module does not run as written (F85).
pub(super) const VERSION: ir::Code = ir::Code::new(b'W', 2111);
/// A parameter the target module does not have.
pub(super) const LACKED: ir::Code = ir::Code::new(b'W', 2102);

/// One `Model` row as its `model` line.
pub(super) fn model(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let row: ir::Model = d.row(at.index);
    let module = word(d, &w.words, &row);
    w.line();
    w.out.push_str("model ");
    name::spell(d.str(row.name), Place::Other, w.out);
    w.out.push(' ');
    name::spell(&module, Place::Other, w.out);
    // Always stated: `print model` tells a given `type` from the default (run t1). The reverse
    // case is spelling only: a deck that omits `type` prints it as given after a round trip (no
    // settle deck prints one); the numbers are the same.
    match row.polarity {
        ir::Polarity::N => w.out.push_str(" type=1"),
        ir::Polarity::P => w.out.push_str(" type=-1"),
        ir::Polarity::None => {}
    }
    // sp_bsim3v3 has no `version` parameter: it is 3.3.0, which the reader states (`admit`
    // refuses another).
    let given = families::versions(row.family).is_none_or(|v| v.given);
    if !row.version.is_none() && given {
        w.out.push_str(" version=");
        write_expr(d, row.version, w.out);
    }
    for &(key, value) in d.params(row.params) {
        w.out.push(' ');
        let key = d.str(key);
        let key = match row.family {
            ir::ModelFamily::Module => key.to_string(),
            _ => families::model_name_out(&module, key),
        };
        name::spell(&key, Place::Other, w.out);
        w.out.push('=');
        write_expr(d, value, w.out);
    }
    w.out.push('\n');
}

/// The module word a model is written with.
pub(super) fn word(d: &ir::Design, words: &families::Words, row: &ir::Model) -> String {
    match row.family {
        ir::ModelFamily::Module => d.str(row.module.name).to_string(),
        // Two libraries of different modules: `admits` refused the design.
        f => words.get(f).unwrap_or_default().to_string(),
    }
}

/// Appends a refusal for each reachable model VACASK cannot spell for a reason no feature
/// states (R21): a name holding white space, or a parameter its module lacks.
pub(super) fn admit(
    design: &ir::Design,
    words: &families::Words,
    at: ir::ItemRef,
    diags: &mut ir::Diags,
) {
    let row: ir::Model = design.row(at.index);
    let module = word(design, words, &row);
    if module.is_empty() {
        diags.push_at(LIBRARIES, design, at, 0);
    }
    // A family no run proved VACASK's module implements reads back as a user module (R41 a).
    if row.family != ir::ModelFamily::Module && !families::PROVEN.contains(&module.as_str()) {
        diags.push_at(UNPROVEN, design, at, 0);
    }
    if [design.str(row.name), module.as_str()]
        .iter()
        .any(|n| n.chars().any(char::is_whitespace))
    {
        diags.push_at(UNSPELLED, design, at, 0);
    }
    let module_family = row.family == ir::ModelFamily::Module;
    let mut lacked = design.params(row.params).iter().any(|&(key, _)| {
        let key = design.str(key);
        let written = match module_family {
            true => key.to_string(),
            false => families::model_name_out(&module, key),
        };
        match families::model_name_in(&module, &written) {
            Name::Ir(back) => !module_family && back != key,
            Name::Misplaced | Name::Unknown => true,
        }
    });
    // A user module's `version=` is one of its parameters to the reader.
    lacked |= !row.version.is_none() && module_family;
    let versions = families::versions(row.family);
    lacked |= !row.version.is_none()
        && versions.is_none()
        && families::model_name_in(&module, "version") != Name::Ir("version".into());
    // F85: a version the module does not run as written (BSIM4 4.5 runs as 4.8.3).
    if let Some(v) = versions {
        let runs = match design.expr(row.version) {
            ir::Expr::Str(s) => v.runs.contains(&design.str(s)),
            _ => false,
        };
        if !runs {
            diags.push_at(VERSION, design, at, 0);
        }
    }
    if lacked {
        diags.push_at(LACKED, design, at, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: R91a and `families/params.txt` (sp_diode has no `rth0` or `cth0`, ngspice-45's
    /// self-heating parameters): a diode model giving one is refused, the rest is spelled.
    #[test]
    fn a_parameter_the_module_lacks_is_refused() {
        let lacked = |key: &str| {
            let mut design = ir::Design::new();
            let at = {
                let mut b = design.builder();
                let (_, top) = b.file("t.sim", ir::Role::Deck);
                let lib = ir::DeviceLibrary {
                    path: ir::NameId::NONE,
                    family: ir::ModelFamily::Diode,
                };
                b.device_library(top, lib, ir::Span::NONE);
                let value = b.num(1.0);
                let params = b.params(&[(key, value)]);
                let row = ir::Model {
                    name: b.name("d"),
                    params,
                    bin_of: ir::NameId::NONE,
                    ako: ir::Ref::unresolved(ir::NameId::NONE),
                    module: ir::Ref::unresolved(ir::NameId::NONE),
                    version: ir::ExprId::NONE,
                    word: ir::NameId::NONE,
                    family: ir::ModelFamily::Diode,
                    polarity: ir::Polarity::None,
                };
                b.model(top, row, ir::Span::NONE)
            };
            let index = super::super::write_elements::Index::of(&design);
            let mut diags = ir::Diags::new();
            admit(&design, &index.words, at, &mut diags);
            diags.rows().iter().any(|d| d.code == LACKED)
        };
        assert!(lacked("rth0"));
        assert!(lacked("cth0"));
        assert!(!lacked("is"));
        assert!(!lacked("ikp"));
    }
}
