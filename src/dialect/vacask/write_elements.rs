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
//! Contents: `MASTERLESS`, `PORTS`, `UNRESOLVED_MULT`, `KIND`, `BEHAV_FIELD`, `admit`, `element` (crate-private)

use super::expr_write::write_expr;
use super::families::{self, Name};
use super::name::{self, Place};
use super::write::Writer;
use super::write_models;
use crate::ir;

/// An element whose master VACASK would not find from its place.
pub(super) const MASTERLESS: ir::Code = ir::Code::new(b'W', 2104);
/// A device whose terminal count is not its shipped module's (a subcircuit instance's is the IR
/// invariant X0022: R61, A36).
pub(super) const PORTS: ir::Code = ir::Code::new(b'W', 2105);

/// An unresolved element with a multiplicity: VACASK's `$mfactor` is the multiplicity on a
/// device and an ordinary parameter on a subckt that declares it (VD cir-mfactor; run n2), so
/// with no master it reads back as a parameter. VACASK's own.
pub(super) const UNRESOLVED_MULT: ir::Code = ir::Code::new(b'W', 2109);
/// An element whose kind is not the one its master's family gives.
pub(super) const KIND: ir::Code = ir::Code::new(b'W', 2108);
/// A behavioural source with a field VACASK's `b` source has no parameter for.
pub(super) const BEHAV_FIELD: ir::Code = ir::Code::new(b'W', 2106);

/// The kinds the sources area writes (`write.rs` routes them): their masters are its check.
const SOURCE_KINDS: [ir::ElemKind; 8] = [
    ir::ElemKind::VSource,
    ir::ElemKind::ISource,
    ir::ElemKind::Vcvs,
    ir::ElemKind::Vccs,
    ir::ElemKind::Ccvs,
    ir::ElemKind::Cccs,
    ir::ElemKind::Coupling,
    ir::ElemKind::LosslessLine,
];

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
        super::expr_write::write_behavioural(d, row.expr, w.out);
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
/// no feature states (R21): a name holding white space, a parameter its module lacks, a master
/// VACASK would not find from the element's place (it takes the kind from the master, so the
/// element would read back `Unresolved`; a source's too), an unresolved element with a
/// multiplicity (W2109), a device whose terminal count is not its shipped module's (VACASK
/// connects the missing ones to internal nodes: read back `Private`), a kind other than its model's family gives (W2108), a behavioural source with
/// `tc1 tc2 temp dtemp rpar` or a multiplicity (VD dev-builtin-behavioral: its expression only;
/// W2106).
pub(super) fn admit(
    design: &ir::Design,
    at: ir::ItemRef,
    scope: ir::ScopeId,
    diags: &mut ir::Diags,
) {
    let e: ir::Element = design.row(at.index);
    // Every kind with a master, sources included: VACASK takes the kind from the master,
    // so one it does not find reads back `Unresolved` (law L5, seed 1).
    if let Some(kind) = e.kind.master_kind()
        && !e.model.name.is_none()
        && !visible(design, scope, kind, e.model.name)
    {
        diags.push_at(MASTERLESS, design, at, 0);
    }
    // VACASK takes the kind from the master: a model of another kind's family reads back as
    // that kind.
    if e.kind.master_kind() == Some(ir::DefKind::Model) && !SOURCE_KINDS.contains(&e.kind) {
        let kinds = model_kinds(design, scope, e.model.name);
        if !kinds.is_empty() && !kinds.contains(&e.kind) {
            diags.push_at(KIND, design, at, 0);
        }
    }
    if e.kind == ir::ElemKind::Unresolved && !e.mult.is_none() {
        diags.push_at(UNRESOLVED_MULT, design, at, 0);
    }
    if e.kind == ir::ElemKind::Behav {
        let b = design.behav(&e);
        // VD dev-builtin-behavioral: no `$mfactor` either (law L5 seed 15).
        let extra = [b.tc1, b.tc2, b.temp_abs, b.dtemp, b.rpar, e.mult];
        if extra.iter().any(|x| !x.is_none()) {
            diags.push_at(BEHAV_FIELD, design, at, 0);
        }
    }
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
        return;
    };
    if families::module(&word).is_some_and(|m| usize::from(m.terminals) != e.terminals.len()) {
        diags.push_at(PORTS, design, at, 0);
    }
    let lacked = fields(design, &e).iter().any(|(key, _)| {
        key != "$mfactor" && !matches!(families::instance_name_in(&word, key), Name::Ir(_))
    });
    if lacked {
        diags.push_at(write_models::LACKED, design, at, 0);
    }
}

// ---- private ----

/// Whether a reachable `model` (of `kind` Model) or `subckt` (Subckt) statement named `name`
/// stands where VACASK looks from `scope`: in the element's own subckt (any scope inside it) or
/// at the top level of any file, sections and branches at the top included (the reader's
/// declared table; VD cir-masters "Scope"). A statement kept as VACASK's text counts too.
fn visible(design: &ir::Design, scope: ir::ScopeId, kind: ir::DefKind, name: ir::NameId) -> bool {
    let own = subckt_of(design, scope);
    let word = match kind {
        ir::DefKind::Subckt => "subckt ",
        _ => "model ",
    };
    let head = format!("{word}{} ", design.str(name));
    design.reachable().into_iter().any(|(at, s)| {
        let place = subckt_of(design, s);
        // A subckt's own row stands in its parent's scope.
        if place != own && !place.is_none() {
            return false;
        }
        match (at.kind, kind) {
            (ir::ItemKind::Model, ir::DefKind::Model) => {
                design.row::<ir::Model>(at.index).name == name
            }
            (ir::ItemKind::Subckt, ir::DefKind::Subckt) => {
                design.row::<ir::item::Subckt>(at.index).name == name
            }
            (ir::ItemKind::Opaque, _) => design
                .text(design.row::<ir::item::Opaque>(at.index).text)
                .starts_with(&head),
            _ => false,
        }
    })
}

/// The element kinds the visible models named `name` give an instance (`families::kind`).
fn model_kinds(design: &ir::Design, scope: ir::ScopeId, name: ir::NameId) -> Vec<ir::ElemKind> {
    let own = subckt_of(design, scope);
    design
        .reachable()
        .into_iter()
        .filter(|&(at, s)| {
            let place = subckt_of(design, s);
            at.kind == ir::ItemKind::Model && (place == own || place.is_none())
        })
        .map(|(at, _)| design.row::<ir::Model>(at.index))
        .filter(|m| m.name == name)
        .map(|m| families::kind(m.family))
        .collect()
}

/// The innermost subckt scope holding `scope` (itself included), `NONE` at the top level.
fn subckt_of(design: &ir::Design, scope: ir::ScopeId) -> ir::ScopeId {
    let mut at = scope;
    // Bounded: each step moves to a parent; the scope tree is acyclic (`ir::check`).
    for _ in 0..1 << 16 {
        if at.is_none() {
            break;
        }
        let s = design.scope(at);
        if s.kind == ir::ScopeKind::Subckt {
            return at;
        }
        at = s.parent;
    }
    ir::ScopeId::NONE
}

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
