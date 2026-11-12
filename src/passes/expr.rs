//! Passes group `expr` (expressions): a design in, the design without one feature out, one
//! pass per file. Empty: the group's owner adds its passes here.
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
//! Contents: `pub(super) const PASSES: &[Pass]`, `pub(super) const CODES: &[(ir::Code, ir::Info)]`

use super::Pass;
use crate::ir;

/// The group's passes, in the order `legalize` tries them.
pub(super) const PASSES: &[Pass] = &[];

/// The group's catalogue rows (`U0100..=U0199`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];
