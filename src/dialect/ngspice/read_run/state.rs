//! The state cards: `.options` → one `Option` per key the IR has (`OptionKey`), one `Private`
//! opaque per key it has not; `.ic`/`.nodeset` → one `NodeSet` per node. Each card is read whole
//! into its parts before the first push.
//!
//! Cites: ngspice-45 `cktsopt.c:246-330` (the options and their types), `inppas3.c:47-170`
//! (`.nodeset v(n)=x … | all=x`, `.ic v(n)=x …`, values by `INPevaluate`); runs: a repeated
//! key or node, the last wins; `.ic` in a subcircuit is each instance's own node;
//! `.options reltol 1e-4` is ngspice's "bad type" error and runs on without the key.
//!
//! Contents: `pub(in super::super) const KEYS`, `pub(super) fn options(&mut Lower, Stmt)`, `pub(super) fn node_sets(&mut Lower,
//! Stmt, ir::NodeSetKind)`

use super::super::read::{Lower, Stmt};
use super::words::{self, Val};
use super::{OPTION_FORM, PRIVATE_OPTION, RUN_INVALID};
use crate::ir::{self, OptionKey};

/// ngspice's spelling of each option the IR has a key for. `srcsteps` and `itl6` are one option
/// (`cktsopt.c:275-276`); `method=` is read by [`method`].
pub(in super::super) const KEYS: &[(&str, OptionKey)] = &[
    ("reltol", OptionKey::RelTol),
    ("abstol", OptionKey::AbsTol),
    ("vntol", OptionKey::VnTol),
    ("chgtol", OptionKey::ChgTol),
    ("trtol", OptionKey::TrTol),
    ("pivtol", OptionKey::PivTol),
    ("pivrel", OptionKey::PivRel),
    ("maxord", OptionKey::MaxOrd),
    ("itl1", OptionKey::Itl1),
    ("itl2", OptionKey::Itl2),
    ("itl4", OptionKey::Itl4),
    ("gminsteps", OptionKey::GminSteps),
    ("srcsteps", OptionKey::SrcSteps),
    ("itl6", OptionKey::SrcSteps),
    ("noopiter", OptionKey::NoOpIter),
    ("gmin", OptionKey::Gmin),
    ("rshunt", OptionKey::Rshunt),
    ("gshunt", OptionKey::Gshunt),
    ("cshunt", OptionKey::Cshunt),
    ("rseries", OptionKey::Rseries),
    ("scale", OptionKey::Scale),
    ("defl", OptionKey::DefL),
    ("defw", OptionKey::DefW),
    ("defad", OptionKey::DefAd),
    ("defas", OptionKey::DefAs),
    ("klu", OptionKey::Klu),
    ("sparse", OptionKey::Sparse),
    ("seed", OptionKey::Seed),
    ("autostop", OptionKey::Autostop),
    ("interp", OptionKey::Interp),
    ("acct", OptionKey::Acct),
    ("noacct", OptionKey::NoAcct),
    ("list", OptionKey::List),
    ("nomod", OptionKey::NoMod),
    ("nopage", OptionKey::NoPage),
    ("node", OptionKey::Node),
    ("opts", OptionKey::Opts),
    ("warn", OptionKey::Warn),
    ("maxwarns", OptionKey::MaxWarns),
    ("norefvalue", OptionKey::NoRefValue),
    ("keepopinfo", OptionKey::KeepOpInfo),
    ("noinit", OptionKey::NoInit),
];

/// One key of an `.options` card, read.
enum Part {
    /// A key the IR has, and its value (`None` for a flag).
    Typed(OptionKey, Option<Val>),
    /// Any other key: its text as one card.
    Private(String),
}

