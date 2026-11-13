//! Run items → ngspice dot cards: `Analysis` → `.op .dc .ac .tran .noise .tf .sens .pz .disto
//! .sp .pss .four`, each in its positional form; `Option`, `Env`, `NodeSet` → `.options`, `.temp`,
//! `.ic`, `.nodeset` (with AMEND-A12 A14). Items are written in IR order (R25: no writer
//! reorders run items); ngspice has no statistics block.
//!
//! Each function writes one item of a design legal for `FEATURES` (the spine checked it), sees
//! only the IR, and never learns a source. `FEATURES` is `read_run::FEATURES`, the reader's
//! image (L5); what a feature cannot state and ngspice cannot spell is refused by [`admits`]
//! (R21). Codes: `W4300–W4349` (this file).
//!
//! Cites: ngspice-45 `inp2dot.c` (each card's form), `traninit.c:29` (a zero `tmax` is none);
//! `rewrite/DIALECT-HSPICE.md` §4; `rewrite/RULINGS.md` R21, R24–R26.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn analysis`, `loop_`, `option`, `env`, `node_set`, `statistics`

use super::codes::Row;
use super::expr_write;
use super::number;
use super::read_run;
use super::write::Out;
use super::write_outputs;
use crate::ir::dc_ac::{Ac, Dc, Disto, Noise, Pz, PzFind, PzTransfer, Sens, SensAc, Sp, Tf};
use crate::ir::periodic::Pss;
use crate::ir::tran::{Four, StepCeiling, Tran};
use crate::ir::{self, AnalysisKind};

/// What the run area spells: exactly what its reader produces (L5).
pub(super) const FEATURES: ir::FeatureSet = read_run::FEATURES;

/// This area's writing codes, `W4300–W4349`.
pub(super) const CODES: &[Row] = &[
    refusal(
        FOUR_HARMONICS,
        "a Fourier analysis with another harmonic count than ngspice's default",
        "ngspice-45 fourier.c:69-70",
        "ngspice takes the count from the nfreqs option, not from the .four card; refused.",
    ),
    refusal(
        READS_TEMP,
        "a run card value that reads the temperature",
        "ngspice-45 inpcom.c inp_fix_temper_in_param (.param only); numparam",
        "numparam evaluates a dot card's braces once before the run, with no temper; refused.",
    ),
    refusal(
        CARD_FORM,
        "an analysis field ngspice's card has no position for",
        "ngspice-45 dctrcurv.c:89-149, inp2dot.c dot_pss",
        "a DC axis on an element ngspice does not sweep, a .pss missing an argument, or an output no card spells; refused.",
    ),
    refusal(
        LOOP_FORM,
        "a loop HSPICE's sweep tail cannot spell",
        "rewrite/DIALECT-HSPICE.md §6; settle dc_sweep_param.sp",
        "HSPICE sweeps a parameter around one .dc, .ac or .tran card (`sweep p a b step`); another loop is refused.",
    ),
];

/// A loop that is no `sweep` tail: not a sweep of one parameter around one `.dc`, `.ac` or
/// `.tran` card (settle `dc_sweep_param.sp`).
const LOOP_FORM: ir::Code = ir::Code::new(b'W', 4303);

/// A Fourier analysis whose harmonic count is not ngspice's default (DC and 9 harmonics): its
/// count is the deck's `nfreqs` option, which no `.four` card states.
const FOUR_HARMONICS: ir::Code = ir::Code::new(b'W', 4300);
/// An analysis field ngspice's card has no position for: a DC source axis on an element that is
/// no V or I source, an element-value axis on one that is no resistor; an output no ngspice
/// card spells (`write_outputs::probe_admits`). (A `.pss` missing an argument is
/// `PssArgumentOmitted`, an output with a part an invariant: R68, A42.)
const CARD_FORM: ir::Code = ir::Code::new(b'W', 4301);

