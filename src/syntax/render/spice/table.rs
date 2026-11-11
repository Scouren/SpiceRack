//! An E/G `TABLE` → the value of a behavioural source: a chain of
//! ternaries over the table's segments. Xyce's `TABLE` is straight lines
//! between the points with the ends held, and at a vertical step the later
//! point (Xyce 7.10 over `(-1,1) (1,-1) (2,0)`: 1.0 at -1, 0.0 at 2),
//! which the chain says exactly; the native `TABLE` of ngspice-45 and
//! ESPice 0.3.0 rounds the corners (0.95 and -0.025 there), and ESPice's
//! B source has no `pwl()` or `table()` ("UnsupportedBsourceExpression").
//!
//! Cites: Xyce RG 7.10 (`E`, `G`: `TABLE`); ESPice 0.3.0 runs (its B
//! source's limits are its error's own text: "8 probed nets and currents,
//! 64 ops, 32 constants, stack depth 16").
//!
//! Contents:
//! - `pub(super) fn ternary_chain(x: &str, points: &[(f64, f64)], num: impl Fn(f64) -> String) -> String`
//! - `pub(super) fn fits(x: &str, points: &[(f64, f64)]) -> bool`

/// The segments of nonzero width a table may have under
/// `Alt::TernaryValue`. ESPice 0.3.0 compiles a B source of 64 operations
/// (a probe, a constant or an operator each, constant subtrees folded);
/// the chain over one probe is 6 and 11 a segment (run: five segments
/// pass, six stop with "TooManyOps").
const SEGMENTS: usize = 5;

/// The table `points` over the control `x` as one expression: `x < x1 ?
/// y1 : x < x2 ? y1 + (x - x1) * ((y2 - y1) / (x2 - x1)) : … : yn`, each
/// number spelled by `num` (a negative one in parentheses). `x` is
/// spliced as written: one probe, or an expression in its own
/// parentheses. `<`, so at a point the segment that starts there holds;
/// a vertical step (two points at one x) has no segment, and the later
/// point holds there, as on Xyce. The slope stays a constant subtree of
/// the table's own numbers, which the simulator folds.
///
/// # Panics
/// On a table with no point.
pub(super) fn ternary_chain(x: &str, points: &[(f64, f64)], num: impl Fn(f64) -> String) -> String {
    assert!(!points.is_empty(), "a table has a point");
    debug_assert!(
        points.iter().all(|p| p.0.is_finite() && p.1.is_finite()),
        "a table's numbers are finite"
    );
    let n = |v: f64| match num(v) {
        s if s.starts_with('-') => format!("({s})"),
        s => s,
    };
    let (x1, y1) = points[0];
    let mut out = format!("{x} < {} ? {}", n(x1), n(y1));
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x0 == x1 {
            continue;
        }
        let slope = format!("(({} - {}) / ({} - {}))", n(y1), n(y0), n(x1), n(x0));
        let line = format!("{} + ({x} - {}) * {slope}", n(y0), n(x0));
        out.push_str(&format!(" : {x} < {} ? {line}", n(x1)));
    }
    let (_, yn) = points[points.len() - 1];
    out.push_str(&format!(" : {}", n(yn)));
    out
}

/// Whether the table's [`ternary_chain`] over `x` is one
/// `Alt::TernaryValue` writes: `x` one probe, and at most [`SEGMENTS`]
/// segments of nonzero width.
pub(super) fn fits(x: &str, points: &[(f64, f64)]) -> bool {
    // ponytail: an expression for `x` is refused, though ESPice takes four
    // points over `2*v(1)` (its ops count twice a segment); count the
    // control's ops when a deck needs one.
    let segments = points.windows(2).filter(|w| w[0].0 != w[1].0).count();
    one_probe(x) && segments <= SEGMENTS
}

// ---- private ----

/// `x` is one probe: `v(a)`, `v(a,b)` or `i(source)`, names only inside.
fn one_probe(x: &str) -> bool {
    let x = x.trim();
    let head = x
        .get(..2)
        .is_some_and(|h| h.eq_ignore_ascii_case("v(") || h.eq_ignore_ascii_case("i("));
    let inner = x.get(2..x.len().saturating_sub(1)).unwrap_or("");
    let name = |c: char| c.is_ascii_alphanumeric() || "_.:, ".contains(c);
    head && x.ends_with(')') && !inner.is_empty() && inner.chars().all(name)
}

#[cfg(test)]
mod tests {
    use super::{fits, ternary_chain};

    // Oracle: this text, as `E1 2 0 value={…}`, ran on ESPice 0.3.0 equal
    // to Xyce 7.10's `E1 2 0 TABLE {V(1)} = (-1,1) (1,-1) (2,0)` at the 11
    // points of `.dc V1 -2 3 0.5` (evidence P3-070, `es_e.cir`, `src.cir`).
    #[test]
    fn a_table_is_its_segments_as_ternaries() {
        let table = [(-1.0, 1.0), (1.0, -1.0), (2.0, 0.0)];
        assert_eq!(
            ternary_chain("V(1)", &table, |v| format!("{v}")),
            "V(1) < (-1) ? 1 : V(1) < 1 ? 1 + (V(1) - (-1)) * (((-1) - 1) / (1 - (-1))) \
             : V(1) < 2 ? (-1) + (V(1) - 1) * ((0 - (-1)) / (2 - 1)) : 0"
        );
    }

    // Oracle: Xyce 7.10 on `(0,0) (1,0) (1,5) (2,5)` gives 5 at V(1) = 1,
    // the later point; this text on ESPice 0.3.0 gives 0, 0, 5, 5, 5 at
    // 0.5 … 1.5 as Xyce does (`xy_step.cir`, `es_step.cir`).
    #[test]
    fn a_vertical_step_has_no_segment() {
        let table = [(0.0, 0.0), (1.0, 0.0), (1.0, 5.0), (2.0, 5.0)];
        assert_eq!(
            ternary_chain("V(1)", &table, |v| format!("{v}")),
            "V(1) < 0 ? 0 : V(1) < 1 ? 0 + (V(1) - 0) * ((0 - 0) / (1 - 0)) \
             : V(1) < 2 ? 5 + (V(1) - 1) * ((5 - 5) / (2 - 1)) : 5"
        );
    }

    // Oracle: ESPice 0.3.0 compiles the chain of six points over `v(1)`,
    // `V(1,0)`, `V(1, 2)`, `i(v1)` and stops on seven ("TooManyOps"); over
    // `2*v(1)` it stops on five (`chain2.py`, `chain.py`).
    #[test]
    fn the_ternary_spelling_holds_five_segments_of_one_probe() {
        let rising = |n: u32| -> Vec<(f64, f64)> { (0..n).map(|k| (f64::from(k), 1.0)).collect() };
        for probe in ["v(1)", "V(1,0)", "V(1, 2)", "i(v1)"] {
            assert!(fits(probe, &rising(6)), "{probe}");
            assert!(!fits(probe, &rising(7)), "{probe}");
        }
        assert!(!fits("2*v(1)", &rising(2)));
        assert!(!fits("V(2)*(V(1)+30)/60", &rising(2)));
        // A vertical step adds a point and no segment.
        let mut step = rising(6);
        step.push((5.0, 2.0));
        assert!(fits("v(1)", &step));
    }
}
