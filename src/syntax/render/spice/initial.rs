//! A device's initial condition where the source holds it in the
//! operating point and the target only under UIC.
//!
//! Cites: Xyce RG (C, L, Q, M `IC=`); ngspice manual §3.3.5, §3.3.9,
//! §15.2.2 (`.ic`).
//!
//! Contents:
//! - `W::device_ic(&mut self, letter: char, nodes: &[&String], params: &[(String, String)], tok: u32)`
//! - `W::mos_ic(&mut self, n: u32, letter: char, master: &str, nodes: &[&String], params: &[(String, String)]) -> bool`

use super::W;
use crate::syntax::ast::NONE;
use crate::syntax::diag::Code;
use crate::syntax::render::read::Ana;

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

    /// A top-level MOSFET's `IC=Vds[,Vgs]` that Xyce holds in its DCOP (RG
    /// 7.10 `M`, `IC`: a BSIM3 or BSIM4 instance; Xyce 7.10 run: drain and
    /// gate sit at the values at t = 0), as the `.ic` of its drain and
    /// gate: exact when source and bulk are ground and the deck runs
    /// transients only (ngspice manual §15.2.2: `.ic` holds a node in the
    /// transient's operating point). `false`: not that case, the line is
    /// [`W::device_ic`]'s to judge. ngspice reads the `ic=` left on the
    /// line only under UIC (manual §7.6.1).
    pub(super) fn mos_ic(
        &mut self,
        n: u32,
        letter: char,
        master: &str,
        nodes: &[&String],
        params: &[(String, String)],
    ) -> bool {
        let Some((_, v)) = params.iter().find(|(k, _)| k.eq_ignore_ascii_case("ic")) else {
            return false;
        };
        let [d, g, s, b] = nodes else {
            return false;
        };
        let vals: Vec<&str> = v.split(',').map(str::trim).collect();
        let grounded = s.as_str() == "0" && b.as_str() == "0";
        let free = d.as_str() != "0" && g.as_str() != "0" && d != g;
        // `Vds,Vgs` as two plain values: a third (`Vbs`) contradicts the
        // grounded bulk, a call's comma is not a separator.
        let plain = vals.len() <= 2 && vals.iter().all(|x| !x.is_empty()) && !v.contains('(');
        if letter != 'M' || !self.dcop_ic || self.scope != NONE || !(grounded && free && plain) {
            return false;
        }
        if !self.holds_ic(n, master) || !self.tran_only() {
            return false;
        }
        debug_assert!(!vals.is_empty(), "a split yields a piece");
        debug_assert!(vals.len() <= 2, "Vds and Vgs");
        for (node, x) in [d, g].into_iter().zip(&vals) {
            self.device_ics.push((node.to_string(), self.spelled(x)));
        }
        true
    }
}

// ---- private ----

impl W<'_, '_> {
    /// Instance `n`'s model card (master `master`) is one whose `IC=` Xyce
    /// holds in the DCOP: a BSIM3 (Xyce's level 9, read as 49) or a BSIM4
    /// (14, 54), in this file or in the kit file that declares it. Levels
    /// 1 and 3 take the IC as a first Newton guess only (XYCE_CORPUS
    /// `MOS13_IC/invert50_mos1.cir`).
    fn holds_ic(&self, n: u32, master: &str) -> bool {
        let params = self.rd.instance_params(n, master);
        let level = params.and_then(|p| model_level(&p));
        matches!(level, Some(49.0 | 14.0 | 54.0))
    }

    /// Every analysis the deck runs is a transient: ngspice holds an `.ic`
    /// node only in a transient's operating point (ngspice-45 run: `.ic
    /// v(a)=0.3` under `.op` leaves a at its 1 V), where Xyce holds a
    /// device's `IC=` in an `.op` and at each `.dc` point too (7.10 runs).
    fn tran_only(&self) -> bool {
        // ponytail: the testbench folded again per MOS `IC=` line.
        let tb = self.fold();
        tb.runs() && tb.plan.runs.iter().all(|a| matches!(a, Ana::Tran { .. }))
    }
}

/// A model card's `level`, when it is a number.
fn model_level(params: &[(String, String)]) -> Option<f64> {
    let is_level = |(k, _): &&(String, String)| k.eq_ignore_ascii_case("level");
    params.iter().find(is_level)?.1.parse().ok()
}
