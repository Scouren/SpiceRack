//! Passes `pwl_to_ternary` and `pwl_to_ternary_dyn`: a design in, the same design out with every
//! `pwl(x, x₁, y₁, …, xₙ, yₙ)` of one feature context written as its definition in ternaries:
//! `x <= x₁ ? y₁ : (x >= xₙ ? yₙ : (x < x₂ ? s₁ : (x < x₃ ? s₂ : … sₙ₋₁)))` with each segment
//! `sₖ = yₖ + (yₖ₊₁ − yₖ) · (x − xₖ) / (xₖ₊₁ − xₖ)`, the evaluator's operations in its order.
//! Only when every point is a literal (or a negated one) and the `xᵢ` strictly increase: the
//! points then read no draw and raise no error, which a ternary would skip, and `pwl`'s own
//! domain holds. Otherwise, or when `x` may hold a draw (it is read up to four times), the call
//! keeps its form with note `U0106`.
//!
//! `pwl_to_ternary` removes `Func(Pwl)` and requires `Expr(Ternary)` and `Le`, `Ge`, `Lt`,
//! `Add`, `Sub`, `Mul`, `Div`; `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1; `crates/ir/src/eval.rs` (`pwl`).
//!
//! Contents: `pub fn pwl_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn pwl_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::PWL_POINTS;
use super::nodes::{Draws, form, func_form, literal, note_left, plain};
use crate::ir;

/// Rewrites `Pwl` in every root of the plain form.
pub fn pwl_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn pwl_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let d = b.design();
        let ir::Expr::Call(ir::Func::Pwl, args) = d.expr(e) else {
            return None;
        };
        let all = d.exprs(args).to_vec();
        let (x, pts) = all.split_first().expect("check: pwl has arguments");
        if pts.is_empty() || pts.len() % 2 != 0 || draws.holds(d, *x) {
            return None;
        }
        let values: Option<Vec<f64>> = pts.iter().map(|&p| literal(d, p)).collect();
        let values = values?;
        if values
            .chunks_exact(2)
            .zip(values.chunks_exact(2).skip(1))
            .any(|(a, b)| b[0] <= a[0])
        {
            return None;
        }
        // an Int point is spelled as the Real literal of its value: `pwl` is Real, and a
        // point passed through or subtracted would keep the Int tag (and its 2⁵³ bound)
        let pts: Vec<ir::ExprId> = pts
            .iter()
            .zip(&values)
            .map(|(&p, &v)| match int_form(b.design(), p) {
                true => plain(b, v as i64, 0),
                false => p,
            })
            .collect();
        let point = |k: usize| (pts[2 * k], pts[2 * k + 1]);
        let n = pts.len() / 2;
        let (x1, y1) = point(0);
        let (xn, yn) = point(n - 1);
        // inner first: the chain of segments (the last needs no test), then the two ends
        let chain = match n {
            1 => y1,
            _ => {
                let mut c = segment(b, *x, point(n - 2), point(n - 1));
                for k in (0..n - 2).rev() {
                    let s = segment(b, *x, point(k), point(k + 1));
                    let before = b.binary(ir::BinOp::Lt, *x, point(k + 1).0);
                    c = b.ternary(before, s, c);
                }
                c
            }
        };
        let high = b.binary(ir::BinOp::Ge, *x, xn);
        let inner = b.ternary(high, yn, chain);
        let low = b.binary(ir::BinOp::Le, *x, x1);
        Some(b.ternary(low, y1, inner))
    });
    note_left(design, diags, func_form(ir::Func::Pwl, dynamic), PWL_POINTS);
}

/// `y₀ + (y₁ − y₀) · (x − x₀) / (x₁ − x₀)`, as the evaluator computes it.
fn segment(
    b: &mut ir::Builder<'_>,
    x: ir::ExprId,
    (x0, y0): (ir::ExprId, ir::ExprId),
    (x1, y1): (ir::ExprId, ir::ExprId),
) -> ir::ExprId {
    let rise = b.binary(ir::BinOp::Sub, y1, y0);
    let along = b.binary(ir::BinOp::Sub, x, x0);
    let run = b.binary(ir::BinOp::Sub, x1, x0);
    let up = b.binary(ir::BinOp::Mul, rise, along);
    let slope = b.binary(ir::BinOp::Div, up, run);
    b.binary(ir::BinOp::Add, y0, slope)
}

/// Whether a literal (under negations) has the `Int` form.
fn int_form(d: &ir::Design, e: ir::ExprId) -> bool {
    let mut at = e;
    for _ in 0..=d.expr_count() {
        match d.expr(at) {
            ir::Expr::Num(n) => return d.num(n).form == ir::NumForm::Int,
            ir::Expr::Unary(ir::UnOp::Neg, a) => at = a,
            _ => return false,
        }
    }
    unreachable!("a chain of negations is shorter than the table")
}
