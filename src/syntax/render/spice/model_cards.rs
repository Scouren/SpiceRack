//! A block's `.model` cards: each as the target reads it (on Xyce as its
//! device reads it, `xyce_models`, its R model's multiplier; a Xyce card
//! as ngspice, ESPice or HSPICE read it, `ngspice_models`; ngspice's
//! built-in device names).
//!
//! Cites: Xyce RG Table 2-36 (BSIM3 level 49), resistor model `R`; ngspice
//! manual §2.3 (`.model`).
//!
//! Contents:
//! - `W::models(&mut self, b: &Block)`, `W::model(&mut self, n: u32, bin: u32) -> bool`

use super::{W, ngspice_models, values, xyce_models};
use crate::syntax::behaviour::{LoadForm, ModelSemantics, NetlistRules, ParamForm};
use crate::syntax::diag::Code;
use crate::syntax::render::out::Space;
use crate::syntax::render::read::Block;

impl W<'_, '_> {
    /// Each of block `b`'s `.model` cards, a blank line after each.
    pub(super) fn models(&mut self, b: &Block) {
        for &(m, bin) in &b.models {
            if self.model(m, bin) {
                self.o.token("", Space::Line);
            }
        }
    }

    /// `.model name kind (k=v …)`, on Xyce as its device reads it
    /// (`xyce_models`; one it cannot compute is left out, W0053, and false
    /// returned: nothing written), a Xyce card on ngspice, ESPice or HSPICE
    /// as that device reads it (`ngspice_models`; a level the target has no
    /// device for is refused, W0104). The parameter list is separated from the
    /// type by a space: ngspice-45 drops the parameters of an OSDI model
    /// written `vres(r=3k)`.
    pub(super) fn model(&mut self, n: u32, bin: u32) -> bool {
        let mut m = self.rd.model_view(n, bin);
        if self.b.param_form == ParamForm::Xyce {
            let ngspice = self.rd.b.model_semantics == ModelSemantics::Ngspice;
            match xyce_models::xyce_model(ngspice, &m.kind, &m.params) {
                Ok(Some(params)) => m.params = params,
                Ok(None) => {}
                // Left out: an instance of it is refused (`xyce_cards`).
                Err(_) => {
                    self.note(Code::W0053, self.rd.main(n));
                    return false;
                }
            }
        }
        // A Xyce card on ngspice, ESPice or HSPICE: its level as the target
        // numbers it (`ngspice_models`).
        if self.pair.xyce_models_on_ngspice {
            let hspice = self.b.netlist_rules == NetlistRules::Hspice;
            match ngspice_models::ngspice_model(hspice, &m.kind, &m.params) {
                Ok(Some(params)) => m.params = params,
                Ok(None) => {}
                Err(code) => self.refuse(code, self.rd.main(n)),
            }
        }
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
        let mut line = format!(".model {} {}", m.name, m.kind);
        // Xyce's R model `R` is a multiplier: the instance takes the
        // value (`W::r_default`).
        let multiplier =
            self.pair.r_model_multiplies && values::r_value(&m.kind, &m.params).is_some();
        let params: Vec<&(String, String)> = m
            .params
            .iter()
            .filter(|(k, _)| {
                !(multiplier
                    && (k.eq_ignore_ascii_case("r") || k.eq_ignore_ascii_case("resistance")))
            })
            .collect();
        for (i, (k, v)) in params.iter().enumerate() {
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
        true
    }
}
