//! Passes group `expr` (expressions): a design in, the design without one feature out, one
//! pass per file.
//!
//! Owns the passes that rewrite expression trees: the functions, operators, leaves and draws one
//! target lacks (`div_to_div_typed`, `powabs_to_pow`, `round_half_even_to_away`,
//! `sgn_to_ternary`, `substitute_tran_leaves`, `inline_funcs`, … in `rewrite/DIALECT-VACASK.md`
//! §6). A pass that would duplicate an operand holding a `Draw` leaves the item with a note.
//!
//! Contract (`passes` module header, `rewrite/SPINE.md` §4):
//! - while the group holds no pass it is this file; with its first pass it becomes
//!   `src/passes/expr/mod.rs` with `src/passes/expr/<pass>.rs` beside it (one pass per file,
//!   a helper shared by this group's passes in this folder too);
//! - a pass imports `crate::ir`, `crate::passes` (for `Pass`) and this group only;
//! - `PASSES` lists the passes in the order `legalize` tries them within the group;
//! - `CODES` holds the notes the passes put at items they leave, codes `U0100..=U0199`, each
//!   named in its pass's `Pass::notes`;
//! - each pass has its fixtures in `tests/fixtures/passes/expr/<pass>/<case>.ir` and
//!   `.out.ir`, and meets the laws of `tests/pass_laws.rs` (P1–P5); its meaning (the reference
//!   evaluator or a simulator on both forms) is its own test.
//!
//! Cites: `rewrite/SPINE.md` §4; `rewrite/DIALECT-VACASK.md` §6.
//!
//! A function or operator is two features, `Func(f)` and `FuncDyn(f)` by the root it stands in
//! (`ir::features_used`), so each rule is two rows, `<rule>` and `<rule>_dyn`, one file:
//! `context.rs` applies a rule to one context's roots and leaves the other's.
//!
//! Contents: `pub mod div_to_div_typed`, `pub mod div_typed_to_div`, `pub mod powabs_to_pow`, `pub mod sgnzeropos_to_ternary`, `pub mod toint_to_trunc`, `pub mod toreal_to_mul`, `pub(super) const PASSES: &[Pass]`,
//! `pub(super) const CODES: &[(ir::Code, ir::Info)]`, `pub(super) const MIXED_DIVISION`,
//! `pub(super) const TAG_READ`

mod context;
pub mod div_to_div_typed;
pub mod div_typed_to_div;
mod nodes;
pub mod powabs_to_pow;
pub mod sgnzeropos_to_ternary;
mod tags;
pub mod toint_to_trunc;
pub mod toreal_to_mul;

use super::Pass;
use crate::ir;

