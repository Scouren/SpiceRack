//! A validated analysis ([`Card`], `analysis.rs`) → its IR rows: the detail row, its sweep and
//! probes, the header in `st.scope`. Values are appended in the IR text's field order, so a read
//! and a parse of the expected IR intern alike.
//!
//! Cites: `crates/ir/src/{dc_ac,tran,periodic}.rs` (the rows and their text order);
//! `traninit.c:29-34` (`StepCap::NgspiceRule`), `fourier.c:69` (the default count, 9).
//!
//! Contents: `pub(in super::super) fn push(&mut Lower, Stmt, &Card)`

use super::super::read::{Lower, Stmt};
use super::super::value::emit;
use super::analysis::{Axis, Card, Sweep, Swept};
use crate::ir;
use crate::ir::dc_ac::{
    Ac, Dc, DcAxis, Disto, Noise, NoiseRef, Pz, PzInput, Sens, SensAc, Sp, Tf, TimedOp,
};
use crate::ir::periodic::{Pss, PssMode};
use crate::ir::tran::{Four, StepCap, StepCeiling, Tran};

/// Appends `card` as one `Analysis` in `st.scope`.
/// The control area calls it with the card `analysis::read` made of a script command's words.
pub(in super::super) fn push(lw: &mut Lower<'_, '_>, st: Stmt, card: &Card) {
    let span = lw.span(st.node);
    let scope = st.scope;
    let none = ir::ExprId::NONE;
    match card {
        Card::Op => {
            lw.b.op(scope, None, span);
        }
        Card::TimedOp(times) => {
            let t: Vec<ir::ExprId> = times.iter().map(|v| emit(lw, st, v)).collect();
            let times = lw.b.exprs(&t);
            lw.b.analysis(scope, None, TimedOp { times }, span);
        }
        Card::Dc(axes, outer) => {
            // a `sweep` tail is a loop over a parameter around the analysis (settle
            // dc_sweep_param.sp: one plot per value)
            let mut inner = st;
            if let Some((name, values)) = outer {
                let p = lw.names.fold(name);
                let target = lw.b.target(ir::TargetKind::Param, &[], &p, None);
                let row = ir::plan::SweepLoop {
                    target,
                    sweep: emit_axis(lw, st, values),
                };
                let (item, body) = lw.b.loop_(scope, None, row, ir::LoopEnd::Restore, span);
                let id = lw.b.name(&p);
                lw.refer(item, 0, scope, ir::DefKind::Param, id);
                inner = Stmt { scope: body, ..st };
            }
            let mut rows = Vec::new();
            for (swept, values) in axes {
                let target = match swept {
                    Swept::Source(n) => lw.b.target(ir::TargetKind::SourceDc, &[], n, None),
                    Swept::Resistor(n) => lw.b.target(ir::TargetKind::ElementValue, &[], n, None),
                    Swept::Temp => ir::Target::TEMP,
                };
                let sweep = emit_axis(lw, st, values);
                // ponytail: the ngspice lead (R75: each outer step afresh), not settled on ESPice
                let afresh = !rows.is_empty();
                rows.push(DcAxis {
                    target,
                    sweep,
                    name: ir::NameId::NONE,
                    afresh,
                });
            }
            let axes = lw.b.dc_axes(&rows);
            lw.b.analysis(inner.scope, None, Dc { axes }, span);
        }
        Card::Ac(s) => {
            let sweep = emit_sweep(lw, st, s);
            lw.b.analysis(scope, None, Ac { sweep }, span);
        }
        Card::Tran {
            step,
            stop,
            start,
            max,
            uic,
        } => {
            // in the IR text's field order, so a read and a parse intern alike
            let stop = emit(lw, st, stop);
            let record_start = start.as_ref().map_or(none, |v| emit(lw, st, v));
            let print_step = emit(lw, st, step);
            let ceiling = match max {
                Some(v) => StepCeiling::Given(emit(lw, st, v)),
                None => StepCeiling::Rule(StepCap::NgspiceRule),
            };
            let row = Tran {
                stop,
                record_start,
                print_step,
                initial_step: none,
                noise_fmax: none,
                noise_fmin: none,
                noise_seed: none,
                segments: ir::List::EMPTY,
                ceiling,
                uic: *uic,
                startup: false,
            };
            lw.b.analysis(scope, None, row, span);
        }
        Card::Noise {
            out,
            source,
            sweep,
            summary,
        } => {
            let output = *out;
            let source = lw.b.name(source);
            let sweep = emit_sweep(lw, st, sweep);
            let points_per_summary = summary.as_ref().map_or(none, |v| emit(lw, st, v));
            let row = Noise {
                output,
                source,
                sweep,
                points_per_summary,
                ac_sweep: false,
                input_ref: NoiseRef::Unit,
            };
            lw.b.analysis(scope, None, row, span);
        }
        Card::Tf { out, source } => {
            let output = *out;
            let source = lw.b.name(source);
            lw.b.analysis(scope, None, Tf { output, source }, span);
        }
        Card::Sens { out, ac: None } => {
            let output = *out;
            lw.b.analysis(scope, None, Sens { output }, span);
        }
        Card::Sens { out, ac: Some(s) } => {
            let output = *out;
            let sweep = emit_sweep(lw, st, s);
            lw.b.analysis(scope, None, SensAc { output, sweep }, span);
        }
        Card::Pz {
            nodes,
            transfer,
            find,
        } => {
            let [in_p, in_n, out_p, out_n] = nodes.clone().map(|n| lw.b.name(&n));
            let row = Pz {
                input: PzInput::Nodes,
                transfer: *transfer,
                find: *find,
                in_p,
                in_n,
                out_p,
                out_n,
                source: ir::NameId::NONE,
            };
            lw.b.analysis(scope, None, row, span);
        }
        Card::Disto { sweep, f2 } => {
            let sweep = emit_sweep(lw, st, sweep);
            let f2_over_f1 = f2.as_ref().map_or(none, |v| emit(lw, st, v));
            lw.b.analysis(scope, None, Disto { sweep, f2_over_f1 }, span);
        }
        Card::Sp { sweep, noise } => {
            let sweep = emit_sweep(lw, st, sweep);
            let row = Sp {
                sweep,
                ports: ir::List::EMPTY,
                noise: *noise,
            };
            lw.b.analysis(scope, None, row, span);
        }
        Card::Four {
            fundamental,
            outs,
            default_count,
        } => {
            let fundamental = emit(lw, st, fundamental);
            let harmonics = if *default_count {
                lw.b.literal(plain(9))
            } else {
                none
            };
            let periods = lw.b.literal(plain(1));
            let outputs = lw.b.probes(outs);
            let row = Four {
                fundamental,
                harmonics,
                periods,
                grid: none,
                outputs,
            };
            lw.b.analysis(scope, None, row, span);
        }
        Card::Pss {
            fundamental,
            tstab,
            osc,
            points,
            harmonics,
            sc_iter,
            steady_coeff,
            uic,
        } => {
            // in the IR text's field order, so a read and a parse intern alike
            let fundamental = emit(lw, st, fundamental);
            let tstab = emit(lw, st, tstab);
            let harmonics = emit(lw, st, harmonics);
            let points = emit(lw, st, points);
            let osc_p = lw.b.name(osc);
            let osc_n = lw.b.name("0");
            let sc_iter = emit(lw, st, sc_iter);
            let steady_coeff = emit(lw, st, steady_coeff);
            let row = Pss {
                mode: PssMode::Autonomous,
                uic: *uic,
                fundamental,
                tstab,
                harmonics,
                points,
                osc_p,
                osc_n,
                sc_iter,
                steady_coeff,
            };
            lw.b.analysis(scope, None, row, span);
        }
    }
}

