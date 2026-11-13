//! The state cards, each item `WholeRun` (R24: ngspice's state holds for the whole deck wherever
//! it stands): `.options` → one `Option` per key the IR has (`OptionKey`), `temp=`/`tnom=` one
//! `Env`, one `Private` (or `Hint`, R19) opaque per key it has not; `.temp` → an `Env` of the
//! temperature; `.ic`/`.nodeset` → one `NodeSet` per node. Each card is read whole into its parts
//! before the first push.
//!
//! Cites: ngspice-45 `cktsopt.c:246-330` (the options and their types), `inppas3.c:47-170`
//! (`.nodeset v(n)=x … | all=x`, `.ic v(n)=x …`, values by `INPevaluate`); runs: a repeated
//! key or node, the last wins; `.ic` in a subcircuit is each instance's own node;
//! `.options reltol 1e-4` is ngspice's "bad type" error and runs on without the key; `.temp`
//! beats `.options temp=` in either order and the last `.temp` wins (`inp.c:1159-1186`).
//!
//! Contents: `pub(in super::super) const KEYS`, `pub(super) fn options(&mut Lower, Stmt)`,
//! `pub(super) fn temp(&mut Lower, Stmt)`, `pub(super) fn node_sets(&mut Lower, Stmt,
//! ir::NodeSetKind)`

use super::super::read::{Lower, Stmt};
use super::super::value::{Val, emit};
use super::words;
use super::{OPTION_FORM, PRIVATE_OPTION, RUN_INVALID, TEMP_BEATEN};
use crate::ir::{self, OptionKey};

/// HSPICE's spelling of each option the IR has a key for (the ngspice dialect's list, a lead, but
/// `gmin`, `gmindc`, `delmax`, `search`). `srcsteps` and `itl6` are one option
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
    // HSPICE's `gmin` is the transient's, `gmindc` the DC analyses' (IR `option_key.rs`; the
    // VAMS conformance suite's gmindc.sp, cshunt_delmax.sp, search_lib.sp run on ESPice)
    ("gmin", OptionKey::GminTran),
    ("gmindc", OptionKey::GminDc),
    ("delmax", OptionKey::MaxStep),
    ("search", OptionKey::Search),
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
    ("nfreqs", OptionKey::NFreqs),
    ("fourgridsize", OptionKey::FourGridSize),
];

/// ngspice's options that tune its solver and name no accuracy target (R19: `Hint`), from
/// `cktsopt.c`'s table: the stepping factor, device bypass, SPICE2 limiting, Newton node
/// damping and its limits, the skipped operating point before a linear AC, nodeset copying, the
/// KLU memory growth, and XSPICE's analog/event alternation and code-model convergence aids.
/// Every other key it has (`xmu`, `minbreak`, `epsmin`, `ramptime`, `defm`, `badmos3`,
/// `trytocompact`, the SPICE2 output keys) names an accuracy target, changes the equations or is
/// not established: `Private`.
const HINTS: &[&str] = &[
    "gminfactor",
    "bypass",
    "oldlimit",
    "nodedamping",
    "absdv",
    "reldv",
    "noopac",
    "copynodesets",
    "klu_memgrow_factor",
    "noopalter",
    "maxopalter",
    "maxevtiter",
    "convlimit",
    "convstep",
    "convabsstep",
    "autopartial",
];

/// One key of an `.options` card, read.
enum Part {
    /// A key the IR has, and its value (`None` for a flag).
    Typed(OptionKey, Option<Val>),
    /// `temp=` and `tnom=`: one `Env` (either may be absent).
    Env(Option<Val>, Option<Val>),
    /// Any other key, or a `temp=` a `.temp` card beats: its class and its text as one card.
    Kept(ir::OpaqueClass, ir::Code, String),
}

