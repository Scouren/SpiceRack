//! Rule `sweep_end_list` (F33, R99, R99a): a logarithmic frequency sweep whose values follow a
//! simulator's own end law becomes the list of those values, each the expression of the law (no
//! value is computed here; only the count is). Two rows over one body:
//! - `sweep_end_stretch_to_list` removes `Run(SweepEndStretch)` (VACASK's `dec`/`oct`): the
//!   interval count `m = ceil(|n·ln(stop/start)/ln(base)|)` and the values
//!   `start·(stop/start)^(k/m)`, `k = 0..=m` (VACASK `lib/answeep.cpp`,
//!   `ScalarSweep::setupLogSweep` and `compute`; runs `tests/passes_run/decks/f33_vacask_*`);
//! - `sweep_end_ng_decade_to_list` removes `Run(SweepEndNgDecade)` (ngspice's `dec`): under one
//!   decade (`stop/10 < start`) the exact grid `start·10^(k/n)`, at one decade or more the
//!   stretched grid `start·(stop/start)^(k/m)` with `m = floor(|log10(stop/start)|·n)`; the
//!   count is ngspice-45's own loop (`acan.c` lines 84–99, 239, 283: multiply by the step while
//!   `f ≤ stop + step·stop·reltol`), run here in `f64` with its default `reltol` 1e-3 (runs
//!   `tests/passes_run/decks/f33_ac_*`).
//!
//! Both require `Run(SweepFreqList)` and the operators the values use. Exact: the list states
//! the values the law states, in the same count, so every simulator's double of each is within
//! its own rounding of the source simulator's (the pair check, `freq_grids.rs`). Left with
//! `SWEEP_END_KEPT`: a sweep whose start, stop or count is no literal (R99: a literal count is
//! an integer by the reader, R99b), whose start is not below its stop or not positive, of a
//! loop, a DC axis or an analysis other than AC, noise, S-parameter, transfer function, AC
//! sensitivity or distortion,
//! in a design that sets `reltol` (ngspice's end test reads it), or past `VALUES_MAX` values.
//!
//! Cites: `rewrite/IR-RUN.md` R99; ngspice-45 `spicelib/analysis/acan.c`; VACASK `lib/answeep.cpp`.
//!
//! Contents: `pub fn sweep_end_stretch_to_list(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn sweep_end_ng_decade_to_list(&mut ir::Design, &mut ir::Diags)`,
//! `pub const SWEEP_END_KEPT: ir::Code`, `pub const REQUIRES: ir::FeatureSet`

use super::names_dropped::of_kind;
use crate::ir;

/// The note at an analysis whose end-law sweep is not written as a list.
pub const SWEEP_END_KEPT: ir::Code = ir::Code::new(b'U', 427);

/// What the rows add: the frequency list and the values' operators.
pub const REQUIRES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Run(ir::RunExtra::SweepFreqList))
    .with(ir::Feature::Expr(ir::ExprTag::Num))
    .with(ir::Feature::Expr(ir::ExprTag::Binary))
    .with(ir::Feature::Expr(ir::ExprTag::Call))
    .with(ir::Feature::Binary(ir::BinOp::Mul))
    .with(ir::Feature::Binary(ir::BinOp::Div))
    .with(ir::Feature::Func(ir::Func::Pow));

/// A law's values: `start·base^(k/per)` for `k` in `0..count` (`base` the stop over the start
/// for a stretched grid, ten for ngspice's exact decade grid).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Law {
    /// Whether the base is ten (else `stop/start`).
    ten: bool,
    /// The exponent's denominator.
    per: u64,
    /// The number of values.
    count: usize,
}

/// The most values a list is written with.
const VALUES_MAX: usize = 100_000;
/// ngspice-45's default `reltol` (`CKTreltol`), which its AC end test scales.
const NG_RELTOL: f64 = 1e-3;

/// Rewrites every VACASK-stretched `dec`/`oct` frequency sweep as its list of values.
pub fn sweep_end_stretch_to_list(design: &mut ir::Design, diags: &mut ir::Diags) {
    to_list(design, diags, ir::SweepEnd::Stretch);
}

