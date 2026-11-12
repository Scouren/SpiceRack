//! Waveform rules past a card's plain values, over the [`Num`]s and the
//! `Wave` view every writer takes: LTspice's PWL time relative to the
//! point before it; a PULSE with a cycle count as the PWL of its pulses;
//! LTspice's SINE with a cycle count, and an SFFM ngspice's own form runs
//! otherwise, as expressions of time.
//!
//! Cites: ngspice manual §4.1.1 (PULSE's table; NP); LTspice help, Current
//! Source (PULSE's Ncycles); LTspice help, Voltage Source (PWL: a time
//! written `+t` counts
//! from the previous point); LTspice 26.1.1, run: `PWL(0 0 1u 1 +1u 2 + 1u
//! 0 +{dt} 3)` with dt 2u is (0,0) (1u,1) (2u,2) (3u,0) (5u,3).
//!
//! Contents:
//! - `Rd::pwl_relative(&self, vals: &[u32]) -> Option<Vec<Num>>`
//! - `Wave::counted(&self) -> bool`, `Wave::pulses(&self) -> Option<Wave>`
//! - `Wave::cycled(&self) -> bool`, `Wave::sin_cycles(&self, t: &str) -> Option<String>`
//! - `Wave::ngspice_sffm(&self) -> bool`, `Wave::spice3_sffm(&self) -> bool`,
//!   `Wave::sffm_of(&self, t: &str, num: impl Fn(&Num) -> String) -> String`

use super::{Num, Rd, Wave, WaveKind};
use crate::syntax::behaviour::CardRules;

/// The most pulses [`Wave::pulses`] writes as PWL points (four a pulse).
const PULSES_MAX: f64 = 1000.0;

/// π to an f64's digits, for an expression's text.
const PI: &str = "3.141592653589793";

impl Wave {
    /// Whether this is LTspice's SINE with a cycle count (its seventh
    /// value; 0 is none).
    pub(in crate::syntax::render) fn cycled(&self) -> bool {
        self.kind == WaveKind::Sin && self.args[6] != Num::Num(0.0)
    }

    /// LTspice's `SINE(Voff Vamp Freq Td Theta Phi Ncycles)` as an
    /// expression of the target's time variable `t`: Voff + Vamp·sin(φ)
    /// before Td; Voff + Vamp·e^(−(t−Td)·Theta)·sin(2π·Freq·(t−Td) + φ)
    /// for Ncycles/Freq after it; then the value it has there, held
    /// (LTspice help, Current Source; LTspice 26.1.1, run: `SINE(0.5 1
    /// 100k 2u 0 30 1.5)` is 1.0 V before 2 µs and 0 V from 17 µs). The
    /// text is `+ - * /`, `<`, `exp`, `sin` and a parenthesised `?:`,
    /// which ngspice, ESPice, Xyce, HSPICE, Spectre and VACASK read alike
    /// (run on each but Spectre). `None`: a value that is an expression.
    ///
    /// # Panics
    /// When the wave is not [`Wave::cycled`].
    pub(in crate::syntax::render) fn sin_cycles(&self, t: &str) -> Option<String> {
        assert!(self.cycled(), "a SINE with a cycle count");
        assert!(!t.is_empty(), "a time variable");
        let a: Vec<String> = self
            .args
            .iter()
            .map(|x| x.num().map(operand))
            .collect::<Option<_>>()?;
        let [vo, va, f, td, theta, phi, n] = &a[..] else {
            unreachable!("a SINE holds seven values")
        };
        // The wave `dt` after Td, `cycles` into its sine.
        let at = |dt: &str, cycles: &str| {
            format!("{vo}+{va}*exp(-{dt}*{theta})*sin(2*{PI}*{cycles}+{phi}*{PI}/180)")
        };
        let first = at("0", "0");
        let run = at(&format!("({t}-{td})"), &format!("{f}*({t}-{td})"));
        let held = at(&format!("{n}/{f}"), n);
        Some(format!(
            "{t}<{td} ? {first} : ({t}<{td}+{n}/{f} ? {run} : {held})"
        ))
    }

