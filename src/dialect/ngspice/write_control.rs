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
//! `pub(super) const PARAM_RESET`, `LOOP_FORM`, `ALTER_VALUE`, `LOAD_SOURCE`, `LOAD_MODULES`, `RESET_PART`, `ALTER_TARGET`, `TARGET_MISSING`,
//! `pub(super) fn needs_script(&ir::Design, ir::ScopeId) -> bool`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn missing_targets(&ir::Design, &mut ir::Diags)`,
//! `pub(super) fn load`, `loop_`, `alter`, `reset`

use super::codes::Row;
use super::expr_write;
use super::write::Out;
use crate::ir;

/// What this area's writer spells: exactly its reader's image (L5).
pub(super) const FEATURES: ir::FeatureSet = super::read_control::FEATURES;

/// A parameter alter not right before a `Reset` (or a parameter loop whose body does not start
/// with one): ngspice applies an `alterparam` only at a `reset` (S2; settle
/// `alter_param_noreset.sp`: the next op runs at the old value, silently). Here in `admits`
/// (R21) until the IR states an alter's effect as a feature (R70, A43); the branch goes with it.
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

/// An alter or loop target `alter <name> <value>` does not set as the IR means it: an element
/// value of a name that is no R, C or L, a source DC of one that is no V or I (the reader's
/// image, `read_control/args.rs` `alter`; a bare alter of another device sets its own default
/// parameter, `spiceif.c:951`).
pub(super) const ALTER_TARGET: ir::Code = ir::Code::new(b'W', 1407);

/// An alter, loop or DC axis whose element or model no statement of the design defines (an
/// unbound model, an element name no element has): ngspice prints "no such device or model
/// name" and runs on at the old value (settle `alter_missing_device.sp`, F7), or aborts the DC
/// analysis ("… is not in the circuit", `dc_missing_source.sp`). Here in `admits` (R21) until
/// the IR states an alter's effect as a feature (R70, A43); the branch goes with it.
pub(super) const TARGET_MISSING: ir::Code = ir::Code::new(b'W', 1408);

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
        ALTER_TARGET,
        "an alter of an element value or source DC by a name of another device",
        "ngspice-45 device.c:1249-1420, spiceif.c:951 (a bare alter sets the device's default parameter)",
        "an element value is an R's, C's or L's, a source DC a V's or I's; another letter's default parameter means something else.",
    ),
    refusal(
        TARGET_MISSING,
        "an alter of a device or model the design does not hold",
        "ngspice-45 run (settle alter_missing_device.sp: 'no such device or model name', the old value kept, exit 0)",
        "ngspice reports the alter and runs every later analysis at the old value: a wrong number; refused.",
    ),
    refusal(
        LOAD_MODULES,
        "a load that lists its modules",
        "ngspice-45 inp.c:791-797 (pre_osdi takes a path), osdi/osdiregistry.c",
        "pre_osdi names a file; the modules are the binary's, so ngspice has no text for the list.",
    ),
];

/// Whether `scope`'s run plan needs the script form: among its run items, and those of the
/// `.lib` sections it defines (every `.control` block of a file is one script, read_control:
/// L5 seed 19, a section's `option` beside a top `.op` card read back as P1404), an `Alter`, a
/// `Loop`, a `Reset`, a `FromHere` state item or save, or a script command kept as its own
/// `.control` pair (a display command's place among the analyses is kept only by a script). A
/// whole-script `Script` opaque does not: its deck keeps its cards (R49 (3)).
#[allow(
    dead_code,
    reason = "the spine calls it once ng-scaffold's `Out::scripted` lands"
)]
pub(super) fn needs_script(design: &ir::Design, scope: ir::ScopeId) -> bool {
    let mut scopes = vec![scope];
    // Bounded: each section body is pushed once by the section item that owns it.
    while let Some(at) = scopes.pop() {
        for &item in design.items(at) {
            if item.kind == ir::ItemKind::Section {
                scopes.push(design.row::<ir::item::Section>(item.index).body);
            }
        }
        if scripted_items(design, at) {
            return true;
        }
    }
    false
}

