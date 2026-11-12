//! Script items → ngspice's control script: a `Load` as `pre_osdi`, an `Alter` as `alter`,
//! `altermod` or `alterparam` + `reset`, a KeepLast sweep `Loop` over a list as `foreach` (its
//! name the variable), each a command of the spine's script region (`Out::script`: one
//! `.control` block for adjacent commands; blocks concatenate, so the split is exact,
//! `rewrite/S3-CONTROL.md` O1). `needs_script` is the spine's one choice
//! per deck: a plan with an alter, a loop or a `FromHere` state item is written whole as a
//! script, analyses as commands (R49 (3) (i), the reader's own image).
//!
//! Each function writes one item of a design legal for `FEATURES`, sees only the IR and never
//! learns a source. Codes: `W1400–W1499`.
//!
//! Cites: `rewrite/S3-CONTROL.md` S1–S5, V1, L1; `rewrite/RULINGS.md` R49; ngspice-45
//! `device.c:1249-1420`, `inp.c:1735-1857`, `control.c:462-466`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) const PARAM_RESET`, `LOOP_FORM`, `ALTER_VALUE`,
//! `pub(super) fn needs_script(&ir::Design, ir::ScopeId) -> bool`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn load`, `loop_`, `alter`

use super::codes::Row;
use super::expr_write;
use super::write::Out;
use crate::ir;

/// What this area's writer spells: exactly its reader's image (L5).
pub(super) const FEATURES: ir::FeatureSet = super::read_control::FEATURES;

/// An `Alter` of a parameter where another alter or a `FromHere` state item holds: the
/// `reset` ngspice needs to apply an `alterparam` would drop it (S1).
pub(super) const PARAM_RESET: ir::Code = ir::Code::new(b'W', 1401);
/// A loop ngspice's `foreach` cannot spell: not a sweep over a list of numbers.
pub(super) const LOOP_FORM: ir::Code = ir::Code::new(b'W', 1402);
/// An alter whose value is not a number (ngspice's `alter` reads the frontend's grammar, not
/// numparam), or a parameter alter whose value has no one-token spelling.
pub(super) const ALTER_VALUE: ir::Code = ir::Code::new(b'W', 1403);

/// This area's codes, `W1400–W1499`.
pub(super) const CODES: &[Row] = &[
    refusal(
        PARAM_RESET,
        "a parameter alter where other script state holds",
        "ngspice-45 runcoms2.c:172-240, inp.c:1735-1857; S3-CONTROL S1, S2",
        "ngspice applies an alterparam only at a reset, which drops every alter and script option before it.",
    ),
    refusal(
        LOOP_FORM,
        "a loop ngspice's foreach cannot spell",
        "ngspice-45 control.c:462-466",
        "foreach runs over words: a sweep loop over a list of numbers, keeping the last value.",
    ),
    refusal(
        ALTER_VALUE,
        "an alter value ngspice cannot read",
        "ngspice-45 device.c:1249-1420 (ft_getpnames), inp.c:1761 (gettok)",
        "an alter takes one number; an alterparam one token.",
    ),
];

/// Whether `scope`'s run plan needs the script form: among its run items (loop bodies
/// included) an `Alter`, a `Loop`, a `FromHere` state item, or a script command kept as its
/// own `.control` pair (a display command's place among the analyses is kept only by a
/// script). A whole-script `Script` opaque does not: its deck keeps its cards (R49 (3)).
#[allow(
    dead_code,
    reason = "the spine calls it once ng-scaffold's `Out::scripted` lands"
)]
pub(super) fn needs_script(design: &ir::Design, scope: ir::ScopeId) -> bool {
    design.items(scope).iter().any(|&item| match item.kind {
        ir::ItemKind::Alter | ir::ItemKind::Loop => true,
        ir::ItemKind::Option => {
            design.row::<ir::OptionRow>(item.index).holds == ir::Applies::FromHere
        }
        ir::ItemKind::Env => design.row::<ir::Env>(item.index).holds == ir::Applies::FromHere,
        ir::ItemKind::Opaque => {
            let o: ir::item::Opaque = design.row(item.index);
            o.class != ir::OpaqueClass::Script
                && design.str(o.dialect) == "ngspice"
                && design.text(o.text).starts_with(".control\n")
        }
        _ => false,
    })
}

/// Appends a refusal for this area's items ngspice cannot spell for a reason no feature states
/// (R21, `Write::admits`).
pub(super) fn admits(design: &ir::Design, item: ir::ItemRef, diags: &mut ir::Diags) {
    let span = design.span_of(item);
    let mut refuse = |code| diags.push(code, span, 0);
    match item.kind {
        ir::ItemKind::Alter => {
            let a: ir::Alter = design.row(item.index);
            if !alter_value(design, &a) {
                refuse(ALTER_VALUE);
            }
            // ponytail: refused, not re-issued after the reset; re-issue the survivors and fold
            // them on read when a pass first needs it (lead's Q1 answer)
            if a.target.kind == ir::TargetKind::Param && live_before(design, item) {
                refuse(PARAM_RESET);
            }
        }
        ir::ItemKind::Loop if sweep(design, item).is_none() => refuse(LOOP_FORM),
        _ => {}
    }
}

