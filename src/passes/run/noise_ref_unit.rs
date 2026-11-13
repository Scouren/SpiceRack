//! Rule `noise_ref_source_ac_to_unit`: a noise analysis whose input-referred noise is divided by
//! the gain to the input source's own AC magnitude (`NoiseRef::SourceAc`,
//! `RunExtra::NoiseRefSourceAc`, Xyce) made one divided by the gain to a unit input
//! (`NoiseRef::Unit`, ngspice), when that magnitude is the literal 1: the two divisors are then
//! the same number, and nothing else of the analysis reads the reference.
//!
//! Exact when: the source is a top-level element of the design with a `Source` row whose
//! `ac_mag` is a number literal equal to 1, and the design holds no loop or alter (either may
//! set that magnitude between runs; ponytail: any loop or alter refuses, a scan of their
//! targets for the source's `ac` when a deck needs it). Otherwise the analysis keeps its
//! reference with `NOISE_REF_KEPT`.
//!
//! Removes `Run(NoiseRefSourceAc)`, requires nothing (`Unit` carries no feature).
//!
//! Cites: `crates/ir/src/dc_ac.rs` (`NoiseRef`), `INV-semantics.md` 8.10; xyce-2's pair deck
//! `tests/fixtures/pair/xyce-ngspice/noise.cir` (`v1 in 0 dc 0 ac 1`).
//!
//! Contents: `pub fn noise_ref_source_ac_to_unit(&mut ir::Design, &mut ir::Diags)`,
//! `pub const NOISE_REF_KEPT: ir::Code`

use super::names_dropped::of_kind;
use crate::ir;
use ir::dc_ac::{Noise, NoiseRef};

/// The note at a noise analysis whose input source's AC magnitude is not surely 1.
pub const NOISE_REF_KEPT: ir::Code = ir::Code::new(b'U', 444);

/// Makes every reachable `SourceAc` noise analysis `Unit` where its source's AC magnitude is
/// the literal 1; notes every other.
pub fn noise_ref_source_ac_to_unit(design: &mut ir::Design, diags: &mut ir::Diags) {
    let runs_change = !of_kind(design, ir::ItemKind::Loop).is_empty()
        || !of_kind(design, ir::ItemKind::Alter).is_empty();
    for item in of_kind(design, ir::ItemKind::Analysis) {
        let a = design.row::<ir::Analysis>(item.index);
        if a.kind != ir::AnalysisKind::Noise {
            continue;
        }
        let mut n = design.detail::<Noise>(a.detail);
        if n.input_ref != NoiseRef::SourceAc {
            continue;
        }
        if runs_change || !unit_ac(design, n.source) {
            diags.push_at(NOISE_REF_KEPT, design, item, 0);
            continue;
        }
        n.input_ref = NoiseRef::Unit;
        design.editor().set_detail(a.detail, n);
        assert!(
            design.detail::<Noise>(a.detail).input_ref == NoiseRef::Unit,
            "the reference went"
        );
    }
}

// ---- private ----

/// Whether the top-level element `name` is a source whose AC magnitude is the literal 1.
fn unit_ac(design: &ir::Design, name: ir::NameId) -> bool {
    let mut found = design.reachable().into_iter().filter(|&(i, scope)| {
        i.kind == ir::ItemKind::Element
            && design.scope(scope).owner == ir::ItemRef::NONE
            && design.row::<ir::Element>(i.index).name == name
    });
    let (Some((item, _)), None) = (found.next(), found.next()) else {
        return false;
    };
    let e = design.row::<ir::Element>(item.index);
    if e.kind.detail() != ir::DetailKind::Source {
        return false;
    }
    let mag = design.source(&e).ac_mag;
    !mag.is_none()
        && matches!(design.expr(mag), ir::Expr::Num(k) if design.literal_value(design.num(k)) == 1.0)
}
