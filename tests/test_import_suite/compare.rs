//! Two ngspice runs -> pass or the first difference: every plot's sweep grid
//! and values, a transient through a time tube (`tube.rs`), and the reordered
//! control deck a failed comparison is retried against.
//!
//! Cites: none (the oracle is ngspice itself, run on both decks).
//!
//! Contents:
//! - `pub(super) fn compare(a: &Run, b: &Run) -> Result<(), String>`
//! - `pub(super) fn reordered(deck: &str) -> String`

use super::*;

/// Every plot of `b` must match `a`'s, in order. Op, dc and ac sweep grids
/// must be the same and every point agree; a transient must end at the same
/// time and each of `a`'s points lie in `b`'s tube (`in_tube`, a window of
/// 1% of the stop time): reordering an unconverted deck's own cards moves a
/// switching edge by a few timesteps, so pointwise equality is not a test of
/// the conversion. A vector passes within `1e-3` of its largest magnitude
/// (plus `1e-9`), so a node near zero is judged against its own swing.
pub(super) fn compare(a: &Run, b: &Run) -> Result<(), String> {
    let names = |r: &Run| {
        r.plots
            .iter()
            .map(|p| p.plot_name.clone())
            .collect::<Vec<_>>()
    };
    if names(a) != names(b) {
        return Err(format!("plots {:?} became {:?}", names(a), names(b)));
    }
    for (pa, pb) in a.plots.iter().zip(&b.plots) {
        let transient = pa
            .variables
            .first()
            .is_some_and(|v| v.name.eq_ignore_ascii_case("time"));
        let (xa, xb) = (pa.re(0), pb.re(0));
        // Span-relative: a sweep crossing zero lands within an ulp of it, and
        // `-50m` and `-0.05` are two such ulps in ngspice's number parser.
        let span = xa.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        if !transient
            && (xa.len() != xb.len()
                || xa
                    .iter()
                    .zip(xb.iter())
                    .any(|(x, y)| (x - y).abs() > 1e-9 * span))
        {
            return Err(format!(
                "{}: sweep grid changed ({} -> {} points)",
                pa.plot_name,
                xa.len(),
                xb.len()
            ));
        }
        if transient && !matches!((xa.last(), xb.last()), (Some(&x), Some(&y)) if close(x, y)) {
            return Err(format!(
                "{}: stop time {:?} -> {:?}",
                pa.plot_name,
                xa.last(),
                xb.last()
            ));
        }
        let mut shared = 0;
        // A name can repeat (a TXL line's two `i(y1)` branches): pair the
        // k-th occurrence in `a` with the k-th in `b`. Indexed by name, since
        // a power grid saves ~10^6 vectors and a scan per vector is quadratic.
        let mut in_b =
            std::collections::HashMap::<String, std::collections::VecDeque<usize>>::new();
        for (j, w) in pb.variables.iter().enumerate() {
            in_b.entry(w.name.to_ascii_lowercase())
                .or_default()
                .push_back(j);
        }
        for (i, v) in pa.variables.iter().enumerate() {
            let Some(j) = in_b
                .get_mut(&v.name.to_ascii_lowercase())
                .and_then(|q| q.pop_front())
            else {
                continue;
            };
            if i == 0 {
                continue; // the scale; popped so the k-th pairing counts it
            }
            shared += 1;
            let (ya, yb) = (column(pa, i), column(pb, j));
            let swing = ya.iter().map(|c| c.norm()).fold(0.0, f64::max);
            let tol = 1e-9 + 1e-3 * swing;
            // The same grid and every point within `tol` is inside any tube:
            // the common case, and the tube scan costs a window per point.
            let pointwise = || xa == xb && ya.iter().zip(&yb).all(|(p, q)| (p - q).norm() <= tol);
            if transient && !pointwise() {
                let (ra, rb) = (pa.re(i), pb.re(j));
                let dx = 0.01 * xa.last().unwrap();
                if let Some(k) = leaves_tube(&xa, &ra, &xb, &rb, dx, tol) {
                    return Err(format!(
                        "{} {}: {} leaves the tube at {}",
                        pa.plot_name, v.name, ra[k], xa[k]
                    ));
                }
            } else if let Some(k) = (0..ya.len()).find(|&k| (ya[k] - yb[k]).norm() > tol) {
                return Err(format!(
                    "{} {}: {} vs {} at {}",
                    pa.plot_name, v.name, ya[k], yb[k], xa[k]
                ));
            }
        }
        if shared == 0 && pa.variables.len() > 1 {
            return Err(format!("{}: no vector in common", pa.plot_name));
        }
    }
    let (ma, mb): (Vec<_>, Vec<_>) = (
        a.measures.iter().map(|m| &m.0).collect(),
        b.measures.iter().map(|m| &m.0).collect(),
    );
    if ma != mb {
        return Err(format!(".meas {ma:?} became {mb:?}"));
    }
    for ((n, x), (_, y)) in a.measures.iter().zip(&b.measures) {
        if (x - y).abs() > 1e-9 + 1e-3 * x.abs() {
            return Err(format!(".meas {n}: {x} vs {y}"));
        }
    }
    Ok(())
}

/// The control for a failed comparison: `deck` with its top-level element
/// cards (and their `+` lines) in reverse order, unconverted. The circuit is
/// the same; only ngspice's matrix and timestep order change.
pub(super) fn reordered(deck: &str) -> String {
    let mut lines = deck.lines();
    let mut out = vec![lines.next().unwrap_or("").to_string()];
    let (mut depth, mut cards, mut tail) = (0, Vec::<String>::new(), Vec::new());
    for l in lines {
        let t = l.trim_start().to_ascii_lowercase();
        if t.starts_with(".subckt") || t.starts_with(".macro") {
            depth += 1;
        } else if t.starts_with(".ends") || t.starts_with(".eom") {
            depth -= 1;
            out.push(l.to_string());
            continue;
        }
        let element = depth == 0 && t.starts_with(|c: char| c.is_ascii_alphabetic());
        match (element, t.starts_with('+'), cards.last_mut()) {
            (true, _, _) => cards.push(l.to_string()),
            (false, true, Some(last)) if tail.is_empty() && depth == 0 => {
                last.push('\n');
                last.push_str(l);
            }
            _ if t.starts_with(".end") && !t.starts_with(".ends") => tail.push(l.to_string()),
            _ => out.push(l.to_string()),
        }
    }
    out.extend(cards.into_iter().rev());
    out.extend(tail);
    out.join("\n") + "\n"
}

// ---- private ----

fn close(x: f64, y: f64) -> bool {
    (x - y).abs() <= 1e-9 * x.abs().max(y.abs())
}
