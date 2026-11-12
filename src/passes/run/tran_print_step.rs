//! Rule `tran_print_step_to_simulator`: a transient's given print step (`Tran::print_step`,
//! `RunExtra::TranPrintStep`) made absent (`TranSimulatorStep`), with a `PRINT_STEP_DROPPED`
//! note at the analysis whose argument is the interval's expression (its `ExprId`: rows are
//! never removed), so the run side gives the results back on the source's grid (R67, A41).
//!
//! Exact on every simulated number once nothing reads the interval: it only resamples the
//! output (and ngspice's runner records the accepted steps anyway, R49). Three things read it,
//! and each leaves the transient with `PRINT_STEP_READ`:
//! - a step ceiling that is still ngspice's rule `min(print_step, span / 50)`
//!   (`TranCapNgspice`): `tran_cap_ngspice_to_given` writes it out first;
//! - a `$tstep` leaf anywhere in the design (`Expr(TStep)`, which is this interval):
//!   `substitute_tstep` replaces it first;
//! - HSPICE's further segments (`TranSegments`), each an interval of its own.
//!
//! Removes `Run(TranPrintStep)`, requires `Run(TranSimulatorStep)`.
//!
//! Cites: R67; `rewrite/AMEND-A12.md` A41, A24; `crates/ir/src/tran.rs` (`Tran`).
//!
//! Contents: `pub fn tran_print_step_to_simulator(&mut ir::Design, &mut ir::Diags)`,
//! `pub const PRINT_STEP_DROPPED: ir::Code`, `pub const PRINT_STEP_READ: ir::Code`

use super::names_dropped::of_kind;
use crate::ir;
use ir::tran::{StepCap, StepCeiling, Tran};

/// The note at a transient whose print step was dropped: `arg` is the interval's `ExprId`.
pub const PRINT_STEP_DROPPED: ir::Code = ir::Code::new(b'U', 412);
/// The note at a transient whose print step something still reads.
pub const PRINT_STEP_READ: ir::Code = ir::Code::new(b'U', 413);

/// Drops every reachable transient's print step that nothing reads; notes every other.
pub fn tran_print_step_to_simulator(design: &mut ir::Design, diags: &mut ir::Diags) {
    let tstep = ir::features_used(design).contains(ir::Feature::Expr(ir::ExprTag::TStep));
    for item in of_kind(design, ir::ItemKind::Analysis) {
        let a = design.row::<ir::Analysis>(item.index);
        if a.kind != ir::AnalysisKind::Tran {
            continue;
        }
        let mut t = design.detail::<Tran>(a.detail);
        if t.print_step.is_none() {
            continue;
        }
        let read = t.ceiling == StepCeiling::Rule(StepCap::NgspiceRule) || t.segments.len > 0;
        if read || tstep {
            diags.push_at(PRINT_STEP_READ, design, item, 0);
            continue;
        }
        diags.push_at(PRINT_STEP_DROPPED, design, item, t.print_step.0);
        t.print_step = ir::ExprId::NONE;
        design.editor().set_detail(a.detail, t);
        assert!(
            design.detail::<Tran>(a.detail).print_step.is_none(),
            "the interval went"
        );
    }
}
