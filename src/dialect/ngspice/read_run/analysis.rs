//! One analysis from its words (the card's dot and tag already dropped, so a `.control` command
//! line reads through the same rules): [`read`] validates the whole analysis into a [`Card`],
//! [`push`] appends it. ngspice-45's analyses (`inp2dot.c` `INP2dot`, `analysis.c:36-58`, this
//! build has PSS and RFSPICE, not HB or SENSE2): `op`, `dc`, `ac`, `tran`, `noise`, `tf`,
//! `sens`, `pz`, `disto`, `sp`, `pss`, and `four`/`fourier`, which the frontend runs on the
//! transient (`fourier.c`).
//!
//! Cites: ngspice-45 `inp2dot.c` `dot_*` (each argument's position and word), `traninit.c:29-34`
//! (no `tmax`, or a zero one: the ceiling `min(tstep, span/50)`, `StepCap::NgspiceRule`),
//! `dctrcurv.c:89-149` (a sweep's name: a resistor, a V or I source, or `temp`), `fourier.c:69`
//! (`nfreqs` 10: DC and 9 harmonics), `pzsetp.c:78-87`; runs in `rewrite/DIALECT-NGSPICE.md` §3.
//!
//! Contents: `pub(super) enum Card`, `pub(super) enum Swept`, `pub(super) struct Sweep`, `pub(super) fn read(&mut Lower, Stmt, &str, &[String]) ->
//! Result<Card, Fail>`, `pub(super) fn push(&mut Lower, Stmt, &Card)`

use super::super::expr_read::Fail;
use super::super::read::{Lower, Stmt};
use super::words::{self, Out, Val, invalid};
use super::{NO_FIELD, RUN_DEFAULTS};
use crate::ir;
use crate::ir::dc_ac::{Ac, Dc, DcAxis, Disto, Noise, NoiseRef, Pz, PzFind, PzInput, PzTransfer};
use crate::ir::dc_ac::{Sens, SensAc, Sp, Tf};
use crate::ir::tran::{Four, StepCap, Tran};

/// An analysis read and validated, nothing appended yet.
#[derive(Clone, Debug)]
pub(super) enum Card {
    /// `op`.
    Op,
    /// `dc`: the axes, innermost first: what is swept, start, stop, step.
    Dc(Vec<(Swept, [Val; 3])>),
    /// `ac`.
    Ac(Sweep),
    /// `tran tstep tstop [tstart [tmax]] [uic]` (a zero `tmax` already dropped).
    Tran {
        step: Val,
        stop: Val,
        start: Option<Val>,
        max: Option<Val>,
        uic: bool,
    },
    /// `noise v(out[,ref]) src sweep [ptspersum]`.
    Noise {
        out: Out,
        source: String,
        sweep: Sweep,
        summary: Option<Val>,
    },
    /// `tf out src`.
    Tf { out: Out, source: String },
    /// `sens out [dc | ac sweep]`.
    Sens { out: Out, ac: Option<Sweep> },
    /// `pz in+ in- out+ out- vol|cur pol|zer|pz`.
    Pz {
        nodes: [String; 4],
        transfer: PzTransfer,
        find: PzFind,
    },
    /// `disto sweep [f2overf1]`.
    Disto { sweep: Sweep, f2: Option<Val> },
    /// `sp sweep [donoise]`.
    Sp { sweep: Sweep, noise: bool },
    /// `four freq out …`.
    Four { fundamental: Val, outs: Vec<Out> },
}

/// What a DC axis sweeps.
#[derive(Clone, Debug)]
pub(super) enum Swept {
    /// A V or I source's DC value, by name.
    Source(String),
    /// A resistor's value, by name.
    Resistor(String),
    /// The temperature.
    Temp,
}

/// `DEC|OCT|LIN np fstart fstop`.
#[derive(Clone, Debug)]
pub(super) struct Sweep {
    kind: ir::SweepKind,
    points: Val,
    start: Val,
    stop: Val,
}

