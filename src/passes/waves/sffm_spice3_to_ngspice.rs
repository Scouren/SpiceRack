//! Pass `sffm_spice3_to_ngspice`: a design in, the same design out with every SFFM under
//! SPICE3's law (`SffmLaw::Spice3`, the index as given) written under ngspice's (`Ngspice`,
//! the index limited to `[0, carrier/signal]`) where the limit changes nothing: the index,
//! carrier and signal are literals with `0 ≤ index ≤ carrier/signal` (as `wave_eval` divides
//! it). Then both laws compute one formula: exact, bit for bit. Any other SFFM (an index ngspice
//! would clip, a field that is no literal, or ngspice's zero rule) keeps its law with note
//! `U0204` (a behavioural source is its fallback, `wave_to_behavioural`).
//!
//! Removes `Circuit(SffmSpice3)`, requires `Circuit(SffmNgspice)`. One row.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2 (`sffm_swap`); `crates/ir/src/wave.rs` (`SffmLaw`).
//!
//! Contents: `pub fn sffm_spice3_to_ngspice(&mut ir::Design, &mut ir::Diags)`

use super::SFFM_LAW_LEFT;
use super::nodes::{literal, note_left, sffms};
use crate::ir;
use crate::ir::wave::{GivenZero, Sffm, SffmLaw};

/// Writes every SPICE3 SFFM ngspice's limit leaves alone under ngspice's law; notes the rest.
pub fn sffm_spice3_to_ngspice(design: &mut ir::Design, diags: &mut ir::Diags) {
    for index in sffms(design) {
        let s: Sffm = design.detail(index);
        if s.law != SffmLaw::Spice3 || s.given_zero != GivenZero::AsGiven {
            continue;
        }
        let get = |e| literal(design, e);
        let (Some(mdi), Some(fc), Some(fs)) =
            (get(s.mod_index), get(s.carrier_freq), get(s.signal_freq))
        else {
            continue;
        };
        if !(mdi >= 0.0 && mdi <= fc / fs) {
            continue;
        }
        let row = Sffm {
            law: SffmLaw::Ngspice,
            ..s
        };
        design.editor().set_detail(index, row);
    }
    note_left(
        design,
        diags,
        ir::Feature::Circuit(ir::CircuitExtra::SffmSpice3),
        SFFM_LAW_LEFT,
    );
}
