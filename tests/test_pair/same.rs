//! Step 4 of a same-dialect cell (`ngspice-ngspice`, `vacask-vacask`): the source deck and the
//! deck written from it run on the same simulator, the same build, so every plot, vector name,
//! point count and value must be the same.
//!
//! The bound. One build given the same circuit does the same floating-point operations in the
//! same order, so its raw file holds the same doubles: the bound is zero. Only an input that
//! differs can move a number. Each such input, and its bound:
//!
//! 1. A literal spelt otherwise (`1k` written `1000`): ngspice's number reader is not
//!    correctly rounded (a mantissa times a power of ten), so the two spellings may read one
//!    ulp apart. A value that depends smoothly on that parameter moves by the parameter's
//!    relative change times its sensitivity; a transient whose step control decides otherwise
//!    moves anywhere inside its truncation-error tube.
//! 2. The items in another order: another matrix ordering, another rounding in the LU;
//!    Newton then stops at another iterate inside its own test, `|Δx| ≤ RELTOL·|x| + VNTOL`
//!    (ABSTOL for a current), so two converged answers lie within twice that of each other.
//!
//! Neither is a fact this file can see, so it does not excuse them: it compares bit for bit and
//! names the largest difference, relative and absolute, for the census to class. A deck whose
//! difference a spelling or an order explains is a listed row with that proof, never a
//! tolerance.
//!
//! Cites: ngspice-45 `INPevaluate` (src/spicelib/parser/inpeval.c, the mantissa times a power
//! of ten); the Newton test (`NIconvTest`, niconv.c).
//!
//! Contents:
//! - `pub fn identical(a: &[RawData], b: &[RawData]) -> Result<(), String>`

use spicerack::result::RawData;

use crate::cmp;

/// Whether the two runs of one simulator hold the same plots, names, points and values bit for
/// bit. `Err` names the first plot or name that differs, or the largest value difference (its
/// vector, point, both values) with the count of points that differ.
pub fn identical(a: &[RawData], b: &[RawData]) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!("{} plots -> {}", a.len(), b.len()));
    }
    // (relative difference, vector, point, the two values), the largest; points that differ.
    let mut worst: Option<(f64, String, usize, String, String)> = None;
    let mut differ = 0usize;
    for (pa, pb) in a.iter().zip(b) {
        let names = |p: &RawData| -> Vec<String> {
            p.variables.iter().map(|v| cmp::normal(&v.name)).collect()
        };
        if names(pa) != names(pb) {
            return Err(format!(
                "{}: vectors {:?} -> {:?}",
                pa.plot_name,
                names(pa),
                names(pb)
            ));
        }
        for i in 0..pa.variables.len() {
            let (ya, yb) = (column(pa, i), column(pb, i));
            if ya.len() != yb.len() {
                return Err(format!(
                    "{} {}: {} -> {} points",
                    pa.plot_name,
                    pa.variables[i].name,
                    ya.len(),
                    yb.len()
                ));
            }
            // A complex plot's scale (frequency) is real: ngspice-45's SP analysis leaves its
            // imaginary part unset, a different subnormal in each run of one deck
            // (Tschebyschef-LP.sp: 4.63557106232844e-310 against …765e-310).
            let scale = i == 0;
            for (p, (x, y)) in ya.iter().zip(&yb).enumerate() {
                // The same number: the same bits, or -0 against +0 (gf180's `.dc vds -0 …`
                // written `0`: one value, two spellings).
                let same = |u: f64, v: f64| u.to_bits() == v.to_bits() || (u == 0.0 && v == 0.0);
                if same(x.0, y.0) && (scale || same(x.1, y.1)) {
                    continue;
                }
                differ += 1;
                let d = (x.0 - y.0).hypot(x.1 - y.1);
                let rel = d / x.0.hypot(x.1).max(y.0.hypot(y.1)).max(f64::MIN_POSITIVE);
                if worst.as_ref().is_none_or(|w| rel > w.0 || rel.is_nan()) {
                    let at = |v: (f64, f64)| format!("{:e}{:+e}i", v.0, v.1);
                    let name = format!("{} {}", pa.plot_name, pa.variables[i].name);
                    worst = Some((rel, name, p, at(*x), at(*y)));
                }
            }
        }
    }
    match worst {
        None => Ok(()),
        Some((rel, name, p, x, y)) => Err(format!(
            "{differ} values differ; the largest, relative {rel:.3e}: {name} at point {p}: {x} -> {y}"
        )),
    }
}

// ---- private ----

/// Column `i` of `p` as (re, im) pairs.
fn column(p: &RawData, i: usize) -> Vec<(f64, f64)> {
    match p.complex(i) {
        Some(c) => c.iter().map(|z| (z.re, z.im)).collect(),
        None => p.re(i).iter().map(|&x| (x, 0.0)).collect(),
    }
}
