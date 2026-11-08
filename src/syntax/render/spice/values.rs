//! A value's text as the target reads it: HSPICE's quoting, and the
//! expression spellings another family needs (`ternary`: grouping,
//! `if()`, `log`'s base, `nint`'s rounding, a clamp, statistical calls at
//! their nominal, a Xyce `M` renamed), and an R model's value where Xyce
//! reads its model `R` as a multiplier.
//!
//! Cites: per item.
//!
//! Contents:
//! - `W::spelled(&self, v: &str) -> String`, `W::ternaries(&self, e: &str) -> String`,
//!   `W::ternaries_as(&self, e: &str, read: Read) -> String`
//! - `W::m_rename(&self, params: &[(String, String)]) -> Option<String>`
//! - `W::declares(&self, master: &str, p: &str) -> bool`, `W::r_default(&self, letter: char, m: &str) -> Option<String>`
//! - `W::encloses(&self, outer: u32, inner: u32) -> bool`, `W::inherited_models(&self, sc: &Block) -> Vec<(u32, u32)>`
//! - `pub(super) fn r_value(kind: &str, params: &[(String, String)]) -> Option<String>`

use super::W;
use super::ternary::{
    Read, nominal, parenthesize, rename_call, rename_word, spell_clamp, spell_if, spell_nint,
};
use crate::syntax::behaviour::{ExprEval, NetlistRules, ParamForm, ValueQuoting};

impl W<'_, '_> {
    /// A value kept as SPICE text, as the target reads it: as written, but
    /// for HSPICE, which reads braces as name bytes and an unquoted value
    /// as one word starting with a letter, or a number: any other
    /// expression is quoted (`{a*2}` is `'a*2'`, `{a} + b` is `'(a) + b'`);
    /// a run of numbers (a matrix) is not. A ternary is
    /// spelled for the target ([`W::ternaries`]).
    pub(super) fn spelled(&self, v: &str) -> String {
        // HSPICE reads a double-quoted value as an expression too
        // (`.param t2="a^3"`; ESPice's HSPICE reading: 8), the others as a
        // string (ngspice-45: "Undefined parameter [t2]").
        let quoted = v.len() > 1 && v.starts_with('"') && v.ends_with('"');
        let hspice =
            self.rd.b.expr_eval == ExprEval::Hspice && self.b.expr_eval != ExprEval::Hspice;
        let v = match hspice && quoted {
            true => format!("{{{}}}", &v[1..v.len() - 1]),
            false => v.to_string(),
        };
        let v = self.ternaries(&v);
        if self.b.value_quoting == ValueQuoting::Hspice {
            hspice_value(&v)
        } else {
            v
        }
    }

    /// The name a Xyce subcircuit's declared `M` (in `params`, its head's)
    /// is written under on ngspice, where an `X` line's `M` is the
    /// multiplicity only where the subcircuit does not declare `m`; Xyce's
    /// is the multiplicity even where declared (Xyce RG `X`;
    /// ISSUE_267/test_a3.cir: 5 copies; ngspice run by hand). `None`: as
    /// written.
    pub(super) fn m_rename(&self, params: &[(String, String)]) -> Option<String> {
        let taken = |n: &str| params.iter().any(|(k, _)| k.eq_ignore_ascii_case(n));
        let renamed = self.rd.b.netlist_rules == NetlistRules::Xyce
            && self.b.netlist_rules == NetlistRules::Ngspice
            && self.b.subckt_multiplicity
            && taken("m");
        renamed.then(|| (1..).map(|i| format!("m_{i}")).find(|n| !taken(n)))?
    }

