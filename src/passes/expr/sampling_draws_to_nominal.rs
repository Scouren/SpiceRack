//! Pass `sampling_draws_to_nominal`: a design in, the same design out with every draw that
//! samples only in a sampling run (`DrawWhen::Sampling`) replaced by its nominal argument (the
//! first: every `Dist` is centred on it), when the design runs no sampling loop (no
//! `MonteCarlo` loop): there each such draw reads its nominal. A design that samples keeps
//! them with note `U0110`. The nominal is evaluated once, as the draw evaluated it; the other
//! arguments are no longer read (R12: a spread outside its domain is not carried). A draw is
//! Real: a nominal that may be Int is written `1 * nominal` while a typed division stands. One row: a
//! draw's `when` is one feature in both contexts.
//!
//! Removes `DrawWhen(Sampling)`, requires `Binary(Mul)` and `BinaryDyn(Mul)`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2 (RUN g2, `stat_nominal`); `crates/ir/src/expr.rs`
//! (`DrawWhen::Sampling`).
//!
//! Contents: `pub fn sampling_draws_to_nominal(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_SAMPLES;
use super::nodes::{note_left, plain, reads_tags, samples};
use super::tags::{REAL, Tags};
use crate::ir;

/// Replaces every `Sampling` draw by its nominal when the design runs no sampling loop.
pub fn sampling_draws_to_nominal(design: &mut ir::Design, diags: &mut ir::Diags) {
    if !samples(design) {
        let untyped = !reads_tags(design);
        let mut tags = Tags::new(design);
        ir::rewrite_exprs(design, &mut |b, e| {
            tags.of(b.design(), e);
            let ir::Expr::Draw(_, ir::DrawWhen::Sampling, _, args) = b.design().expr(e) else {
                return None;
            };
            let nominal = b.design().exprs(args)[0];
            if untyped || tags.of(b.design(), nominal) == REAL {
                return Some(nominal);
            }
            let one = plain(b, 1, 0);
            let m = b.binary(ir::BinOp::Mul, one, nominal);
            tags.of(b.design(), m);
            Some(m)
        });
    }
    let feature = ir::Feature::DrawWhen(ir::DrawWhen::Sampling);
    note_left(design, diags, feature, DRAW_SAMPLES);
}
