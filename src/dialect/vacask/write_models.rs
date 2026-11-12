//! `Model` rows → `model name module params`, the inverse of `read_models`: the module word
//! from the family (`families::default_word`; a user module's own name), `type=-1` for a
//! p-type model (n-type is VACASK's default, written as nothing), `version=` from the model's
//! version, each family parameter under the module's name for it (`model_<p>`).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cir-masters; R21 (`admits`).
//!
//! Contents: `UNSPELLED`, `LIBRARIES`, `LACKED` (their rows are in `read_elements::CODES`), `admits`, `model`, `word` (crate-private)

use super::expr_write::write_expr;
use super::families::{self, Name};
use super::name::{self, Place};
use super::write::Writer;
use crate::ir;

/// A name no quoting spells (VACASK's quoted names hold no white space: VD input-identifiers).
pub(super) const UNSPELLED: ir::Code = ir::Code::new(b'W', 2101);
/// Two device libraries of one family naming files of different modules.
pub(super) const LIBRARIES: ir::Code = ir::Code::new(b'W', 2103);
/// A parameter the target module does not have.
pub(super) const LACKED: ir::Code = ir::Code::new(b'W', 2102);

/// One `Model` row as its `model` line.
pub(super) fn model(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let row: ir::Model = d.row(at.index);
    let module = word(d, &row);
    w.line();
    w.out.push_str("model ");
    name::spell(d.str(row.name), Place::Other, w.out);
    w.out.push(' ');
    name::spell(&module, Place::Other, w.out);
    if row.polarity == ir::Polarity::P {
        w.out.push_str(" type=-1");
    }
    if !row.version.is_none() {
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
pub(super) fn word(d: &ir::Design, row: &ir::Model) -> String {
    match row.family {
        ir::ModelFamily::Module => d.str(row.module.name).to_string(),
        // Two libraries of different modules: `admits` refused the design.
        f => families::design_word(d, f).unwrap_or_default().to_string(),
    }
}

/// Appends a refusal for each reachable model VACASK cannot spell for a reason no feature
/// states (R21): a name holding white space, or a parameter its module lacks.
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    for (at, _) in design.reachable() {
        if at.kind != ir::ItemKind::Model {
            continue;
        }
        let row: ir::Model = design.row(at.index);
        let module = word(design, &row);
        if module.is_empty() {
            diags.push_at(LIBRARIES, design, at, 0);
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
        lacked |= !row.version.is_none()
            && families::model_name_in(&module, "version") != Name::Ir("version".into());
        if lacked {
            diags.push_at(LACKED, design, at, 0);
        }
    }
}
