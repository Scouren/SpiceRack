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
//! `ir::rewrite_exprs_in` applies a rule to one form's roots and leaves the other's (A26).
//!
//! **The size bound.** A rule whose definition reads an operand more than once (`sqr_to_mul`,
//! `pow_signed_to_ternary`, the two roundings, `sgn`, `step_half`, `ramp`, `explin14`, `pwl`'s
//! input, `inline_funcs`' formal read twice) shares that operand's rows in the IR, but every
//! writer spells each read in full, so nested copies grow the text exponentially, and a copy
//! made early grows again with every later rewrite under it. So the bound is on what a root
//! spells: each such rule runs through `nodes::keep_bounded`, which points back at its old tree
//! every root the rule made spell more than `nodes::ROOT_PATHS_MAX` rows by paths (8192: the
//! longest corpus expression, 132 tokens, through the two most copying rules, under 4800) and
//! more than before; that item keeps its node with note `U0116`. Every walk of this group visits
//! each row once, and every path count saturates. ponytail: a let-binding (the operand bound
//! once to a new parameter, read twice) lifts the bound; it changes the design's definitions, a
//! structure-level decision, so it is the upgrade path, not this version.
//!
//! Contents: `pub mod div_to_div_typed`, `pub mod div_typed_to_div`, `pub mod draws_always_to_sampling`, `pub mod draws_per_reference_to_call_site`, `pub mod explin14_to_ternary`, `pub mod fold_constant_bitops`, `pub mod inline_funcs`, `pub mod int_literal_to_plain`, `pub mod ln_abs_to_ln`, `pub mod long_literal_to_shortest`, `pub mod param_reads_to_typed`, `pub mod pow_signed_to_ternary`, `pub mod pow_to_powabs`, `pub mod powabs_to_pow`, `pub mod pwl_to_ternary`, `pub mod ramp_to_ternary`, `pub mod round_half_away_to_floor`, `pub mod round_half_even_to_away`, `pub mod sampling_draws_to_nominal`, `pub mod sgn_to_ternary`, `pub mod sgnzeropos_to_ternary`, `pub mod sqr_to_mul`, `pub mod sqrt_abs_to_sqrt`, `pub mod step_half_to_ternary`, `pub mod step_to_ternary`, `pub mod substitute_tran_leaves`, `pub mod tnom_to_env`, `pub mod toint_to_trunc`, `pub mod toreal_to_mul`, `pub mod trunc_to_ternary`, `pub(super) const PASSES: &[Pass]`
//! (`rows.rs`), `pub(super) const CODES: &[(ir::Code, ir::Info)]` and the codes (`codes.rs`)

mod codes;
pub mod div_to_div_typed;
pub mod div_typed_to_div;
pub mod draws_always_to_sampling;
pub mod draws_per_reference_to_call_site;
pub mod explin14_to_ternary;
pub mod fold_constant_bitops;
pub mod inline_funcs;
pub mod int_literal_to_plain;
pub mod ln_abs_to_ln;
pub mod long_literal_to_shortest;
mod nodes;
pub mod param_reads_to_typed;
pub mod pow_signed_to_ternary;
pub mod pow_to_powabs;
pub mod powabs_to_pow;
pub mod pwl_to_ternary;
pub mod ramp_to_ternary;
mod reach;
pub mod round_half_away_to_floor;
pub mod round_half_even_to_away;
mod rows;
pub mod sampling_draws_to_nominal;
pub mod sgn_to_ternary;
pub mod sgnzeropos_to_ternary;
pub mod sqr_to_mul;
pub mod sqrt_abs_to_sqrt;
pub mod step_half_to_ternary;
pub mod step_to_ternary;
pub mod substitute_tran_leaves;
mod tags;
pub mod tnom_to_env;
pub mod toint_to_trunc;
pub mod toreal_to_mul;
pub mod trunc_to_ternary;

pub(super) use codes::*;
pub(super) use rows::PASSES;
