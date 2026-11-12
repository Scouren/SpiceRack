//! Pass `draws_per_reference_to_call_site`: a design in, the same design out with every draw
//! that redraws at each reference (`DrawPer::Reference`) relabelled once per call site
//! (`CallSite`), where that changes no distribution: the draw samples only in a sampling run
//! (`DrawWhen::Sampling`) and the design runs none (no `MonteCarlo` loop), so every reference
//! reads its nominal either way. Any other such draw keeps its label with note `U0110` (R22:
//! where references see different samples, per-reference and per-call-site are two joint
//! distributions). One row: a draw's `per` is one feature in both contexts.
//!
//! Removes `DrawPer(Reference)`, requires `DrawPer(CallSite)`.
//!
//! Cites: `rewrite/RULINGS.md` R22; `rewrite/DIALECT-VACASK.md` §6.1.
//!
//! Contents: `pub fn draws_per_reference_to_call_site(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_SAMPLES;
use super::nodes::{note_left, samples};
use crate::ir;

/// Relabels every `Sampling` per-reference draw per call site when the design runs no
/// sampling loop; notes every item left holding a per-reference draw.
pub fn draws_per_reference_to_call_site(design: &mut ir::Design, diags: &mut ir::Diags) {
    if !samples(design) {
        ir::rewrite_exprs(design, &mut |b, e| {
            let ir::Expr::Draw(dist, when @ ir::DrawWhen::Sampling, ir::DrawPer::Reference, l) =
                b.design().expr(e)
            else {
                return None;
            };
            let args = b.design().exprs(l).to_vec();
            Some(b.draw(dist, when, ir::DrawPer::CallSite, &args))
        });
    }
    let feature = ir::Feature::DrawPer(ir::DrawPer::Reference);
    note_left(design, diags, feature, DRAW_SAMPLES);
}
