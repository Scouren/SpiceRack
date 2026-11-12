//! `am_pre_hold_to_zero`'s proof, with the oracle of `tests/passes_waves.rs`
//! (`ir::source_value` before and after, at the operating point and the delay's instants).
//!
//! Contents: tests only.

use spicerack::ir;

use super::{Tally, compare, deck, generated, sources_only};

// ---- private ----

/// An AM that holds its delay value before the delay, written as ngspice's zero before it:
/// drawn AMs (offset +0, -0 or 0.5; carrier phase 0, 45 or 90; amplitude 0 or 1; delay 0 or
/// past 0), each compared at the operating point and at the delay's boundary instants when the
/// rule converts it; generated ones. Bit for bit.
#[test]
fn am_pre_hold_to_zero_keeps_every_value() {
    let mut tally = Tally::default();
    for offset in ["0", "-0", "0.5"] {
        for phase in ["0", "45", "90"] {
            for amplitude in ["0", "1"] {
                for delay in ["0", "3n", "1u"] {
                    let text = format!(
                        "file \"p.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=am(offset={offset} amplitude={amplitude} mod_offset=1 mod_index=0.5 mod_freq=1meg carrier_freq=10meg delay={delay} phase_m_deg=30 phase_c_deg={phase} pre=hold_delay_value)\n"
                    );
                    compare("am_pre_hold_to_zero", deck(&text), &mut tally, None);
                }
            }
        }
    }
    let zero = ir::Feature::Circuit(ir::CircuitExtra::AmPreZero);
    let own = sources_only(&[ir::WaveKind::Am], &[zero]);
    generated("am_pre_hold_to_zero", own, &mut tally, None);
    eprintln!(
        "am_pre_hold_to_zero: {} sources, {} instants, worst {} ulp",
        tally.sources, tally.points, tally.worst
    );
    assert_eq!(tally.worst, 0, "zero at the delay either way");
    // +0 offset with phase 0 (3 delays x 2 amplitudes) or amplitude 0 (3 delays x 2 phases)
    assert!(tally.sources >= 12, "{} converted", tally.sources);
}
