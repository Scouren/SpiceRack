//! Passes group `run` (the run): a design in, the design without one feature out, one
//! pass per file. Empty: the group's owner adds its passes here.
//!
//! Owns the passes over analyses, options, state items, loops and outputs
//! (`tran_explicit_cap`, `tf_to_dcxf`, `whole_run_state_first`, `hoist_run_items`,
//! `loop_keep_last_to_alter`, `rshunt_to_gshunt`, … in `rewrite/DIALECT-VACASK.md` §6). It runs
//! last and may read what every other group produced.
//!
//! Contract (`passes` module header, `rewrite/SPINE.md` §4):
//! - while the group holds no pass it is this file; with its first pass it becomes
//!   `src/passes/run/mod.rs` with `src/passes/run/<pass>.rs` beside it (one pass per file,
//!   a helper shared by this group's passes in this folder too);
//! - a pass imports `crate::ir`, `crate::passes` (for `Pass`) and this group only;
//! - `PASSES` lists the passes in the order `legalize` tries them within the group;
//! - `CODES` holds the notes the passes put at items they leave, codes `U0400..=U0499`, each
//!   named in its pass's `Pass::notes`;
//! - each pass has its fixtures in `tests/fixtures/passes/run/<pass>/<case>.ir` and
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

/// The group's catalogue rows (`U0400..=U0499`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];