/// Rewrites every ngspice decade frequency sweep as its list of values.
pub fn sweep_end_ng_decade_to_list(design: &mut ir::Design, diags: &mut ir::Diags) {
    to_list(design, diags, ir::SweepEnd::NgDecade);
}

// ---- private ----

/// The rows' body: each analysis whose sweep ends by `end` gets its list; every other item
/// still carrying the feature (a loop's or a DC axis's sweep, another analysis) gets the note.
fn to_list(design: &mut ir::Design, diags: &mut ir::Diags, end: ir::SweepEnd) {
    let feature = match end {
        ir::SweepEnd::Stretch => ir::RunExtra::SweepEndStretch,
        ir::SweepEnd::NgDecade => ir::RunExtra::SweepEndNgDecade,
        _ => unreachable!("the rows' two end laws"),
    };
    let reltol_set = of_kind(design, ir::ItemKind::Option)
        .iter()
        .any(|o| design.row::<ir::OptionRow>(o.index).key == ir::OptionKey::RelTol);
    for item in carrying(design, ir::Feature::Run(feature)) {
        let law = match item.kind {
            ir::ItemKind::Analysis => {
                let a = design.row::<ir::Analysis>(item.index);
                sweep(design, a).filter(|s| s.end == end).and_then(|s| {
                    let law = match end {
                        ir::SweepEnd::Stretch => stretch(design, &s),
                        _ if reltol_set => None,
                        _ => ng_decade(design, &s),
                    };
                    law.filter(|l| l.count >= 1 && l.count <= VALUES_MAX)
                        .map(|l| (a, s, l))
                })
            }
            _ => None,
        };
        let Some((a, s, law)) = law else {
            diags.push_at(SWEEP_END_KEPT, design, item, 0);
            continue;
        };
        let mut editor = design.editor();
        let mut b = editor.builder();
        let values: Vec<ir::ExprId> = (0..law.count as u64)
            .map(|k| value(&mut b, &s, law, k))
            .collect();
        let list = ir::SweepSpec::list(b.exprs(&values));
        set_sweep(&mut editor, a, list);
    }
}

/// The reachable items that carry `feature`: `check_legal` against every feature but it.
fn carrying(design: &ir::Design, feature: ir::Feature) -> Vec<ir::ItemRef> {
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    let mut out: Vec<ir::ItemRef> = Vec::new();
    for d in rows.rows() {
        if d.code == ir::diag::UNSUPPORTED && d.arg == index && !out.contains(&d.item) {
            out.push(d.item);
        }
    }
    out
}

/// `start·base^(k/per)`: the start itself at `k = 0`.
fn value(b: &mut ir::Builder<'_>, s: &ir::SweepSpec, law: Law, k: u64) -> ir::ExprId {
    if k == 0 {
        return s.start;
    }
    let base = match law.ten {
        true => b.literal(whole(10)),
        false => b.binary(ir::BinOp::Div, s.stop, s.start),
    };
    // real literals: the writer spells each so its simulator divides in reals
    let (num, den) = (b.literal(whole(k)), b.literal(whole(law.per)));
    let exponent = b.binary(ir::BinOp::Div, num, den);
    let power = b.call(ir::Func::Pow, &[base, exponent]);
    b.binary(ir::BinOp::Mul, s.start, power)
}

/// VACASK's law: `ceil(|n·ln(stop/start)/ln(base)|) + 1` values on the stretched grid.
fn stretch(design: &ir::Design, s: &ir::SweepSpec) -> Option<Law> {
    let (start, stop, n) = bounds(design, s)?;
    let base: f64 = match s.kind {
        ir::SweepKind::Dec => 10.0,
        ir::SweepKind::Oct => 2.0,
        _ => return None,
    };
    let m = (n * (stop / start).ln() / base.ln()).abs().ceil();
    let m = u64::try_from(m as i64).ok().filter(|&m| m >= 1)?;
    let count = usize::try_from(m + 1).ok()?;
    Some(Law {
        ten: false,
        per: m,
        count,
    })
}

