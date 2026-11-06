//! An LTspice capacitor's, inductor's or voltage source's own parasitics
//! (`Rser`, `Lser`, `Rpar`, `Cpar`) → explicit elements on Spectre and
//! VACASK, whose C, L and V take none: the same network the SPICE targets
//! get (`spice::parasitics::parts`), in each target's instance form.
//!
//! Cites: LTspice help, Capacitor, Inductor, Voltage Source; Spectre
//! Reference 19.1 p.494 (instances: `name (nodes) master k=v`); VACASK
//! `devices/spice/{resistor,capacitor,inductor}.va` (`r`, `c`, `l`).
//!
//! Contents:
//! - `W::parasitics(&self, letter: char, n: u32, n1: &str, n2: &str, params: &mut Vec<(String, String)>) -> Option<(String, Vec<Part>)>`
//! - `W::part_lines(&self, parts: Option<(String, Vec<Part>)>) -> String`

use super::W;
use super::vacask::q;
use crate::syntax::behaviour::{CardRules, Writer};
use crate::syntax::render::spice::parasitics::{Part, parts};

impl W<'_, '_> {
    /// Element `n`'s parasitics taken out of `params` (an LTspice C, L or
    /// V card's; another dialect's names mean nothing here): the node the
    /// element then ends at in place of `n1`, and the parts.
    pub(super) fn parasitics(
        &self,
        letter: char,
        n: u32,
        n1: &str,
        n2: &str,
        params: &mut Vec<(String, String)>,
    ) -> Option<(String, Vec<Part>)> {
        if self.rd.b.card_rules != CardRules::Ltspice || letter == 'R' {
            return None;
        }
        parts(&self.names.decl(n).to_ascii_lowercase(), n1, n2, params)
    }

    /// The parts' instance statements, each after a line end: a resistor,
    /// inductor or capacitor of the target's own master (VACASK's model
    /// lines name each module after itself, `vacask.rs`).
    pub(super) fn part_lines(&self, parts: Option<(String, Vec<Part>)>) -> String {
        let mut s = String::new();
        for p in parts.map(|(_, p)| p).unwrap_or_default() {
            let (l, master) = match p.letter {
                'R' => ('r', "resistor"),
                'L' => ('l', "inductor"),
                _ => ('c', "capacitor"),
            };
            let (name, a, b, v) = match self.b.writer {
                Writer::Vacask => (q(&p.name), q(&p.a), q(&p.b), self.expr(&p.value)),
                _ => (
                    self.node(&p.name),
                    self.node(&p.a),
                    self.node(&p.b),
                    self.value_text(&p.value),
                ),
            };
            s.push_str(&format!("\n{l}{name} ({a} {b}) {master} {l}={v}"));
        }
        s
    }
}
