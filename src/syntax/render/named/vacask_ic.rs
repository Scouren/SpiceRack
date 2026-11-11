//! A capacitor's `ic=` on VACASK: `sp_capacitor` has no such parameter
//! ("Parameter 'ic' not found."), so the top's capacitors' initial
//! conditions are entries of the transients' `ic` list, each a node pair
//! and the voltage across it.
//!
//! Cites: VACASK 36be560 `docs/cmd-analysis-tran.md` ("Initial
//! conditions": `icmode`, the differential entry of an `ic` list); ngspice
//! manual §3.3.5 (a C's IC "only if the uic option is specified"); Xyce RG
//! `C` (the IC held in the DCOP; run: 1 V at t = 0 with no UIC); LTspice
//! 26.1.1 (run: the same).
//!
//! Contents:
//! - `W::cap_ics(&self) -> Vec<(u32, String, bool)>`
//! - `W::held_ics(&mut self, a: &Ana, caps: &[(u32, String, bool)]) -> Vec<String>`

use super::W;
use crate::syntax::behaviour::NetlistRules;
use crate::syntax::diag::Code;
use crate::syntax::render::read::{Ana, Card, Comp};

impl W<'_, '_> {
    /// The top's capacitors that set `ic=`: each card's first token, its
    /// entry of a transient's `ic` list (`"a", "b", <v>`: the node pair
    /// and the voltage across it; ground is `"0"` on either side, run) and
    /// whether a node is ground.
    pub(super) fn cap_ics(&self) -> Vec<(u32, String, bool)> {
        let mut out = Vec::new();
        for &c in &self.deck.top.comps {
            let Comp::Card(n) = c else { continue };
            let Card::Rcl {
                letter: 'C',
                n1,
                n2,
                params,
                ..
            } = self.rd.card_view(n)
            else {
                continue;
            };
            let Some((_, v)) = params.iter().find(|(k, _)| k.eq_ignore_ascii_case("ic")) else {
                continue;
            };
            assert!(!n1.is_empty() && !n2.is_empty(), "a card names two nodes");
            let pair = format!(
                "\"{}\", \"{}\", {}",
                n1.to_ascii_lowercase(),
                n2.to_ascii_lowercase(),
                self.expr(v)
            );
            out.push((self.rd.main(n), pair, n1 == "0" || n2 == "0"));
        }
        out
    }

    /// The entries of `caps` ([`W::cap_ics`]) the transient `a` starts
    /// from; a capacitor's IC with no VACASK form there is refused
    /// (W0129). Under UIC every one (each simulator read here starts a
    /// capacitor at its IC then). With an operating point: none from
    /// ngspice, which ignores a device IC there; a capacitor to ground
    /// from Xyce or LTspice, which hold it and VACASK's `icmode="op"`
    /// forces (run: 0.99999 for 1). Any other is refused: VACASK forces a
    /// floating pair's difference and moves its common mode (run: a = 0, b
    /// = -1 where Xyce gives 1 and 0), and what HSPICE, ESPice and Spectre
    /// hold was not run.
    pub(super) fn held_ics(&mut self, a: &Ana, caps: &[(u32, String, bool)]) -> Vec<String> {
        assert!(
            matches!(a, Ana::Tran { .. } | Ana::MonteCarlo { .. }),
            "a transient's"
        );
        let in_op = match a {
            Ana::Tran { uic: true, .. } => return caps.iter().map(|c| c.1.clone()).collect(),
            Ana::Tran { .. } => match self.rd.b.netlist_rules {
                NetlistRules::Ngspice => return vec![],
                NetlistRules::Xyce | NetlistRules::Ltspice => true,
                NetlistRules::Hspice | NetlistRules::Spice => false,
            },
            // ponytail: a Monte Carlo's inner transient is refused, not
            // looked into; read its `uic` when a statistics deck sets an IC.
            _ => false,
        };
        let mut out = Vec::new();
        for (tok, pair, grounded) in caps {
            match in_op && *grounded {
                true => out.push(pair.clone()),
                false => self.refuse(Code::W0129, *tok),
            }
        }
        assert!(out.len() <= caps.len());
        out
    }
}
