//! Run items → ngspice dot cards: `Analysis` → `.op .dc .ac .tran .noise .tf .sens .pz .disto
//! .sp .pss .four`, each in its positional form; `Option`, `Env`, `NodeSet` → `.options`, `.temp`,
//! `.ic`, `.nodeset` (with AMEND-A12 A14). Items are written in IR order (R25: no writer
//! reorders run items); ngspice has no statistics block.
//!
//! Each function writes one item of a design legal for `FEATURES` (the spine checked it), sees
//! only the IR, and never learns a source. `FEATURES` is `read_run::FEATURES`, the reader's
//! image (L5); what a feature cannot state and ngspice cannot spell is refused by [`admits`]
//! (R21). Codes: `W1300–W1349` (this file).
//!
//! Cites: ngspice-45 `inp2dot.c` (each card's form), `traninit.c:29` (a zero `tmax` is none);
//! `rewrite/DIALECT-NGSPICE.md` §4; `rewrite/RULINGS.md` R21, R24–R26.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn analysis`, `option`, `env`, `node_set`, `statistics`

use super::codes::Row;
use super::expr_write;
use super::number::{self, Grammar};
use super::read_run;
use super::write::Out;
use super::write_outputs;
use crate::ir::dc_ac::{Ac, Dc, Disto, Noise, Pz, PzFind, PzTransfer, Sens, SensAc, Sp, Tf};
use crate::ir::periodic::Pss;
use crate::ir::tran::{Four, StepCeiling, Tran};
use crate::ir::{self, AnalysisKind};

/// What the run area spells: exactly what its reader produces (L5).
pub(super) const FEATURES: ir::FeatureSet = read_run::FEATURES;

/// This area's writing codes, `W1300–W1349`.
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
];

/// A Fourier analysis whose harmonic count is not ngspice's default (DC and 9 harmonics): its
/// count is the deck's `nfreqs` option, which no `.four` card states.
const FOUR_HARMONICS: ir::Code = ir::Code::new(b'W', 1300);
/// An analysis field ngspice's card has no position for: a DC source axis on an element that is
/// no V or I source, an element-value axis on one that is no resistor; a `.pss` missing one of
/// its last four arguments; an output no ngspice card spells (`write_outputs::probe_admits`).
const CARD_FORM: ir::Code = ir::Code::new(b'W', 1301);

/// A card value that reads the temperature: numparam evaluates a dot card's `{…}` once, before
/// the run, with no `temper` (`inpcom.c` `inp_fix_temper_in_param` rewrites `.param` only).
const READS_TEMP: ir::Code = ir::Code::new(b'W', 1302);

/// Appends a refusal for this area's items ngspice cannot spell for a reason no feature states
/// (R21): a card form (`W1300`, `W1301`) or a value that reads the temperature (`W1302`).
pub(super) fn admits(design: &ir::Design, item: ir::ItemRef, diags: &mut ir::Diags) {
    let span = design.span_of(item);
    let mut refuse = |code| diags.push(code, span, 0);
    let mut values = Vec::new();
    // a `FromHere` state item is `RunExtra::StateFromHere`, which FEATURES lack
    match item.kind {
        ir::ItemKind::Option => values.push(design.row::<ir::OptionRow>(item.index).value),
        ir::ItemKind::Env => {
            let e: ir::Env = design.row(item.index);
            values.extend([e.temp, e.tnom]);
        }
        ir::ItemKind::NodeSet => values.push(design.row::<ir::NodeSet>(item.index).value),
        _ => {}
    }
    if item.kind != ir::ItemKind::Analysis {
        return check_values(design, &values, &mut refuse);
    }
    let a: ir::Analysis = design.row(item.index);
    let mut form_ok = true;
    let mut outs = Vec::new();
    // a sweep's form is a feature (`Sweep{Dc,Freq}*`, A24); its values are checked here
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
            let given = [p.harmonics, p.points, p.sc_iter, p.steady_coeff];
            form_ok &= given.iter().all(|e| !e.is_none());
            values.extend([p.fundamental, p.tstab]);
            values.extend(given);
        }
        _ => {}
    }
    form_ok &= outs.iter().all(|p| write_outputs::probe_admits(design, p));
    if !form_ok {
        refuse(CARD_FORM);
    }
    check_values(design, &values, &mut refuse);
}

