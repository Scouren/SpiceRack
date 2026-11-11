//! Waveform rules past a card's plain values, over the [`Num`]s and the
//! `Wave` view every writer takes: LTspice's PWL time relative to the
//! point before it; a PULSE with a cycle count as the PWL of its pulses.
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

use super::{Num, Rd, Wave, WaveKind};
use crate::syntax::behaviour::CardRules;

/// The most pulses [`Wave::pulses`] writes as PWL points (four a pulse).
const PULSES_MAX: f64 = 1000.0;

impl Wave {
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

/// `x` to 15 significant digits, so a sum of two decimal times prints as
/// the decimal sum (`0.002701`, not `0.0027010000000000003`).
fn round15(x: f64) -> f64 {
    format!("{x:.14e}").parse().unwrap_or(x)
}
