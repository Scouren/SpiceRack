//! A device's initial condition where the source holds it in the
//! operating point and the target only under UIC.
//!
//! Cites: Xyce RG (C, L, Q, M `IC=`); ngspice manual §3.3.5, §3.3.9,
//! §15.2.2 (`.ic`).
//!
//! Contents:
//! - `W::device_ic(&mut self, letter: char, nodes: &[&String], params: &[(String, String)], tok: u32)`

use super::W;
use crate::syntax::diag::Code;

impl W<'_, '_> {
    /// A device's `IC=` where the source applies it in the operating point
    /// and the target only under UIC: Xyce holds `IC=` in its DCOP (RG
    /// `C`, `L`, `Q`, `M`: "the initial condition is applied … in the
    /// DCOP"), ngspice reads a device IC "only if the uic option is
    /// specified on the .tran line" (manual §3.3.5 C, §3.3.9 L, ch.7-11)
    /// and holds a node only by `.ic` (§15.2.2). A capacitor to ground is
    /// an `.ic` on its other node; any other is refused (W0049): ngspice's
    /// `.ic` holds node voltages, not a device's branch or terminal pair.
    pub(super) fn device_ic(
        &mut self,
        letter: char,
        nodes: &[&String],
        params: &[(String, String)],
        tok: u32,
    ) {
        let Some((_, v)) = params.iter().find(|(k, _)| k.eq_ignore_ascii_case("ic")) else {
            return;
        };
        if !self.dcop_ic {
            return;
        }
        let top = self.scope == crate::syntax::ast::NONE;
        match (letter, nodes) {
            ('C', [a, b]) if top && b.as_str() == "0" => {
                self.device_ics.push((a.to_string(), self.spelled(v)));
            }
            ('C', [a, b]) if top && a.as_str() == "0" => {
                let v = self.spelled(v);
                let neg = match v.parse::<f64>() {
                    Ok(x) => self.sp.decimal(-x, self.x),
                    Err(_) => format!("{{-({})}}", v.trim_matches(|c| c == '{' || c == '}')),
                };
                self.device_ics.push((b.to_string(), neg));
            }
            _ => self.refuse(Code::W0049, tok),
        }
    }
}
