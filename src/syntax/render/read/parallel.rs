//! An LTspice capacitor's `x<n>` flag (n units in parallel) → the one
//! capacitor a target without the flag reads: C·n, with each unit's
//! parasitics `Rser`/n, `Lser`/n, `RLshunt`/n, `Rpar`/n, `Cpar`·n.
//!
//! Cites: LTwiki, "Undocumented LTspice" (`x<n>` = `m=<n>`); LTspice
//! help, Capacitor (`m`: parallel units, the parasitics each unit's);
//! LTspice 26.1.1 (`x2` ran equal to the scaled network, ledger P5-006).
//!
//! Contents:
//! - `pub(super) fn units(word: &str) -> Option<u32>`
//! - `Rd::merged_units(&self, n: u32, value: Val, params: Vec<(String, String)>) -> (Val, Vec<(String, String)>)`

use super::{Rd, Val};

/// A unit's parasitics by key, and whether n units multiply it (the shunt
/// capacitance) or divide it (the resistances and the inductance).
const PER_UNIT: [(&str, bool); 5] = [
    ("rser", false),
    ("lser", false),
    ("rlshunt", false),
    ("rpar", false),
    ("cpar", true),
];

/// The count `n` of an `x<n>` flag (`x2`, `X10`): digits only, at least 1.
pub(super) fn units(word: &str) -> Option<u32> {
    let digits = word.strip_prefix(['x', 'X'])?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|&n: &u32| n > 0)
}

impl Rd<'_> {
    /// Capacitor `n`'s `value` and `params` as one unit: its `x<n>` flag
    /// taken out, the capacitance and `Cpar` times n, `Rser`, `Lser`,
    /// `RLshunt` and `Rpar` over n (each written as a plain number).
    /// Unchanged when the card has no such flag, and when a parameter to
    /// scale is no number or the capacitance is raw text: the flag then
    /// stays for the writer to answer.
    pub(super) fn merged_units(
        &self,
        n: u32,
        value: Val,
        mut params: Vec<(String, String)>,
    ) -> (Val, Vec<(String, String)>) {
        let flag = params
            .iter()
            .position(|(k, v)| v.is_empty() && units(k).is_some());
        let Some(at) = flag else {
            return (value, params);
        };
        let count = f64::from(units(&params[at].0).expect("the flag found above"));
        assert!(count >= 1.0, "merged_units: a count of units");
        let lex = self.t.lexicon_at(self.main(n));
        // Every number first: one that is none keeps the card as read.
        let scaled: Option<Vec<(usize, f64)>> = params
            .iter()
            .enumerate()
            .filter_map(|(i, (k, v))| {
                let &(_, times) = PER_UNIT.iter().find(|(w, _)| k.eq_ignore_ascii_case(w))?;
                let x = self.number_text(v.trim(), lex);
                Some(x.map(|x| (i, if times { x * count } else { x / count })))
            })
            .collect();
        let Some(scaled) = scaled else {
            return (value, params);
        };
        let value = match value {
            Val::Num(c) => Val::Num(c * count),
            Val::Expr(e) => Val::Expr(format!("({e})*{count}")),
            raw @ Val::Raw(_) => return (raw, params),
        };
        for (i, x) in scaled {
            params[i].1 = format!("{x:e}");
        }
        params.remove(at);
        (value, params)
    }
}

#[cfg(test)]
mod tests {
    use super::units;

    #[test]
    fn a_units_flag_is_x_and_a_positive_count() {
        // LTwiki, "Undocumented LTspice": `x<n>`, n a count. Checks: 6.
        assert_eq!(units("x2"), Some(2));
        assert_eq!(units("X10"), Some(10));
        assert_eq!(units("x0"), None);
        assert_eq!(units("x"), None);
        assert_eq!(units("x2a"), None);
        assert_eq!(units("xfer"), None);
    }
}
