//! Script items → ngspice's control script: a `Load` as `pre_osdi`, an `Alter` as `alter`,
//! `altermod` or `alterparam` + `reset`, a KeepLast sweep `Loop` over a list as `foreach` (its
//! name the variable), each a command of the spine's script region (`Out::script`: one
//! `.control` block for adjacent commands; blocks concatenate, so the split is exact,
//! `rewrite/S3-CONTROL.md` O1). `needs_script` is the spine's one choice
//! per deck: a plan with an alter, a loop or a `FromHere` state item is written whole as a
//! script, analyses as commands (R49 (3) (i), the reader's own image). An unnamed loop's
//! variable is made up as `sr<depth>` (the loop's depth, itself included); the reader reads
//! exactly that name at that depth back as unnamed, so a user's own `sr1` at depth 1
//! round-trips to the same text (lead's answer, R42's pattern).
//!
//! Each function writes one item of a design legal for `FEATURES`, sees only the IR and never
//! learns a source. Codes: `W1400–W1499`.
//!
//! Cites: `rewrite/S3-CONTROL.md` S1–S5, V1, L1; `rewrite/RULINGS.md` R49; ngspice-45
//! `device.c:1249-1420`, `inp.c:1735-1857`, `control.c:462-466`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) const PARAM_RESET`, `LOOP_FORM`, `ALTER_VALUE`, `LOAD_SOURCE`, `LOAD_MODULES`, `RESET_PART`,
//! `pub(super) fn needs_script(&ir::Design, ir::ScopeId) -> bool`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn load`, `loop_`, `alter`, `reset`

use super::codes::Row;
use super::expr_write;
use super::write::Out;
use crate::ir;

/// What this area's writer spells: exactly its reader's image (L5).
pub(super) const FEATURES: ir::FeatureSet = super::read_control::FEATURES;

/// A parameter alter not right after a `Reset` (or a parameter loop whose body does not start
/// with one): ngspice applies an `alterparam` only at a `reset` (S2).
pub(super) const PARAM_RESET: ir::Code = ir::Code::new(b'W', 1401);
/// A loop ngspice's `foreach` cannot spell: not a sweep over a list of numbers.
pub(super) const LOOP_FORM: ir::Code = ir::Code::new(b'W', 1402);
/// An alter whose value is not a number (ngspice's `alter` reads the frontend's grammar, not
/// numparam), or a parameter alter whose value has no one-token spelling.
pub(super) const ALTER_VALUE: ir::Code = ir::Code::new(b'W', 1403);

/// A Verilog-A load: ngspice loads compiled OSDI binaries only (`pre_osdi`); compiling the
/// source is the backend's (OpenVAF).
pub(super) const LOAD_SOURCE: ir::Code = ir::Code::new(b'W', 1404);

/// A load that names its modules: `pre_osdi` names a file, and the modules are the binary's
/// own (ngspice binds a `.model`'s type word to them at load time), so no ngspice text states
/// the list.
pub(super) const LOAD_MODULES: ir::Code = ir::Code::new(b'W', 1405);

