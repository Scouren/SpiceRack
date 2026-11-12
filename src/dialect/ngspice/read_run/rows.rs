//! A validated analysis ([`Card`], `analysis.rs`) → its IR rows: the detail row, its sweep and
//! probes, the header in `st.scope`. Values are appended in the IR text's field order, so a read
//! and a parse of the expected IR intern alike.
//!
//! Cites: `crates/ir/src/{dc_ac,tran,periodic}.rs` (the rows and their text order);
//! `traninit.c:29-34` (`StepCap::NgspiceRule`), `fourier.c:69` (the default count, 9).
//!
//! Contents: `pub(in super::super) fn push(&mut Lower, Stmt, &Card)`

use super::super::read::{Lower, Stmt};
use super::analysis::{Card, Sweep, Swept};
use super::words;
use crate::ir;
use crate::ir::dc_ac::{Ac, Dc, DcAxis, Disto, Noise, NoiseRef, Pz, PzInput, Sens, SensAc, Sp, Tf};
use crate::ir::periodic::{Pss, PssMode};
use crate::ir::tran::{Four, StepCap, Tran};

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
        Card::Dc(axes) => {
            let mut rows = Vec::new();
            for (swept, [a, b, c]) in axes {
                let target = match swept {
                    Swept::Source(n) => lw.b.target(ir::TargetKind::SourceDc, &[], n, None),
                    Swept::Resistor(n) => lw.b.target(ir::TargetKind::ElementValue, &[], n, None),
                    Swept::Temp => ir::Target::TEMP,
                };
                let (a, b, c) = (
                    words::emit(lw, st, a),
                    words::emit(lw, st, b),
                    words::emit(lw, st, c),
                );
                rows.push(DcAxis {
                    target,
                    sweep: ir::SweepSpec::lin_step(a, b, c),
                    name: ir::NameId::NONE,
                });
            }
            let axes = lw.b.dc_axes(&rows);
            lw.b.analysis(
                scope,
                None,
                Dc {
                    axes,
                    restart: false,
                },
                span,
            );
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
            let stop = words::emit(lw, st, stop);
            let record_start = start.as_ref().map_or(none, |v| words::emit(lw, st, v));
            let print_step = words::emit(lw, st, step);
            let max_step = max.as_ref().map_or(none, |v| words::emit(lw, st, v));
            let row = Tran {
                stop,
                record_start,
                print_step,
                max_step,
                initial_step: none,
                noise_fmax: none,
                noise_fmin: none,
                noise_seed: none,
                segments: ir::List::EMPTY,
                cap: StepCap::NgspiceRule,
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
            let points_per_summary = summary.as_ref().map_or(none, |v| words::emit(lw, st, v));
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
            let f2_over_f1 = f2.as_ref().map_or(none, |v| words::emit(lw, st, v));
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
            let fundamental = words::emit(lw, st, fundamental);
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
            let fundamental = words::emit(lw, st, fundamental);
            let tstab = words::emit(lw, st, tstab);
            let harmonics = words::emit(lw, st, harmonics);
            let points = words::emit(lw, st, points);
            let osc_p = lw.b.name(osc);
            let osc_n = lw.b.name("0");
            let sc_iter = words::emit(lw, st, sc_iter);
            let steady_coeff = words::emit(lw, st, steady_coeff);
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

/// Appends a sweep: `points` per decade or octave, or `points` values in all for `lin`.
fn emit_sweep(lw: &mut Lower<'_, '_>, st: Stmt, s: &Sweep) -> ir::SweepSpec {
    let start = words::emit(lw, st, &s.start);
    let stop = words::emit(lw, st, &s.stop);
    let points = words::emit(lw, st, &s.points);
    match s.kind {
        ir::SweepKind::Dec => ir::SweepSpec::dec(start, stop, points),
        ir::SweepKind::Oct => ir::SweepSpec::oct(start, stop, points),
        ir::SweepKind::Lin => ir::SweepSpec::lin_points(start, stop, points),
        ir::SweepKind::Log | ir::SweepKind::List => {
            unreachable!("sweep reads dec, oct or lin only")
        }
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