/// A state card's value (`.options`, `.temp`, `.ic`, `.nodeset`) that reads the temperature:
/// numparam evaluates a dot card's `{…}` once, before the run, with no `temper` (`inpcom.c`
/// `inp_fix_temper_in_param` rewrites `.param` only). An analysis argument reading it is
/// X0023 (R68, A42).
const READS_TEMP: ir::Code = ir::Code::new(b'W', 4302);

/// Appends a refusal for this area's items ngspice cannot spell for a reason no feature states
/// (R21): a card form (`W4300`, `W4301`), a state card's value that reads the temperature
/// (`W4302`), a script option's value that is no number (`W4304`), a script command's field that
/// is no number (`W4305`). A node set that holds from its place on is
/// `NodeSetFromHere`, which `FEATURES` lack (`commands.c` has no command that sets one).
pub(super) fn admits(design: &ir::Design, item: ir::ItemRef, diags: &mut ir::Diags) {
    let mut refuse = |code| diags.push_at(code, design, item, 0);
    if item.kind == ir::ItemKind::Loop {
        if swept_card(design, item).is_none() {
            refuse(LOOP_FORM);
        }
        return;
    }
    let mut values = Vec::new();
    match item.kind {
        ir::ItemKind::Option => {
            let o: ir::OptionRow = design.row(item.index);
            values.push(o.value);
        }
        ir::ItemKind::Env => {
            let e: ir::Env = design.row(item.index);
            values.extend([e.temp, e.tnom]);
        }
        ir::ItemKind::NodeSet => {
            let n: ir::NodeSet = design.row(item.index);
            values.push(n.value);
        }
        _ => {}
    }
    if item.kind != ir::ItemKind::Analysis {
        return check_values(design, &values, &mut refuse);
    }
    let a: ir::Analysis = design.row(item.index);
    let mut form_ok = true;
    let mut outs = Vec::new();
    // an analysis argument reading the temperature is X0023 and an output with a part
    // FieldDomain (R68, A42); a `.pss` missing an argument is `PssArgumentOmitted`, which
    // FEATURES lack: what is left is ngspice's own card form, and in a script a field that is
    // no number (W4305); a sweep's form is a feature (`Sweep{Dc,Freq}*`, A24)
    let freq = |s: ir::SweepSpec, values: &mut Vec<ir::ExprId>| {
        values.extend([s.start, s.stop, s.points]);
    };
    match a.kind {
        AnalysisKind::Dc => {
            let dc: Dc = design.detail(a.detail);
            for axis in design.dc_axes(dc.axes) {
                let t = axis.target;
                let name = (!t.a.name.is_none())
                    .then(|| design.str(t.a.name).as_bytes()[0].to_ascii_lowercase());
                // ngspice sweeps a resistor, a V or I source, or temp (dctrcurv.c:89-149);
                // the target's kind and place are features
                form_ok &= match t.kind {
                    ir::TargetKind::SourceDc => matches!(name, Some(b'v' | b'i')),
                    ir::TargetKind::ElementValue => name == Some(b'r'),
                    _ => true,
                };
                let s = axis.sweep;
                values.extend([s.start, s.stop, s.step]);
            }
        }
        AnalysisKind::Ac => freq(design.detail::<Ac>(a.detail).sweep, &mut values),
        AnalysisKind::Noise => {
            let n: Noise = design.detail(a.detail);
            outs.push(n.output);
            freq(n.sweep, &mut values);
            values.push(n.points_per_summary);
        }
        AnalysisKind::Tf => outs.push(design.detail::<Tf>(a.detail).output),
        AnalysisKind::Sens => outs.push(design.detail::<Sens>(a.detail).output),
        AnalysisKind::SensAc => {
            let s: SensAc = design.detail(a.detail);
            outs.push(s.output);
            freq(s.sweep, &mut values);
        }
        AnalysisKind::Disto => {
            let d: Disto = design.detail(a.detail);
            freq(d.sweep, &mut values);
            values.push(d.f2_over_f1);
        }
        AnalysisKind::Sp => freq(design.detail::<Sp>(a.detail).sweep, &mut values),
        AnalysisKind::Tran => {
            let t: Tran = design.detail(a.detail);
            values.extend([t.print_step, t.stop, t.record_start]);
            if let StepCeiling::Given(e) = t.ceiling {
                values.push(e);
            }
        }
        AnalysisKind::Four => {
            let f: Four = design.detail(a.detail);
            // a given count is ngspice's default; none is the deck's `nfreqs` (A16)
            let nine = f.harmonics.is_none()
                || matches!(design.expr(f.harmonics), ir::Expr::Num(n) if design.literal_value(design.num(n)) == 9.0);
            if !nine {
                refuse(FOUR_HARMONICS);
            }
            values.push(f.fundamental);
            outs.extend_from_slice(design.probes(f.outputs));
        }
        AnalysisKind::Pss => {
            let p: Pss = design.detail(a.detail);
            values.extend([p.fundamental, p.tstab]);
            values.extend([p.harmonics, p.points, p.sc_iter, p.steady_coeff]);
        }
        _ => {}
    }
    form_ok &= outs.iter().all(|p| write_outputs::probe_admits(design, p));
    if !form_ok {
        refuse(CARD_FORM);
    }
}

