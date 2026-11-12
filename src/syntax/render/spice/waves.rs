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
//! - `W::wave_as_b(&mut self, w: &Wave, tok: u32) -> Option<String>`,
//!   `W::beside_b(&mut self, w: &Wave, value: Val, tok: u32) -> (Option<Wave>, Val)`,
//!   `W::wave_b(&mut self, decl: &str, nodes: [&str; 2], current: bool, expr: &str, tok: u32) -> String`,
//!   `W::b_line(&mut self, decl: &str, nodes: [&str; 2], current: bool, expr: &str, tok: u32) -> String`

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
            WaveKind::Pwl | WaveKind::Sin | WaveKind::Pulse | WaveKind::Exp | WaveKind::Sffm => {}
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
            // A PULSE's cycle count is written only when given (0 is none).
            WaveKind::Pulse if !w.counted() => args.truncate(7),
            // LTspice's SINE cycle count likewise (written for LTspice
            // only: elsewhere the wave is a behavioural source).
            WaveKind::Sin if !w.cycled() => args.truncate(6),
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
            WaveKind::Sin
            | WaveKind::Pulse
            | WaveKind::Pwl
            | WaveKind::Exp
            | WaveKind::Sffm
            | WaveKind::Am
            | WaveKind::Trnoise => {}
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

    /// The behavioural expression of `time` a source is written with in
    /// place of wave `w`, where the target's own form would run another
    /// wave: LTspice's SINE with a cycle count ([`Wave::sin_cycles`]),
    /// SPICE3's SFFM where ngspice's is not it ([`Wave::ngspice_sffm`]).
    /// `None`: `w` is written in its own form, or it has no such
    /// expression and is refused (W0108), as ngspice's SFFM is where
    /// SPICE3's is not it ([`Wave::spice3_sffm`]).
    pub(super) fn wave_as_b(&mut self, w: &Wave, tok: u32) -> Option<String> {
        let expr = match w.kind {
            WaveKind::Sin if self.pair.sin_cycles_as_b && w.cycled() => w.sin_cycles("time"),
            WaveKind::Sffm if self.pair.sffm_as_b && !w.ngspice_sffm() => {
                Some(w.sffm_of("time", |a| match a {
                    Num::Num(v) => self.sp.decimal(*v, self.x),
                    // Bare, as a B source reads a parameter (ESPice 0.3.0
                    // cannot read `{v0}` there).
                    Num::Expr(e) => self.ternaries(e),
                }))
            }
            // ngspice's SFFM for a target whose SFFM is SPICE3's: 0 up to
            // time zero, MDI limited, an FC of 0 a default.
            WaveKind::Sffm if self.pair.sffm_swap && !self.pair.sffm_as_b && !w.spice3_sffm() => {
                None
            }
            _ => return None,
        };
        if expr.is_none() {
            self.refuse(crate::syntax::diag::Code::W0108, tok);
        }
        expr
    }

    /// What a source card is left as beside the behavioural source that
    /// carries its wave `w` ([`W::wave_as_b`]): its wave and its value. A
    /// card whose `value` is the wave's at time zero (it gave no DC
    /// value) is a 0 source. An SFFM is at its offset at time zero, so a
    /// card with a DC value keeps the rest of it, which the analyses that
    /// solve at DC read, and is 0 in a transient: `PWL(0 0)` (ngspice-45
    /// `vsrcload.c:74`; ngspice-45 and ESPice 0.3.0, run: `V1 a V1_w 3
    /// PWL(0 0)` is 3 V at `.op` and 0 V through a `.tran`, as Xyce 7.10
    /// runs `DC 3 SFFM(0 …)`). Any other DC value is refused (W0108): the
    /// behavioural source would give the operating point the wave's.
    pub(super) fn beside_b(&mut self, w: &Wave, value: Val, tok: u32) -> (Option<Wave>, Val) {
        match (&value, w.at_zero()) {
            (v, zero) if *v == zero => (None, Val::Num(0.0)),
            (Val::Num(dc), Val::Num(zero)) if w.kind == WaveKind::Sffm => {
                let hold = Wave {
                    kind: WaveKind::Pwl,
                    args: vec![Num::Num(0.0); 2],
                    repeat: None,
                    delay: None,
                    cut: false,
                };
                (Some(hold), Val::Num(dc - zero))
            }
            _ => {
                self.refuse(crate::syntax::diag::Code::W0108, tok);
                (None, Val::Num(0.0))
            }
        }
    }

    /// The behavioural source that carries card `decl`'s wave, expression
    /// `expr`, between `nodes`: `B{decl}_w`, on HSPICE `E{decl}_w` or
    /// `G{decl}_w` (as `cards::renamed` names a B card there).
    pub(super) fn wave_b(
        &mut self,
        decl: &str,
        nodes: [&str; 2],
        current: bool,
        expr: &str,
        tok: u32,
    ) -> String {
        let vol_cur =
            self.cells[Construct::BehaviouralSource as usize] == Action::Spell(Alt::VolCur);
        let letter = match (vol_cur, current) {
            (false, _) => 'B',
            (true, true) => 'G',
            (true, false) => 'E',
        };
        self.b_line(&format!("{letter}{decl}_w"), nodes, current, expr, tok)
    }

    /// Behavioural source `decl np nm` with expression `expr` (C's `?:`)
    /// in the target's form: HSPICE's `vol=`/`cur=` (`decl` already its
    /// `E`/`G` name), Xyce's braced value, else `V=`/`I=`.
    pub(super) fn b_line(
        &mut self,
        decl: &str,
        [np, nm]: [&str; 2],
        current: bool,
        expr: &str,
        tok: u32,
    ) -> String {
        let key = if current { 'I' } else { 'V' };
        match self.ask(Construct::BehaviouralSource, tok) {
            Action::Spell(Alt::VolCur) if current => {
                format!("{decl} {np} {nm} cur='{expr}'")
            }
            Action::Spell(Alt::VolCur) => format!("{decl} {np} {nm} vol='{expr}'"),
            Action::Spell(Alt::BracedBehaviour) => {
                format!("{decl} {np} {nm} {key}={{{expr}}}")
            }
            _ => format!("{decl} {np} {nm} {key}={expr}"),
        }
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