/// An `Analysis` as its card, or as its script command when the deck takes the script form
/// (`Out::scripted`, R49).
pub(super) fn analysis(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let a: ir::Analysis = d.row(item.index);
    assert!(
        a.name.is_none(),
        "ngspice write: FEATURES lacks AnalysisNamed"
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
                for e in [axis.sweep.start, axis.sweep.stop, axis.sweep.step] {
                    value(d, e, &mut t);
                }
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
        | AnalysisKind::DcInc
        | AnalysisKind::TimedOp) => {
            unreachable!("ngspice write: FEATURES lacks Analysis({other:?})")
        }
    }
    if w.scripted() {
        // R49: a deck whose plan needs a script runs every analysis as a command, the card
        // without its dot (`read_run::read_analysis` reads it back the same way); `.four` is the
        // `fourier` command, which the frontend runs for the card too (`fourier.c`)
        let cmd = t
            .strip_prefix(".four ")
            .map_or_else(|| t[1..].to_string(), |rest| format!("fourier {rest}"));
        w.script(&cmd);
    } else {
        w.line(&t);
    }
}

/// An `Option` as `.options key[=value]` (`method=trap|gear` for the method flags).
pub(super) fn option(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let o: ir::OptionRow = d.row(item.index);
    let mut t = String::from(".options ");
    match o.key {
        ir::OptionKey::MethodTrap => t.push_str("method=trap"),
        ir::OptionKey::MethodGear => t.push_str("method=gear"),
        key => {
            let (word, _) = read_run::KEYS
                .iter()
                .find(|(_, k)| *k == key)
                .expect("ngspice write: FEATURES has only the keys read_run spells");
            t.push_str(word);
            if !o.value.is_none() {
                t.push('=');
                value(d, o.value, &mut t);
            }
        }
    }
    w.line(&t.replace("= ", "="));
}

/// An `Env`: `.temp t` when it sets the temperature only and is the design's one `Env` that
/// sets it, else `.options temp=t tnom=n` (what it sets): `.temp` beats every `.options temp=`
/// (`inp.c:1159-1186`), while among `.options` keys the last wins, the IR's rule (R24), and
/// both spellings read back to the same `Env`.
pub(super) fn env(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let e: ir::Env = d.row(item.index);
    let mut t = String::new();
    if e.tnom.is_none() && temps_set(d) == 1 {
        t.push_str(".temp");
        value(d, e.temp, &mut t);
    } else {
        t.push_str(".options");
        for (key, v) in [("temp", e.temp), ("tnom", e.tnom)] {
            if !v.is_none() {
                t.push(' ');
                t.push_str(key);
                t.push('=');
                value(d, v, &mut t);
            }
        }
    }
    w.line(&t.replace("= ", "="));
}

/// A `NodeSet` as `.ic v(path)=value`, `.nodeset v(path)=value` or `.nodeset all=value`.
pub(super) fn node_set(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let n: ir::NodeSet = d.row(item.index);
    let mut t = String::from(match n.kind {
        ir::NodeSetKind::Ic => ".ic",
        ir::NodeSetKind::Nodeset => ".nodeset",
        ir::NodeSetKind::Device => unreachable!("ngspice write: FEATURES lacks IcDevice"),
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
    unreachable!("ngspice write: FEATURES lacks Item(Statistics): ngspice has no such block")
}

// ---- private ----

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

/// How many `Env` items of `design` set the temperature.
fn temps_set(design: &ir::Design) -> usize {
    let sets = |i: ir::ItemRef| !design.row::<ir::Env>(i.index).temp.is_none();
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

/// ` v`: a literal bare, any other expression in `{…}`.
fn value(d: &ir::Design, e: ir::ExprId, out: &mut String) {
    out.push(' ');
    if let ir::Expr::Num(n) = d.expr(e) {
        number::spell(d, d.num(n), Grammar::Inp, out);
        return;
    }
    out.push('{');
    expr_write::expr(d, e, out);
    out.push('}');
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
    out.push_str(match s.kind {
        ir::SweepKind::Dec => " dec",
        ir::SweepKind::Oct => " oct",
        ir::SweepKind::Lin => " lin",
        other => unreachable!("ngspice write: FEATURES lacks the {other:?} sweep"),
    });
    assert!(!s.points.is_none(), "ngspice write: a sweep by count");
    for e in [s.points, s.start, s.stop] {
        value(d, e, out);
    }
}
