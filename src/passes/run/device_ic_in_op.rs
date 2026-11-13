//! Rule `device_ic_uic_to_in_op`: a device initial condition applied only under `uic`
//! (`IcMode::UicOnly`, `Circuit(IcUicOnly)`: ngspice, HSPICE) made one held in the operating
//! point (`IcMode::InOp`, `Circuit(IcInOp)`: Xyce, LTspice), when every analysis of the design
//! is a transient that skips the operating point (`uic`).
//!
//! The two modes differ only in an operating point: `UicOnly` ignores the condition there,
//! `InOp` holds it. A `uic` transient solves none and starts every storage element from its
//! condition under both; so when the design runs nothing else (no `op`, `dc`, `ac`, `noise`,
//! no transient without `uic`: each solves an operating point), no number read anywhere
//! depends on the mode. A design with no analysis reads no condition. Otherwise every element
//! keeps its mode with `IC_OP_READ` (all or nothing: one solved operating point reads every
//! condition). Xyce's runs (xyce-2): `device_ic_op.cir` (an OP holds the `ic`); the pair decks
//! `ngspice-xyce/tran_uic.sp`, `tran_uic_float.sp`.
//!
//! Removes `Circuit(IcUicOnly)`, requires `Circuit(IcInOp)`. Tried after
//! `device_ic_to_tran_ic`, the rule for a target with node conditions and no device one.
//!
//! Cites: `crates/ir/src/passive.rs` (`IcMode`), `semi.rs`; `crates/ir/src/tran.rs` (`uic`).
//!
//! Contents: `pub fn device_ic_uic_to_in_op(&mut ir::Design, &mut ir::Diags)`,
//! `pub const IC_OP_READ: ir::Code`

use super::names_dropped::of_kind;
use crate::ir;
use ir::passive::IcMode;
use ir::tran::Tran;

/// The note at an element whose `uic`-only condition an operating point of the design would
/// read on the target.
pub const IC_OP_READ: ir::Code = ir::Code::new(b'U', 445);

/// Makes every reachable element's `UicOnly` condition `InOp` when every analysis is a `uic`
/// transient; notes every such element otherwise.
pub fn device_ic_uic_to_in_op(design: &mut ir::Design, diags: &mut ir::Diags) {
    let only_uic = of_kind(design, ir::ItemKind::Analysis)
        .into_iter()
        .all(|item| {
            let a = design.row::<ir::Analysis>(item.index);
            a.kind == ir::AnalysisKind::Tran && design.detail::<Tran>(a.detail).uic
        });
    for item in of_kind(design, ir::ItemKind::Element) {
        let e = design.row::<ir::Element>(item.index);
        let held = match e.kind.detail() {
            ir::DetailKind::Passive => design.passive(&e).ic_mode == IcMode::UicOnly,
            ir::DetailKind::Semi => design.semi(&e).ic_mode == IcMode::UicOnly,
            _ => false,
        };
        if !held {
            continue;
        }
        if !only_uic {
            diags.push_at(IC_OP_READ, design, item, 0);
            continue;
        }
        if e.kind.detail() == ir::DetailKind::Passive {
            let mut p = design.passive(&e);
            p.ic_mode = IcMode::InOp;
            design.editor().set_detail(e.detail, p);
        } else {
            let mut s = design.semi(&e);
            s.ic_mode = IcMode::InOp;
            design.editor().set_detail(e.detail, s);
        }
    }
}
