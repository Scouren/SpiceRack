//! Pass `am_pre_hold_to_zero`: a design in, the same design out with every AM that holds its
//! t = delay value before the delay (`PreDelay::HoldDelayValue`, VACASK: VS V32 runs) written
//! as ngspice's, zero before the delay (`PreDelay::Zero`, DIALECT-NGSPICE "SFFM, AM": 0 until
//! the delay, at the operating point too), where the two are one wave. They differ only for
//! t ≤ delay and at the operating point (`wave_eval::am`: after the delay both evaluate the same
//! formula), where the first is the value at τ = 0, `offset + amplitude·(mod_offset + index·
//! sin(phase_m))·sin(phase_c)`, and the second 0. So the rule is exact when that value is +0:
//! `offset` is the literal +0 (a −0 offset gives −0 where ngspice gives +0) and `phase_c_deg` is
//! the literal 0 (or omitted under ngspice's zero rule: 0), or `amplitude` the literal 0. Any
//! other AM keeps its rule with note `U0206`: a VACASK AM with a nonzero `sinedc` or a carrier
//! phase is not zero before its delay, which ngspice's AM cannot state.
//!
//! One widening: a field that has no value (a domain error) is read at τ = 0 by the first rule
//! and not before the delay by the second, so an error there may become the value 0.
//!
//! Removes `Circuit(AmPreHold)`, requires `Circuit(AmPreZero)`. One row: a wave law has no form.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §3 (`Am.pre`); `rewrite/DIALECT-VACASK.md` §3 (am);
//! `rewrite/DIALECT-NGSPICE.md` (SFFM, AM); `crates/ir/src/wave_eval.rs` (`am`).
//!
//! Contents: `pub fn am_pre_hold_to_zero(&mut ir::Design, &mut ir::Diags)`

use super::AM_PRE_LEFT;
use super::nodes::{literal, note_left, sources};
use crate::ir;
use crate::ir::wave::{Am, GivenZero, PreDelay};

/// Writes every AM that is +0 at its delay as zero before it; notes every other holding one.
pub fn am_pre_hold_to_zero(design: &mut ir::Design, diags: &mut ir::Diags) {
    let rows: Vec<u32> = sources(design)
        .into_iter()
        .filter(|(_, s)| !s.wave.is_none() && s.wave.kind == ir::WaveKind::Am)
        .map(|(_, s)| s.wave.index)
        .collect();
    for index in rows {
        let am: Am = design.detail(index);
        if am.pre == PreDelay::HoldDelayValue && zero_at_delay(design, &am) {
            let zero = Am {
                pre: PreDelay::Zero,
                ..am
            };
            design.editor().set_detail(index, zero);
        }
    }
    let feature = ir::Feature::Circuit(ir::CircuitExtra::AmPreHold);
    note_left(design, diags, feature, AM_PRE_LEFT);
}

// ---- private ----

/// Whether the AM's value at τ = 0 is +0 by its literals (the module header's condition).
fn zero_at_delay(design: &ir::Design, am: &Am) -> bool {
    let plus_zero = |e| literal(design, e).is_some_and(|v| v == 0.0 && v.is_sign_positive());
    let no_phase = literal(design, am.phase_c_deg) == Some(0.0)
        || (am.phase_c_deg.is_none() && am.given_zero == GivenZero::SpiceDefault);
    let silent = literal(design, am.amplitude) == Some(0.0);
    plus_zero(am.offset) && (no_phase || silent)
}
