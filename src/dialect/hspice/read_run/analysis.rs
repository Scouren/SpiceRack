//! One analysis from its words (the card's dot and tag already dropped, so a `.control` command
//! line reads through the same rules): [`read`] validates the whole analysis into a [`Card`];
//! `rows::push` appends it. ngspice-45's analyses (`inp2dot.c` `INP2dot`, `analysis.c:36-58`, this
//! build has PSS and RFSPICE, not HB or SENSE2): `op`, `dc`, `ac`, `tran`, `noise`, `tf`,
//! `sens`, `pz`, `disto`, `sp`, `pss`, and `four`/`fourier`, which the frontend runs on the
//! transient (`fourier.c`).
//!
//! Cites: ngspice-45 `inp2dot.c` `dot_*` (each argument's position and word), `traninit.c:29-34`
//! (no `tmax`, or a zero one: the ceiling `min(tstep, span/50)`, `StepCap::NgspiceRule`),
//! `dctrcurv.c:89-149` (a sweep's name: a resistor, a V or I source, or `temp`), `fourier.c:69`
//! (`nfreqs` 10 counts DC and 9 harmonics: runs four1.cir, four2.cir), `dot_pss` (every
//! argument read, the oscillator node against ground), `pzsetp.c:78-87`; runs in `rewrite/DIALECT-HSPICE.md` §3.
//!
//! HSPICE forms (`rewrite/DIALECT-HSPICE.md` §6, settle decks on ESPice 0.3.0 in HSPICE mode:
//! ESPice is not HSPICE): `.dc src start stop step`, `.dc src start= stop= step=`
//! (`dc_forms.sp`), `.dc src dec n a b` (`dc_dec.sp`), `.dc src poi n v…` (`dc_poi.sp`), a
//! `sweep param a b step` tail (`dc_sweep_param.sp`); `.ac poi n f…` increasing only
//! (`ac_poi.sp`, `ac_poi_sorted.sp`); a fractional count refused (`ac_frac.sp`).
//!
//! Contents: `pub(in super::super) enum Card`, `pub(in super::super) enum Swept`,
//! `pub(in super::super) enum Sweep`, `pub(in super::super) enum Axis`,
//! `pub(in super::super) enum Outer`, `pub(in super::super) fn read(&mut Lower, Stmt, &str,
//! &[String]) -> Result<Card, Fail>`

use super::super::expr_read::Fail;
use super::super::read::{Lower, Stmt};
use super::super::read_outputs;
use super::super::value::Val;
use super::words::{self, invalid};
use super::{NO_FIELD, RUN_DEFAULTS};
use crate::ir;
use crate::ir::dc_ac::{PzFind, PzTransfer};

/// An analysis read and validated, nothing appended yet.
#[derive(Clone, Debug)]
pub(in super::super) enum Card {
    /// `op`.
    Op,
    /// `op t1 t2 …`: operating-point reports at those times of the transient (settle
    /// `timed_op.sp`).
    TimedOp(Vec<Val>),
    /// `dc`: the axes, innermost first (what is swept and its values), and the parameter a
    /// `sweep` tail runs the analysis over (an outer loop).
    Dc(Vec<(Swept, Axis)>, Option<Outer>),
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
        out: ir::Probe,
        source: String,
        sweep: Sweep,
        summary: Option<Val>,
    },
    /// `tf out src`.
    Tf { out: ir::Probe, source: String },
    /// `sens out [dc | ac sweep]`.
    Sens { out: ir::Probe, ac: Option<Sweep> },
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
    /// `four freq out …`; the count is ngspice's default unless the deck sets `nfreqs`.
    Four {
        fundamental: Val,
        outs: Vec<ir::Probe>,
        default_count: bool,
    },
    /// `pss fguess stabtime oscnode points harmonics sc_iter steady_coeff [uic]`.
    Pss {
        fundamental: Val,
        tstab: Val,
        osc: String,
        points: Val,
        harmonics: Val,
        sc_iter: Val,
        steady_coeff: Val,
        uic: bool,
    },
}

/// What a DC axis sweeps.
#[derive(Clone, Debug)]
pub(in super::super) enum Swept {
    /// A V or I source's DC value, by name.
    Source(String),
    /// A resistor's value, by name.
    Resistor(String),
    /// The temperature.
    Temp,
}

/// `DEC|OCT|LIN np fstart fstop`, or `POI n v1 … vn`.
#[derive(Clone, Debug)]
pub(in super::super) enum Sweep {
    /// A range: `points` per decade or octave, or in all (`lin`).
    Range {
        kind: ir::SweepKind,
        points: Val,
        start: Val,
        stop: Val,
    },
    /// The values in run order (`poi`: settle `dc_poi.sp` keeps order and repeats).
    List(Vec<Val>),
}

