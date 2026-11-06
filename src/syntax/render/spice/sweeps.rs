//! A `.dc` axis by decade, octave or list → its SPICE-family text.
//!
//! Cites: Xyce RG `.DC` (`DEC|OCT src start stop points`, `src LIST v…`);
//! LTspice help `.DC`; HSPICE `.DC` (`var DEC|OCT np start stop`, `var POI
//! np v…`).
//!
//! Contents:
//! - `pub(super) fn dc_axis(w: &W, ax: &DcAxis, hspice: bool) -> String`

use super::W;
use crate::syntax::render::read::{DcAxis, DcSweep};

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