/// `.options k=v k …`: every key, in card order; `temp=` and `tnom=` together one `Env` at the
/// place of the first.
pub(super) fn options(lw: &mut Lower<'_, '_>, st: Stmt) {
    let w = words::split(&lw.rest(st.node));
    let beaten = has_temp_card(lw);
    let mut parts = Vec::new();
    let (mut temp, mut tnom, mut env_at) = (None, None, None);
    let mut k = 0;
    // Bounded: each step takes one or three words.
    while k < w.len() {
        let key = w[k].to_ascii_lowercase();
        let valued = w.get(k + 1).is_some_and(|s| s == "=");
        let given = if valued { w.get(k + 2) } else { None };
        if key == "=" || (valued && given.is_none()) {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, RUN_INVALID);
        }
        let text = |v: Option<&String>| match v {
            Some(v) => format!(".options {}={v}", w[k]),
            None => format!(".options {}", w[k]),
        };
        let typed = KEYS.iter().find(|(s, _)| *s == key).map(|&(_, o)| o);
        let class = if HINTS.contains(&key.as_str()) {
            ir::OpaqueClass::Hint
        } else {
            ir::OpaqueClass::Private
        };
        let part = match (key.as_str(), typed, given) {
            ("temp" | "tnom", _, None) => {
                return lw.opaque(st, ir::OpaqueClass::Unread, OPTION_FORM);
            }
            // `.temp` beats `.options temp=` in either order (inp.c:1159-1186; runs)
            ("temp", _, Some(v)) if beaten => {
                Part::Kept(ir::OpaqueClass::Inert, TEMP_BEATEN, text(Some(v)))
            }
            ("temp" | "tnom", _, Some(v)) => {
                let v = match words::value(lw, st, v) {
                    Ok(v) => Some(v),
                    Err(f) => return lw.opaque(st, f.class, f.code),
                };
                if key == "temp" {
                    temp = v;
                } else {
                    tnom = v;
                }
                if env_at.is_some() {
                    k += 3;
                    continue;
                }
                env_at = Some(parts.len());
                Part::Env(None, None)
            }
            (_, Some(o), None) if o.is_flag() => Part::Typed(o, None),
            // a directory, a string (`search='dir'`)
            (_, Some(OptionKey::Search), Some(v)) => {
                Part::Typed(OptionKey::Search, Some(Val::Str(v.trim_matches(['\'', '"']).to_string())))
            }
            (_, Some(o), Some(v)) if !o.is_flag() => match words::value(lw, st, v) {
                Ok(v) => Part::Typed(o, Some(v)),
                Err(f) => return lw.opaque(st, f.class, f.code),
            },
            // a value key with no `=`: ngspice's "bad type", the key left out (run)
            (_, Some(_), None) => return lw.opaque(st, ir::OpaqueClass::Unread, OPTION_FORM),
            ("method", None, Some(v)) if method(v).is_some() => {
                Part::Typed(method(v).expect("checked"), None)
            }
            // another key, or a flag given a value: kept as ngspice's
            (_, _, v) => Part::Kept(class, PRIVATE_OPTION, text(v)),
        };
        parts.push(part);
        k += if valued { 3 } else { 1 };
    }
    if let Some(at) = env_at {
        parts[at] = Part::Env(temp, tnom);
    }
    if parts.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, RUN_INVALID);
    }
    let span = lw.span(st.node);
    let none = ir::ExprId::NONE;
    for part in &parts {
        match part {
            Part::Typed(key, v) => {
                let value = v.as_ref().map_or(none, |v| emit(lw, st, v));
                let row = ir::OptionRow {
                    key: *key,
                    private: ir::NameId::NONE,
                    dialect: ir::NameId::NONE,
                    value,
                    holds: ir::Applies::WholeRun,
                };
                lw.b.push(st.scope, row, span);
            }
            Part::Env(t, n) => {
                let temp = t.as_ref().map_or(none, |v| emit(lw, st, v));
                let tnom = n.as_ref().map_or(none, |v| emit(lw, st, v));
                let row = ir::Env {
                    temp,
                    tnom,
                    holds: ir::Applies::WholeRun,
                };
                lw.b.push(st.scope, row, span);
            }
            Part::Kept(class, code, text) => {
                lw.b.opaque(st.scope, "hspice", *class, text, span);
                lw.diags.push(*code, span, 0);
            }
        }
    }
}

/// `.temp t` or `.temp=t`: one `Env` setting the temperature. The deck's one top-level list
/// (`.temp 27 50`) is the loop around its analyses (`read_run::deck_loop`; the card itself is
/// framing); a list anywhere else, or one whose values do not read, is `Unread`.
pub(super) fn temp(lw: &mut Lower<'_, '_>, st: Stmt) {
    if lw.temp_card == Some(st.node) && super::deck_loop(lw, st).is_some() {
        return lw.framing();
    }
    let rest = lw.rest(st.node);
    let text = rest.trim().strip_prefix('=').unwrap_or(rest.trim()).trim();
    let w = words::split(text);
    let [one] = w.as_slice() else {
        return lw.opaque(st, ir::OpaqueClass::Unread, OPTION_FORM);
    };
    let v = match words::value(lw, st, one) {
        Ok(v) => v,
        Err(f) => return lw.opaque(st, f.class, f.code),
    };
    let span = lw.span(st.node);
    let temp = emit(lw, st, &v);
    let row = ir::Env {
        temp,
        tnom: ir::ExprId::NONE,
        holds: ir::Applies::WholeRun,
    };
    lw.b.push(st.scope, row, span);
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
            words::node(lw, &w[k])
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
        let value = emit(lw, st, v);
        let node = lw.b.names(&node);
        let row = ir::NodeSet {
            kind,
            applies,
            next_only: false,
            node,
            reference: ir::List::EMPTY,
            value,
            holds: ir::Applies::WholeRun,
        };
        lw.b.push(st.scope, row, span);
    }
}

// ---- private ----

/// Whether this file has a `.temp` card (ngspice's `ciprefix(".temp")`, `inp.c:1159`).
/// ponytail: this file's lines only; a `.temp` in another file of the deck is not seen.
fn has_temp_card(lw: &Lower<'_, '_>) -> bool {
    let title = usize::from(lw.role == ir::Role::Deck);
    lw.toks.source().lines().skip(title).any(|l| {
        let l = l.trim_start().to_ascii_lowercase();
        l.strip_prefix(".temp")
            .is_some_and(|r| !r.starts_with(|c: char| c.is_ascii_alphanumeric()))
    })
}

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
