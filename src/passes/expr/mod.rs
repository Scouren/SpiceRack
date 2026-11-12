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
//! Contents: `pub mod div_to_div_typed`, `pub mod div_typed_to_div`, `pub mod draws_always_to_sampling`, `pub mod draws_per_reference_to_call_site`, `pub mod fold_constant_bitops`, `pub mod pow_to_powabs`, `pub mod powabs_to_pow`, `pub mod pwl_to_ternary`, `pub mod ramp_to_ternary`, `pub mod round_half_away_to_floor`, `pub mod round_half_even_to_away`, `pub mod sampling_draws_to_nominal`, `pub mod sgn_to_ternary`, `pub mod sgnzeropos_to_ternary`, `pub mod step_half_to_ternary`, `pub mod step_to_ternary`, `pub mod substitute_tran_leaves`, `pub mod tnom_to_env`, `pub mod toint_to_trunc`, `pub mod toreal_to_mul`, `pub(super) const PASSES: &[Pass]`,
//! `pub(super) const CODES: &[(ir::Code, ir::Info)]` and the codes (`codes.rs`)

mod codes;
mod context;
pub mod div_to_div_typed;
pub mod div_typed_to_div;
pub mod draws_always_to_sampling;
pub mod draws_per_reference_to_call_site;
pub mod fold_constant_bitops;
mod nodes;
pub mod pow_to_powabs;
pub mod powabs_to_pow;
pub mod pwl_to_ternary;
pub mod ramp_to_ternary;
pub mod round_half_away_to_floor;
pub mod round_half_even_to_away;
pub mod sampling_draws_to_nominal;
pub mod sgn_to_ternary;
pub mod sgnzeropos_to_ternary;
pub mod step_half_to_ternary;
pub mod step_to_ternary;
pub mod substitute_tran_leaves;
mod tags;
pub mod tnom_to_env;
pub mod toint_to_trunc;
pub mod toreal_to_mul;

pub(super) use codes::*;

use super::Pass;
use crate::ir;