/// A `sweep` tail: a parameter over values, or a `.data` table's rows (`sweep data=name`:
/// the table's name, its columns, its values row-major).
#[derive(Clone, Debug)]
pub(in super::super) enum Outer {
    /// `sweep p values`.
    Param(String, Axis),
    /// `sweep data=name` (settle `data_sweep.sp`: one plot per row; ESPice only).
    Data(String, Vec<String>, Vec<Val>),
}

/// The values of a DC axis: `start stop step` (or `start= stop= step=`), or a [`Sweep`].
#[derive(Clone, Debug)]
pub(in super::super) enum Axis {
    /// `start stop step`.
    Step([Val; 3]),
    /// `dec`, `oct`, `lin`, `poi`.
    Sweep(Sweep),
}

/// The analysis `name` with its words, validated whole. `Err` names the code and class of the
/// `Opaque` the statement becomes. The control area calls it with a script command's words
/// (`tran 1n 1u uic`: `name` "tran", `w` `["1n", "1u", "uic"]` as `words::split` gives them),
/// then `rows::push` with the card.
pub(in super::super) fn read(
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
        "op" => Card::TimedOp(
            w.iter()
                .map(|x| words::value(lw, st, x))
                .collect::<Result<_, _>>()
                .map_err(|_| no_field())?,
        ),
        "dc" => dc(lw, st, w)?,
        "ac" => {
            let sweep = sweep(lw, st, w, 0)?;
            if w.len() > words_of(&sweep) {
                return Err(no_field());
            }
            if matches!(sweep, Sweep::List(_)) && defaults(&sweep) {
                return Err(invalid());
            }
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
            // `dot_noise`: the output is `V(…)` (inp2dot.c:48), else ngspice's error
            let first = w.first().ok_or_else(invalid)?;
            let v = first.len() > 2 && first[..2].eq_ignore_ascii_case("v(");
            // HSPICE's `.noise ov src inter` takes the `.ac` sweep (the suite's
            // noise_ac_sweep.sp): no rule yet
            if !v || w.len() < 6 {
                return Err(no_field());
            }
            let out = read_outputs::probe(lw, st, first, ir::AnalysisKind::Noise)?;
            let source = lw.names.fold(&w[1]);
            let sweep = sweep(lw, st, w, 2)?;
            let n = 2 + words_of(&sweep);
            if w.len() > n + 1 {
                return Err(invalid());
            }
            let summary = (w.len() == n + 1).then(|| val(lw, n)).transpose()?;
            Card::Noise {
                out,
                source,
                sweep,
                summary,
            }
        }
        "tf" if w.len() == 2 => {
            let out = read_outputs::probe(lw, st, &w[0], ir::AnalysisKind::Tf)?;
            Card::Tf {
                out,
                source: lw.names.fold(&w[1]),
            }
        }
        "sens" => {
            let first = w.first().ok_or_else(invalid)?;
            let out = read_outputs::probe(lw, st, first, ir::AnalysisKind::Sens)?;
            match w.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
                None if w.len() == 1 => Card::Sens { out, ac: None },
                Some("dc") if w.len() == 2 => Card::Sens { out, ac: None },
                Some("ac") if w.len() >= 4 => {
                    let s = sweep(lw, st, w, 2)?;
                    if w.len() != 2 + words_of(&s) {
                        return Err(no_field());
                    }
                    Card::Sens { out, ac: Some(s) }
                }
                // a parameter filter (`Sens_filter`) has no field
                _ => return Err(no_field()),
            }
        }
        // HSPICE's `.pz ov src` (the suite's pz_source.sp): no rule yet
        "pz" if w.len() == 2 => return Err(no_field()),
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
        "disto" => {
            let sweep = sweep(lw, st, w, 0)?;
            let n = words_of(&sweep);
            if w.len() > n + 1 {
                return Err(invalid());
            }
            let f2 = (w.len() == n + 1).then(|| val(lw, n)).transpose()?;
            Card::Disto { sweep, f2 }
        }
        "sp" => {
            let sweep = sweep(lw, st, w, 0)?;
            let n = words_of(&sweep);
            if w.len() > n + 1 {
                return Err(invalid());
            }
            let noise = match w.get(n).map(|s| words::value(lw, st, s)).transpose()? {
                None => false,
                Some(Val::Lit(l, _)) if l.mantissa == 0 => false,
                Some(Val::Lit(..)) => true,
                Some(Val::Tree(_) | Val::Str(_)) => return Err(no_field()),
            };
            Card::Sp { sweep, noise }
        }
        "four" | "fourier" if w.len() >= 2 => {
            // `fourier.c:69`: the count is `nfreqs` (DC and the harmonics). An `.options
            // nfreqs=` makes the count the option's (`harmonics` none, A16); one a script sets
            // has no item to name.
            // ponytail: a text scan of this file's non-comment lines; an `nfreqs` in another
            // file of the deck is not seen (a deck-wide option table would see it)
            let (mut by_option, mut by_script) = (false, false);
            // a deck's line 1 is its title
            let title = usize::from(lw.role == ir::Role::Deck);
            for l in lw.toks.source().lines().skip(title) {
                let l = l.trim_start().to_ascii_lowercase();
                // only nfreqs gives the count; fourgridsize is the grid, which stays the
                // deck's option (`Four.grid` none either way)
                let card = l.starts_with(".opt");
                let named = l.contains("nfreqs");
                if named && card {
                    by_option = true;
                } else if (named || l.contains("fourgridsize")) && !card && !l.starts_with('*') {
                    by_script = true;
                }
            }
            if by_script {
                return Err(no_field());
            }
            let fundamental = val(lw, 0)?;
            let outs = w[1..]
                .iter()
                .map(|o| read_outputs::probe(lw, st, o, ir::AnalysisKind::Four))
                .collect::<Result<Vec<_>, _>>()?;
            Card::Four {
                fundamental,
                outs,
                default_count: !by_option,
            }
        }
        // `.pss fguess stabtime oscnode points harmonics sc_iter steady_coeff [uic]`
        "pss" if w.len() == 7 || w.len() == 8 => {
            let uic = w.len() == 8;
            if uic && !w[7].eq_ignore_ascii_case("uic") {
                return Err(invalid());
            }
            let v = |lw: &Lower<'_, '_>, k: usize| words::value(lw, st, &w[k]);
            Card::Pss {
                fundamental: v(lw, 0)?,
                tstab: v(lw, 1)?,
                osc: lw.names.node(&w[2]),
                points: v(lw, 3)?,
                harmonics: v(lw, 4)?,
                sc_iter: v(lw, 5)?,
                steady_coeff: v(lw, 6)?,
                uic,
            }
        }
        // fewer arguments: `dot_pss` reads every one from the line regardless
        "pss" => return Err(no_field()),
        _ => return Err(invalid()),
    })
}

