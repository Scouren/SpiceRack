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
//! The inverse row also writes VACASK's single pulses (`period ≤ 0`, VACASK runs once.sim and
//! step.sim in `tests/fixtures/passes/waves/pulse_vacask_to_spice/`: a pulse with no period
//! rises, holds, falls once and stays at `val0`; with no fall either it rises and holds `val1`
//! to the stop) as SPICE pulses equal to them up to the design's last transient stop, where
//! every value is read. With τ_max = stop − delay (every stop and the delay literals, τ_max >
//! 0): a pulse with a fall keeps its fields and gets `period = 2·τ_max + rise + width + fall`
//! (no wrap before the stop, longer than one pulse); a step (`fall ≤ 0`) gets `width = fall =
//! 2·τ_max` and the same period (its fall is never reached). Every number is the deck's,
//! derived, never chosen (R13); past the last stop the two differ, and no analysis reads there.
//! A design with no transient, a stop or a field that is no literal, or a sum that rounds onto
//! τ_max keeps the pulse with `U0201`.
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

/// Relabels every VACASK pulse SPICE's law reproduces, and writes a VACASK single pulse or step
/// as a SPICE pulse that does not repeat before the design's last stop; notes every other.
pub fn pulse_vacask_to_spice(design: &mut ir::Design, diags: &mut ir::Diags) {
    relabel(design, (PulseLaw::Vacask, PulseLaw::Spice));
    if let Some(stop) = last_stop(design) {
        once(design, stop);
    }
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

/// The largest stop of the design's reachable transients; `None` when there is none or one
/// is no literal.
fn last_stop(design: &ir::Design) -> Option<f64> {
    let mut last: Option<f64> = None;
    for (item, _) in design.reachable() {
        if item.kind != ir::ItemKind::Analysis {
            continue;
        }
        let a = design.row::<ir::Analysis>(item.index);
        if a.kind != ir::AnalysisKind::Tran {
            continue;
        }
        let stop = literal(design, design.detail::<ir::tran::Tran>(a.detail).stop)?;
        last = Some(last.map_or(stop, |l| l.max(stop)));
    }
    last
}

/// Every VACASK pulse with no period written as a SPICE pulse equal to it up to `stop` (the
/// module header's construction), where its literals allow.
fn once(design: &mut ir::Design, stop: f64) {
    let rows: Vec<u32> = sources(design)
        .into_iter()
        .filter(|(_, s)| !s.wave.is_none() && s.wave.kind == ir::WaveKind::Pulse)
        .map(|(_, s)| s.wave.index)
        .collect();
    for index in rows {
        let p: Pulse = design.detail(index);
        let get = |e| literal(design, e);
        let fields = (
            get(p.delay),
            get(p.rise),
            get(p.fall),
            get(p.width),
            get(p.period),
        );
        let (Some(delay), Some(rise), Some(fall), Some(width), Some(period)) = fields else {
            continue;
        };
        let vacask = p.law == PulseLaw::Vacask && p.given_zero == GivenZero::AsGiven;
        let tau = stop - delay;
        if !vacask || !p.cycles.is_none() || period > 0.0 || rise <= 0.0 || tau <= 0.0 {
            continue;
        }
        // a step holds val1 past the stop: its flat top and its fall outlast τ_max (2·τ_max
        // is exact, and rise + 2·τ_max > τ_max)
        let step = fall <= 0.0;
        let (width, fall) = match step {
            true => (2.0 * tau, 2.0 * tau),
            false => (width, fall),
        };
        let period = 2.0 * tau + rise + width + fall;
        // no wrap before the stop (SPICE wraps for τ > period), and a period longer than one
        // pulse; checked in the doubles the simulator computes with
        if width < 0.0 || period <= tau || period <= rise + width + fall {
            continue;
        }
        let mut editor = design.editor();
        let mut b = editor.builder();
        let mut num = |v: f64| b.literal(ir::Literal::from_f64(v));
        let spice = Pulse {
            width: if step { num(width) } else { p.width },
            fall: if step { num(fall) } else { p.fall },
            period: num(period),
            law: PulseLaw::Spice,
            ..p
        };
        editor.set_detail(index, spice);
    }
}