/// The group's passes, in the order `legalize` tries them: the typed arithmetic first (a later
/// rule may read the tags it settles), then the powers (whose output the rounding and sign
/// rules never need), the roundings (even before away: the first emits the second), the
/// ternaries, and the rest.
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
        name: "pow_to_powabs",
        removes: ir::Feature::Func(ir::Func::Pow),
        requires: set(&[
            ir::Feature::Func(ir::Func::PowAbs),
            ir::Feature::Func(ir::Func::PowSigned),
        ]),
        notes: &[POW_BASE],
        run: pow_to_powabs::pow_to_powabs,
    },
    Pass {
        name: "pow_to_powabs_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::Pow),
        requires: set(&[
            ir::Feature::FuncDyn(ir::Func::PowAbs),
            ir::Feature::FuncDyn(ir::Func::PowSigned),
        ]),
        notes: &[POW_BASE],
        run: pow_to_powabs::pow_to_powabs_dyn,
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
    Pass {
        name: "round_half_even_to_away",
        removes: ir::Feature::Func(ir::Func::RoundHalfEven),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Eq),
            ir::Feature::Binary(ir::BinOp::Sub),
            ir::Feature::Binary(ir::BinOp::Mul),
            ir::Feature::Func(ir::Func::Floor),
            ir::Feature::Func(ir::Func::RoundHalfAway),
        ]),
        notes: &[DRAW_TWICE],
        run: round_half_even_to_away::round_half_even_to_away,
    },
    Pass {
        name: "round_half_even_to_away_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::RoundHalfEven),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Eq),
            ir::Feature::BinaryDyn(ir::BinOp::Sub),
            ir::Feature::BinaryDyn(ir::BinOp::Mul),
            ir::Feature::FuncDyn(ir::Func::Floor),
            ir::Feature::FuncDyn(ir::Func::RoundHalfAway),
        ]),
        notes: &[DRAW_TWICE],
        run: round_half_even_to_away::round_half_even_to_away_dyn,
    },
    Pass {
        name: "round_half_away_to_floor",
        removes: ir::Feature::Func(ir::Func::RoundHalfAway),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Ge),
            ir::Feature::Binary(ir::BinOp::Sub),
            ir::Feature::Binary(ir::BinOp::Add),
            ir::Feature::Func(ir::Func::Floor),
            ir::Feature::Func(ir::Func::Ceil),
        ]),
        notes: &[DRAW_TWICE],
        run: round_half_away_to_floor::round_half_away_to_floor,
    },
    Pass {
        name: "round_half_away_to_floor_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::RoundHalfAway),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Ge),
            ir::Feature::BinaryDyn(ir::BinOp::Sub),
            ir::Feature::BinaryDyn(ir::BinOp::Add),
            ir::Feature::FuncDyn(ir::Func::Floor),
            ir::Feature::FuncDyn(ir::Func::Ceil),
        ]),
        notes: &[DRAW_TWICE],
        run: round_half_away_to_floor::round_half_away_to_floor_dyn,
    },
    Pass {
        name: "sgn_to_ternary",
        removes: ir::Feature::Func(ir::Func::Sgn),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Gt),
            ir::Feature::Binary(ir::BinOp::Lt),
        ]),
        notes: &[DRAW_TWICE],
        run: sgn_to_ternary::sgn_to_ternary,
    },
    Pass {
        name: "sgn_to_ternary_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::Sgn),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Gt),
            ir::Feature::BinaryDyn(ir::BinOp::Lt),
        ]),
        notes: &[DRAW_TWICE],
        run: sgn_to_ternary::sgn_to_ternary_dyn,
    },
    Pass {
        name: "step_to_ternary",
        removes: ir::Feature::Func(ir::Func::Step),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Gt),
        ]),
        notes: &[],
        run: step_to_ternary::step_to_ternary,
    },
    Pass {
        name: "step_to_ternary_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::Step),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Gt),
        ]),
        notes: &[],
        run: step_to_ternary::step_to_ternary_dyn,
    },
    Pass {
        name: "step_half_to_ternary",
        removes: ir::Feature::Func(ir::Func::StepHalf),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Gt),
            ir::Feature::Binary(ir::BinOp::Lt),
        ]),
        notes: &[DRAW_TWICE],
        run: step_half_to_ternary::step_half_to_ternary,
    },
    Pass {
        name: "step_half_to_ternary_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::StepHalf),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Gt),
            ir::Feature::BinaryDyn(ir::BinOp::Lt),
        ]),
        notes: &[DRAW_TWICE],
        run: step_half_to_ternary::step_half_to_ternary_dyn,
    },
    Pass {
        name: "ramp_to_ternary",
        removes: ir::Feature::Func(ir::Func::Ramp),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Gt),
            ir::Feature::Binary(ir::BinOp::Mul),
        ]),
        notes: &[DRAW_TWICE],
        run: ramp_to_ternary::ramp_to_ternary,
    },
    Pass {
        name: "ramp_to_ternary_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::Ramp),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Gt),
            ir::Feature::BinaryDyn(ir::BinOp::Mul),
        ]),
        notes: &[DRAW_TWICE],
        run: ramp_to_ternary::ramp_to_ternary_dyn,
    },
    Pass {
        name: "pwl_to_ternary",
        removes: ir::Feature::Func(ir::Func::Pwl),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::Binary(ir::BinOp::Le),
            ir::Feature::Binary(ir::BinOp::Ge),
            ir::Feature::Binary(ir::BinOp::Lt),
            ir::Feature::Binary(ir::BinOp::Add),
            ir::Feature::Binary(ir::BinOp::Sub),
            ir::Feature::Binary(ir::BinOp::Mul),
            ir::Feature::Binary(ir::BinOp::Div),
        ]),
        notes: &[PWL_POINTS],
        run: pwl_to_ternary::pwl_to_ternary,
    },
    Pass {
        name: "pwl_to_ternary_dyn",
        removes: ir::Feature::FuncDyn(ir::Func::Pwl),
        requires: set(&[
            ir::Feature::Expr(ir::ExprTag::Ternary),
            ir::Feature::BinaryDyn(ir::BinOp::Le),
            ir::Feature::BinaryDyn(ir::BinOp::Ge),
            ir::Feature::BinaryDyn(ir::BinOp::Lt),
            ir::Feature::BinaryDyn(ir::BinOp::Add),
            ir::Feature::BinaryDyn(ir::BinOp::Sub),
            ir::Feature::BinaryDyn(ir::BinOp::Mul),
            ir::Feature::BinaryDyn(ir::BinOp::Div),
        ]),
        notes: &[PWL_POINTS],
        run: pwl_to_ternary::pwl_to_ternary_dyn,
    },
    Pass {
        name: "fold_constant_bitand",
        removes: ir::Feature::Binary(ir::BinOp::BitAnd),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitand,
    },
    Pass {
        name: "fold_constant_bitand_dyn",
        removes: ir::Feature::BinaryDyn(ir::BinOp::BitAnd),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitand_dyn,
    },
    Pass {
        name: "fold_constant_bitor",
        removes: ir::Feature::Binary(ir::BinOp::BitOr),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitor,
    },
    Pass {
        name: "fold_constant_bitor_dyn",
        removes: ir::Feature::BinaryDyn(ir::BinOp::BitOr),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitor_dyn,
    },
    Pass {
        name: "fold_constant_bitxor",
        removes: ir::Feature::Binary(ir::BinOp::BitXor),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitxor,
    },
    Pass {
        name: "fold_constant_bitxor_dyn",
        removes: ir::Feature::BinaryDyn(ir::BinOp::BitXor),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitxor_dyn,
    },
    Pass {
        name: "fold_constant_shl",
        removes: ir::Feature::Binary(ir::BinOp::Shl),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_shl,
    },
    Pass {
        name: "fold_constant_shl_dyn",
        removes: ir::Feature::BinaryDyn(ir::BinOp::Shl),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_shl_dyn,
    },
    Pass {
        name: "fold_constant_shr",
        removes: ir::Feature::Binary(ir::BinOp::Shr),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_shr,
    },
    Pass {
        name: "fold_constant_shr_dyn",
        removes: ir::Feature::BinaryDyn(ir::BinOp::Shr),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_shr_dyn,
    },
    Pass {
        name: "fold_constant_bitnot",
        removes: ir::Feature::Unary(ir::UnOp::BitNot),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitnot,
    },
    Pass {
        name: "fold_constant_bitnot_dyn",
        removes: ir::Feature::UnaryDyn(ir::UnOp::BitNot),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[BITOP_NOT_CONSTANT],
        run: fold_constant_bitops::fold_constant_bitnot_dyn,
    },
    Pass {
        name: "tnom_to_env",
        removes: ir::Feature::Expr(ir::ExprTag::Tnom),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[NO_LEAF_VALUE],
        run: tnom_to_env::tnom_to_env,
    },
    Pass {
        name: "substitute_tstep",
        removes: ir::Feature::Expr(ir::ExprTag::TStep),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[NO_LEAF_VALUE],
        run: substitute_tran_leaves::substitute_tstep,
    },
    Pass {
        name: "substitute_tstop",
        removes: ir::Feature::Expr(ir::ExprTag::TStop),
        requires: set(&[ir::Feature::Expr(ir::ExprTag::Num)]),
        notes: &[NO_LEAF_VALUE],
        run: substitute_tran_leaves::substitute_tstop,
    },
    Pass {
        name: "draws_always_to_sampling",
        removes: ir::Feature::DrawWhen(ir::DrawWhen::Always),
        requires: set(&[ir::Feature::DrawWhen(ir::DrawWhen::Sampling)]),
        notes: &[],
        run: draws_always_to_sampling::draws_always_to_sampling,
    },
    Pass {
        name: "draws_per_reference_to_call_site",
        removes: ir::Feature::DrawPer(ir::DrawPer::Reference),
        requires: set(&[ir::Feature::DrawPer(ir::DrawPer::CallSite)]),
        notes: &[DRAW_SAMPLES],
        run: draws_per_reference_to_call_site::draws_per_reference_to_call_site,
    },
    Pass {
        name: "sampling_draws_to_nominal",
        removes: ir::Feature::DrawWhen(ir::DrawWhen::Sampling),
        requires: set(&[
            ir::Feature::Binary(ir::BinOp::Mul),
            ir::Feature::BinaryDyn(ir::BinOp::Mul),
        ]),
        notes: &[DRAW_SAMPLES],
        run: sampling_draws_to_nominal::sampling_draws_to_nominal,
    },
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