    /// Whether subcircuit `master`, as the deck defines it, declares
    /// parameter `p`; `false` for one the deck does not define.
    pub(super) fn declares(&self, master: &str, p: &str) -> bool {
        let def = self
            .deck
            .defs
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(master));
        def.and_then(|b| self.rd.subckt_head(b.node))
            .is_some_and(|h| h.params.iter().any(|(k, _)| k.eq_ignore_ascii_case(p)))
    }

    /// The resistance an R model `m` gives an instance with no value,
    /// on Xyce from another dialect: ngspice's model `r` or `resistance`
    /// is the default resistance (manual §3.3.2), Xyce's model `R` a
    /// multiplier on the instance's 1k (RG 7.10 resistor; Xyce by hand:
    /// `.model my r r=2k` with `r1 2 0 my` is 2 MΩ), so the model leaves
    /// it out (`W::model`) and the instance takes it. `None` elsewhere, or
    /// for a model that also sets `rsh` (its geometry decides).
    pub(super) fn r_default(&self, letter: char, m: &str) -> Option<String> {
        let xyce = |r: NetlistRules| r == NetlistRules::Xyce;
        if letter != 'R' || !xyce(self.b.netlist_rules) || xyce(self.rd.b.netlist_rules) {
            return None;
        }
        // The innermost block around this one that declares `m` (SPICE
        // scopes a `.model` to its `.subckt`, ngspice §2.4.3).
        let (scope, none) = (self.scope, crate::syntax::ast::NONE);
        let depth = |b: u32| if b == none { 0 } else { self.rd.main(b) + 1 };
        let view = self
            .blocks()
            .into_iter()
            .filter(|b| b.node == none || self.encloses(b.node, scope))
            .flat_map(|b| {
                b.models
                    .iter()
                    .map(move |&(n, bin)| (depth(b.node), n, bin))
            })
            .map(|(d, n, bin)| (d, self.rd.model_view(n, bin)))
            .filter(|(_, v)| v.name.eq_ignore_ascii_case(m))
            .max_by_key(|(d, _)| *d)?;
        r_value(&view.1.kind, &view.1.params)
    }

    /// Whether subcircuit `outer` is `inner` or holds it in the source
    /// (by token position: its first statement to its last).
    pub(super) fn encloses(&self, outer: u32, inner: u32) -> bool {
        let none = crate::syntax::ast::NONE;
        if outer == inner {
            return true;
        }
        let end = self.rd.body(outer).iter().map(|&s| self.rd.main(s)).max();
        inner != none
            && self.rd.main(outer) < self.rd.main(inner)
            && end.is_some_and(|e| self.rd.main(inner) <= e)
    }

    /// The models a subcircuit nested in the source sees from the
    /// subcircuits around it, innermost first, less those it or a nearer
    /// one declares: written hoisted, it holds them itself (ngspice §2.4.3
    /// scopes a `.model` to its `.subckt` and those inside it).
    pub(super) fn inherited_models(
        &self,
        sc: &crate::syntax::render::read::Block,
    ) -> Vec<(u32, u32)> {
        let none = crate::syntax::ast::NONE;
        let mut around: Vec<_> = self
            .blocks()
            .into_iter()
            .filter(|b| b.node != none && b.node != sc.node && self.encloses(b.node, sc.node))
            .collect();
        around.sort_by_key(|b| std::cmp::Reverse(self.rd.main(b.node)));
        let name = |&(n, bin): &(u32, u32)| self.rd.model_view(n, bin).name.to_ascii_lowercase();
        let mut seen: Vec<String> = sc.models.iter().map(name).collect();
        let mut out = Vec::new();
        for b in around {
            for m in &b.models {
                let n = name(m);
                if !seen.contains(&n) {
                    seen.push(n);
                    out.push(*m);
                }
            }
        }
        out
    }

    /// `e`'s ternaries, in a `{}` value, as another family reads them
    /// (`ternary`): grouped as the source groups a then-branch's (C's, or
    /// ngspice's `{}` evaluator's), a blank before each `:` on Xyce.
    pub(super) fn ternaries(&self, e: &str) -> String {
        let read = match self.rd.b.expr_eval == ExprEval::Ngspice {
            true => Read::Ngspice,
            false => Read::C,
        };
        self.ternaries_as(e, read)
    }

    /// `e`'s ternaries for another family, grouped as `read` groups them,
    /// an `if()` one on ngspice's family; `log` renamed where its base
    /// differs (`ternary::rename_call`), `nint` rounding as the source's.
    pub(super) fn ternaries_as(&self, e: &str, read: Read) -> String {
        if self.rd.b.text_family == self.b.text_family {
            return e.to_string();
        }
        let (from, to) = (self.rd.b.expr_eval, self.b.expr_eval);
        // ngspice has no `if` (Xyce's, LTspice's): a ternary there.
        let e = match to == ExprEval::Ngspice {
            true => spell_if(e),
            false => e.to_string(),
        };
        // `log` is base 10 on Xyce, natural on ngspice, HSPICE, LTspice.
        let natural =
            |x: ExprEval| matches!(x, ExprEval::Ngspice | ExprEval::Hspice | ExprEval::Ltspice);
        let e = match (from, to) {
            (f, ExprEval::Xyce) if natural(f) => rename_call(&e, "log", "ln"),
            (ExprEval::Xyce, t) if natural(t) => rename_call(&e, "log", "log10"),
            _ => e,
        };
        let e = if self.nominal { nominal(&e) } else { e };
        let clamp = matches!(from, ExprEval::Xyce | ExprEval::Ltspice) && to == ExprEval::Ngspice;
        let e = if clamp { spell_clamp(&e) } else { e };
        let e = match &self.m_param {
            Some(m) => rename_word(&e, "m", m),
            None => e,
        };
        let e = match (from, to) {
            (ExprEval::Ngspice, ExprEval::Xyce) => spell_nint(&e, true),
            (ExprEval::Xyce, ExprEval::Ngspice) => spell_nint(&e, false),
            _ => e,
        };
        parenthesize(&e, read, self.b.param_form == ParamForm::Xyce)
    }
}

