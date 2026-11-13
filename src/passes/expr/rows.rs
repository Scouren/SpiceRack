//! The expression group's pass rows, in the order `legalize` tries them: the typed arithmetic
//! first after the functions are inlined (every other rule then sees the inlined trees, and a
//! later rule may read the tags the typed ones settle), then the powers and `clamp`, the roundings (even
//! before away: the first emits the second), the ternaries and the other functions, the bit
//! folds, the leaves, the draws, the long literals and the integer literals. Each rule is its plain row then its
//! `_dyn` row (A26); each `requires` is what its output may add, per form.
//!
//! Contents: `pub(crate) const PASSES: &[Pass]`

use super::*;
use crate::ir;
use crate::ir::{
    BinOp as O, DrawPer as P, DrawWhen as W, ExprTag as T, Extra as E, Feature as X, Func as F,
    UnOp as U,
};
use crate::passes::Pass;

/// The group's passes, in the order `legalize` tries them. One row per line: a table.
#[rustfmt::skip]
pub(crate) const PASSES: &[Pass] = &[
    row("inline_funcs", X::Expr(T::UserCall), &[], &[CALL_LEFT, OPERAND_TOO_LARGE], inline_funcs::inline_funcs),
    row("drop_unused_funcs", X::Item(ir::ItemKind::Func), &[], &[FUNC_CALLED], inline_funcs::drop_unused_funcs),
    row("div_to_div_typed", X::Binary(O::Div), &[X::Binary(O::DivTyped), X::Binary(O::Mul), X::Expr(T::Binary)], &[BOTH_FORMS], div_to_div_typed::div_to_div_typed),
    row("div_to_div_typed_dyn", X::BinaryDyn(O::Div), &[X::BinaryDyn(O::DivTyped), X::BinaryDyn(O::Mul), X::Expr(T::Binary)], &[BOTH_FORMS], div_to_div_typed::div_to_div_typed_dyn),
    row("div_typed_to_div", X::Binary(O::DivTyped), &[X::Binary(O::Div), X::Func(F::Trunc), X::Expr(T::Binary), X::Expr(T::Call)], &[MIXED_DIVISION, OTHER_FORM_READS, BOTH_FORMS], div_typed_to_div::div_typed_to_div),
    row("div_typed_to_div_dyn", X::BinaryDyn(O::DivTyped), &[X::BinaryDyn(O::Div), X::FuncDyn(F::Trunc), X::Expr(T::Binary), X::Expr(T::Call)], &[MIXED_DIVISION, OTHER_FORM_READS, BOTH_FORMS], div_typed_to_div::div_typed_to_div_dyn),
    row("toint_to_trunc", X::Func(F::ToInt), &[X::Func(F::Trunc), X::Expr(T::Call)], &[TAG_READ, BOTH_FORMS], toint_to_trunc::toint_to_trunc),
    row("toint_to_trunc_dyn", X::FuncDyn(F::ToInt), &[X::FuncDyn(F::Trunc), X::Expr(T::Call)], &[TAG_READ, BOTH_FORMS], toint_to_trunc::toint_to_trunc_dyn),
    // after the two rules that write `trunc`
    row("trunc_to_ternary", X::Func(F::Trunc), &[X::Expr(T::Ternary), X::Binary(O::Lt), X::Func(F::Ceil), X::Func(F::Floor), X::Expr(T::Binary), X::Expr(T::Call), X::Expr(T::Num)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], trunc_to_ternary::trunc_to_ternary),
    row("trunc_to_ternary_dyn", X::FuncDyn(F::Trunc), &[X::Expr(T::Ternary), X::BinaryDyn(O::Lt), X::FuncDyn(F::Ceil), X::FuncDyn(F::Floor), X::Expr(T::Binary), X::Expr(T::Call), X::Expr(T::Num)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], trunc_to_ternary::trunc_to_ternary_dyn),
    row("toreal_to_mul", X::Func(F::ToReal), &[X::Binary(O::Mul), X::Expr(T::Binary)], &[TEXT_OPERAND, BOTH_FORMS], toreal_to_mul::toreal_to_mul),
    row("toreal_to_mul_dyn", X::FuncDyn(F::ToReal), &[X::BinaryDyn(O::Mul), X::Expr(T::Binary)], &[TEXT_OPERAND, BOTH_FORMS], toreal_to_mul::toreal_to_mul_dyn),
    row("sgnzeropos_to_ternary", X::Func(F::SgnZeroPos), &[X::Expr(T::Ternary), X::Binary(O::Lt), X::Expr(T::Binary)], &[TAG_READ, BOTH_FORMS], sgnzeropos_to_ternary::sgnzeropos_to_ternary),
    row("sgnzeropos_to_ternary_dyn", X::FuncDyn(F::SgnZeroPos), &[X::Expr(T::Ternary), X::BinaryDyn(O::Lt), X::Expr(T::Binary)], &[TAG_READ, BOTH_FORMS], sgnzeropos_to_ternary::sgnzeropos_to_ternary_dyn),
    row("powabs_to_powre", X::Func(F::PowAbs), &[X::Func(F::Abs), X::Func(F::PowRe), X::Expr(T::Call)], &[BOTH_FORMS], powabs_to_powre::powabs_to_powre),
    row("powabs_to_powre_dyn", X::FuncDyn(F::PowAbs), &[X::FuncDyn(F::Abs), X::FuncDyn(F::PowRe), X::Expr(T::Call)], &[BOTH_FORMS], powabs_to_powre::powabs_to_powre_dyn),
    row("powre_to_pow", X::Func(F::PowRe), &[X::Func(F::Pow), X::Expr(T::Call)], &[POW_BASE, BOTH_FORMS], powre_to_pow::powre_to_pow),
    row("powre_to_pow_dyn", X::FuncDyn(F::PowRe), &[X::FuncDyn(F::Pow), X::Expr(T::Call)], &[POW_BASE, BOTH_FORMS], powre_to_pow::powre_to_pow_dyn),
    row("pow_to_powabs", X::Func(F::Pow), &[X::Func(F::PowAbs), X::Func(F::PowSigned), X::Expr(T::Call)], &[POW_BASE, BOTH_FORMS], pow_to_powabs::pow_to_powabs),
    row("pow_to_powabs_dyn", X::FuncDyn(F::Pow), &[X::FuncDyn(F::PowAbs), X::FuncDyn(F::PowSigned), X::Expr(T::Call)], &[POW_BASE, BOTH_FORMS], pow_to_powabs::pow_to_powabs_dyn),
    row("clamp_to_min_max", X::Func(F::Clamp), &[X::Func(F::Min), X::Func(F::Max), X::Binary(O::Mul), X::Expr(T::Binary), X::Expr(T::Call)], &[BOTH_FORMS], clamp_to_min_max::clamp_to_min_max),
    row("clamp_to_min_max_dyn", X::FuncDyn(F::Clamp), &[X::FuncDyn(F::Min), X::FuncDyn(F::Max), X::BinaryDyn(O::Mul), X::Expr(T::Binary), X::Expr(T::Call)], &[BOTH_FORMS], clamp_to_min_max::clamp_to_min_max_dyn),
    row("round_half_even_to_away", X::Func(F::RoundHalfEven), &[X::Expr(T::Ternary), X::Binary(O::Eq), X::Binary(O::Sub), X::Binary(O::Mul), X::Func(F::Floor), X::Func(F::RoundHalfAway), X::Expr(T::Binary), X::Expr(T::Call)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], round_half_even_to_away::round_half_even_to_away),
    row("round_half_even_to_away_dyn", X::FuncDyn(F::RoundHalfEven), &[X::Expr(T::Ternary), X::BinaryDyn(O::Eq), X::BinaryDyn(O::Sub), X::BinaryDyn(O::Mul), X::FuncDyn(F::Floor), X::FuncDyn(F::RoundHalfAway), X::Expr(T::Binary), X::Expr(T::Call)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], round_half_even_to_away::round_half_even_to_away_dyn),
    row("round_half_away_to_floor", X::Func(F::RoundHalfAway), &[X::Expr(T::Ternary), X::Binary(O::Ge), X::Binary(O::Sub), X::Binary(O::Add), X::Func(F::Floor), X::Func(F::Ceil), X::Expr(T::Binary), X::Expr(T::Call)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], round_half_away_to_floor::round_half_away_to_floor),
    row("round_half_away_to_floor_dyn", X::FuncDyn(F::RoundHalfAway), &[X::Expr(T::Ternary), X::BinaryDyn(O::Ge), X::BinaryDyn(O::Sub), X::BinaryDyn(O::Add), X::FuncDyn(F::Floor), X::FuncDyn(F::Ceil), X::Expr(T::Binary), X::Expr(T::Call)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], round_half_away_to_floor::round_half_away_to_floor_dyn),
    row("sgn_to_ternary", X::Func(F::Sgn), &[X::Expr(T::Ternary), X::Binary(O::Gt), X::Binary(O::Lt), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], sgn_to_ternary::sgn_to_ternary),
    row("sgn_to_ternary_dyn", X::FuncDyn(F::Sgn), &[X::Expr(T::Ternary), X::BinaryDyn(O::Gt), X::BinaryDyn(O::Lt), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], sgn_to_ternary::sgn_to_ternary_dyn),
    row("step_half_to_ternary", X::Func(F::StepHalf), &[X::Expr(T::Ternary), X::Binary(O::Gt), X::Binary(O::Lt), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], step_half_to_ternary::step_half_to_ternary),
    row("step_half_to_ternary_dyn", X::FuncDyn(F::StepHalf), &[X::Expr(T::Ternary), X::BinaryDyn(O::Gt), X::BinaryDyn(O::Lt), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], step_half_to_ternary::step_half_to_ternary_dyn),
    row("ramp_to_ternary", X::Func(F::Ramp), &[X::Expr(T::Ternary), X::Binary(O::Gt), X::Binary(O::Mul), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], ramp_to_ternary::ramp_to_ternary),
    row("ramp_to_ternary_dyn", X::FuncDyn(F::Ramp), &[X::Expr(T::Ternary), X::BinaryDyn(O::Gt), X::BinaryDyn(O::Mul), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], ramp_to_ternary::ramp_to_ternary_dyn),
    row("pow_signed_to_ternary", X::Func(F::PowSigned), &[X::Expr(T::Ternary), X::Binary(O::Lt), X::Unary(U::Neg), X::Func(F::Pow), X::Expr(T::Binary), X::Expr(T::Call), X::Expr(T::Unary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], pow_signed_to_ternary::pow_signed_to_ternary),
    row("pow_signed_to_ternary_dyn", X::FuncDyn(F::PowSigned), &[X::Expr(T::Ternary), X::BinaryDyn(O::Lt), X::UnaryDyn(U::Neg), X::FuncDyn(F::Pow), X::Expr(T::Binary), X::Expr(T::Call), X::Expr(T::Unary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], pow_signed_to_ternary::pow_signed_to_ternary_dyn),
    row("explin14_to_ternary", X::Func(F::ExpLin14), &[X::Expr(T::Ternary), X::Binary(O::Le), X::Binary(O::Mul), X::Binary(O::Add), X::Binary(O::Sub), X::Func(F::Exp), X::Expr(T::Binary), X::Expr(T::Call)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], explin14_to_ternary::explin14_to_ternary),
    row("explin14_to_ternary_dyn", X::FuncDyn(F::ExpLin14), &[X::Expr(T::Ternary), X::BinaryDyn(O::Le), X::BinaryDyn(O::Mul), X::BinaryDyn(O::Add), X::BinaryDyn(O::Sub), X::FuncDyn(F::Exp), X::Expr(T::Binary), X::Expr(T::Call)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], explin14_to_ternary::explin14_to_ternary_dyn),
    row("sqr_to_mul", X::Func(F::Sqr), &[X::Binary(O::Mul), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], sqr_to_mul::sqr_to_mul),
    row("sqr_to_mul_dyn", X::FuncDyn(F::Sqr), &[X::BinaryDyn(O::Mul), X::Expr(T::Binary)], &[DRAW_TWICE, BOTH_FORMS, OPERAND_TOO_LARGE], sqr_to_mul::sqr_to_mul_dyn),
    row("pwl_to_ternary", X::Func(F::Pwl), &[X::Expr(T::Ternary), X::Binary(O::Le), X::Binary(O::Ge), X::Binary(O::Lt), X::Binary(O::Add), X::Binary(O::Sub), X::Binary(O::Mul), X::Binary(O::Div), X::Expr(T::Binary)], &[PWL_POINTS, BOTH_FORMS, OPERAND_TOO_LARGE], pwl_to_ternary::pwl_to_ternary),
    row("pwl_to_ternary_dyn", X::FuncDyn(F::Pwl), &[X::Expr(T::Ternary), X::BinaryDyn(O::Le), X::BinaryDyn(O::Ge), X::BinaryDyn(O::Lt), X::BinaryDyn(O::Add), X::BinaryDyn(O::Sub), X::BinaryDyn(O::Mul), X::BinaryDyn(O::Div), X::Expr(T::Binary)], &[PWL_POINTS, BOTH_FORMS, OPERAND_TOO_LARGE], pwl_to_ternary::pwl_to_ternary_dyn),
    row("fold_constant_bitand", X::Binary(O::BitAnd), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitand),
    row("fold_constant_bitand_dyn", X::BinaryDyn(O::BitAnd), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitand_dyn),
    row("fold_constant_bitor", X::Binary(O::BitOr), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitor),
    row("fold_constant_bitor_dyn", X::BinaryDyn(O::BitOr), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitor_dyn),
    row("fold_constant_bitxor", X::Binary(O::BitXor), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitxor),
    row("fold_constant_bitxor_dyn", X::BinaryDyn(O::BitXor), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitxor_dyn),
    row("fold_constant_shl", X::Binary(O::Shl), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_shl),
    row("fold_constant_shl_dyn", X::BinaryDyn(O::Shl), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_shl_dyn),
    row("fold_constant_shr", X::Binary(O::Shr), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_shr),
    row("fold_constant_shr_dyn", X::BinaryDyn(O::Shr), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_shr_dyn),
    row("fold_constant_bitnot", X::Unary(U::BitNot), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitnot),
    row("fold_constant_bitnot_dyn", X::UnaryDyn(U::BitNot), &[X::Expr(T::Num)], &[BITOP_NOT_CONSTANT, BOTH_FORMS], fold_constant_bitops::fold_constant_bitnot_dyn),
    row("tnom_to_env", X::Expr(T::Tnom), &[X::Expr(T::Num)], &[NO_LEAF_VALUE], tnom_to_env::tnom_to_env),
    row("substitute_tstep", X::Expr(T::TStep), &[X::Expr(T::Num)], &[NO_LEAF_VALUE], substitute_tran_leaves::substitute_tstep),
    row("substitute_tstop", X::Expr(T::TStop), &[X::Expr(T::Num)], &[NO_LEAF_VALUE], substitute_tran_leaves::substitute_tstop),
    row("draws_always_to_sampling", X::DrawWhen(W::Always), &[X::DrawWhen(W::Sampling)], &[], draws_always_to_sampling::draws_always_to_sampling),
    row("draws_per_reference_to_call_site", X::DrawPer(P::Reference), &[X::DrawPer(P::CallSite)], &[DRAW_SAMPLES], draws_per_reference_to_call_site::draws_per_reference_to_call_site),
    row("sampling_draws_to_nominal", X::DrawWhen(W::Sampling), &[X::Binary(O::Mul), X::BinaryDyn(O::Mul), X::Expr(T::Binary)], &[DRAW_SAMPLES, TEXT_OPERAND], sampling_draws_to_nominal::sampling_draws_to_nominal),
    row("long_literal_to_shortest", X::Extra(E::LongLiteral), &[X::Expr(T::Num)], &[], long_literal_to_shortest::long_literal_to_shortest),
    // last: the typed rules decide by an integer literal's tag first
    row("int_literal_to_plain", X::Extra(E::IntLiteral), &[], &[INT_TAG_READ], int_literal_to_plain::int_literal_to_plain),
    row("param_reads_to_typed", X::Circuit(ir::CircuitExtra::BehavUntypedParam), &[X::FuncDyn(F::ToReal), X::FuncDyn(F::ToInt), X::Expr(T::Call)], &[UNTYPED_READ], param_reads_to_typed::param_reads_to_typed),
];

// ---- private ----

/// A row: its name, the feature it removes, what its output may add, its notes and its run.
const fn row(
    name: &'static str,
    removes: ir::Feature,
    requires: &[ir::Feature],
    notes: &'static [ir::Code],
    run: fn(&mut ir::Design, &mut ir::Diags),
) -> Pass {
    Pass {
        name,
        removes,
        requires: set(requires),
        notes,
        run,
    }
}

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