/// Appends a refusal for this area's items ngspice cannot spell for a reason no feature states
/// (R21, `Write::admits`). `next` is the item after `item` in its scope (`ItemRef::NONE` at the
/// end), handed by the spine so the check needs no walk of its own.
pub(super) fn admits(
    design: &ir::Design,
    item: ir::ItemRef,
    next: ir::ItemRef,
    diags: &mut ir::Diags,
) {
    let mut refuse = |code| diags.push_at(code, design, item, 0);
    match item.kind {
        ir::ItemKind::Alter => {
            let a: ir::Alter = design.row(item.index);
            if !alter_value(design, &a) {
                refuse(ALTER_VALUE);
            }
            if !bare_target(design, &a.target) {
                refuse(ALTER_TARGET);
            }

            if a.target.kind == ir::TargetKind::Param && !applied(design, next) {
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
            Some(s) if !bare_target(design, &s.target) => refuse(ALTER_TARGET),
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

/// A [`TARGET_MISSING`] refusal at each alter, sweep loop and DC axis whose element or model
/// the design does not hold: one walk to collect the element names, one over the items
/// (`write::admits` calls it once).
pub(super) fn missing_targets(design: &ir::Design, diags: &mut ir::Diags) {
    let items = design.reachable();
    let names: std::collections::HashSet<String> = items
        .iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Element)
        .map(|(i, _)| {
            let e: ir::Element = design.row(i.index);
            design.str(e.name).to_ascii_lowercase()
        })
        .collect();
    // ponytail: an element of that name anywhere; the target's instance path is not followed
    let held = |t: &ir::Target| match t.kind {
        // `altermod` takes a model or an instance of it (corpus vdmos/…-quasisat.sp: `mn1`)
        ir::TargetKind::ModelParam => {
            let name = design.str(t.a.name).to_ascii_lowercase();
            !t.a.def.is_none() || names.contains(&name)
        }
        ir::TargetKind::ElementValue | ir::TargetKind::SourceDc | ir::TargetKind::DeviceParam => {
            names.contains(&design.str(t.a.name).to_ascii_lowercase())
        }
        _ => true,
    };
    for &(item, _) in &items {
        let targets: Vec<ir::Target> = match item.kind {
            ir::ItemKind::Alter => vec![design.row::<ir::Alter>(item.index).target],
            ir::ItemKind::Loop => sweep(design, item).map(|s| s.target).into_iter().collect(),
            ir::ItemKind::Analysis => {
                let a: ir::Analysis = design.row(item.index);
                if a.kind == ir::AnalysisKind::Dc {
                    let dc: ir::dc_ac::Dc = design.detail(a.detail);
                    design.dc_axes(dc.axes).iter().map(|x| x.target).collect()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
        if !targets.iter().all(held) {
            diags.push_at(TARGET_MISSING, design, item, 0);
        }
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

/// A `Reset` (every class: admits): `reset`, which also applies the `alterparam`s written
/// right before it (`inp.c:1735-1857`).
pub(super) fn reset(w: &mut Out<'_>, _item: ir::ItemRef) {
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

/// An `Alter`: `alter`, `altermod`, or a parameter's `alterparam`, which the `Reset` right
/// after it applies (admits).
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

/// Whether one scope's own items need the script form (`needs_script`'s test per scope).
fn scripted_items(design: &ir::Design, scope: ir::ScopeId) -> bool {
    design.items(scope).iter().any(|&item| match item.kind {
        ir::ItemKind::Alter | ir::ItemKind::Loop | ir::ItemKind::Reset => true,
        ir::ItemKind::Option => {
            design.row::<ir::OptionRow>(item.index).holds == ir::Applies::FromHere
        }
        ir::ItemKind::Env => design.row::<ir::Env>(item.index).holds == ir::Applies::FromHere,
        ir::ItemKind::Save => design.row::<ir::Save>(item.index).holds == ir::Applies::FromHere,
        ir::ItemKind::Opaque => {
            let o: ir::item::Opaque = design.row(item.index);
            o.class != ir::OpaqueClass::Script
                && design.str(o.dialect) == "ngspice"
                && design.text(o.text).starts_with(".control\n")
        }
        _ => false,
    })
}

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

/// Whether a bare `alter <name>` sets `t` as the IR means it: an element value of an R, C or L,
/// a source DC of a V or I; every other kind by its own form.
fn bare_target(d: &ir::Design, t: &ir::Target) -> bool {
    let letter = d
        .str(t.a.name)
        .as_bytes()
        .first()
        .map(u8::to_ascii_lowercase);
    match t.kind {
        ir::TargetKind::ElementValue => matches!(letter, Some(b'r' | b'c' | b'l')),
        ir::TargetKind::SourceDc => matches!(letter, Some(b'v' | b'i')),
        _ => true,
    }
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

/// `i` as a parameter alter.
fn param_alter(d: &ir::Design, i: ir::ItemRef) -> Option<ir::Alter> {
    let a: ir::Alter = (i.kind == ir::ItemKind::Alter).then(|| d.row(i.index))?;
    (a.target.kind == ir::TargetKind::Param).then_some(a)
}

/// Whether a parameter alter followed by `next` is applied by a `Reset`: `next` is the
/// `Reset`, or another parameter alter (whose own check looks one item further).
fn applied(d: &ir::Design, next: ir::ItemRef) -> bool {
    next.kind == ir::ItemKind::Reset
        || (next != ir::ItemRef::NONE && param_alter(d, next).is_some())
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
