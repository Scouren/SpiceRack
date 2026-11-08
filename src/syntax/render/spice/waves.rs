//! A source waveform → its SPICE-family spelling: `KIND(v …)`, PWL's
//! `r=`/`td=`, AM in the target's order, and the refusal of one the
//! target lacks.
//!
//! Cites: ngspice manual §4.1 and ngspice-45 `vsrcload.c` (AM's order);
//! HSPICE UG, AM source; Xyce RG and LTspice help (no AM).
//!
//! Contents:
//! - `W::wave_num(&self, n: &Num) -> String`, `W::wave_ok(&mut self, letter: char, w: &Wave, tok: u32)`,
//!   `W::wave(&self, w: &Wave) -> String`

use super::W;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::read::{Num, Val, Wave, WaveKind};

impl W<'_, '_> {
    /// A waveform value: a number as plain decimal, an expression
    /// delimited.
    pub(super) fn wave_num(&self, n: &Num) -> String {
        match n {
            Num::Num(v) => self.sp.decimal(*v, self.x),
            Num::Expr(e) => self.value(&Val::Expr(e.clone())),
        }
    }

    /// Records a refusal for a waveform the target lacks.
    pub(super) fn wave_ok(&mut self, letter: char, w: &Wave, tok: u32) {
        match w.kind {
            WaveKind::Pwl if w.repeat.is_some() || w.delay.is_some() => {
                let c = match letter {
                    'V' => Construct::PwlRepeatV,
                    _ => Construct::PwlRepeatI,
                };
                self.ask(c, tok);
            }
            WaveKind::Trnoise | WaveKind::Trrandom => {
                self.ask(Construct::NoiseWave, tok);
            }
            WaveKind::Am => {
                let act = self.ask(Construct::AmWave, tok);
                let x: Vec<Option<f64>> = w.args.iter().map(Num::num).collect();
                let zero = |i: usize| x[i] == Some(0.0);
                // ESPice's AM has no offset; HSPICE's neither, nor phases,
                // and its carrier amplitude is SA·OC (`am_args`).
                let bad = match act {
                    Action::Spell(Alt::HspiceAm) => {
                        !(zero(0) && zero(6) && zero(7)) || x[2].is_none_or(|v| v == 0.0)
                    }
                    Action::Spell(Alt::AmNoOffset) => !zero(0),
                    _ => false,
                };
                if bad {
                    self.refuse(crate::syntax::diag::Code::W0108, tok);
                }
            }
            _ => {}
        }
    }

    /// `w`'s spelling: `KIND(v …)`, PWL's `r=`/`td=` after it.
    pub(super) fn wave(&self, w: &Wave) -> String {
        let word = match w.kind {
            WaveKind::Sin => "SIN",
            WaveKind::Pulse => "PULSE",
            WaveKind::Pwl => "PWL",
            WaveKind::Exp => "EXP",
            WaveKind::Sffm => "SFFM",
            WaveKind::Am => "AM",
            WaveKind::Trnoise => "TRNOISE",
            WaveKind::Trrandom => "TRRANDOM",
        };
        let mut args: Vec<String> = w.args.iter().map(|a| self.wave_num(a)).collect();
        if w.kind == WaveKind::Am {
            args = self.am_args(w);
        }
        match w.kind {
            // The RTS fields are left off when all zero, as the manual's
            // white and 1/f examples write the card.
            WaveKind::Trnoise if w.args[4..].iter().all(|a| a.num() == Some(0.0)) => {
                args.truncate(4)
            }
            WaveKind::Trrandom => {
                let kind = w.args[0].num().expect("a TRRANDOM type");
                args[0] = format!("{}", kind as u8);
            }
            // ngspice-45's SFFM is (VO VA FM MDI FC), the modulation
            // frequency third and the carrier fifth (`vsrcload.c`; by hand:
            // sffm(0 1 10meg 0 1meg) at 25 ns is 0.156 on ngspice, 1.000 on
            // Xyce); SPICE3's, HSPICE's, Xyce's and LTspice's is (VO VA FC
            // MDI FS). The reader keeps the source's order.
            // CDL's cards are ngspice's.
            WaveKind::Sffm if self.pair.sffm_swap => args.swap(2, 4),
            _ => {}
        }
        let mut s = format!("{word}({})", args.join(" "));
        if let Some(r) = &w.repeat {
            s.push_str(&format!(" r={}", self.wave_num(r)));
        }
        if let Some(td) = &w.delay {
            s.push_str(&format!(" td={}", self.wave_num(td)));
        }
        s
    }
}

// ---- private ----

impl W<'_, '_> {
    /// An AM's values for the target: HSPICE's `SA OC FM FC TD` (SA the
    /// modulation amplitude VMA, OC VMO/VMA: HSPICE UG, AM source), else
    /// ngspice-45's `VO VMO VMA FM FC [TD [PHASEM PHASEC]]` (`Wave::args`),
    /// the zero tail left off.
    fn am_args(&self, w: &Wave) -> Vec<String> {
        let a = &w.args;
        if self.cells[Construct::AmWave as usize] == Action::Spell(Alt::HspiceAm) {
            let (vmo, vma) = (a[1].num().unwrap_or(0.0), a[2].num().unwrap_or(1.0));
            let oc = Num::Num(vmo / vma);
            return [&a[2], &oc, &a[3], &a[4], &a[5]]
                .map(|x| self.wave_num(x))
                .to_vec();
        }
        let zero = |x: &Num| x.num() == Some(0.0);
        let n = if !zero(&a[6]) || !zero(&a[7]) {
            8
        } else if !zero(&a[5]) {
            6
        } else {
            5
        };
        a[..n].iter().map(|x| self.wave_num(x)).collect()
    }
}