/// An R model's default resistance (`r=` or `resistance=`), unless it
/// sets `rsh`.
pub(super) fn r_value(kind: &str, params: &[(String, String)]) -> Option<String> {
    let r = matches!(kind.to_ascii_lowercase().as_str(), "r" | "res");
    let has = |k: &str| params.iter().find(|(p, _)| p.eq_ignore_ascii_case(k));
    if !r || has("rsh").is_some() {
        return None;
    }
    has("r").or(has("resistance")).map(|(_, v)| v.clone())
}

// ---- private ----

/// [`W::spelled`] for HSPICE.
fn hspice_value(v: &str) -> String {
    let t = v.trim();
    let number = |s: &str| {
        let m = s.strip_prefix(['-', '+']).unwrap_or(s);
        crate::unit::parse_spice_number(m)
            .is_some_and(|(_, end, _)| m[end..].bytes().all(|c| c.is_ascii_alphabetic()))
    };
    let word = |s: &str| s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
    let quoted = (t.starts_with('\'') || t.starts_with('"')) && t.len() > 1;
    // Words split at blanks outside parentheses (`unif(1k, 0.1)` is one); a
    // run of numbers is a vector (a CPL model's matrix), not an expression.
    let mut depth = 0i32;
    let words: Vec<&str> = t
        .split(|c: char| {
            depth += i32::from(c == '(') - i32::from(c == ')');
            depth == 0 && c.is_whitespace()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let plain = match words[..] {
        [w] => number(w) || word(w),
        _ => words.iter().all(|w| number(w)),
    };
    if !t.contains('{') && (quoted || plain) {
        return v.to_string();
    }
    let t = t
        .strip_prefix('\'')
        .and_then(|r| r.strip_suffix('\''))
        .unwrap_or(t);
    let mut depth = 0i32;
    let closes = t.char_indices().find(|&(_, c)| {
        depth += i32::from(c == '{') - i32::from(c == '}');
        depth == 0
    });
    let single = t.starts_with('{') && closes.is_some_and(|(i, _)| i + 1 == t.len());
    let e = if single { &t[1..t.len() - 1] } else { t };
    format!("'{}'", e.trim().replace('{', "(").replace('}', ")"))
}