// ---- private ----

/// Appends a sweep: `points` per decade or octave, `points` values in all for `lin`, or the
/// listed values.
fn emit_sweep(lw: &mut Lower<'_, '_>, st: Stmt, s: &Sweep) -> ir::SweepSpec {
    let (kind, points, start, stop) = match s {
        Sweep::List(values) => {
            let v: Vec<ir::ExprId> = values.iter().map(|x| emit(lw, st, x)).collect();
            let list = lw.b.exprs(&v);
            return ir::SweepSpec::list(list);
        }
        Sweep::Range {
            kind,
            points,
            start,
            stop,
        } => (kind, points, start, stop),
    };
    let start = emit(lw, st, start);
    let stop = emit(lw, st, stop);
    let points = emit(lw, st, points);
    match kind {
        ir::SweepKind::Dec => ir::SweepSpec::dec(start, stop, points),
        ir::SweepKind::Oct => ir::SweepSpec::oct(start, stop, points),
        ir::SweepKind::Lin => ir::SweepSpec::lin_points(start, stop, points),
        ir::SweepKind::Log | ir::SweepKind::List => {
            unreachable!("a range is dec, oct or lin")
        }
    }
}

/// Appends a DC axis's or a sweep tail's values.
fn emit_axis(lw: &mut Lower<'_, '_>, st: Stmt, a: &Axis) -> ir::SweepSpec {
    match a {
        Axis::Step([a, b, c]) => {
            let (a, b, c) = (emit(lw, st, a), emit(lw, st, b), emit(lw, st, c));
            ir::SweepSpec::lin_step(a, b, c)
        }
        Axis::Sweep(s) => emit_sweep(lw, st, s),
    }
}

/// A whole number as a plain literal (a default the reader fills).
const fn plain(n: i64) -> ir::Literal {
    ir::Literal {
        mantissa: n,
        frac: 0,
        exp10: 0,
        form: ir::NumForm::Plain,
    }
}
