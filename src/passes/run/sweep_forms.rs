//! Rule `sweep_forms`: sweep forms a target lacks rewritten into the forms ngspice has, where
//! the rewrite is exact. Three rows:
//! - `sweep_dc_lin_points_to_step` (`SweepDcLinPoints`) and `sweep_dc_list_to_step`
//!   (`SweepDcList`) require `SweepDcLinStep`: a DC axis of `n` values becomes `start`, `stop`
//!   and the step `(stop − start)/(n − 1)` (a list: its first and last value and the common
//!   difference of an arithmetic list; one value: the step 1);
//! - `sweep_freq_list_to_lin_points` (`SweepFreqList`) requires `SweepFreqLinPoints`: an AC
//!   analysis at one listed frequency `f`, a constant (`1/(2*M_PI)` in VACASK's demo decks),
//!   becomes the linear sweep of one point from `f` to `f`.
//!
//! Exact, by the arithmetic of the literals as written (R1: a literal's double is each
//! simulator's own arithmetic on its digits, so the rewrite is exact when the new literals
//! state the same decimal values): the step is computed in decimal and must terminate within
//! a literal's 18 digits (0.2 over 6 intervals does not: the axis is left); and the number of
//! points ngspice-45 runs must be `n`. Its DC loop (`dctrcurv.c`) adds the step to the value
//! and stops once `sign(step)·(value − stop) > DBL_EPSILON·1e3` (2.2e-13, absolute), which runs
//! an extra point when the step is that small (VACASK's `test/test_sweep.sim`, 1p to 10p in 50
//! intervals: run, `.dc v1 1e-12 10e-12 1.8e-13` writes 52 points; `.dc v1 0 1.8 0.018` 101); the rule runs that loop in `f64` and keeps the axis unless it counts
//! `n`. Its linear AC sweep of one point (`acan.c`: `ACnumberSteps − 1 > 1` else a delta of 0)
//! computes the start frequency alone. Two or more listed frequencies are left: ngspice's linear
//! AC sweep of two points runs one (the same test), and its end test scales with `reltol`.
//! A value that is no literal (a parameter, an expression) is left.
//!
//! Left items carry a note: `SWEEP_KEPT` at a DC analysis, `FREQ_LIST_KEPT` at an analysis
//! with a frequency list. The decade, octave and log DC forms have no row: ngspice's DC axis
//! is linear, and a log grid of three or more values is none.
//!
//! Cites: `crates/ir/src/sweep.rs`; ngspice-45 `spicelib/analysis/dctrcurv.c` (lines 219–260,
//! 505), `acan.c` (lines 110–121, 283); VACASK `lib/answeep.cpp` (`ScalarSweep::compute`).
//!
//! Contents: `pub fn sweep_dc_lin_points_to_step(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn sweep_dc_list_to_step(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn sweep_freq_list_to_lin_points(&mut ir::Design, &mut ir::Diags)`,
//! `pub const SWEEP_KEPT: ir::Code`, `pub const FREQ_LIST_KEPT: ir::Code`,
//! `pub(super) fn values(&ir::Design, &ir::SweepSpec) -> Option<Vec<ir::Literal>>`

use super::names_dropped::of_kind;
use crate::ir;

/// The note at a DC analysis with an axis of the row's form no linear step states exactly.
pub const SWEEP_KEPT: ir::Code = ir::Code::new(b'U', 413);
/// The note at an analysis whose frequency list is not one value of an AC analysis.
pub const FREQ_LIST_KEPT: ir::Code = ir::Code::new(b'U', 414);

/// ngspice-45's DC end test: `DBL_EPSILON * 1e+03` (`dctrcurv.c`).
const NG_DC_FUZZ: f64 = f64::EPSILON * 1e3;
/// The most points a rewritten axis may have (the count model's bound).
const POINTS_MAX: u64 = 1 << 24;
/// The most values [`values`] lists (a loop is written value by value).
const VALUES_MAX: u64 = 100_000;

/// A decimal `m · 10^e`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dec {
    m: i128,
    e: i32,
}

/// Bindings that bind nothing: an expression with a value under them is a constant.
struct Nothing;

