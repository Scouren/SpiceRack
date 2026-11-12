//! Pass `pulse_spice_to_vacask`: a design in, the same design out with every SPICE-law pulse
//! (`PulseLaw::Spice`) relabelled VACASK's (`PulseLaw::Vacask`) where the two laws give one
//! value at every instant (`wave_eval`'s two laws, A30): the zero rule is `AsGiven`, and
//! `rise`, `fall`, `width` and `period` are literals with `rise > 0`, `fall > 0` and
//! `period > rise + width + fall`. Then both laws are `v1` up to the delay and at every
//! period start (SPICE's `τ ≥ rise + width + fall` is VACASK's last branch, never reached
//! inside a period only when the period is longer), the same ramps and the same flat top, and
//! the same count of pulses. SPICE wraps `τ` as `τ − period·⌊τ/period⌋` and VACASK as
//! `fmod(τ, period)`: one number, rounded by two formulas; measured in `tests/passes_waves.rs`,
//! the value moves by at most 1.007·ulp(t)·slope (the steepest edge), bit for bit elsewhere.
//! Any other pulse keeps its law with note `U0201`: a zero fall (VACASK: no falling edge), a
//! period of zero (VACASK: one pulse) or of exactly `rise + width + fall` (RUN q13), a field
//! that is no literal, or ngspice's zero rule (its own pass makes it `AsGiven` first).
//!
//! Removes `Circuit(PulseSpice)`, requires `Circuit(PulseVacask)`. Its inverse row,
//! `pulse_vacask_to_spice`, relabels a VACASK pulse SPICE's under the same condition (the two
//! laws agree on it either way) and notes every other with the same code; it removes
//! `Circuit(PulseVacask)` and requires `Circuit(PulseSpice)`. A wave law has no form.
//!
//! Cites: `rewrite/AMEND-A12.md` C1, A30; `rewrite/DIALECT-VACASK.md` §6.1 (RUN p1, p2, q13);
//! `crates/ir/src/wave.rs` (`PulseLaw`).
//!
//! Contents: `pub fn pulse_spice_to_vacask(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn pulse_vacask_to_spice(&mut ir::Design, &mut ir::Diags)`

use super::PULSE_LAWS_DIFFER;
use super::nodes::{literal, note_left, sources};
use crate::ir;
use crate::ir::wave::{GivenZero, Pulse, PulseLaw};

/// Relabels every SPICE pulse VACASK's law reproduces; notes every other.
pub fn pulse_spice_to_vacask(design: &mut ir::Design, diags: &mut ir::Diags) {
    relabel(design, (PulseLaw::Spice, PulseLaw::Vacask));
    let feature = ir::Feature::Circuit(ir::CircuitExtra::PulseSpice);
    note_left(design, diags, feature, PULSE_LAWS_DIFFER);
}

/// Relabels every VACASK pulse SPICE's law reproduces; notes every other.
pub fn pulse_vacask_to_spice(design: &mut ir::Design, diags: &mut ir::Diags) {
    relabel(design, (PulseLaw::Vacask, PulseLaw::Spice));
    let feature = ir::Feature::Circuit(ir::CircuitExtra::PulseVacask);
    note_left(design, diags, feature, PULSE_LAWS_DIFFER);
}

// ---- private ----

/// Every reachable pulse of law `laws.0` on which the two laws agree, relabelled `laws.1`.
fn relabel(design: &mut ir::Design, laws: (PulseLaw, PulseLaw)) {
    assert!(laws.0 != laws.1, "a relabel changes the law");
    let rows: Vec<u32> = sources(design)
        .into_iter()
        .filter(|(_, s)| !s.wave.is_none() && s.wave.kind == ir::WaveKind::Pulse)
        .map(|(_, s)| s.wave.index)
        .collect();
    for index in rows {
        let pulse: Pulse = design.detail(index);
        if pulse.law == laws.0 && agree(design, &pulse) {
            let new = Pulse {
                law: laws.1,
                ..pulse
            };
            design.editor().set_detail(index, new);
        }
    }
}

/// Whether the two laws agree on `pulse` at every instant (the module header's condition).
fn agree(design: &ir::Design, pulse: &Pulse) -> bool {
    if pulse.given_zero != GivenZero::AsGiven {
        return false;
    }
    let get = |e| literal(design, e);
    let (Some(rise), Some(fall), Some(width), Some(period)) = (
        get(pulse.rise),
        get(pulse.fall),
        get(pulse.width),
        get(pulse.period),
    ) else {
        return false;
    };
    // SPICE's own sum (`τ ≥ rise + width + fall`), in its order
    rise > 0.0 && fall > 0.0 && width >= 0.0 && period > rise + width + fall
}