/// ngspice-45's decade law: the step and the count its AC loop runs (`acan.c`).
fn ng_decade(design: &ir::Design, s: &ir::SweepSpec) -> Option<Law> {
    if s.kind != ir::SweepKind::Dec {
        return None;
    }
    let (start, stop, n) = bounds(design, s)?;
    let (ten, per, delta) = if stop / 10.0 < start {
        (true, n as u64, (10f64.ln() / n).exp())
    } else {
        let m = ((stop / start).log10().abs() * n).floor();
        (false, m as u64, ((stop / start).ln() / m).exp())
    };
    let tol = delta * stop * NG_RELTOL;
    let mut freq = start;
    let mut count = 0;
    // bounded by VALUES_MAX: a longer list is refused by the caller
    while freq <= stop + tol && count <= VALUES_MAX {
        count += 1;
        freq *= delta;
    }
    Some(Law { ten, per, count })
}

/// A sweep's literal start, stop and count with `0 < start < stop` and a whole count of one or
/// more by start and stop, or `None`.
fn bounds(design: &ir::Design, s: &ir::SweepSpec) -> Option<(f64, f64, f64)> {
    if s.bounds != ir::SweepBounds::StartStop {
        return None;
    }
    let (start, stop, n) = (
        num(design, s.start)?,
        num(design, s.stop)?,
        num(design, s.points)?,
    );
    let whole = n >= 1.0 && n.fract() == 0.0 && n <= VALUES_MAX as f64;
    (0.0 < start && start < stop && whole).then_some((start, stop, n))
}

/// The value of a literal number, or `None`.
fn num(design: &ir::Design, e: ir::ExprId) -> Option<f64> {
    if e.is_none() {
        return None;
    }
    match design.expr(e) {
        ir::Expr::Num(n) => Some(design.num(n).value()),
        _ => None,
    }
}

/// A whole real literal.
fn whole(k: u64) -> ir::Literal {
    let mantissa = i64::try_from(k).expect("a count below VALUES_MAX");
    ir::Literal {
        mantissa,
        frac: 0,
        exp10: 0,
        form: ir::NumForm::Plain,
    }
}

/// The frequency sweep of an analysis that has one this rule rewrites.
fn sweep(design: &ir::Design, a: ir::Analysis) -> Option<ir::SweepSpec> {
    use ir::dc_ac::{Ac, Disto, Noise, SensAc, Sp, Xf};
    Some(match a.kind {
        ir::AnalysisKind::Ac => design.detail::<Ac>(a.detail).sweep,
        ir::AnalysisKind::Noise => design.detail::<Noise>(a.detail).sweep,
        ir::AnalysisKind::Sp => design.detail::<Sp>(a.detail).sweep,
        ir::AnalysisKind::Xf => design.detail::<Xf>(a.detail).sweep,
        ir::AnalysisKind::SensAc => design.detail::<SensAc>(a.detail).sweep,
        ir::AnalysisKind::Disto => design.detail::<Disto>(a.detail).sweep,
        _ => return None,
    })
}

/// Stores `s` as the analysis's frequency sweep (one of [`sweep`]'s kinds).
fn set_sweep(editor: &mut ir::Editor<'_>, a: ir::Analysis, s: ir::SweepSpec) {
    use ir::dc_ac::{Ac, Disto, Noise, SensAc, Sp, Xf};
    let d = editor.design();
    match a.kind {
        ir::AnalysisKind::Ac => editor.set_detail(a.detail, Ac { sweep: s }),
        ir::AnalysisKind::Noise => {
            let row = Noise {
                sweep: s,
                ..d.detail(a.detail)
            };
            editor.set_detail(a.detail, row);
        }
        ir::AnalysisKind::Sp => {
            let row = Sp {
                sweep: s,
                ..d.detail(a.detail)
            };
            editor.set_detail(a.detail, row);
        }
        ir::AnalysisKind::Xf => {
            let row = Xf {
                sweep: s,
                ..d.detail(a.detail)
            };
            editor.set_detail(a.detail, row);
        }
        ir::AnalysisKind::SensAc => {
            let row = SensAc {
                sweep: s,
                ..d.detail(a.detail)
            };
            editor.set_detail(a.detail, row);
        }
        ir::AnalysisKind::Disto => {
            let row = Disto {
                sweep: s,
                ..d.detail(a.detail)
            };
            editor.set_detail(a.detail, row);
        }
        _ => unreachable!("only the kinds `sweep` reads"),
    }
}
