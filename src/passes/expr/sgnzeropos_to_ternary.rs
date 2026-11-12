//! Passes `sgnzeropos_to_ternary` and `sgnzeropos_to_ternary_dyn`: a design in, the same design
//! out with every `SgnZeroPos(x)` (VACASK's `sgn`: −1 below zero, +1 otherwise, ±0 included, of
//! its operand's type) of one feature context written `x < 0 ? -1 : 1` with Real literals. `x`
//! is evaluated once; `-0 < 0` is false, so ±0 give 1 as before. The literals are Real, so the
//! value keeps its tag when `x` is Real on every evaluation (the tag table), or when no typed
//! division is left to read a tag (A12 K1); otherwise the call keeps its form with note `U0103`.
//!
//! `sgnzeropos_to_ternary` removes `Func(SgnZeroPos)` and requires `Expr(Ternary)`,
//! `Binary(Lt)`; `_dyn` the dynamic form of `Lt`.
//!
//! Cites: `rewrite/AMEND-A12.md` K1 addendum (`SgnZeroPos`, runs e1, f4).
//!
//! Contents: `pub fn sgnzeropos_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn sgnzeropos_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::TAG_READ;
use super::context::rewrite_in;
use super::nodes::{func_form, note_left, plain, reads_tags};
use super::tags::{REAL, Tags};
use crate::ir;

/// Rewrites `SgnZeroPos` as a ternary in every root of the plain form.
pub fn sgnzeropos_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn sgnzeropos_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let untyped = !reads_tags(design);
    let mut tags = Tags::new(design);
    rewrite_in(design, dynamic, &mut |b, e| {
        tags.of(b.design(), e);
        let ir::Expr::Call(ir::Func::SgnZeroPos, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        if !untyped && tags.of(b.design(), x) != REAL {
            return None;
        }
        let zero = plain(b, 0, 0);
        let below = b.binary(ir::BinOp::Lt, x, zero);
        let (minus, plus) = (plain(b, -1, 0), plain(b, 1, 0));
        let t = b.ternary(below, minus, plus);
        for n in [zero, below, minus, plus, t] {
            tags.of(b.design(), n);
        }
        Some(t)
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::SgnZeroPos, dynamic),
        TAG_READ,
    );
}