/// An `Analysis` as its card.
pub(super) fn analysis(w: &mut Out<'_>, item: ir::ItemRef) {
    let t = card(w.design, item);
    w.line(&t);
}

/// A `Loop` as its body's one analysis card with a `sweep param values` tail (settle
/// `dc_sweep_param.sp`); `admits` refused every other loop.
pub(super) fn loop_(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let (body, s) = swept_card(d, item).expect("HSPICE write: an admitted loop");
    let mut t = card(d, body);
    t.push_str(" sweep ");
    t.push_str(d.str(s.target.a.name));
    axis(d, s.sweep, &mut t);
    w.line(&t);
}

/// An `Option` as `.options key[=value]` (`method=trap|gear` for the method flags), or the
/// script command `option key[=value]` when it holds from its place on.
pub(super) fn option(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let o: ir::OptionRow = d.row(item.index);
    let mut t = String::new();
    match o.key {
        ir::OptionKey::MethodTrap => t.push_str("method=trap"),
        ir::OptionKey::MethodGear => t.push_str("method=gear"),
        key => {
            let (word, _) = read_run::KEYS
                .iter()
                .find(|(_, k)| *k == key)
                .expect("HSPICE write: FEATURES has only the keys read_run spells");
            t.push_str(word);
            if !o.value.is_none() {
                t.push('=');
                value(d, o.value, &mut t);
            }
        }
    }
    state(w, o.holds, ".options", &t);
}

/// An `Env`: `.temp t` when it sets the temperature only, is the design's one whole-run `Env`
/// that sets it, and `t` is one `.temp` reads whole (`strtod_reads`), else `.options temp=t tnom=n` (what it sets): `.temp` beats every
/// `.options temp=` (`inp.c:1159-1186`), while among `.options` keys the last wins, the IR's
/// rule (R24), and both spellings read back to the same `Env`. A `FromHere` one is the script
/// command `option temp=t tnom=n`.
pub(super) fn env(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let e: ir::Env = d.row(item.index);
    let whole = e.holds == ir::Applies::WholeRun;
    if whole && e.tnom.is_none() && temps_set(d) == 1 && strtod_reads(d, e.temp) {
        let mut t = String::from(".temp");
        value(d, e.temp, &mut t);
        return w.line(&t);
    }
    let mut t = String::new();
    for (key, v) in [("temp", e.temp), ("tnom", e.tnom)] {
        if !v.is_none() {
            if !t.is_empty() {
                t.push(' ');
            }
            t.push_str(key);
            t.push('=');
            value(d, v, &mut t);
        }
    }
    state(w, e.holds, ".options", &t);
}