/// A `Load`: `pre_osdi <path>` (an OSDI binary; FEATURES lacks a Verilog-A load).
pub(super) fn load(w: &mut Out<'_>, item: ir::ItemRef) {
    let l: ir::item::Load = w.design.row(item.index);
    assert!(
        l.kind == ir::LoadKind::Osdi,
        "ngspice write: only OSDI loads are in FEATURES"
    );
    let cmd = format!("pre_osdi {}", w.design.str(l.path));
    w.script(&cmd);
}

/// A sweep `Loop`: `foreach <name> <values>`, the target set from the variable, the body, `end`.
pub(super) fn loop_(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let l: ir::Loop = d.row(item.index);
    let s = sweep(d, item).expect("ngspice write: admits refused every other loop");
    let var = if l.name.is_none() {
        format!("sr{}", depth(d, item))
    } else {
        d.str(l.name).to_string()
    };
    let mut head = format!("foreach {var}");
    for &e in d.exprs(s.sweep.values) {
        head.push(' ');
        expr_write::value(d, e, &mut head);
    }
    w.script(&head);
    let set = set_command(d, &s.target, &format!("${var}"));
    for line in set.lines() {
        w.script(line);
    }
    w.open_script(l.body, "end".to_string());
}

/// An `Alter`: `alter`, `altermod`, or `alterparam` + `reset`.
pub(super) fn alter(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let a: ir::Alter = d.row(item.index);
    let mut value = String::new();
    expr_write::value(d, a.value, &mut value);
    let value: String = value.split_whitespace().collect();
    for line in set_command(d, &a.target, &value).lines() {
        w.script(line);
    }
}

// ---- private ----

/// A refusal row.
const fn refusal(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
    (
        code,
        ir::Info {
            title,
            cite,
            explain,
            kind: ir::Kind::Refusal,
        },
    )
}

/// The command(s) that set `target` to `value` (a number or `$var`).
fn set_command(d: &ir::Design, t: &ir::Target, value: &str) -> String {
    let a = d.str(t.a.name);
    match t.kind {
        ir::TargetKind::ElementValue | ir::TargetKind::SourceDc => format!("alter {a} {value}"),
        ir::TargetKind::DeviceParam => format!("alter @{a}[{}]={value}", d.str(t.b)),
        ir::TargetKind::ModelParam => format!("altermod {a} {}={value}", d.str(t.b)),
        ir::TargetKind::Param => format!("alterparam {a}={value}\nreset"),
        other => unreachable!("ngspice write: FEATURES lacks an alter or loop of {other:?}"),
    }
}

/// A loop's sweep detail, when it is ngspice's `foreach`: a KeepLast sweep over a list.
fn sweep(d: &ir::Design, item: ir::ItemRef) -> Option<ir::plan::SweepLoop> {
    let l: ir::Loop = d.row(item.index);
    if l.kind != ir::LoopKind::Sweep || l.after != ir::LoopEnd::KeepLast {
        return None;
    }
    let s: ir::plan::SweepLoop = d.detail(l.detail);
    let numbers = d
        .exprs(s.sweep.values)
        .iter()
        .all(|&e| matches!(d.expr(e), ir::Expr::Num(_)));
    (s.sweep.kind == ir::SweepKind::List && numbers).then_some(s)
}

/// Whether an alter's value has a spelling ngspice reads: a number for a device, model or
/// source; for a parameter any expression (written with its blanks removed: one token).
fn alter_value(d: &ir::Design, a: &ir::Alter) -> bool {
    a.target.kind == ir::TargetKind::Param || matches!(d.expr(a.value), ir::Expr::Num(_))
}

/// Whether an alter or `FromHere` state item stands before `item` in its scope (the `reset`
/// of an `alterparam` would drop it).
fn live_before(d: &ir::Design, item: ir::ItemRef) -> bool {
    let Some(&(_, scope)) = d.reachable().iter().find(|(i, _)| *i == item) else {
        return false;
    };
    for &i in d.items(scope) {
        if i == item {
            return false;
        }
        let live = match i.kind {
            ir::ItemKind::Alter => d.row::<ir::Alter>(i.index).target.kind != ir::TargetKind::Param,
            ir::ItemKind::Option => d.row::<ir::OptionRow>(i.index).holds == ir::Applies::FromHere,
            ir::ItemKind::Env => d.row::<ir::Env>(i.index).holds == ir::Applies::FromHere,
            _ => false,
        };
        if live {
            return true;
        }
    }
    false
}

/// How many loops enclose `item`'s body, itself included (a made-up variable's number).
fn depth(d: &ir::Design, item: ir::ItemRef) -> usize {
    let l: ir::Loop = d.row(item.index);
    let mut n = 0;
    let mut s = l.body;
    // Bounded: each step goes one scope outward.
    while !s.is_none() {
        if d.scope(s).kind == ir::ScopeKind::Loop {
            n += 1;
        }
        s = d.scope(s).parent;
    }
    n
}