/// The group's passes, in the order `legalize` tries them: the typed arithmetic first (a later
/// rule may read the tags it settles), then the functions.
pub(super) const PASSES: &[Pass] = &[
    Pass {
        name: "div_to_div_typed",
        removes: ir::Feature::Binary(ir::BinOp::Div),
        requires: set(&[
            ir::Feature::Binary(ir::BinOp::DivTyped),
            ir::Feature::Binary(ir::BinOp::Mul),
        ]),
        notes: &[],
        run: div_to_div_typed::div_to_div_typed,
    },
    Pass {
        name: "div_to_div_typed_dyn",
        removes: ir::Feature::BinaryDyn(ir::BinOp::Div),
        requires: set(&[
            ir::Feature::BinaryDyn(ir::BinOp::DivTyped),
            ir::Feature::BinaryDyn(ir::BinOp::Mul),
        ]),
        notes: &[],
        run: div_to_div_typed::div_to_div_typed_dyn,
    },
    Pass {
        name: "div_typed_to_div",
        removes: ir::Feature::Binary(ir::BinOp::DivTyped),
        requires: set(&[
            ir::Feature::Binary(ir::BinOp::Div),
            ir::Feature::BinaryDyn(ir::BinOp::Div),
            ir::Feature::Func(ir::Func::Trunc),
            ir::Feature::FuncDyn(ir::Func::Trunc),
        ]),
        notes: &[MIXED_DIVISION],
        run: div_typed_to_div::div_typed_to_div,
    },
    Pass {
        name: "div_typed_to_div_dyn",
        removes: ir::Feature::BinaryDyn(ir::BinOp::DivTyped),
        requires: set(&[
            ir::Feature::Binary(ir::BinOp::Div),
            ir::Feature::BinaryDyn(ir::BinOp::Div),
            ir::Feature::Func(ir::Func::Trunc),
            ir::Feature::FuncDyn(ir::Func::Trunc),
        ]),
        notes: &[MIXED_DIVISION],
        run: div_typed_to_div::div_typed_to_div_dyn,
    },
    Pass {
        name: "toint_to_trunc",
        removes: ir::Feature::Func(ir::Func::ToInt),
        requires: set(&[ir::Feature::Func(ir::Func::Trunc)]),
        notes: &[TAG_READ],
        run: toint_to_trunc::toint_to_trunc,
    },
    Pass {
        name: "toint_to_trunc_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::ToInt),
        requires: set(&[ir::Feature::FuncDyn(ir::Func::Trunc)]),
        notes: &[TAG_READ],
        run: toint_to_trunc::toint_to_trunc_dyn,
    },
    Pass {
        name: "toreal_to_mul",
        removes: ir::Feature::Func(ir::Func::ToReal),
        requires: set(&[ir::Feature::Binary(ir::BinOp::Mul)]),
        notes: &[],
        run: toreal_to_mul::toreal_to_mul,
    },
    Pass {
        name: "toreal_to_mul_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::ToReal),
        requires: set(&[ir::Feature::BinaryDyn(ir::BinOp::Mul)]),
        notes: &[],
        run: toreal_to_mul::toreal_to_mul_dyn,
    },
    Pass {
        name: "sgnzeropos_to_ternary",
        removes: ir::Feature::Func(ir::Func::SgnZeroPos),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Lt),
        ]),
        notes: &[TAG_READ],
        run: sgnzeropos_to_ternary::sgnzeropos_to_ternary,
    },
    Pass {
        name: "sgnzeropos_to_ternary_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::SgnZeroPos),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Lt),
        ]),
        notes: &[TAG_READ],
        run: sgnzeropos_to_ternary::sgnzeropos_to_ternary_dyn,
    },
    Pass {
        name: "powabs_to_pow",
        removes: ir::Feature::Func(ir::Func::PowAbs),
        requires: set(&[
            ir::Feature::Func(ir::Func::Abs),
            ir::Feature::Func(ir::Func::Pow),
        ]),
        notes: &[],
        run: powabs_to_pow::powabs_to_pow,
    },
    Pass {
        name: "powabs_to_pow_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::PowAbs),
        requires: set(&[
            ir::Feature::FuncDyn(ir::Func::Abs),
            ir::Feature::FuncDyn(ir::Func::Pow),
        ]),
        notes: &[],
        run: powabs_to_pow::powabs_to_pow_dyn,
    },
];

/// A typed division whose operands' tags the design does not fix.
pub(super) const MIXED_DIVISION: ir::Code = ir::Code::new(b'U', 101);
/// A rewrite that drops a tag while a typed division may read it.
pub(super) const TAG_READ: ir::Code = ir::Code::new(b'U', 103);

/// The group's catalogue rows (`U0100..=U0199`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        MIXED_DIVISION,
        ir::Info {
            title: "a typed division whose operand types the design does not fix",
            cite: "rewrite/AMEND-A12.md K1; rewrite/DIALECT-VACASK.md §6.2",
            explain: "The division truncates when both operands are integers and divides exactly otherwise, and the design gives an operand both types (two instances pass a parameter an integer and a real, two calls pass a formal differently, or a loop or an alter sets it). Real division cannot say which, so the item is refused. While such a division stands, no integer quotient elsewhere is rewritten either: it could read that quotient's type.",
            kind: ir::Kind::Note,
        },
    ),
    (
        TAG_READ,
        ir::Info {
            title: "a rewrite that drops an integer type while a typed division may read it",
            cite: "rewrite/AMEND-A12.md K1",
            explain: "The target has no spelling for an integer-typed value here; its real spelling has the same value, but a typed division still in the design reads types, so the item is refused until none is left.",
            kind: ir::Kind::Note,
        },
    ),
];

// ---- private ----

/// The set of `features`.
const fn set(features: &[ir::Feature]) -> ir::FeatureSet {
    let mut s = ir::FeatureSet::EMPTY;
    let mut i = 0;
    while i < features.len() {
        s = s.with(features[i]);
        i += 1;
    }
    s
}