/// A `NodeSet` as `.ic v(path)=value`, `.nodeset v(path)=value` or `.nodeset all=value`.
pub(super) fn node_set(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let n: ir::NodeSet = d.row(item.index);
    let mut t = String::from(match n.kind {
        ir::NodeSetKind::Ic => ".ic",
        ir::NodeSetKind::Nodeset => ".nodeset",
        ir::NodeSetKind::Device => unreachable!("HSPICE write: FEATURES lacks IcDevice"),
    });
    if n.node.len == 0 {
        t.push_str(" all");
    } else {
        t.push_str(" v(");
        path(d, n.node, &mut t);
        t.push(')');
    }
    t.push('=');
    value(d, n.value, &mut t);
    w.line(&t.replace("= ", "="));
}

/// A `Statistics` block: ngspice has none (`FEATURES` lacks it; a citation row).
pub(super) fn statistics(_w: &mut Out<'_>, _item: ir::ItemRef) {
    unreachable!("HSPICE write: FEATURES lacks Item(Statistics): ngspice has no such block")
}

// ---- private ----

/// The one analysis card a sweep loop wraps and its sweep, when HSPICE's tail spells it: a
/// sweep of a parameter, restored after, unnamed, around one `.dc`, `.ac` or `.tran`.
fn swept_card(d: &ir::Design, item: ir::ItemRef) -> Option<(ir::ItemRef, ir::plan::SweepLoop)> {
    let l: ir::Loop = d.row(item.index);
    if l.kind != ir::LoopKind::Sweep || l.after != ir::LoopEnd::Restore || !l.name.is_none() {
        return None;
    }
    let s: ir::plan::SweepLoop = d.detail(l.detail);
    let body = d.items(l.body);
    let one = match body {
        [one] if one.kind == ir::ItemKind::Analysis => *one,
        _ => return None,
    };
    let a: ir::Analysis = d.row(one.index);
    let kind_ok = matches!(
        a.kind,
        AnalysisKind::Dc | AnalysisKind::Ac | AnalysisKind::Tran
    );
    (kind_ok && s.target.kind == ir::TargetKind::Param && s.target.path.len == 0)
        .then_some((one, s))
}

