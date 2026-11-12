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
//! argument read, the oscillator node against ground), `pzsetp.c:78-87`; runs in `rewrite/DIALECT-NGSPICE.md` §3.
//!
//! Contents: `pub(in super::super) enum Card`, `pub(in super::super) enum Swept`,
//! `pub(in super::super) struct Sweep`, `pub(in super::super) fn read(&mut Lower, Stmt, &str,
//! &[String]) -> Result<Card, Fail>`

use super::super::expr_read::Fail;
use super::super::read::{Lower, Stmt};
use super::super::read_outputs;
use super::words::{self, Val, invalid};
use super::{NO_FIELD, RUN_DEFAULTS};
use crate::ir;
use crate::ir::dc_ac::{PzFind, PzTransfer};

/// An analysis read and validated, nothing appended yet.
#[derive(Clone, Debug)]
pub(in super::super) enum Card {
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

/// `DEC|OCT|LIN np fstart fstop`.
#[derive(Clone, Debug)]
pub(in super::super) struct Sweep {
    pub(super) kind: ir::SweepKind,
    pub(super) points: Val,
    pub(super) start: Val,
    pub(super) stop: Val,
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
            // `dot_noise`: the output is `V(…)` (inp2dot.c:48), else ngspice's error
            let first = w.first().ok_or_else(invalid)?;
            let v = first.len() > 2 && first[..2].eq_ignore_ascii_case("v(");
            if !v || w.len() < 6 || w.len() > 7 {
                return Err(invalid());
            }
            let out = read_outputs::probe(lw, st, first, ir::AnalysisKind::Noise)?;
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
                let named = l.contains("nfreqs") || l.contains("fourgridsize");
                if named && l.starts_with(".opt") {
                    by_option = true;
                } else if named && !l.starts_with('*') {
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

/// `P1303`, `Unread`: a form the IR has no field for.
fn no_field() -> Fail {
    Fail {
        code: NO_FIELD,
        class: ir::OpaqueClass::Unread,
    }
}