/// The analysis `name` with its words, validated whole. `Err` names the code and class of the
/// `Opaque` the statement becomes.
pub(super) fn read(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    name: &str,
    w: &[String],
) -> Result<Card, Fail> {
    let val = |lw: &Lower<'_, '_>, k: usize| {
        w.get(k)
            .ok_or_else(invalid)
            .and_then(|s| words::value(lw, st, s))
    };
    Ok(match name {
        "op" if w.is_empty() => Card::Op,
        "op" => return Err(no_field()),
        "dc" => dc(lw, st, w)?,
        "ac" => {
            if w.len() > 4 {
                return Err(no_field());
            }
            let sweep = sweep(lw, st, w, 0)?;
            if defaults(&sweep) {
                return Err(Fail {
                    code: RUN_DEFAULTS,
                    class: ir::OpaqueClass::Unread,
                });
            }
            Card::Ac(sweep)
        }
        "tran" => tran(lw, st, w)?,
        "noise" => {
            let out = words::output(lw, w.first().ok_or_else(invalid)?).ok_or_else(invalid)?;
            if !matches!(out, Out::V(..)) || w.len() < 6 || w.len() > 7 {
                return Err(invalid());
            }
            let source = lw.names.fold(&w[1]);
            let sweep = sweep(lw, st, w, 2)?;
            let summary = (w.len() == 7).then(|| val(lw, 6)).transpose()?;
            Card::Noise {
                out,
                source,
                sweep,
                summary,
            }
        }
        "tf" if w.len() == 2 => {
            let out = words::output(lw, &w[0]).ok_or_else(invalid)?;
            Card::Tf {
                out,
                source: lw.names.fold(&w[1]),
            }
        }
        "sens" => {
            let out = words::output(lw, w.first().ok_or_else(invalid)?).ok_or_else(invalid)?;
            match w.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
                None if w.len() == 1 => Card::Sens { out, ac: None },
                Some("dc") if w.len() == 2 => Card::Sens { out, ac: None },
                Some("ac") if w.len() == 6 => Card::Sens {
                    out,
                    ac: Some(sweep(lw, st, w, 2)?),
                },
                // a parameter filter (`Sens_filter`) has no field
                _ => return Err(no_field()),
            }
        }
        "pz" if w.len() == 6 => {
            let transfer = match w[4].to_ascii_lowercase().as_str() {
                "vol" => PzTransfer::Voltage,
                "cur" => PzTransfer::Current,
                _ => return Err(invalid()),
            };
            let find = match w[5].to_ascii_lowercase().as_str() {
                "pol" => PzFind::Poles,
                "zer" => PzFind::Zeros,
                "pz" => PzFind::Both,
                _ => return Err(invalid()),
            };
            let nodes = [0, 1, 2, 3].map(|k| lw.names.node(&w[k]));
            Card::Pz {
                nodes,
                transfer,
                find,
            }
        }
        "disto" if (4..=5).contains(&w.len()) => Card::Disto {
            sweep: sweep(lw, st, w, 0)?,
            f2: (w.len() == 5).then(|| val(lw, 4)).transpose()?,
        },
        "sp" if (4..=5).contains(&w.len()) => {
            let sweep = sweep(lw, st, w, 0)?;
            let noise = match w.get(4).map(|s| words::value(lw, st, s)).transpose()? {
                None => false,
                Some(Val::Lit(l, _)) if l.mantissa == 0 => false,
                Some(Val::Lit(..)) => true,
                Some(Val::Expr(_)) => return Err(no_field()),
            };
            Card::Sp { sweep, noise }
        }
        "four" | "fourier" if w.len() >= 2 => {
            // the harmonic count is the deck's `nfreqs` (fourier.c:69): an option's value
            // copied into this row would repeat an operand (R13 b)
            // ponytail: a text scan of this file's non-comment lines; an `nfreqs` set in
            // another file of the deck is not seen (a deck-wide option table would see it)
            let sets = lw.toks.source().lines().any(|l| {
                let l = l.trim_start().to_ascii_lowercase();
                !l.starts_with('*') && (l.contains("nfreqs") || l.contains("fourgridsize"))
            });
            if sets {
                return Err(no_field());
            }
            let fundamental = val(lw, 0)?;
            let outs = w[1..]
                .iter()
                .map(|o| words::output(lw, o).ok_or_else(no_field))
                .collect::<Result<Vec<_>, _>>()?;
            Card::Four { fundamental, outs }
        }
        // sc_iter and steady_coeff have no `Pss` field yet
        "pss" => return Err(no_field()),
        _ => return Err(invalid()),
    })
}

/// Appends `card` as one `Analysis` in `st.scope`.
pub(super) fn push(lw: &mut Lower<'_, '_>, st: Stmt, card: &Card) {
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
            let output = words::probe(lw, out);
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
            let output = words::probe(lw, out);
            let source = lw.b.name(source);
            lw.b.analysis(scope, None, Tf { output, source }, span);
        }
        Card::Sens { out, ac: None } => {
            let output = words::probe(lw, out);
            lw.b.analysis(scope, None, Sens { output }, span);
        }
        Card::Sens { out, ac: Some(s) } => {
            let output = words::probe(lw, out);
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
        Card::Four { fundamental, outs } => {
            let fundamental = words::emit(lw, st, fundamental);
            let harmonics = lw.b.literal(plain(9));
            let periods = lw.b.literal(plain(1));
            let probes: Vec<ir::Probe> = outs.iter().map(|o| words::probe(lw, o)).collect();
            let outputs = lw.b.probes(&probes);
            let row = Four {
                fundamental,
                harmonics,
                periods,
                grid: none,
                outputs,
            };
            lw.b.analysis(scope, None, row, span);
        }
    }
}