/// An analysis's card text.
fn card(d: &ir::Design, item: ir::ItemRef) -> String {
    let a: ir::Analysis = d.row(item.index);
    assert!(
        a.name.is_none(),
        "HSPICE write: FEATURES lacks AnalysisNamed"
    );
    let mut t = String::new();
    match a.kind {
        AnalysisKind::Op => t.push_str(".op"),
        AnalysisKind::Dc => {
            t.push_str(".dc");
            let dc: Dc = d.detail(a.detail);
            for axis in d.dc_axes(dc.axes) {
                t.push(' ');
                match axis.target.kind {
                    ir::TargetKind::Temp => t.push_str("temp"),
                    _ => t.push_str(d.str(axis.target.a.name)),
                }
                self::axis(d, axis.sweep, &mut t);
            }
        }
        AnalysisKind::Ac => {
            t.push_str(".ac");
            sweep(d, d.detail::<Ac>(a.detail).sweep, &mut t);
        }
        AnalysisKind::Tran => {
            let r: Tran = d.detail(a.detail);
            t.push_str(".tran");
            value(d, r.print_step, &mut t);
            value(d, r.stop, &mut t);
            match (r.record_start, r.ceiling) {
                // `tstart` stands before `tmax`: none is recording from 0, spelled `0`
                (start, StepCeiling::Given(max)) => {
                    if start.is_none() {
                        t.push_str(" 0");
                    } else {
                        value(d, start, &mut t);
                    }
                    value(d, max, &mut t);
                }
                (start, StepCeiling::Rule(_)) if !start.is_none() => value(d, start, &mut t),
                (_, StepCeiling::Rule(_)) => {}
            }
            if r.uic {
                t.push_str(" uic");
            }
        }
        AnalysisKind::Noise => {
            let n: Noise = d.detail(a.detail);
            t.push_str(".noise");
            probe(d, &n.output, &mut t);
            t.push(' ');
            t.push_str(d.str(n.source));
            sweep(d, n.sweep, &mut t);
            if !n.points_per_summary.is_none() {
                value(d, n.points_per_summary, &mut t);
            }
        }
        AnalysisKind::Tf => {
            let f: Tf = d.detail(a.detail);
            t.push_str(".tf");
            probe(d, &f.output, &mut t);
            t.push(' ');
            t.push_str(d.str(f.source));
        }
        AnalysisKind::Sens => {
            t.push_str(".sens");
            probe(d, &d.detail::<Sens>(a.detail).output, &mut t);
        }
        AnalysisKind::SensAc => {
            let s: SensAc = d.detail(a.detail);
            t.push_str(".sens");
            probe(d, &s.output, &mut t);
            t.push_str(" ac");
            sweep(d, s.sweep, &mut t);
        }
        AnalysisKind::Pz => {
            let p: Pz = d.detail(a.detail);
            t.push_str(".pz");
            for n in [p.in_p, p.in_n, p.out_p, p.out_n] {
                t.push(' ');
                t.push_str(d.str(n));
            }
            t.push_str(match p.transfer {
                PzTransfer::Voltage => " vol",
                PzTransfer::Current => " cur",
            });
            t.push_str(match p.find {
                PzFind::Poles => " pol",
                PzFind::Zeros => " zer",
                PzFind::Both => " pz",
            });
        }
        AnalysisKind::Disto => {
            let s: Disto = d.detail(a.detail);
            t.push_str(".disto");
            sweep(d, s.sweep, &mut t);
            if !s.f2_over_f1.is_none() {
                value(d, s.f2_over_f1, &mut t);
            }
        }
        AnalysisKind::Sp => {
            let s: Sp = d.detail(a.detail);
            t.push_str(".sp");
            sweep(d, s.sweep, &mut t);
            if s.noise {
                t.push_str(" 1");
            }
        }
        AnalysisKind::Pss => {
            let p: Pss = d.detail(a.detail);
            t.push_str(".pss");
            value(d, p.fundamental, &mut t);
            value(d, p.tstab, &mut t);
            t.push(' ');
            t.push_str(d.str(p.osc_p));
            for e in [p.points, p.harmonics, p.sc_iter, p.steady_coeff] {
                value(d, e, &mut t);
            }
            if p.uic {
                t.push_str(" uic");
            }
        }
        AnalysisKind::Four => {
            let f: Four = d.detail(a.detail);
            t.push_str(".four");
            value(d, f.fundamental, &mut t);
            for p in d.probes(f.outputs) {
                probe(d, p, &mut t);
            }
        }
        other @ (AnalysisKind::DcXf
        | AnalysisKind::Xf
        | AnalysisKind::Net
        | AnalysisKind::Hb
        | AnalysisKind::Pac
        | AnalysisKind::Pnoise
        | AnalysisKind::Pxf
        | AnalysisKind::Pstb
        | AnalysisKind::Stb
        | AnalysisKind::DcInc) => {
            unreachable!("HSPICE write: FEATURES lacks Analysis({other:?})")
        }
        AnalysisKind::TimedOp => {
            t.push_str(".op");
            let o: ir::dc_ac::TimedOp = d.detail(a.detail);
            for &e in d.exprs(o.times) {
                value(d, e, &mut t);
            }
        }
    }
    t
}

/// Refuses a card value that reads the temperature (numparam evaluates a dot card's `{…}` once,
/// with no `temper`; a value that reads the circuit is `ir::check`'s `ProbeContext` in these
/// constant fields; a literal is never refused: R28).
fn check_values(design: &ir::Design, values: &[ir::ExprId], refuse: &mut impl FnMut(ir::Code)) {
    for &e in values.iter().filter(|e| !e.is_none()) {
        if ir::expr_class(design, e) == ir::ExprClass::Temp {
            refuse(READS_TEMP);
        }
    }
}