/// `.options k=v k …`: every key, in card order.
pub(super) fn options(lw: &mut Lower<'_, '_>, st: Stmt) {
    let w = words::split(&lw.rest(st.node));
    if w.iter()
        .any(|s| s.eq_ignore_ascii_case("temp") || s.eq_ignore_ascii_case("tnom"))
    {
        // ponytail: `temp=`/`tnom=` are `Env` rows, optional fields with A14; until it lands
        return lw.unread(st);
    }
    let mut parts = Vec::new();
    let mut k = 0;
    // Bounded: each step takes one or three words.
    while k < w.len() {
        let key = w[k].to_ascii_lowercase();
        let valued = w.get(k + 1).is_some_and(|s| s == "=");
        let given = if valued { w.get(k + 2) } else { None };
        if key == "=" || (valued && given.is_none()) {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, RUN_INVALID);
        }
        let typed = KEYS.iter().find(|(s, _)| *s == key).map(|&(_, o)| o);
        let part = match (typed, given) {
            (Some(o), None) if o.is_flag() => Part::Typed(o, None),
            (Some(o), Some(v)) if !o.is_flag() => match words::value(lw, st, v) {
                Ok(v) => Part::Typed(o, Some(v)),
                Err(f) => return lw.opaque(st, f.class, f.code),
            },
            // a value key with no `=`: ngspice's "bad type", the key left out (run)
            (Some(_), None) => return lw.opaque(st, ir::OpaqueClass::Unread, OPTION_FORM),
            (None, Some(v)) if key == "method" => match method(v) {
                Some(o) => Part::Typed(o, None),
                None => Part::Private(format!(".options {}={v}", w[k])),
            },
            (None, Some(v)) => Part::Private(format!(".options {}={v}", w[k])),
            (None, None) => Part::Private(format!(".options {}", w[k])),
            // a flag given a value: kept as ngspice's
            (Some(_), Some(v)) => Part::Private(format!(".options {}={v}", w[k])),
        };
        parts.push(part);
        k += if valued { 3 } else { 1 };
    }
    if parts.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, RUN_INVALID);
    }
    let span = lw.span(st.node);
    for part in &parts {
        match part {
            Part::Typed(key, v) => {
                let value = v
                    .as_ref()
                    .map_or(ir::ExprId::NONE, |v| words::emit(lw, st, v));
                // ponytail: A14 adds `applies: WholeRun` to this row when the run owner lands it
                lw.b.option(st.scope, *key, value, span);
            }
            Part::Private(text) => {
                // R19: a solver knob becomes `OpaqueClass::Hint` when that class is merged
                lw.b.opaque(st.scope, "ngspice", ir::OpaqueClass::Private, text, span);
                lw.diags.push(PRIVATE_OPTION, span, 0);
            }
        }
    }
}

/// `.ic v(n)=x …` (`Ic`, held in the transient's operating point only) or `.nodeset v(n)=x … |
/// all=x` (`Nodeset`, every solve): one row per node, in card order.
pub(super) fn node_sets(lw: &mut Lower<'_, '_>, st: Stmt, kind: ir::NodeSetKind) {
    let w = words::split(&lw.rest(st.node));
    let mut rows: Vec<(Vec<String>, Val)> = Vec::new();
    let mut k = 0;
    // Bounded: each step takes three words.
    while k < w.len() {
        let all = kind == ir::NodeSetKind::Nodeset && w[k].eq_ignore_ascii_case("all");
        let node = if all {
            Some(Vec::new())
        } else {
            match words::output(lw, &w[k]) {
                Some(words::Out::V(n, None)) => Some(n),
                _ => None,
            }
        };
        let (Some(node), Some("="), Some(v)) =
            (node, w.get(k + 1).map(String::as_str), w.get(k + 2))
        else {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, RUN_INVALID);
        };
        let v = match words::value(lw, st, v) {
            Ok(v) => v,
            Err(f) => return lw.opaque(st, f.class, f.code),
        };
        rows.push((node, v));
        k += 3;
        if all && k < w.len() {
            // `all=` ends ngspice's loop (inppas3.c:79-86): the rest is not read
            return lw.opaque(st, ir::OpaqueClass::Unread, OPTION_FORM);
        }
    }
    if rows.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, RUN_INVALID);
    }
    let applies = match kind {
        ir::NodeSetKind::Ic => ir::IcScope::Transient,
        _ => ir::IcScope::EveryOp,
    };
    let span = lw.span(st.node);
    for (node, v) in &rows {
        let node: Vec<&str> = node.iter().map(String::as_str).collect();
        // the path's names before the value's, the IR text's order (so a parse interns alike)
        for n in &node {
            lw.b.name(n);
        }
        let value = words::emit(lw, st, v);
        lw.b.node_set(st.scope, kind, applies, &node, value, span);
    }
}

// ---- private ----

/// The integration method `method=` names (`cktsopt.c:142-144`: a word that starts with
/// `trap`, or `gear`), or `None` for another word.
fn method(v: &str) -> Option<OptionKey> {
    let v = v.to_ascii_lowercase();
    if v.starts_with("trap") {
        Some(OptionKey::MethodTrap)
    } else {
        (v == "gear").then_some(OptionKey::MethodGear)
    }
}
