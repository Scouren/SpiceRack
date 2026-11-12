//! An IR expression → VACASK text: VACASK's precedence table (VD expr-operators: `? :`
//! loosest and right-associative, then `|| && | ^ & == != < <= > >= << >> + - * /`
//! left-associative, `**` right-associative, unary `- ! ~` tighter than `**`), the fewest
//! parentheses that read back as the same tree, one space around a binary operator. `Pow` is
//! `**`, `Ln` is `ln`, `Trunc` is `integer`, `RoundHalfAway` is `round`; a draw is its call;
//! `Temp` is `$temp`, `Tnom` is `$tnom`. A negative literal is `-` and its digits, a `Neg` of a
//! literal `-(…)`. Explicit stack (C11). Total over the features of `FEATURES`.
//!
//! Cites: VD expr-operators, expr-functions, expr-special; `rewrite/DIALECT-VACASK.md` §4.
//!
//! Contents: `write_expr` (crate-private)

use crate::ir;

/// Appends expression `e` of `design` as VACASK text.
pub(super) fn write_expr(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    let _ = (design, e, out);
    todo!("S4.0: expr_write::write_expr")
}
