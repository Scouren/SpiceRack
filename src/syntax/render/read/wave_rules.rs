//! Waveform rules past a card's plain values, over the [`Num`]s and the
//! `Wave` view every writer takes: LTspice's PWL time relative to the
//! point before it.
//!
//! Cites: LTspice help, Voltage Source (PWL: a time written `+t` counts
//! from the previous point); LTspice 26.1.1, run: `PWL(0 0 1u 1 +1u 2 + 1u
//! 0 +{dt} 3)` with dt 2u is (0,0) (1u,1) (2u,2) (3u,0) (5u,3).
//!
//! Contents:
//! - `Rd::pwl_relative(&self, vals: &[u32]) -> Option<Vec<Num>>`

use super::{Num, Rd};
use crate::syntax::behaviour::CardRules;

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
