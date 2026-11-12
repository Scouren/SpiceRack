//! Rule `tran_ceiling_given`: a transient's default step ceiling (`StepCeiling::Rule`) made the
//! deck's own (`StepCeiling::Given`) by the rule's arithmetic, so a target with another default
//! runs the same ceiling (A24, R47; `StepCap` docs; span = `stop − record_start`, `stop` when
//! `record_start` is `NONE`). One row per rule, each removing its feature and requiring
//! `Run(TranMaxStep)` and the operators its expression builds:
//! - `tran_cap_ngspice_to_given` (`TranCapNgspice`): `min(print_step, span / 50)`; a transient
//!   with no print step keeps the rule with `NO_PRINT_STEP` (the rule reads it);
//! - `tran_cap_span50_to_given` (`TranCapSpan50`): `span / 50`;
//! - `tran_cap_span10_to_given` (`TranCapSpan10`): `span / 10`;
//! - `tran_cap_none_to_given` (`TranCapNone`): `span` (no ceiling beyond the span).
//!
//! Exact: the simulator evaluates the same operations on the same doubles its default rule
//! does (ngspice `traninit.c`: `MIN(tstep, (tstop − tstart) / 50)`), and a given ceiling is
//! used as given. The span's subtraction is built only when `record_start` is set.
//!
//! Cites: `rewrite/AMEND-A12.md` A24; R47; `crates/ir/src/tran.rs` (`StepCeiling`, `StepCap`).
//!
//! Contents: `pub fn tran_cap_ngspice_to_given`, `pub fn tran_cap_span50_to_given`,
//! `pub fn tran_cap_span10_to_given`, `pub fn tran_cap_none_to_given` (each
//! `(&mut ir::Design, &mut ir::Diags)`), `pub const NO_PRINT_STEP: ir::Code`

use crate::ir;
use ir::tran::{StepCap, StepCeiling, Tran};

/// The note at a transient under ngspice's rule with no print step.
pub const NO_PRINT_STEP: ir::Code = ir::Code::new(b'U', 405);

/// `min(print_step, span / 50)` for ngspice's rule.
pub fn tran_cap_ngspice_to_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    given(design, diags, StepCap::NgspiceRule);
}

/// `span / 50` for Spectre's and VACASK's rule.
pub fn tran_cap_span50_to_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    given(design, diags, StepCap::SpanOver50);
}

/// `span / 10` for Xyce's rule.
pub fn tran_cap_span10_to_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    given(design, diags, StepCap::SpanOver10);
}

/// `span` for no ceiling beyond the span.
pub fn tran_cap_none_to_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    given(design, diags, StepCap::None);
}

// ---- private ----

/// Every reachable transient under `cap` given that rule's ceiling, or noted.
fn given(design: &mut ir::Design, diags: &mut ir::Diags, cap: StepCap) {
    let trans: Vec<(ir::ItemRef, u32)> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Analysis)
        .map(|(i, _)| (i, design.row::<ir::Analysis>(i.index)))
        .filter(|(_, a)| a.kind == ir::AnalysisKind::Tran)
        .map(|(i, a)| (i, a.detail))
        .collect();
    for (item, detail) in trans {
        let mut t = design.detail::<Tran>(detail);
        if t.ceiling != StepCeiling::Rule(cap) {
            continue;
        }
        if cap == StepCap::NgspiceRule && t.print_step.is_none() {
            diags.push_at(NO_PRINT_STEP, design, item, 0);
            continue;
        }
        let mut editor = design.editor();
        let mut b = editor.builder();
        let span = match t.record_start.is_none() {
            true => t.stop,
            false => b.binary(ir::BinOp::Sub, t.stop, t.record_start),
        };
        let ceiling = match cap {
            StepCap::None => span,
            StepCap::SpanOver10 => {
                let ten = b.num(10.0);
                b.binary(ir::BinOp::Div, span, ten)
            }
            StepCap::SpanOver50 | StepCap::NgspiceRule => {
                let fifty = b.num(50.0);
                let over = b.binary(ir::BinOp::Div, span, fifty);
                match cap {
                    StepCap::NgspiceRule => b.call(ir::Func::Min, &[t.print_step, over]),
                    _ => over,
                }
            }
        };
        assert!(!ceiling.is_none(), "a ceiling is an expression");
        t.ceiling = StepCeiling::Given(ceiling);
        editor.set_detail(detail, t);
    }
}
