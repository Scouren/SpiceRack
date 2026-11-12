//! Pass `draws_always_to_sampling`: a design in, the same design out with every draw that
//! samples in every run (`DrawWhen::Always`, ngspice's `agauss` outside Monte Carlo) relabelled
//! to sample only in a sampling run (`Sampling`), with note `U0109` at each item it changed
//! (R22: a plain run then uses the nominal; a random plain-run value is the oracle of nothing).
//! In a sampling run the two are one distribution. Not an expression rule of one context: a
//! draw's `when` is one feature in both, so one row.
//!
//! Removes `DrawWhen(Always)`, requires `DrawWhen(Sampling)`.
//!
//! Cites: `rewrite/RULINGS.md` R22; `rewrite/DIALECT-VACASK.md` §6.1 (`VacaskCalls`).
//!
//! Contents: `pub fn draws_always_to_sampling(&mut ir::Design, &mut ir::Diags)`

use super::PLAIN_RUN_NOMINAL;
use super::nodes::carrying;
use crate::ir;

/// Relabels every `Always` draw `Sampling`, with a note at each item that held one.
pub fn draws_always_to_sampling(design: &mut ir::Design, diags: &mut ir::Diags) {
    let feature = ir::Feature::DrawWhen(ir::DrawWhen::Always);
    let held = carrying(design, feature);
    ir::rewrite_exprs(design, &mut |b, e| {
        let ir::Expr::Draw(dist, ir::DrawWhen::Always, per, args) = b.design().expr(e) else {
            return None;
        };
        let args = b.design().exprs(args).to_vec();
        Some(b.draw(dist, ir::DrawWhen::Sampling, per, &args))
    });
    for item in held {
        diags.push_at(PLAIN_RUN_NOMINAL, design, item, u32::from(feature.index()));
    }
}
