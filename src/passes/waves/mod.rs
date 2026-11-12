//! Passes group `waves` (sources and elements): a design in, the design without one feature
//! out, one rule per file (a rule's rows share its file, each row's fixtures in a folder of its
//! row name, SPINE §4).
//!
//! Owns the passes over waveforms and element kinds (`pulse_spice_to_vacask`,
//! `pulse_cycles_to_pwl`, `sffm_ngspice_to_spice3`, `wave_to_behavioural`, `poly_to_behav`,
//! `charge_to_behav`, `port_to_source_r`, … in `rewrite/DIALECT-VACASK.md` §6). It runs after
//! `expr`: the transient leaves are numbers by then.
//!
//! Contract (`passes` module header, `rewrite/SPINE.md` §4):
//! - a pass imports `crate::ir`, `crate::passes` (for `Pass`) and this group only;
//! - `PASSES` (`rows.rs`) lists the passes in the order `legalize` tries them within the group;
//! - `CODES` (`codes.rs`) holds the notes the passes put at items they leave, codes
//!   `U0200..=U0299`, each named in its pass's `Pass::notes`;
//! - each pass has its fixtures in `tests/fixtures/passes/waves/<row>/<case>.ir`, `.out.ir`
//!   and `.notes`, and meets the laws of `tests/pass_laws.rs` (P1–P5); its meaning is the
//!   reference evaluator of sources (`ir::wave_eval`, A30) on both forms, its own test.
//!
//! Cites: `rewrite/SPINE.md` §4; `rewrite/DIALECT-VACASK.md` §6; `rewrite/AMEND-A12.md` A30.
//!
//! Contents: `pub mod pulse_spice_to_vacask`, `pub(super) const PASSES: &[Pass]` (`rows.rs`),
//! `pub(super) const CODES: &[(ir::Code, ir::Info)]` and the codes (`codes.rs`)

mod codes;
mod nodes;
pub mod pulse_spice_to_vacask;
mod rows;

pub(super) use codes::*;
pub(super) use rows::PASSES;