// ---- private ----

/// `dc`: one or two axes (a name, then its values), then an optional `sweep param values` tail.
fn dc(lw: &mut Lower<'_, '_>, st: Stmt, w: &[String]) -> Result<Card, Fail> {
    let tail = w.iter().position(|x| x.eq_ignore_ascii_case("sweep"));
    let (inner, outer) = tail.map_or((w, None), |k| (&w[..k], Some(&w[k + 1..])));
    let mut axes = Vec::new();
    let mut at = 0;
    // Bounded: each turn takes an axis of at least two words.
    while at < inner.len() {
        if axes.len() == 2 {
            return Err(no_field());
        }
        let name = inner[at].to_ascii_lowercase();
        let swept = match name.as_bytes()[0] {
            _ if name == "temp" => Swept::Temp,
            b'v' | b'i' => Swept::Source(lw.names.fold(&inner[at])),
            b'r' => Swept::Resistor(lw.names.fold(&inner[at])),
            _ => return Err(no_field()),
        };
        let (values, used) = axis(lw, st, &inner[at + 1..])?;
        axes.push((swept, values));
        at += 1 + used;
    }
    if axes.is_empty() {
        return Err(invalid());
    }
    let outer = match outer {
        None => None,
        Some([data, eq, table]) if data.eq_ignore_ascii_case("data") && eq == "=" => {
            let (name, columns, texts) = lw
                .data
                .iter()
                .find(|d| d.1.eq_ignore_ascii_case(table))
                .map(|d| (d.1.clone(), d.2.clone(), d.3.clone()))
                .ok_or_else(no_field)?;
            let values = texts
                .iter()
                .map(|v| words::value(lw, st, v))
                .collect::<Result<Vec<_>, _>>()?;
            Some(Outer::Data(name, columns, values))
        }
        Some([name, rest @ ..]) if !rest.is_empty() => {
            let (values, used) = axis(lw, st, rest)?;
            if used != rest.len() {
                return Err(invalid());
            }
            Some(Outer::Param(name.clone(), values))
        }
        Some(_) => return Err(no_field()),
    };
    Ok(Card::Dc(axes, outer))
}