/// A partial restore (`Reset` of fewer than every class): ngspice's `reset` restores all.
pub(super) const RESET_PART: ir::Code = ir::Code::new(b'W', 1406);

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
    refusal(
        LOAD_SOURCE,
        "a Verilog-A source load",
        "ngspice-45 inp.c:791-797 (pre_osdi), osdi/osdiregistry.c",
        "ngspice loads a compiled OSDI binary, never a Verilog-A source.",
    ),
    refusal(
        RESET_PART,
        "a restore of some classes only",
        "ngspice-45 runcoms2.c:172-240 (com_rset rebuilds the circuit)",
        "ngspice's reset restores every alter, script option, temperature and save at once.",
    ),
    refusal(
        LOAD_MODULES,
        "a load that lists its modules",
        "ngspice-45 inp.c:791-797 (pre_osdi takes a path), osdi/osdiregistry.c",
        "pre_osdi names a file; the modules are the binary's, so ngspice has no text for the list.",
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
        ir::ItemKind::Alter | ir::ItemKind::Loop | ir::ItemKind::Reset => true,
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
    let mut refuse = |code| diags.push_at(code, design, item, 0);
    match item.kind {
        ir::ItemKind::Alter => {
            let a: ir::Alter = design.row(item.index);
            if !alter_value(design, &a) {
                refuse(ALTER_VALUE);
            }
            if a.target.kind == ir::TargetKind::Param && !after_reset(design, item) {
                refuse(PARAM_RESET);
            }
        }
        ir::ItemKind::Loop => match sweep(design, item) {
            None => refuse(LOOP_FORM),
            Some(s) if s.target.kind == ir::TargetKind::Param => {
                let l: ir::Loop = design.row(item.index);
                let first = design.items(l.body).first();
                if first.is_none_or(|i| i.kind != ir::ItemKind::Reset) {
                    refuse(PARAM_RESET);
                }
            }
            Some(_) => {}
        },
        // ponytail: only ngspice's whole `reset`; a partial restore (VACASK `clear saves`)
        // needs re-issuing the survivors after it and folding them on read
        ir::ItemKind::Reset
            if design.row::<ir::Reset>(item.index).restores != ir::ResetSet::ALL =>
        {
            refuse(RESET_PART);
        }
        ir::ItemKind::Load => {
            if design.row::<ir::item::Load>(item.index).kind != ir::LoadKind::Osdi {
                refuse(LOAD_SOURCE);
            }
            let defines = |d: &ir::scope::Def| d.kind == ir::DefKind::Module && d.item == item;
            if design.definitions().iter().any(defines) {
                refuse(LOAD_MODULES);
            }
        }
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

/// A `Reset` (every class: admits): the parameter alters listed right after it as
/// `alterparam`, then `reset`, which applies them (`inp.c:1735-1857`).
pub(super) fn reset(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let (items, at) = siblings(d, item);
    for &next in &items[at + 1..] {
        let Some(a) = param_alter(d, next) else {
            break;
        };
        let mut value = String::new();
        expr_write::value(d, a.value, &mut value);
        let value: String = value.split_whitespace().collect();
        w.script(&set_command(d, &a.target, &value));
    }
    w.script("reset");
}

/// A sweep `Loop`: `foreach <name> <values>`, the target set from the variable, the body, `end`
/// (a parameter's `alterparam`: the body's leading `Reset` applies it).
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

/// An `Alter`: `alter`, `altermod`; a parameter's `alterparam` is written by the `Reset`
/// before it (admits), since ngspice applies it at that reset.
pub(super) fn alter(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let a: ir::Alter = d.row(item.index);
    if a.target.kind == ir::TargetKind::Param {
        return;
    }
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
        ir::TargetKind::Param => format!("alterparam {a}={value}"),
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

/// The items of `item`'s scope and its index there.
// ponytail: a scan of `reachable` per call (quadratic in a deck's items); keep a scope index
// on `Out` if a corpus deck makes it show
fn siblings(d: &ir::Design, item: ir::ItemRef) -> (&[ir::ItemRef], usize) {
    let scope = d
        .reachable()
        .iter()
        .find(|(i, _)| *i == item)
        .map(|&(_, s)| s)
        .expect("ngspice write: the item is reachable");
    let items = d.items(scope);
    let at = items
        .iter()
        .position(|&i| i == item)
        .expect("an item of its scope");
    (items, at)
}

/// `i` as a parameter alter.
fn param_alter(d: &ir::Design, i: ir::ItemRef) -> Option<ir::Alter> {
    let a: ir::Alter = (i.kind == ir::ItemKind::Alter).then(|| d.row(i.index))?;
    (a.target.kind == ir::TargetKind::Param).then_some(a)
}

/// Whether a parameter alter stands right after a `Reset`, other parameter alters between.
fn after_reset(d: &ir::Design, item: ir::ItemRef) -> bool {
    let (items, at) = siblings(d, item);
    let before = items[..at]
        .iter()
        .rev()
        .find(|&&i| param_alter(d, i).is_none());
    before.is_some_and(|i| i.kind == ir::ItemKind::Reset)
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
