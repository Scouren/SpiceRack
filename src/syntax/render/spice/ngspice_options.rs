//! The testbench's portable options on a target other than Xyce, and an
//! HSPICE deck's defaults made explicit where the target's differ.
//!
//! HSPICE's TNOM and TEMP default to 25 °C, ngspice's, LTspice's and
//! Xyce's to 27 °C (ngspice manual §11.1.1; Xyce RG `.OPTIONS DEVICE`):
//! a running HSPICE deck that sets neither gets them written (ngspice-45
//! on `tnom_default.sp` with both at 25: v(a) = 0.785904, ESPice's HSPICE
//! reading of the source; 0.791142 at 27). On ngspice, HSPICE's GSHUNT
//! (a conductance from every node to ground) is RSHUNT, its inverse
//! (ngspice manual §11.1.2); GMINDC has no ngspice option (ngspice reads
//! and ignores it: `gmindc.sp` gives 1.01e-12 A for 1e-3), refused W0017.
//!
//! Contents:
//! - `W::hspice_defaults(&self, tb: &mut Tb, runs: bool)`, `W::options_line(&mut self, tb: &Tb) -> Option<String>`

use super::W;
use super::bench::Tb;
use crate::syntax::behaviour::{ControlBlock, NetlistRules};
use crate::syntax::diag::Code;

impl W<'_, '_> {
    /// HSPICE's TNOM = TEMP = 25 °C written for a target defaulting to 27,
    /// where a running HSPICE deck sets neither.
    pub(super) fn hspice_defaults(&self, tb: &mut Tb, runs: bool) {
        let hspice = |r: NetlistRules| r == NetlistRules::Hspice;
        if !hspice(self.rd.b.netlist_rules) || hspice(self.b.netlist_rules) || !runs {
            return;
        }
        // `.option tnom=27` is a portable option, not a `Tnom`.
        let set = |k: &str| {
            tb.portable
                .iter()
                .any(|(p, _, _)| p.eq_ignore_ascii_case(k))
        };
        if !set("temp") {
            tb.temp.get_or_insert(25.0);
        }
        if !set("tnom") {
            tb.tnom.get_or_insert(25.0);
        }
    }

    /// The `.options` card of the portable options, `None` for none.
    pub(super) fn options_line(&mut self, tb: &Tb) -> Option<String> {
        let from_hspice = self.rd.b.netlist_rules == NetlistRules::Hspice;
        let hspice = from_hspice && self.b.control_block == ControlBlock::Ngspice;
        let mut opts: Vec<String> = Vec::with_capacity(tb.portable.len());
        for (k, v, tok) in &tb.portable {
            let k = if k == "max_iterations" {
                "ITL1"
            } else {
                k.as_str()
            };
            let lower = k.to_ascii_lowercase();
            // HSPICE's SEARCH directories for `.include`/`.lib`: ngspice
            // has no such option (manual §11.1), so the files would not be
            // found (`search_lib.sp`: "Could not find include file").
            if from_hspice && self.b.netlist_rules != NetlistRules::Hspice && lower == "search" {
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
            opts.push(if v.is_empty() {
                k.to_string()
            } else {
                format!("{k}={v}")
            });
        }
        (!opts.is_empty()).then(|| format!(".options {}", opts.join(" ")))
    }
}
