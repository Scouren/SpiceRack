//! The time tube two transients are compared through: a point passes when
//! it lies within a tolerance of the band the other run spans around its
//! time. Shared by `test_import_suite` and `test_cross_sim` (`#[path]`).
//!
//! Cites: none (a comparison; the oracle is the simulator run on both decks).
//!
//! Contents:
//! - `pub fn in_tube(xs: &[f64], ys: &[f64], x: f64, dx: f64, y: f64, tol: f64) -> bool`
//! - `pub fn leaves_tube(xa: &[f64], ya: &[f64], xb: &[f64], yb: &[f64], dx: f64, tol: f64) -> Option<usize>`

/// Whether `y` lies within `tol` of the band `ys` spans over `[x - dx, x + dx]`
/// (the samples in the window and the two that bracket it). A switching edge
/// that ngspice's timestep control moved by less than `dx` still passes; a
/// wrong value does not.
pub fn in_tube(xs: &[f64], ys: &[f64], x: f64, dx: f64, y: f64, tol: f64) -> bool {
    let lo = xs.partition_point(|&t| t < x - dx).saturating_sub(1);
    let hi = (xs.partition_point(|&t| t <= x + dx) + 1).min(xs.len());
    let band = &ys[lo..hi];
    let (min, max) = band
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    y >= min - tol && y <= max + tol
}

/// The first `k` whose `ya[k]` is not [`in_tube`] of `(xb, yb)`. Both time
/// axes rise, so the window only slides forward: a min and a max deque make
/// it O(n) where `in_tube` per point is O(n * window), hours on a 2M-point
/// TRNOISE plot.
pub fn leaves_tube(
    xa: &[f64],
    ya: &[f64],
    xb: &[f64],
    yb: &[f64],
    dx: f64,
    tol: f64,
) -> Option<usize> {
    if !xa.is_sorted() || !xb.is_sorted() {
        return (0..ya.len()).find(|&k| !in_tube(xb, yb, xa[k], dx, ya[k], tol));
    }
    let (mut mins, mut maxs) = (
        std::collections::VecDeque::<usize>::new(),
        std::collections::VecDeque::<usize>::new(),
    );
    let (mut lo, mut hi, mut pushed) = (0, 0, 0);
    for (k, (&x, &y)) in xa.iter().zip(ya).enumerate() {
        while lo < xb.len() && xb[lo] < x - dx {
            lo += 1;
        }
        while hi < xb.len() && xb[hi] <= x + dx {
            hi += 1;
        }
        let (start, end) = (lo.saturating_sub(1), (hi + 1).min(xb.len()));
        while pushed < end {
            while maxs.back().is_some_and(|&j| yb[j] <= yb[pushed]) {
                maxs.pop_back();
            }
            while mins.back().is_some_and(|&j| yb[j] >= yb[pushed]) {
                mins.pop_back();
            }
            maxs.push_back(pushed);
            mins.push_back(pushed);
            pushed += 1;
        }
        while maxs.front().is_some_and(|&j| j < start) {
            maxs.pop_front();
        }
        while mins.front().is_some_and(|&j| j < start) {
            mins.pop_front();
        }
        let min = mins.front().map_or(f64::INFINITY, |&j| yb[j]);
        let max = maxs.front().map_or(f64::NEG_INFINITY, |&j| yb[j]);
        if !(y >= min - tol && y <= max + tol) {
            return Some(k);
        }
    }
    None
}