/// One axis's values at the start of `w`, and the words it took: `start stop step`,
/// `start=a stop=b step=c`, `dec|oct|lin n a b`, `poi n v…`. A zero step is refused.
fn axis(lw: &Lower<'_, '_>, st: Stmt, w: &[String]) -> Result<(Axis, usize), Fail> {
    let first = w.first().ok_or_else(invalid)?.to_ascii_lowercase();
    if matches!(first.as_str(), "dec" | "oct" | "lin" | "poi") {
        let s = sweep(lw, st, w, 0)?;
        let used = words_of(&s);
        return Ok((Axis::Sweep(s), used));
    }
    let keyed = w.len() >= 9
        && ["start", "stop", "step"]
            .iter()
            .enumerate()
            .all(|(k, key)| w[3 * k].eq_ignore_ascii_case(key) && w[3 * k + 1] == "=");
    let (picks, used) = if keyed {
        ([2, 5, 8], 9)
    } else {
        ([0, 1, 2], 3)
    };
    let vals = picks.map(|k| {
        w.get(k)
            .ok_or_else(invalid)
            .and_then(|x| words::value(lw, st, x))
    });
    let [a, b, c] = vals;
    let (a, b, c) = (a?, b?, c?);
    if matches!(c, Val::Lit(l, _) if l.mantissa == 0) {
        return Err(invalid());
    }
    Ok((Axis::Step([a, b, c]), used))
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
    // a zero tmax is no tmax (traninit.c:29); a zero tstart is recording from 0, the IR's none
    let zero = |v: &Val| matches!(v, Val::Lit(l, _) if l.mantissa == 0);
    let start = optional.next().filter(|v| !zero(v));
    let max = optional.next().filter(|v| !zero(v));
    Ok(Card::Tran {
        step,
        stop,
        start,
        max,
        uic,
    })
}

/// `DEC|OCT|LIN np fstart fstop` at `w[at..at + 4]`, or `POI n v1 … vn` (strictly increasing
/// literals: settle `ac_poi.sp` refused, `ac_poi_sorted.sp`; `dc_poi.sp` keeps order and
/// repeats, so the caller checks the order where it matters).
fn sweep(lw: &Lower<'_, '_>, st: Stmt, w: &[String], at: usize) -> Result<Sweep, Fail> {
    let word = |k: usize| w.get(at + k).ok_or_else(invalid);
    let kind = match word(0)?.to_ascii_lowercase().as_str() {
        "dec" => ir::SweepKind::Dec,
        "oct" => ir::SweepKind::Oct,
        "lin" => ir::SweepKind::Lin,
        "poi" => ir::SweepKind::List,
        _ => return Err(invalid()),
    };
    let points = words::value(lw, st, word(1)?)?;
    // a fractional count is refused (settle ac_frac.sp: ESPice "InvalidAnalysisArguments")
    let count = match &points {
        Val::Lit(l, _) if l.value().fract() != 0.0 => return Err(invalid()),
        Val::Lit(l, _) => Some(l.value()),
        Val::Tree(_) | Val::Str(_) => None,
    };
    if kind == ir::SweepKind::List {
        let n = count.ok_or_else(no_field)?;
        if !(1.0..=65_535.0).contains(&n) {
            return Err(invalid());
        }
        let n = n as usize;
        let values = (0..n)
            .map(|k| word(2 + k).and_then(|x| words::value(lw, st, x)))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Sweep::List(values));
    }
    Ok(Sweep::Range {
        kind,
        points,
        start: words::value(lw, st, word(2)?)?,
        stop: words::value(lw, st, word(3)?)?,
    })
}

/// The words a sweep takes on its card: 4 for a range, `poi n` and the values for a list.
fn words_of(s: &Sweep) -> usize {
    match s {
        Sweep::List(v) => 2 + v.len(),
        Sweep::Range { .. } => 4,
    }
}

/// Whether ngspice runs other numbers than an `.ac` sweep's literals (`dot_ac`: `np < 1` is 10,
/// `fstart < 0` is 1, `fstop < fstart` is `1000 fstart`). Comparing two literals is no
/// arithmetic on them (R1).
fn defaults(s: &Sweep) -> bool {
    let lit = |v: &Val| match v {
        Val::Lit(l, _) => Some(l.value()),
        Val::Tree(_) | Val::Str(_) => None,
    };
    match s {
        Sweep::Range {
            points,
            start,
            stop,
            ..
        } => {
            let np = lit(points).is_some_and(|n| n < 1.0);
            let start = lit(start);
            let below = start.is_some_and(|a| a < 0.0);
            let reversed = start.zip(lit(stop)).is_some_and(|(a, b)| b < a);
            np || below || reversed
        }
        // a list whose literals do not increase (settle ac_poi.sp: ESPice refuses it)
        Sweep::List(values) => values
            .windows(2)
            .any(|p| lit(&p[0]).zip(lit(&p[1])).is_some_and(|(a, b)| b <= a)),
    }
}

/// `P4303`, `Unread`: a form the IR has no field for.
fn no_field() -> Fail {
    Fail {
        code: NO_FIELD,
        class: ir::OpaqueClass::Unread,
    }
}
