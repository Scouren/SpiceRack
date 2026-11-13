//! Pass `int_literal_to_plain`: a design in, the same design out with every integer literal
//! (`NumForm::Int`, A12 K1) whose tag nothing reads written as the plain literal of the same
//! digits. The value is the same double; only the tag goes from Int to Real, and with it the
//! tag of every node the literal's tag flows into.
//!
//! A tag is read by: the typed division `DivTyped` (Int operands truncate), and the rules that
//! judge their operands by the tag table: the bit operators, `ToInt`, `ToReal`,
//! `SgnZeroPos` (A21, A25; their own rules run first, and `legalize`'s fixed point comes back
//! here once they are gone). A literal is left when its tag can reach an operand of one of
//! them, following the tag table's flow backwards: through `+ - *`, `/:`, `-x`, both branches
//! of a ternary, `min`, `max`, `sgn_zero_pos`; from a parameter read to every value of its
//! definition (its value, a head default, each instance's actual, a function body); from a
//! function's formal read to every call's argument; from a parameter a loop or an alter sets to
//! every expression of that item. Every other node (a comparison, a call, a field) reads a
//! value only. Each item left holding an integer literal gets `U0118`.
//!
//! Exact in value: the literal's double is its digits' either way. One widening: an Int result
//! beyond 2⁵³ is out of the domain (R12) where its Real twin is a value.
//!
//! Removes `Extra(IntLiteral)`; requires nothing (a plain literal is no feature). One row: a
//! literal has no form.
//!
//! Cites: `rewrite/AMEND-A12.md` K1, A21, A25; `crates/ir/src/eval.rs` (`apply`'s tags).
//!
//! Contents: `pub fn int_literal_to_plain(&mut ir::Design, &mut ir::Diags)`

use super::INT_TAG_READ;
use super::nodes::{kids, note_left};
use super::reach::tag_reach;
use crate::ir;

/// Writes every integer literal whose tag nothing reads as a plain literal; notes the items
/// left holding one.
pub fn int_literal_to_plain(design: &mut ir::Design, diags: &mut ir::Diags) {
    let read = tag_read(design);
    assert_eq!(read.len(), design.expr_count_all(), "one mark per row");
    ir::rewrite_exprs(design, &mut |b, e| {
        let ir::Expr::Num(n) = b.design().expr(e) else {
            return None;
        };
        let l = b.design().num(n);
        if l.form != ir::NumForm::Int || read[e.index()] {
            return None;
        }
        assert!(
            l.exp10 == 0 && l.frac == 0,
            "an Int literal is whole digits"
        );
        Some(b.literal(ir::Literal {
            form: ir::NumForm::Plain,
            ..l
        }))
    });
    note_left(
        design,
        diags,
        ir::Feature::Extra(ir::Extra::IntLiteral),
        INT_TAG_READ,
    );
}

// ---- private ----

/// Per expression row: whether its tag may reach an operand of a node that reads a tag.
/// The live readers only: a rewritten row stays in the table (rows are appended), and a typed
/// division another rule replaced reads nothing any more.
fn tag_read(design: &ir::Design) -> Vec<bool> {
    let mut seeds = Vec::new();
    for e in design.live_exprs() {
        if reads_tag(design.expr(e)) {
            kids(design, e, &mut seeds);
        }
    }
    tag_reach(design, seeds)
}

/// Whether `e` reads its operands' tags.
fn reads_tag(e: ir::Expr) -> bool {
    matches!(
        e,
        ir::Expr::Binary(
            ir::BinOp::DivTyped
                | ir::BinOp::BitAnd
                | ir::BinOp::BitOr
                | ir::BinOp::BitXor
                | ir::BinOp::Shl
                | ir::BinOp::Shr,
            ..
        ) | ir::Expr::Unary(ir::UnOp::BitNot, _)
            | ir::Expr::Call(ir::Func::ToInt | ir::Func::ToReal | ir::Func::SgnZeroPos, _)
    )
}
