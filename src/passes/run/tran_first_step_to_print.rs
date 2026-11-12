//! Rule `tran_first_step_to_print`: a transient with a first step (`Tran::initial_step`) and no
//! print step (`TranSimulatorStep`) given its first step as its print step, the first step then
//! the target's own rule (R73). The value is moved, never chosen (R13): `print_step :=
//! initial_step`, `initial_step := NONE`, with a `FIRST_STEP_MOVED` note at the analysis whose
//! argument is the value's `ExprId`. One body, two rows, each requiring `Run(TranPrintStep)`:
//! - `tran_simulator_step_to_print` removes `Run(TranSimulatorStep)`;
//! - `tran_first_step_to_print` removes `Run(TranInitialStep)`.
//!
//! The bound: the first step a deck gives is an upper limit its author set, and a conversion
//! may take a smaller one, never a larger. ngspice-45's own first step is
//! `min(print_step, stop / 100) / 100`, never larger than the print step: the settle run
//! (`settle_small.cir`, `settle_large.cir` in this rule's fixture folder; an RC driven by a
//! sine, no breakpoint, read by `-b -r`) gives a second time point of 1e-11 for a print step of
//! 1n under stop/100 = 100n, and 1e-9 for 1u above it.
//!
//! ponytail: sound for a target whose own first step does not exceed its print step (ngspice,
//! pinned above); a second such target pins that by a run before its row joins, and if two
//! differ the first-step rule becomes an IR fact like `StepCap`.
//!
//! It fires only where nothing else reads the new print step: the ceiling is given (the rows
//! stand after the ceiling rows, so a `SpanOver50` is written out first) and no `$tstep` leaf
//! stands in the design. Otherwise the transient stays, with `FIRST_STEP_KEPT`, as does a first
//! step beside a given print step (no field takes it). A transient with neither a print step
//! nor a first step gets `NO_STEP_VALUE`: nothing in the deck determines a value.
//!
//! Cites: R73, R13, R67; `rewrite/AMEND-A12.md` A41, A24; `crates/ir/src/tran.rs` (`Tran`).
//!
//! Contents: `pub fn tran_first_step_to_print(&mut ir::Design, &mut ir::Diags)`,
//! `pub const FIRST_STEP_MOVED: ir::Code`, `pub const FIRST_STEP_KEPT: ir::Code`,
//! `pub const NO_STEP_VALUE: ir::Code`

use super::names_dropped::of_kind;
use crate::ir;
use ir::tran::{StepCeiling, Tran};

/// The note at a transient whose first step became its print step: `arg` is its `ExprId`.
pub const FIRST_STEP_MOVED: ir::Code = ir::Code::new(b'U', 419);
/// The note at a transient whose first step cannot move.
pub const FIRST_STEP_KEPT: ir::Code = ir::Code::new(b'U', 440);
/// The note at a transient with neither a print step nor a first step.
pub const NO_STEP_VALUE: ir::Code = ir::Code::new(b'U', 441);

/// Moves every reachable transient's first step into its absent print step where nothing else
/// reads it; notes every other with a first step or no print step.
pub fn tran_first_step_to_print(design: &mut ir::Design, diags: &mut ir::Diags) {
    let tstep = ir::features_used(design).contains(ir::Feature::Expr(ir::ExprTag::TStep));
    for item in of_kind(design, ir::ItemKind::Analysis) {
        let a = design.row::<ir::Analysis>(item.index);
        if a.kind != ir::AnalysisKind::Tran {
            continue;
        }
        let mut t = design.detail::<Tran>(a.detail);
        let (first, print) = (!t.initial_step.is_none(), !t.print_step.is_none());
        let given = matches!(t.ceiling, StepCeiling::Given(_));
        match (first, print) {
            (false, true) => continue,
            (false, false) => diags.push_at(NO_STEP_VALUE, design, item, 0),
            (true, true) => diags.push_at(FIRST_STEP_KEPT, design, item, 0),
            (true, false) if !given || tstep => diags.push_at(FIRST_STEP_KEPT, design, item, 0),
            (true, false) => {
                diags.push_at(FIRST_STEP_MOVED, design, item, t.initial_step.0);
                t.print_step = t.initial_step;
                t.initial_step = ir::ExprId::NONE;
                design.editor().set_detail(a.detail, t);
                let moved = design.detail::<Tran>(a.detail);
                assert!(moved.initial_step.is_none(), "the first step went");
                assert!(!moved.print_step.is_none(), "into the print step");
            }
        }
    }
}