/// `body` (`k=v …`) as a state card (`WholeRun`: `.options body`) or, holding from its place on,
/// as the script command `option body` (ngspice's `set` of each key: `cp_vset`; ng-control's
/// run state_option_temp).
fn state(w: &mut Out<'_>, holds: ir::Applies, card: &str, body: &str) {
    let body = body.replace("= ", "=");
    match holds {
        ir::Applies::WholeRun => w.line(&format!("{card} {body}")),
        ir::Applies::FromHere => unreachable!("HSPICE write: FEATURES lacks a state from here"),
    }
}

/// Whether `.temp`'s `strtod` reads `e` as written whole (`inp.c:1173-1182`): a literal with no
/// scale letter, or a `{…}` numparam replaces with a number; else ngspice runs 27 and the value
/// goes as `.options temp=`, which `INPevaluate` reads (L5 seed 4).
fn strtod_reads(d: &ir::Design, e: ir::ExprId) -> bool {
    match d.expr(e) {
        ir::Expr::Num(n) => matches!(d.num(n).form, ir::NumForm::Plain | ir::NumForm::Exponent),
        _ => true,
    }
}

/// How many whole-run `Env` items of `design` set the temperature.
fn temps_set(design: &ir::Design) -> usize {
    let sets = |i: ir::ItemRef| {
        let e: ir::Env = design.row(i.index);
        !e.temp.is_none() && e.holds == ir::Applies::WholeRun
    };
    design
        .reachable()
        .into_iter()
        .filter(|&(i, _)| i.kind == ir::ItemKind::Env && sets(i))
        .count()
}

/// A refusal row of this area.
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

/// ` v`: a literal bare, any other expression in `'…'`.
fn value(d: &ir::Design, e: ir::ExprId, out: &mut String) {
    out.push(' ');
    if let ir::Expr::Num(n) = d.expr(e) {
        number::spell(d, d.num(n), out);
        return;
    }
    out.push('\'');
    expr_write::expr(d, e, out);
    out.push('\'');
}

/// A DC axis's or a sweep tail's values: ` start stop step` for a linear sweep by step, else
/// [`sweep`]'s form.
fn axis(d: &ir::Design, s: ir::SweepSpec, out: &mut String) {
    if s.kind == ir::SweepKind::Lin && !s.step.is_none() {
        for e in [s.start, s.stop, s.step] {
            value(d, e, out);
        }
    } else {
        sweep(d, s, out);
    }
}

/// ` probe`, as the outputs area spells it (one spelling of probes: `write_outputs::probe`).
fn probe(d: &ir::Design, p: &ir::Probe, out: &mut String) {
    out.push(' ');
    write_outputs::probe(d, p, out);
}

/// A path's names joined by `.`.
fn path(d: &ir::Design, list: ir::NameList, out: &mut String) {
    for (k, &n) in d.names(list).iter().enumerate() {
        if k > 0 {
            out.push('.');
        }
        out.push_str(d.str(n));
    }
}

/// ` dec|oct|lin np fstart fstop`. Asserts a sweep by count (`admits` refuses one by step).
fn sweep(d: &ir::Design, s: ir::SweepSpec, out: &mut String) {
    if s.kind == ir::SweepKind::List {
        let values = d.exprs(s.values);
        out.push_str(&format!(" poi {}", values.len()));
        for &e in values {
            value(d, e, out);
        }
        return;
    }
    out.push_str(match s.kind {
        ir::SweepKind::Dec => " dec",
        ir::SweepKind::Oct => " oct",
        ir::SweepKind::Lin => " lin",
        other => unreachable!("HSPICE write: FEATURES lacks the {other:?} sweep"),
    });
    assert!(!s.points.is_none(), "HSPICE write: a sweep by count");
    for e in [s.points, s.start, s.stop] {
        value(d, e, out);
    }
}