// ---- private ----

/// `dc`: one or two axes of four words (`dot_dc` reads no third; a zero step is "Bad syntax").
fn dc(lw: &mut Lower<'_, '_>, st: Stmt, w: &[String]) -> Result<Card, Fail> {
    if w.len() != 4 && w.len() != 8 {
        return Err(if w.len() > 8 { no_field() } else { invalid() });
    }
    let mut axes = Vec::new();
    for chunk in w.chunks(4) {
        let name = chunk[0].to_ascii_lowercase();
        let swept = match name.as_bytes()[0] {
            _ if name == "temp" => Swept::Temp,
            b'v' | b'i' => Swept::Source(lw.names.fold(&chunk[0])),
            b'r' => Swept::Resistor(lw.names.fold(&chunk[0])),
            _ => return Err(invalid()),
        };
        let vals = [1, 2, 3].map(|k| words::value(lw, st, &chunk[k]));
        let [a, b, c] = vals;
        let (a, b, c) = (a?, b?, c?);
        if matches!(c, Val::Lit(l, _) if l.mantissa == 0) {
            return Err(invalid());
        }
        axes.push((swept, [a, b, c]));
    }
    Ok(Card::Dc(axes))
}

/// `tran tstep tstop [tstart [tmax]] [uic]`: numbers up to two after the first two
/// (`INPevaluate`), then `uic` or nothing (another word is ngspice's error).
fn tran(lw: &mut Lower<'_, '_>, st: Stmt, w: &[String]) -> Result<Card, Fail> {
    if w.len() < 2 {
        return Err(invalid());
    }
    let step = words::value(lw, st, &w[0])?;
    let stop = words::value(lw, st, &w[1])?;
    let mut rest = &w[2..];
    let uic = rest.last().is_some_and(|s| s.eq_ignore_ascii_case("uic"));
    if uic {
        rest = &rest[..rest.len() - 1];
    }
    if rest.len() > 2 {
        return Err(invalid());
    }
    let mut optional = Vec::new();
    for s in rest {
        optional.push(words::value(lw, st, s)?);
    }
    let mut optional = optional.into_iter();
    let start = optional.next();
    // a zero tmax is no tmax (traninit.c:29)
    let max = optional
        .next()
        .filter(|v| !matches!(v, Val::Lit(l, _) if l.mantissa == 0));
    Ok(Card::Tran {
        step,
        stop,
        start,
        max,
        uic,
    })
}

/// `DEC|OCT|LIN np fstart fstop` at `w[at..at + 4]`.
fn sweep(lw: &Lower<'_, '_>, st: Stmt, w: &[String], at: usize) -> Result<Sweep, Fail> {
    let word = |k: usize| w.get(at + k).ok_or_else(invalid);
    let kind = match word(0)?.to_ascii_lowercase().as_str() {
        "dec" => ir::SweepKind::Dec,
        "oct" => ir::SweepKind::Oct,
        "lin" => ir::SweepKind::Lin,
        _ => return Err(invalid()),
    };
    Ok(Sweep {
        kind,
        points: words::value(lw, st, word(1)?)?,
        start: words::value(lw, st, word(2)?)?,
        stop: words::value(lw, st, word(3)?)?,
    })
}

/// Whether ngspice runs other numbers than an `.ac` sweep's literals (`dot_ac`: `np < 1` is 10,
/// `fstart < 0` is 1, `fstop < fstart` is `1000 fstart`). Comparing two literals is no
/// arithmetic on them (R1).
fn defaults(s: &Sweep) -> bool {
    let lit = |v: &Val| match v {
        Val::Lit(l, _) => Some(l.value()),
        Val::Expr(_) => None,
    };
    let np = lit(&s.points).is_some_and(|n| n < 1.0);
    let start = lit(&s.start);
    let below = start.is_some_and(|a| a < 0.0);
    let reversed = start.zip(lit(&s.stop)).is_some_and(|(a, b)| b < a);
    np || below || reversed
}

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

/// `P1303`, `Unread`: a form the IR has no field for.
fn no_field() -> Fail {
    Fail {
        code: NO_FIELD,
        class: ir::OpaqueClass::Unread,
    }
}
