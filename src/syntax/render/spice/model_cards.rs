//! A block's `.model` cards: each as the target reads it (Xyce's BSIM3
//! level, its R model's multiplier; ngspice's built-in device names).
//!
//! Cites: Xyce RG Table 2-36 (BSIM3 level 49), resistor model `R`; ngspice
//! manual §2.3 (`.model`).
//!
//! Contents:
//! - `W::models(&mut self, b: &Block)`, `W::model(&mut self, n: u32, bin: u32)`

use super::{W, values};
use crate::syntax::behaviour::{LoadForm, NetlistRules, ParamForm};
use crate::syntax::diag::Code;
use crate::syntax::render::out::Space;
use crate::syntax::render::read::Block;

impl W<'_, '_> {
    /// Each of block `b`'s `.model` cards, a blank line after each.
    pub(super) fn models(&mut self, b: &Block) {
        for &(m, bin) in &b.models {
            self.model(m, bin);
            self.o.token("", Space::Line);
        }
    }

    /// `.model name kind (k=v …)`. Xyce rejects BSIM3 as level 8 and reads
    /// 49. The parameter list is separated from the type by a space:
    /// ngspice-45 drops the parameters of an OSDI model written
    /// `vres(r=3k)`.
    pub(super) fn model(&mut self, n: u32, bin: u32) {
        let m = self.rd.model_view(n, bin);
        // A loaded module named as a built-in device: ngspice reads the
        // built-in (W0121's run).
        if self.b.load_form == LoadForm::Ngspice
            && self.rd.osdi_module(n)
            && self
                .b
                .builtin_devices
                .iter()
                .any(|d| d.eq_ignore_ascii_case(&m.kind))
        {
            self.refuse(Code::W0121, self.rd.main(n));
        }
        let mos = m.kind.eq_ignore_ascii_case("nmos") || m.kind.eq_ignore_ascii_case("pmos");
        let mut line = format!(".model {} {}", m.name, m.kind);
        // Xyce's R model `R` is a multiplier: the instance takes the
        // value (`W::r_default`).
        let multiplier = self.b.netlist_rules == NetlistRules::Xyce
            && self.rd.b.netlist_rules != NetlistRules::Xyce
            && values::r_value(&m.kind, &m.params).is_some();
        let params: Vec<&(String, String)> = m
            .params
            .iter()
            .filter(|(k, _)| {
                !(multiplier
                    && (k.eq_ignore_ascii_case("r") || k.eq_ignore_ascii_case("resistance")))
            })
            .collect();
        for (i, (k, v)) in params.iter().enumerate() {
            let v = if self.b.param_form == ParamForm::Xyce
                && mos
                && k.eq_ignore_ascii_case("level")
                && v.trim() == "8"
            {
                "49"
            } else {
                v
            };
            let open = if i == 0 { " (" } else { " " };
            if v.is_empty() {
                line.push_str(&format!("{open}{k}"));
            } else {
                line.push_str(&format!("{open}{k}={}", self.spelled(v)));
            }
        }
        if !params.is_empty() {
            line.push(')');
        }
        self.o.token(&line, Space::None);
    }
}
