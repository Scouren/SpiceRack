//! Pass `sffm_ngspice_to_spice3`: a design in, the same design out with every SFFM under
//! ngspice's law (`SffmLaw::Ngspice`: the modulation index limited to `[0, carrier/signal]`)
//! written under SPICE3's (`Spice3`: the index as given), the limit applied to the literal:
//! `wave_eval` computes the same formula after the limit, so an index already inside it is
//! kept and one outside becomes the literal of the bound (`carrier/signal` as the evaluator
//! divides it, or 0), which reads back as that double. Exact, bit for bit. The index, carrier
//! and signal must be literals and the zero rule `AsGiven` (its own pass first); otherwise the
//! wave keeps its law with note `U0204`.
//!
//! Removes `Circuit(SffmNgspice)`, requires `Circuit(SffmSpice3)`, `Expr(Num)`. One row.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1 (`sffm_slots`); `crates/ir/src/wave.rs` (`SffmLaw`);
//! `vsrcload.c:256-266` through `wave_eval`'s doc.
//!
//! Contents: `pub fn sffm_ngspice_to_spice3(&mut ir::Design, &mut ir::Diags)`

use super::SFFM_LAW_LEFT;
use super::nodes::{literal, note_left, sffms};
use crate::ir;
use crate::ir::wave::{GivenZero, Sffm, SffmLaw};

/// Writes every ngspice SFFM under SPICE3's law, its index limited; notes the rest.
pub fn sffm_ngspice_to_spice3(design: &mut ir::Design, diags: &mut ir::Diags) {
    for index in sffms(design) {
        let s: Sffm = design.detail(index);
        if s.law != SffmLaw::Ngspice || s.given_zero != GivenZero::AsGiven {
            continue;
        }
        let get = |e| literal(design, e);
        let (Some(mdi), Some(fc), Some(fs)) =
            (get(s.mod_index), get(s.carrier_freq), get(s.signal_freq))
        else {
            continue;
        };
        // wave_eval's limit, in its order
        let bound = fc / fs;
        let limited = if mdi > bound {
            Some(bound)
        } else if mdi < 0.0 {
            Some(0.0)
        } else {
            None
        };
        if limited.is_some_and(|v| !v.is_finite()) {
            continue;
        }
        let mut editor = design.editor();
        let mod_index = match limited {
            Some(v) => editor.builder().literal(ir::Literal::from_f64(v)),
            None => s.mod_index,
        };
        let row = Sffm {
            mod_index,
            law: SffmLaw::Spice3,
            ..s
        };
        editor.set_detail(index, row);
    }
    note_left(
        design,
        diags,
        ir::Feature::Circuit(ir::CircuitExtra::SffmNgspice),
        SFFM_LAW_LEFT,
    );
}
