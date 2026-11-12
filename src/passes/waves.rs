//! Passes group `waves` (sources and elements): a design in, the design without one feature out, one
//! pass per file. Empty: the group's owner adds its passes here.
//!
//! Owns the passes over waveforms and element kinds (`pulse_spice_to_vacask`,
//! `pulse_cycles_to_pwl`, `sffm_ngspice_to_spice3`, `wave_to_behavioural`, `poly_to_behav`,
//! `charge_to_behav`, `port_to_source_r`, `device_ic_to_nodeset`, … in
//! `rewrite/DIALECT-VACASK.md` §6). It runs after `expr`: the transient leaves are numbers by
//! then.
//!
//! Contract (`passes` module header, `rewrite/SPINE.md` §4):
//! - while the group holds no pass it is this file; with its first pass it becomes
//!   `src/passes/waves/mod.rs` with `src/passes/waves/<pass>.rs` beside it (one pass per file,
//!   a helper shared by this group's passes in this folder too);
//! - a pass imports `crate::ir`, `crate::passes` (for `Pass`) and this group only;
//! - `PASSES` lists the passes in the order `legalize` tries them within the group;
//! - `CODES` holds the notes the passes put at items they leave, codes `U0200..=U0299`, each
//!   named in its pass's `Pass::notes`;
//! - each pass has its fixtures in `tests/fixtures/passes/waves/<pass>/<case>.ir` and
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

/// The group's catalogue rows (`U0200..=U0299`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];
