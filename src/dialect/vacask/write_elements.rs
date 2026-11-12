//! `Element` rows → instance statements, the inverse of `read_elements`: `name (nodes) master
//! params`, the typed fields under the module's names (R, C, L value `r`/`c`/`l`, `temp`,
//! `dtemp`, `tc1`, `tc2`, `tce`, `scale`, `noisy=0`; diode `area`), the instance parameters
//! by the module's name for them (`instance_<p>`), `mult` as `$mfactor`; a `Behav` as `name (p
//! n) v=<expr>` or `i=<expr>`. Sources, controlled sources, couplings and lines are the sources
//! area's (`write.rs` routes them).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cir-instance, cir-mfactor,
//! dev-builtin-behavioral; R21 (`admits`).
//!
//! Contents: `admits`, `element` (crate-private)

use super::expr_write::write_expr;
use super::families::{self, Name};
use super::name::{self, Place};
use super::write::Writer;
use super::write_models;
use crate::ir;

/// One `Element` row as its instance statement.
pub(super) fn element(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let e: ir::Element = d.row(at.index);
    w.head(&e);
    if e.kind == ir::ElemKind::Behav {
        let row = d.behav(&e);
        w.out.push_str(match row.output {
            ir::controlled::OutKind::Voltage => " v=",
            _ => " i=",
        });
        write_expr(d, row.expr, w.out);
        w.out.push('\n');
        return;
    }
    for (key, value) in fields(d, &e) {
        w.out.push(' ');
        name::spell(&key, Place::Other, w.out);
        w.out.push('=');
        match value {
            Some(v) => write_expr(d, v, w.out),
            None => w.out.push('0'),
        }
    }
    w.out.push('\n');
}

/// Appends a refusal for each reachable element of this area VACASK cannot spell for a reason
/// no feature states (R21): a name holding white space, or a parameter its module lacks.
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    for (at, _) in design.reachable() {
        if at.kind != ir::ItemKind::Element {
            continue;
        }
        let e: ir::Element = design.row(at.index);
        let names = std::iter::once(e.name)
            .chain(std::iter::once(e.model.name))
            .chain(design.names(e.terminals).iter().copied())
            .filter(|n| !n.is_none());
        if names
            .map(|n| design.str(n))
            .any(|n| n.chars().any(char::is_whitespace))
        {
            diags.push_at(write_models::UNSPELLED, design, at, 0);
        }
        let Some(word) = module(design, &e) else {
            continue;
        };
        let lacked = fields(design, &e).iter().any(|(key, _)| {
            key != "$mfactor" && !matches!(families::instance_name_in(&word, key), Name::Ir(_))
        });
        if lacked {
            diags.push_at(write_models::LACKED, design, at, 0);
        }
    }
}

// ---- private ----

/// The module word of `e`'s bound model, when it has one.
fn module(d: &ir::Design, e: &ir::Element) -> Option<String> {
    if e.model.def.is_none() || e.kind.master_kind() != Some(ir::DefKind::Model) {
        return None;
    }
    let def = d.def(e.model.def);
    (def.item.kind == ir::ItemKind::Model).then(|| write_models::word(d, &d.row(def.item.index)))
}

/// `e`'s parameters as written: the typed fields, then the instance parameters, then
/// `$mfactor`; `None` stands for the literal 0 (`noisy=0`).
fn fields(d: &ir::Design, e: &ir::Element) -> Vec<(String, Option<ir::ExprId>)> {
    let word = module(d, e);
    let mut out = Vec::new();
    let mut put = |key: &str, v: ir::ExprId| {
        if !v.is_none() {
            out.push((key.to_string(), Some(v)));
        }
    };
    match e.kind.detail() {
        ir::DetailKind::Passive => {
            let p = d.passive(e);
            let value = match e.kind {
                ir::ElemKind::Resistor => "r",
                ir::ElemKind::Capacitor => "c",
                _ => "l",
            };
            put(value, p.value);
            put("temp", p.temp_abs);
            put("dtemp", p.dtemp);
            put("tc1", p.tc1);
            put("tc2", p.tc2);
            put("tce", p.tce);
            put("scale", p.scale);
            if !p.noisy {
                out.push(("noisy".to_string(), None));
            }
        }
        ir::DetailKind::Semi => {
            let s = d.semi(e);
            put("area", s.area);
            put("temp", s.temp_abs);
            put("dtemp", s.dtemp);
        }
        _ => {}
    }
    let family = e.kind != ir::ElemKind::ModuleModel;
    for &(key, value) in d.params(e.params) {
        let key = d.str(key);
        let key = match (&word, family) {
            (Some(w), true) => families::instance_name_out(w, key),
            _ => key.to_string(),
        };
        out.push((key, Some(value)));
    }
    if !e.mult.is_none() {
        out.push(("$mfactor".to_string(), Some(e.mult)));
    }
    out
}
