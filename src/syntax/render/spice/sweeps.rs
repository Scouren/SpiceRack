//! A `.dc` axis by decade, octave or list → its SPICE-family text.
//!
//! Cites: Xyce RG `.DC` (`DEC|OCT src start stop points`, `src LIST v…`);
//! LTspice help `.DC`; HSPICE `.DC` (`var DEC|OCT np start stop`, `var POI
//! np v…`).
//!
//! Contents:
//! - `pub(super) fn dc_axis(w: &W, ax: &DcAxis, hspice: bool) -> String`
//! - `pub(super) fn xyce_lin(w: &W, sw: &Sweep, tok: u32, ac: &str) -> String`

use super::W;
use crate::syntax::Dialect;
use crate::syntax::render::read::{Ana, Bench, BenchCard, DcAxis, DcSweep, Sweep};

/// One `.dc` axis: Xyce's and LTspice's `[DEC|OCT] src start stop n` and
/// `src LIST v…` (Xyce RG `.DC`), or HSPICE's `src DEC|OCT|LIN np start
/// stop` and `src POI n v…` (HSPICE `.DC`).
pub(super) fn dc_axis(w: &W<'_, '_>, ax: &DcAxis, hspice: bool) -> String {
    let num = |v: f64| w.sp.number(v, w.x);
    let src = &ax.src;
    match (&ax.sweep, hspice) {
        (DcSweep::Lin(a, b, c), _) => format!(" {src} {} {} {}", num(*a), num(*b), num(*c)),
        (DcSweep::Log(s), false) => format!(
            " {} {src} {} {} {}",
            s.variation.to_ascii_uppercase(),
            num(s.start),
            num(s.stop),
            s.points
        ),
        (DcSweep::Log(s), true) => format!(
            " {src} {} {} {} {}",
            s.variation.to_ascii_uppercase(),
            s.points,
            num(s.start),
            num(s.stop)
        ),
        (DcSweep::List(v), _) => {
            let kw = if hspice {
                format!("POI {}", v.len())
            } else {
                "LIST".into()
            };
            let vals: Vec<String> = v.iter().map(|x| num(*x)).collect();
            format!(" {src} {kw} {}", vals.join(" "))
        }
    }
}

/// Xyce RG `.LIN` (S over the `.AC` sweep) for an S-parameter sweep `sw`
/// read at `tok` (`ac` its `.ac` spelling): a Xyce deck's own card as
/// written (its output form), else `.LIN` and, when the deck has no `.ac`
/// of the sweep, one.
pub(super) fn xyce_lin(w: &W<'_, '_>, sw: &Sweep, tok: u32, ac: &str) -> String {
    if w.rd.d == Dialect::Xyce {
        return w.rd.span(tok, w.rd.line_last(tok));
    }
    let same = |s: &Sweep| {
        (s.variation, s.points, s.start, s.stop) == (sw.variation, sw.points, sw.start, sw.stop)
    };
    let has_ac = w.deck.bench.iter().any(|b| match b {
        Bench::Card(n) => matches!(
            w.rd.bench_card(*n),
            Some(BenchCard::Ana(Ana::Ac(s))) if same(&s)
        ),
        _ => false,
    });
    match has_ac {
        true => ".LIN".to_string(),
        false => format!(".ac {ac}\n.LIN"),
    }
}