    /// Whether ngspice's SFFM runs this wave, held in SPICE3's order (VO VA
    /// FC MDI FS: Xyce's, HSPICE's, LTspice's), as SPICE3 defines it.
    /// ngspice-45 (`vsrcload.c:228-277`) is 0 up to time zero, a
    /// transient's operating point, where SPICE3 is VO (`:272`); limits
    /// MDI to FC/FS and to 0 from below (`:256`, "MDI limited to FC/FM");
    /// and takes an FC of 0 as 500/TSTOP (`:242`). By run: `SFFM(0 1 20k
    /// 10 5k)` is -0.6488 V at 30 µs on Xyce 7.10, +0.6587 V as ngspice's
    /// `SFFM(0 1 5k 10 20k)`. False also when one of the four is an
    /// expression.
    ///
    /// # Panics
    /// When the wave is not an SFFM.
    pub(in crate::syntax::render) fn ngspice_sffm(&self) -> bool {
        assert_eq!(self.kind, WaveKind::Sffm, "an SFFM wave");
        assert_eq!(self.args.len(), 5, "an SFFM holds five values");
        let [Some(vo), Some(fc), Some(mdi), Some(fs)] = [0, 2, 3, 4].map(|i| self.args[i].num())
        else {
            return false;
        };
        vo == 0.0 && fc != 0.0 && (0.0..=fc / fs).contains(&mdi)
    }

    /// Whether SPICE3's SFFM runs this wave, held in ngspice's order (VO VA
    /// FM MDI FC), as ngspice-45 defines it: [`Wave::ngspice_sffm`] of the
    /// two frequencies swapped, its three conditions read from the other
    /// side. An offset is SPICE3's value at time zero, where ngspice is 0;
    /// SPICE3 does not limit MDI; an FC of 0 is a 0 Hz carrier there. By
    /// run: ngspice-45's `SFFM(0.5 1 200 4 10k)` starts a transient at 0 V,
    /// Xyce 7.10's and VACASK's `(0.5 1 10k 4 200)` at 0.5 V; `SFFM(0 1 200
    /// 80 10k)` is 0.9995 V at 213.7 µs on ngspice, -0.0957 V as Xyce's and
    /// VACASK's `(0 1 10k 80 200)`. False also when one of the four is an
    /// expression.
    ///
    /// # Panics
    /// When the wave is not an SFFM.
    pub(in crate::syntax::render) fn spice3_sffm(&self) -> bool {
        assert_eq!(self.kind, WaveKind::Sffm, "an SFFM wave");
        assert_eq!(self.args.len(), 5, "an SFFM holds five values");
        let mut spice3 = self.clone();
        spice3.args.swap(2, 4);
        spice3.ngspice_sffm()
    }

    /// SPICE3's `SFFM(VO VA FC MDI FS)` as an expression of the target's
    /// time variable `t`: VO + VA·sin(2π·FC·t + MDI·sin(2π·FS·t)) (Xyce RG
    /// "SFFM"; HSPICE SH98 p.5-17; LTspice help), each value spelled by
    /// `num` and put in parentheses.
    ///
    /// # Panics
    /// When the wave is not an SFFM.
    pub(in crate::syntax::render) fn sffm_of(
        &self,
        t: &str,
        num: impl Fn(&Num) -> String,
    ) -> String {
        assert_eq!(self.kind, WaveKind::Sffm, "an SFFM wave");
        assert!(!t.is_empty(), "a time variable");
        let x: Vec<String> = self.args.iter().map(|a| format!("({})", num(a))).collect();
        let [vo, va, fc, mdi, fs] = &x[..] else {
            unreachable!("an SFFM holds five values")
        };
        format!("{vo}+{va}*sin(2*{PI}*{fc}*{t}+{mdi}*sin(2*{PI}*{fs}*{t}))")
    }

    /// Whether this is a PULSE with a cycle count (its eighth value:
    /// ngspice's NP, LTspice's Ncycles; 0 is none).
    pub(in crate::syntax::render) fn counted(&self) -> bool {
        self.kind == WaveKind::Pulse && self.args[7] != Num::Num(0.0)
    }

