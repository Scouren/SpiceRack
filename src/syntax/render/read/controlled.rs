//! SPICE linear controlled sources (`E`, `F`, `G`, `H`) → their [`Card`]
//! views: a POLY card (`devices`), an E/G `VALUE=`/`VOL=`/`CUR=` expression
//! (`Card::Behavioural`, HSPICE's `SCALE`/`MAX`/`MIN` folded in), or the
//! linear gain (`Card::Linear`, a current output's `M=` folded in). Xyce's
//! E/G `TABLE` is read before the element's spacing check, in
//! `elements::component` (`tables`).
//!
//! Cites: the old SPICE grammar's `E`, `F`, `G`, `H`, `Poly`; HSPICE UG, E
//! and G elements; Xyce RG and ngspice manual, the controlled sources' `m=`.
//!
//! Contents:
//! - `Rd::controlled(&self, n: u32, e: Element, name: String, items: &[u32]) -> Option<Card>`

use super::{Card, Rd};
use crate::syntax::ast::{Node, Tag};
use crate::syntax::behaviour::CardRules;
use crate::syntax::device::Primitive as Element;

impl Rd<'_> {
    /// An `E`, `F`, `G` or `H` card `n` (`items` after its name): a
    /// POLY card, an E/G expression, or the linear gain; `None` for any
    /// other form (kept raw).
    ///
    /// # Panics
    /// When `e` is no controlled source.
    pub(super) fn controlled(
        &self,
        n: u32,
        e: Element,
        name: String,
        items: &[u32],
    ) -> Option<Card> {
        assert!(matches!(
            e,
            Element::Vcvs | Element::Vccs | Element::Cccs | Element::Ccvs
        ));
        if let Some(p) = self.poly(n, name.clone()) {
            return p;
        }
        let (np, nm) = (self.node_text(items, 0)?, self.node_text(items, 1)?);
        let letter = self.letter(n);
        if !matches!(e, Element::Vcvs | Element::Vccs) {
            let (items, m) = self.multiplier(e, items)?;
            let [_, _, v, g] = items else { return None };
            if !self.is_word(*v) || !self.is_word(*g) {
                return None;
            }
            return Some(Card::Linear {
                letter,
                name,
                nodes: vec![np, nm, self.text(*v)],
                gain: self.number(*g)? * m,
            });
        }
        if let [_, _, a, mods @ ..] = items
            && self.tag(*a) == Tag::Assign
        {
            let key = self.key(*a).to_ascii_lowercase();
            let v = self.ast.rhs(Node(*a));
            if !matches!(key.as_str(), "value" | "vol" | "cur") || !self.val_ok(v) {
                return None;
            }
            return Some(Card::Behavioural {
                current: e == Element::Vccs,
                name,
                np,
                nm,
                expr: self.modified(self.body_text(v), mods)?,
                value: v,
            });
        }
        let (items, m) = self.multiplier(e, items)?;
        let [_, _, c, d, g] = items else { return None };
        if ![c, d, g].iter().all(|&&k| self.is_word(k)) {
            return None;
        }
        Some(Card::Linear {
            letter,
            name,
            nodes: vec![np, nm, self.text(*c), self.text(*d)],
            gain: self.number(*g)? * m,
        })
    }
}

// ---- private ----

impl Rd<'_> {
    /// HSPICE's E/G output modifiers after `VALUE=` (HSPICE UG, E and G
    /// elements: `SCALE` multiplies the value, `MAX` and `MIN` clamp it)
    /// applied to `expr`; `None` for any other trailing item.
    fn modified(&self, expr: String, mods: &[u32]) -> Option<String> {
        let mut e = expr;
        for &m in mods {
            if self.b.card_rules != CardRules::Hspice || self.tag(m) != Tag::Assign {
                return None;
            }
            let v = self.body_text(self.ast.rhs(Node(m)));
            e = match self.key(m).to_ascii_lowercase().as_str() {
                "scale" => format!("({e})*({v})"),
                "max" => format!("min({e}, {v})"),
                "min" => format!("max({e}, {v})"),
                _ => return None,
            };
        }
        Some(e)
    }

    /// A linear controlled source's items less a trailing `M=` and its
    /// value (1 without one): `M` parallel copies of a current-output source
    /// (G, F) is the gain times `M` (Xyce RG and ngspice manual, the
    /// controlled sources' `m=`); `None` for an `M=` on E or H, or one not a number.
    fn multiplier<'i>(&self, e: Element, items: &'i [u32]) -> Option<(&'i [u32], f64)> {
        match items.split_last() {
            Some((&a, rest))
                if self.tag(a) == Tag::Assign && self.key(a).eq_ignore_ascii_case("m") =>
            {
                let current = matches!(e, Element::Vccs | Element::Cccs);
                current.then_some(())?;
                Some((rest, self.number(self.ast.rhs(Node(a)))?))
            }
            _ => Some((items, 1.0)),
        }
    }
}