impl ir::Bindings for Nothing {
    fn param(&self, _: ir::Ref) -> Option<f64> {
        None
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        None
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        None
    }
    fn time(&self) -> Option<f64> {
        None
    }
    fn temp(&self) -> Option<f64> {
        None
    }
    fn freq(&self) -> Option<f64> {
        None
    }
    fn tstep(&self) -> Option<f64> {
        None
    }
    fn tstop(&self) -> Option<f64> {
        None
    }
    fn tnom(&self) -> Option<f64> {
        None
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, _: &[f64]) -> Option<f64> {
        None
    }
}

/// Rewrites every DC axis linear by count as linear by step, where exact.
pub fn sweep_dc_lin_points_to_step(design: &mut ir::Design, diags: &mut ir::Diags) {
    dc_axes(design, diags, ir::RunExtra::SweepDcLinPoints);
}

/// Rewrites every DC axis over listed values as linear by step, where exact.
pub fn sweep_dc_list_to_step(design: &mut ir::Design, diags: &mut ir::Diags) {
    dc_axes(design, diags, ir::RunExtra::SweepDcList);
}

/// Rewrites every AC analysis at one listed frequency as a linear sweep of one point.
pub fn sweep_freq_list_to_lin_points(design: &mut ir::Design, diags: &mut ir::Diags) {
    for item in carriers(design, ir::RunExtra::SweepFreqList) {
        let a = design.row::<ir::Analysis>(item.index);
        let one = match a.kind {
            ir::AnalysisKind::Ac => {
                let ac = design.detail::<ir::dc_ac::Ac>(a.detail);
                match design.exprs(ac.sweep.values) {
                    [f] if ir::eval(design, *f, &Nothing).is_ok() => Some((ac, *f)),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some((mut ac, f)) = one else {
            diags.push_at(FREQ_LIST_KEPT, design, item, 0);
            continue;
        };
        let mut editor = design.editor();
        let points = editor.builder().literal(plain(1, 0));
        // a constant (no read, no draw) is one value wherever it stands: start and stop share it
        ac.sweep = ir::SweepSpec::lin_points(f, f, points);
        editor.set_detail(a.detail, ac);
    }
}

/// The values of a linear or listed sweep as decimal literals, in run order: a list's own
/// literals, a linear sweep's `start + i·step` (by count, the step `(stop − start)/(n − 1)` when
/// it terminates; by step, every grid value at or before `stop`), at most `VALUES_MAX`; `None`
/// for a log sweep, a value that is no literal, a value no IR literal holds (`Literal::is_valid`;
/// a zero is `Literal::ZERO`), or a grid past the bound. A sweep from `start` to `start` is the
/// one point `start` whatever its step, as VACASK runs it (run: `from=0 to=0 step=8.02e-15`, one
/// point; ngspice-45's `.dc v1 0 0 8.02e-15` runs 28 by its absolute end test, which
/// `ngspice_count` refuses for the DC rows).
pub(super) fn values(design: &ir::Design, s: &ir::SweepSpec) -> Option<Vec<ir::Literal>> {
    if s.end != ir::SweepEnd::Grid || s.bounds != ir::SweepBounds::StartStop {
        return None;
    }
    if s.kind == ir::SweepKind::List {
        let v: Option<Vec<ir::Literal>> = design
            .exprs(s.values)
            .iter()
            .map(|&e| literal(design, e).and_then(dec).and_then(lit))
            .collect();
        return v.filter(|v| !v.is_empty() && v.len() <= VALUES_MAX as usize);
    }
    if s.kind != ir::SweepKind::Lin {
        return None;
    }
    let start = dec(literal(design, s.start)?)?;
    let stop = dec(literal(design, s.stop)?)?;
    let (step, n) = match s.step.is_none() {
        true => {
            let n = literal(design, s.points)?.value();
            if !(1.0..=VALUES_MAX as f64).contains(&n) || n.fract() != 0.0 {
                return None;
            }
            let n = n as u64;
            match n {
                1 => (Dec { m: 0, e: 0 }, 1),
                _ => (div(sub(stop, start)?, n - 1)?, n),
            }
        }
        false => {
            let step = dec(literal(design, s.step)?)?;
            let (span, by, _) = align(sub(stop, start)?, step)?;
            if by == 0 || (span != 0 && span.signum() != by.signum()) {
                return None;
            }
            let n = u64::try_from(span / by).ok()? + 1;
            (step, n)
        }
    };
    if n > VALUES_MAX {
        return None;
    }
    (0..n).map(|i| lit(add(start, mul(step, i)?)?)).collect()
}

// ---- private ----

/// The DC rows' body: each carrier's axes of `form` rewritten, the analysis noted when one is
/// left.
fn dc_axes(design: &mut ir::Design, diags: &mut ir::Diags, form: ir::RunExtra) {
    assert!(matches!(
        form,
        ir::RunExtra::SweepDcLinPoints | ir::RunExtra::SweepDcList
    ));
    for item in carriers(design, form) {
        let a = design.row::<ir::Analysis>(item.index);
        assert!(
            a.kind == ir::AnalysisKind::Dc,
            "only a DC axis has a DC sweep form"
        );
        let mut dc = design.detail::<ir::dc_ac::Dc>(a.detail);
        let mut axes: Vec<ir::dc_ac::DcAxis> = design.dc_axes(dc.axes).to_vec();
        let mut left = false;
        for axis in axes.iter_mut() {
            let s = axis.sweep;
            let mine = match form {
                ir::RunExtra::SweepDcLinPoints => {
                    s.kind == ir::SweepKind::Lin && !s.points.is_none()
                }
                _ => s.kind == ir::SweepKind::List,
            };
            if !mine || s.end != ir::SweepEnd::Grid || s.bounds != ir::SweepBounds::StartStop {
                left |= mine;
                continue;
            }
            match grid(design, &s) {
                Some((start, stop, step)) => {
                    let mut editor = design.editor();
                    let mut b = editor.builder();
                    let (start, stop, step) = (b.literal(start), b.literal(stop), b.literal(step));
                    axis.sweep = ir::SweepSpec::lin_step(start, stop, step);
                }
                None => left = true,
            }
        }
        let mut editor = design.editor();
        dc.axes = editor.builder().dc_axes(&axes);
        editor.set_detail(a.detail, dc);
        if left {
            diags.push_at(SWEEP_KEPT, design, item, 0);
        }
    }
}

/// The start, stop and step literals of a linear-by-step sweep with exactly `s`'s values, as
/// ngspice-45 runs it, or `None`.
fn grid(design: &ir::Design, s: &ir::SweepSpec) -> Option<(ir::Literal, ir::Literal, ir::Literal)> {
    let (values, n): (Vec<Dec>, u64) = match s.kind {
        ir::SweepKind::List => {
            let v: Option<Vec<Dec>> = design
                .exprs(s.values)
                .iter()
                .map(|&e| literal(design, e).and_then(dec))
                .collect();
            let v = v?;
            let n = v.len() as u64;
            (v, n)
        }
        _ => {
            let n = literal(design, s.points)?.value();
            if !(1.0..=POINTS_MAX as f64).contains(&n) || n.fract() != 0.0 {
                return None;
            }
            let start = dec(literal(design, s.start)?)?;
            let stop = dec(literal(design, s.stop)?)?;
            (vec![start, stop], n as u64)
        }
    };
    let first = *values.first()?;
    let last = if n == 1 { first } else { *values.last()? };
    let step = match (s.kind, n) {
        (_, 1) => Dec { m: 1, e: 0 },
        (ir::SweepKind::List, _) => {
            let step = sub(values[1], values[0])?;
            let arithmetic = values
                .windows(2)
                .all(|w| sub(w[1], w[0]).is_some_and(|d| same(d, step)));
            if !arithmetic {
                return None;
            }
            step
        }
        _ => div(sub(last, first)?, n - 1)?,
    };
    if step.m == 0 || (n > 1 && !same(add(first, mul(step, n - 1)?)?, last)) {
        return None;
    }
    let (first, last_l, step_l) = (lit(first)?, lit(last)?, lit(step)?);
    (ngspice_count(first.value(), last_l.value(), step_l.value(), n) == n)
        .then_some((first, last_l, step_l))
}

/// The points ngspice-45's DC loop runs from `start` to `stop` by `step`, at most `n + 1`.
fn ngspice_count(start: f64, stop: f64, step: f64, n: u64) -> u64 {
    let sign = step.signum();
    let mut value = start;
    let mut count = 0;
    while count <= n && sign * (value - stop) <= NG_DC_FUZZ {
        count += 1;
        value += step;
    }
    count
}

/// The reachable analyses carrying `form` (`ir::check_legal` against every other feature).
fn carriers(design: &ir::Design, form: ir::RunExtra) -> Vec<ir::ItemRef> {
    let f = ir::Feature::Run(form);
    let mut rows = ir::Diags::new();
    let others = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(f));
    ir::check_legal(design, &others, &mut rows);
    let analyses = of_kind(design, ir::ItemKind::Analysis);
    let mut out: Vec<ir::ItemRef> = rows
        .rows()
        .iter()
        .map(|d| d.item)
        .filter(|i| analyses.contains(i))
        .collect();
    out.dedup();
    out
}

/// The literal `e` is, a literal under a minus included, or `None`.
fn literal(design: &ir::Design, e: ir::ExprId) -> Option<ir::Literal> {
    if e.is_none() {
        return None;
    }
    match design.expr(e) {
        ir::Expr::Num(n) => Some(design.num(n)),
        ir::Expr::Unary(ir::UnOp::Neg, x) => match design.expr(x) {
            ir::Expr::Num(n) => {
                let l = design.num(n);
                (l.form != ir::NumForm::Long).then(|| ir::Literal {
                    mantissa: -l.mantissa,
                    ..l
                })
            }
            _ => None,
        },
        _ => None,
    }
}

/// A literal's exact decimal value; `None` for a `Mil` or `Long` literal.
fn dec(l: ir::Literal) -> Option<Dec> {
    match l.form {
        ir::NumForm::Mil | ir::NumForm::Long => None,
        _ => Some(Dec {
            m: i128::from(l.mantissa),
            e: i32::from(l.exp10) - i32::from(l.frac),
        }),
    }
}

/// The real literal of `d`, or `None` when its digits exceed a literal's mantissa.
fn lit(d: Dec) -> Option<ir::Literal> {
    if d.m == 0 {
        return Some(ir::Literal::ZERO); // the one canonical zero (`Literal::is_valid`)
    }
    let (mut m, mut e) = (d.m, d.e);
    while m != 0 && m % 10 == 0 {
        m /= 10;
        e += 1;
    }
    if m.unsigned_abs() >= 1_000_000_000_000_000_000 {
        return None;
    }
    let m = i64::try_from(m).ok()?;
    let l = match e {
        -6..=0 => Some(plain(m, u8::try_from(-e).ok()?)),
        _ => Some(ir::Literal {
            mantissa: m,
            frac: 0,
            exp10: i16::try_from(e).ok()?,
            form: ir::NumForm::Exponent,
        }),
    };
    // a literal past what the IR's literal holds is no literal: the caller keeps the item
    l.filter(|l| l.is_valid())
}

/// A plain real literal `m · 10^-frac`.
const fn plain(m: i64, frac: u8) -> ir::Literal {
    ir::Literal {
        mantissa: m,
        frac,
        exp10: 0,
        form: ir::NumForm::Plain,
    }
}

/// `a` and `b` on one exponent, or `None` when they lie more than 30 decades apart.
fn align(a: Dec, b: Dec) -> Option<(i128, i128, i32)> {
    let e = a.e.min(b.e);
    let up = |d: Dec| {
        let k = u32::try_from(d.e - e).ok().filter(|&k| k <= 30)?;
        d.m.checked_mul(10i128.pow(k))
    };
    Some((up(a)?, up(b)?, e))
}

fn add(a: Dec, b: Dec) -> Option<Dec> {
    let (x, y, e) = align(a, b)?;
    Some(Dec {
        m: x.checked_add(y)?,
        e,
    })
}

fn sub(a: Dec, b: Dec) -> Option<Dec> {
    add(a, Dec { m: -b.m, e: b.e })
}

fn mul(a: Dec, k: u64) -> Option<Dec> {
    Some(Dec {
        m: a.m.checked_mul(i128::from(k))?,
        e: a.e,
    })
}

/// `a / k` when it terminates within 18 more digits.
fn div(a: Dec, k: u64) -> Option<Dec> {
    assert!(k > 0, "a division by a point count");
    let k = i128::from(k);
    let mut d = a;
    for _ in 0..=18 {
        if d.m % k == 0 {
            return Some(Dec { m: d.m / k, e: d.e });
        }
        d = Dec {
            m: d.m.checked_mul(10)?,
            e: d.e - 1,
        };
    }
    None
}

fn same(a: Dec, b: Dec) -> bool {
    align(a, b).is_some_and(|(x, y, _)| x == y)
}
