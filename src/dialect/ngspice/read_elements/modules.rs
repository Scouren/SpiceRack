//! X, N and A cards → `Instance`, `ModuleModel` and `CodeModel` rows.
//!
//! - `X n1 … nk sub [params:] k=v …`: the last word before the parameters is the subcircuit;
//!   `m=` is the multiplier under ngspice's append law (`MultLaw::NgspiceAppend`, R31:
//!   `inp_fix_subckt_multiplier` hands `m={m}` to the inner cards) unless the subcircuit
//!   declares `m` (`inp_fix_inst_calls_for_numparam`, `found_mult_param`; INV 6.01), then it is a
//!   parameter;
//! - `N n1 … nk model k=v …` (OSDI, `inp2n.c`): every parameter, `m` too, is the module's own;
//! - `A n1 … nk model` (XSPICE): typed only when every connection is a plain node; a port
//!   modifier (`%v`, `%id`, `[ ]`, `~`) has no IR field (`Private`).
//!
//! Contents: `pub(super) fn x`, `pub(super) fn n`, `pub(super) fn a` (each
//! `(lw: &mut Lower<'_, '_>, st: Stmt, card: Card, items: &[Item])`)

use super::super::read::{Lower, Stmt};
use super::{Card, Item, Kind, MALFORMED, PRIVATE_FORM, UNREAD_FORM, head, push_done, val};
use crate::ir::{self, ElemKind as K};

/// An X card.
pub(super) fn x(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item]) {
    let params_word = |i: &Item| i.kind == Kind::Word && i.text.eq_ignore_ascii_case("params:");
    let words = items
        .iter()
        .take_while(|i| i.kind == Kind::Word && !params_word(i) && i.text != "=")
        .count();
    let mut rest = &items[words..];
    if rest.first().is_some_and(params_word) {
        rest = &rest[1..];
    }
    if words == 0 || rest.iter().any(|i| i.kind != Kind::Assign) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    let sub = items[words - 1].text.clone();
    let declares_m = lw
        .subckt_formals(st, &sub)
        .is_some_and(|f| f.iter().any(|p| p.eq_ignore_ascii_case("m")));
    card.nodes = items[..words - 1].iter().map(|i| i.text.clone()).collect();
    card.model = Some(sub);
    if !keyed(lw, st, &mut card, rest, !declares_m) {
        return;
    }
    let mut h = head(lw, st, &card);
    if !h.mult.is_none() {
        // inp_fix_subckt_multiplier: ngspice's append law, not parallel copies (R31)
        h.mult_law = ir::MultLaw::NgspiceAppend;
    }
    let span = lw.span(st.node);
    let item = lw.b.instance(st.scope, K::Instance, h, span);
    push_done(lw, st, item, &card);
}

/// An N card.
pub(super) fn n(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item]) {
    let words = items.iter().take_while(|i| i.kind == Kind::Word).count();
    if words == 0 || items[words..].iter().any(|i| i.kind != Kind::Assign) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    card.nodes = items[..words - 1].iter().map(|i| i.text.clone()).collect();
    card.model = Some(items[words - 1].text.clone());
    if !keyed(lw, st, &mut card, &items[words..], false) {
        return;
    }
    let h = head(lw, st, &card);
    let span = lw.span(st.node);
    let item = lw.b.instance(st.scope, K::ModuleModel, h, span);
    push_done(lw, st, item, &card);
}

/// An A card.
pub(super) fn a(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item]) {
    let plain = |i: &Item| i.kind == Kind::Word && !i.text.contains(['%', '[', ']', '~']);
    if items.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    if !items.iter().all(plain) {
        return lw.opaque(st, ir::OpaqueClass::Private, PRIVATE_FORM);
    }
    let (model, nodes) = items.split_last().expect("one item");
    card.nodes = nodes.iter().map(|i| i.text.clone()).collect();
    card.model = Some(model.text.clone());
    let h = head(lw, st, &card);
    let span = lw.span(st.node);
    let item = lw.b.instance(st.scope, K::CodeModel, h, span);
    push_done(lw, st, item, &card);
}

// ---- private ----

/// Reads `k=v` parameters into `card` (`m` into `mult` when `mult` says so); on a refusal
/// pushes the opaque and returns false.
fn keyed(lw: &mut Lower<'_, '_>, st: Stmt, card: &mut Card, rest: &[Item], mult: bool) -> bool {
    for (k, it) in rest.iter().enumerate() {
        let dup = rest[..k]
            .iter()
            .any(|p| p.text.eq_ignore_ascii_case(&it.text));
        if dup {
            // the IR holds one value per key; ngspice's winner is not established
            lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
            return false;
        }
        let v = match val(lw, st, &it.value) {
            Ok(v) => v,
            Err(e) => {
                lw.opaque(st, e.class, e.code);
                return false;
            }
        };
        if mult && it.text.eq_ignore_ascii_case("m") {
            card.mult = Some(v);
        } else {
            card.params.push((it.text.clone(), v));
        }
    }
    true
}
