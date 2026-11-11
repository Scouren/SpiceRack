//! The testbench's portable options on a target other than Xyce, and a
//! deck's default temperatures made explicit where the target's differ.
//!
//! HSPICE's TNOM and TEMP default to 25 °C, ngspice's, LTspice's and
//! Xyce's to 27 °C (ngspice manual §11.1.1; Xyce RG `.OPTIONS DEVICE`):
//! a running HSPICE deck that sets neither gets them written (ngspice-45
//! on `tnom_default.sp` with both at 25: v(a) = 0.785904, ESPice's HSPICE
//! reading of the source; 0.791142 at 27), and a running deck of another
//! dialect gets 27 written for HSPICE (cmdref `.OPTION TNOM`, `.TEMP`;
//! 1 mA into a diode, IS = 1e-14: ngspice-45 gives v(a) = 0.655118,
//! ESPice's HSPICE reading 0.650753 without the two cards and 0.655118
//! with them). On ngspice, HSPICE's GSHUNT
//! (a conductance from every node to ground) is RSHUNT, its inverse
//! (ngspice manual §11.1.2); GMINDC has no ngspice option (ngspice reads
//! and ignores it: `gmindc.sp` gives 1.01e-12 A for 1e-3), refused W0017.
//! The other way, a deck's one GMIN is HSPICE's GMIN (transient) and
//! GMINDC (DC) both (cmdref `.OPTION GMIN`, `.OPTION GMINDC`).
//!
//! Contents:
//! - `W::hspice_defaults(&self, tb: &mut Tb, runs: bool)`, `W::options_line(&mut self, tb: &Tb) -> Option<String>`

use super::W;
use super::bench::Tb;
use crate::syntax::diag::Code;

impl W<'_, '_> {
    /// The source simulator's default TNOM and TEMP written where the
    /// target's differ and a running deck sets neither: HSPICE's 25 °C for
    /// a target defaulting to 27, and 27 °C for HSPICE.
    pub(super) fn hspice_defaults(&self, tb: &mut Tb, runs: bool) {
        let own = match (self.pair.hspice_elsewhere, self.pair.hspice_from_other) {
            (true, _) => 25.0,
            (_, true) => 27.0,
            (false, false) => return,
        };
        if !runs {
            return;
        }
        // `.option temp=` and a `.option tnom=` that is an expression are
        // portable options, not a `Temp` or a `Tnom`.
        let set = |k: &str| {
            tb.portable
                .iter()
                .any(|(p, _, _)| p.eq_ignore_ascii_case(k))
        };
        if !set("temp") {
            tb.temp.get_or_insert(own);
        }
        if !set("tnom") {
            tb.tnom.get_or_insert(own);
        }
    }

    /// The `.options` card of the portable options, `None` for none.
    pub(super) fn options_line(&mut self, tb: &Tb) -> Option<String> {
        let hspice = self.pair.hspice_options_on_ngspice;
        let mut opts: Vec<String> = Vec::with_capacity(tb.portable.len());
        for (k, v, tok) in &tb.portable {
            let k = if k == "max_iterations" {
                "ITL1"
            } else {
                k.as_str()
            };
            let lower = k.to_ascii_lowercase();
            // HSPICE's SEARCH directories for `.include`/`.lib`: the kit
            // reads such a file under the path its card names
            // (`resolve::Kit::load`), so the written kit holds it there and
            // the option says nothing more (`search_lib.sp`: real ngspice
            // on the written files gives its 0.75 V).
            if self.pair.hspice_elsewhere && lower == "search" {
                self.note(Code::W0024, *tok);
                continue;
            }
            // LTspice has no SCALE (26.1.1: `.options scale=1u` is a syntax
            // error) and takes every L and W as written.
            if self.pair.ltspice_from_other && lower == "scale" {
                self.refuse(Code::W0017, *tok);
                continue;
            }
            if hspice && lower == "gmindc" {
                self.refuse(Code::W0017, *tok);
                continue;
            }
            if hspice && lower == "gshunt" {
                let g = crate::syntax::number::value(v, self.sp.lexicon);
                match g > 0.0 {
                    true => opts.push(format!("rshunt={}", self.sp.number(1.0 / g, self.x))),
                    false => self.refuse(Code::W0017, *tok),
                }
                continue;
            }
            // HSPICE's GMIN is the conductance on a junction in a transient
            // and GMINDC the one in DC (cmdref `.OPTION GMIN`, `.OPTION
            // GMINDC`); the other dialects' GMIN is both.
            if self.pair.hspice_from_other && lower == "gmin" && !v.is_empty() {
                opts.push(format!("{k}={v} gmindc={v}"));
                continue;
            }
            opts.push(if v.is_empty() {
                k.to_string()
            } else {
                format!("{k}={v}")
            });
        }
        assert!(
            opts.len() <= tb.portable.len(),
            "one option per portable option at most"
        );
        (!opts.is_empty()).then(|| format!(".options {}", opts.join(" ")))
    }
}
