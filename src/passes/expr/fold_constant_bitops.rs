//! Passes `fold_constant_<op>` and `fold_constant_<op>_dyn` for each bit operator (`bitand`,
//! `bitor`, `bitxor`, `shl`, `shr`, `bitnot`): a design in, the same design out with every such
//! operator of one feature context whose operands are integer literals (a negated one too)
//! replaced by the literal of its value: the one place a constant integer subtree becomes a
//! literal. The value is the operator's definition on 64-bit integers, written out here (`&`,
//! `|`, `^`, `!`, `<<` and arithmetic `>>` with `0 <= b < 64`), never the evaluator's code. The
//! literal is `Plain` (Real) and the operator's value Int, so a fold waits while a typed
//! division stands (only `DivTyped` reads a tag, A12 K1). An operand that is not an integer
//! literal (it may be another bit operator: its own row folds it first, and `legalize`'s next
//! round folds this one), a shift out of range, a value past 2⁵³ (R12), or a standing typed
//! division keeps the operator with note `U0107`.
//!
//! Each row removes `Binary(op)` (`Unary(BitNot)`) or its dynamic form and requires
//! `Expr(Num)`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2 (`caret_reread`); `crates/ir/src/expr.rs` (`BinOp`).
//!
//! Contents: `pub fn fold_constant_bitand`, `pub fn fold_constant_bitor`,
//! `pub fn fold_constant_bitxor`, `pub fn fold_constant_shl`, `pub fn fold_constant_shr`,
//! `pub fn fold_constant_bitnot`, `pub fn fold_constant_bitand_dyn`,
//! `pub fn fold_constant_bitor_dyn`, `pub fn fold_constant_bitxor_dyn`,
//! `pub fn fold_constant_shl_dyn`, `pub fn fold_constant_shr_dyn`,
//! `pub fn fold_constant_bitnot_dyn` (each `fn(&mut ir::Design, &mut ir::Diags)`)

use super::BITOP_NOT_CONSTANT;
use super::nodes::{binary_form, form, literal, note_left, plain, reads_tags};
use crate::ir;

/// The operator a row folds.
#[derive(Clone, Copy, PartialEq)]
enum Op {
    Binary(ir::BinOp),
    Not,
}

/// The largest Int magnitude (R12).
const INT_MAX: u64 = 1 << 53;

/// Folds `a & b` of integer literals in every root of the plain form.
pub fn fold_constant_bitand(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::BitAnd), false);
}

/// The same in every root of the dynamic form.
pub fn fold_constant_bitand_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::BitAnd), true);
}

/// Folds `a | b`.
pub fn fold_constant_bitor(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::BitOr), false);
}

/// The same in every root of the dynamic form.
pub fn fold_constant_bitor_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::BitOr), true);
}

/// Folds `a ^ b`.
pub fn fold_constant_bitxor(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::BitXor), false);
}

/// The same in every root of the dynamic form.
pub fn fold_constant_bitxor_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::BitXor), true);
}

/// Folds `a << b`.
pub fn fold_constant_shl(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::Shl), false);
}

/// The same in every root of the dynamic form.
pub fn fold_constant_shl_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::Shl), true);
}

/// Folds `a >> b`.
pub fn fold_constant_shr(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::Shr), false);
}

/// The same in every root of the dynamic form.
pub fn fold_constant_shr_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Binary(ir::BinOp::Shr), true);
}

/// Folds `~a`.
pub fn fold_constant_bitnot(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Not, false);
}

/// The same in every root of the dynamic form.
pub fn fold_constant_bitnot_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, Op::Not, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, op: Op, dynamic: bool) {
    if !reads_tags(design) {
        ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
            let d = b.design();
            let value = match (d.expr(e), op) {
                (ir::Expr::Binary(o, x, y), Op::Binary(want)) if o == want => {
                    fold(o, int(d, x)?, int(d, y)?)?
                }
                (ir::Expr::Unary(ir::UnOp::BitNot, x), Op::Not) => !int(d, x)?,
                _ => return None,
            };
            (value.unsigned_abs() <= INT_MAX).then(|| plain(b, value, 0))
        });
    }
    let feature = match op {
        Op::Binary(o) => binary_form(o, dynamic),
        Op::Not if dynamic => ir::Feature::UnaryDyn(ir::UnOp::BitNot),
        Op::Not => ir::Feature::Unary(ir::UnOp::BitNot),
    };
    note_left(design, diags, feature, BITOP_NOT_CONSTANT);
}

/// The operator's definition on two integers; `None` for a shift outside `0..64`.
fn fold(op: ir::BinOp, a: i64, b: i64) -> Option<i64> {
    match op {
        ir::BinOp::BitAnd => Some(a & b),
        ir::BinOp::BitOr => Some(a | b),
        ir::BinOp::BitXor => Some(a ^ b),
        ir::BinOp::Shl => (0..64).contains(&b).then(|| a << b),
        ir::BinOp::Shr => (0..64).contains(&b).then(|| a >> b),
        _ => unreachable!("only the bit operators fold"),
    }
}

/// The integer a literal (or a negated one) states, when it is whole and within `i64` as the
/// operators' domain says (`|a| < 9.2e18`).
fn int(d: &ir::Design, e: ir::ExprId) -> Option<i64> {
    let v = literal(d, e)?;
    (v.fract() == 0.0 && v.abs() < 9.2e18).then_some(v as i64)
}