    /// A counted PULSE `(V1 V2 TD TR TF PW PER N)` as the PWL of its N
    /// pulses, for a target whose PULSE has no count: (0, V1), then for
    /// pulse k from t₀ = TD + k·PER the points (t₀, V1) (t₀+TR, V2)
    /// (t₀+TR+PW, V2) (t₀+TR+PW+TF, V1) (ngspice manual §4.1.1's table),
    /// V1 after the last since a PWL holds its last value (LTspice 26.1.1,
    /// ngspice-45, and Xyce 7.10 and VACASK on the PWL, run: `PULSE(0 1 1u
    /// 1n 1n 2u 5u 2)` is 1 V at 2 and 7 µs, 0 V at 12 and 17 µs). `None`
    /// where that table is not the wave: a time that is no number; a zero
    /// TR, TF or PW (each simulator's own default); pulses that overlap
    /// (TR+PW+TF > PER; a single pulse needs no period); a count that is
    /// no whole number in 1..=[`PULSES_MAX`].
    ///
    /// # Panics
    /// When the wave is not [`Wave::counted`].
    pub(in crate::syntax::render) fn pulses(&self) -> Option<Wave> {
        assert!(self.counted(), "a PULSE with a count");
        let t: Vec<f64> = self.args[2..].iter().map(Num::num).collect::<Option<_>>()?;
        let [td, tr, tf, pw, per, n] = t[..] else {
            return None;
        };
        let whole = n.fract() == 0.0 && (1.0..=PULSES_MAX).contains(&n);
        let fits = tr + pw + tf <= per || n == 1.0 && per == 0.0;
        if !(whole && fits && td >= 0.0 && tr > 0.0 && tf > 0.0 && pw > 0.0) {
            return None;
        }
        let (v1, v2) = (&self.args[0], &self.args[1]);
        let mut args = vec![Num::Num(0.0), v1.clone()];
        for k in 0..n as u32 {
            let t0 = td + f64::from(k) * per;
            let edges = [
                (t0, v1),
                (t0 + tr, v2),
                (t0 + tr + pw, v2),
                (t0 + tr + pw + tf, v1),
            ];
            for (t, v) in edges {
                // A pulse that ends where the next starts shares the point.
                let t = round15(t);
                if Some(t) > args[args.len() - 2].num() {
                    args.extend([Num::Num(t), v.clone()]);
                }
            }
        }
        assert!(args.len().is_multiple_of(2), "PWL values come in pairs");
        Some(Wave {
            kind: WaveKind::Pwl,
            args,
            repeat: None,
            delay: None,
            cut: self.cut,
        })
    }
}

impl Rd<'_> {
    /// An LTspice PWL's values, each relative time made absolute: tₖ =
    /// tₖ₋₁ + t. A time is relative when its word starts with `+` (`+1u`)
    /// or a lone `+` word stands before it (`+ 1u`, `+{dt}`: the parser's
    /// items); the first point's counts from 0. Two numbers add
    /// ([`round15`]: `2.7m` and `+1u` are `0.002701`); with an expression
    /// on either side the time is the expression `prev+(t)`. `None`: a
    /// value that does not read, or a lone `+` before a value or at the
    /// end.
    pub(super) fn pwl_relative(&self, vals: &[u32]) -> Option<Vec<Num>> {
        assert!(
            self.b.card_rules == CardRules::Ltspice,
            "a relative PWL time is LTspice's"
        );
        let text = |n: &Num| match n {
            Num::Num(v) => format!("{v}"),
            Num::Expr(e) => e.clone(),
        };
        let mut out: Vec<Num> = Vec::with_capacity(vals.len());
        let mut plus = false;
        for &v in vals {
            let time = out.len().is_multiple_of(2);
            let word = if self.is_word(v) {
                self.text(v)
            } else {
                String::new()
            };
            if word == "+" {
                if plus || !time {
                    return None;
                }
                plus = true;
                continue;
            }
            let x = self.irnum(v)?;
            let relative = time && (plus || word.starts_with('+'));
            plus = false;
            let prev = out
                .len()
                .checked_sub(2)
                .map(|i| &out[i])
                .filter(|_| relative);
            let x = match (prev, x) {
                (Some(Num::Num(p)), Num::Num(t)) => Num::Num(round15(p + t)),
                (Some(p), t) => Num::Expr(format!("{}+({})", text(p), text(&t))),
                (None, x) => x,
            };
            out.push(x);
        }
        assert!(out.len() <= vals.len(), "only a lone `+` is dropped");
        (!plus).then_some(out)
    }
}

// ---- private ----

/// `v` as an operand in an expression's text: as it prints, a negative
/// number in parentheses.
fn operand(v: f64) -> String {
    if v < 0.0 {
        format!("({v})")
    } else {
        format!("{v}")
    }
}

/// `x` to 15 significant digits, so a sum of two decimal times prints as
/// the decimal sum (`0.002701`, not `0.0027010000000000003`).
fn round15(x: f64) -> f64 {
    format!("{x:.14e}").parse().unwrap_or(x)
}
