//! A value's text as the target reads it: HSPICE's quoting, and the
//! expression spellings another family needs (`REWRITES`, each spelled in
//! `ternary`: `if()`, `log`'s base, `nint`'s rounding, a clamp,
//! statistical calls at their nominal, a Xyce `M` renamed; then the
//! ternaries' grouping), and an R model's value where Xyce reads its model
//! `R` as a multiplier.
//!
//! Cites: per item.
//!
//! Contents:
//! - `W::spelled(&self, v: &str) -> String`, `W::ternaries(&self, e: &str) -> String`,
//!   `W::ternaries_as(&self, e: &str, read: Read) -> String`
//! - `W::m_rename(&self, params: &[(String, String)]) -> Option<String>`
//! - `W::declares(&self, master: &str, p: &str) -> bool`, `W::r_default(&self, letter: char, m: &str) -> Option<String>`
//! - `W::undeclared(&mut self, sc: &Block, head: &[(String, String)]) -> Vec<(String, String)>`
//! - `W::encloses(&self, outer: u32, inner: u32) -> bool`, `W::inherited_models(&self, sc: &Block) -> Vec<(u32, u32)>`
//! - `pub(super) fn r_value(kind: &str, params: &[(String, String)]) -> Option<String>`

use super::W;
use super::powers::{Grouping, Law, group_powers};
use super::ternary::{
    Read, nominal, parenthesize, rename_call, rename_word, spell_clamp, spell_if, spell_nint,
};
use super::ternary::{brace_groups, ngspice_power_calls, xyce_domains, xyce_power_calls};
use crate::syntax::behaviour::{ExprEval, ParamForm, ValueQuoting};
use crate::syntax::diag::Code;
use crate::syntax::render::read::{Block, Card};

/// One expression rewrite: `apply` where `holds`.
struct Rewrite {
    holds: fn(&W<'_, '_>) -> bool,
    apply: fn(&W<'_, '_>, &str) -> String,
}

/// The rewrites [`W::ternaries_as`] makes to an expression written for
/// another text family, in order: each reads the one before's output.
/// Adding one: a row at its place in the order, gated on a `Pair` field
/// (`pair.rs`; a pair rule) or the writer's state, its spelling a function
/// in `ternary.rs` (`map_calls` renames or rewrites one call).
const REWRITES: &[Rewrite] = &[
    // ngspice has no `if` (Xyce's, LTspice's): a ternary there.
    Rewrite {
        holds: |w| w.pair.spell_if,
        apply: |_, e| spell_if(e),
    },
    // A brace group inside an expression, which ngspice's B-source
    // reader drops the braces of: parentheses.
    Rewrite {
        holds: |w| w.pair.brace_groups,
        apply: |_, e| brace_groups(e),
    },
    // `log` is base 10 on Xyce, natural on ngspice, HSPICE, LTspice.
    Rewrite {
        holds: |w| w.pair.log_to_ln,
        apply: |_, e| rename_call(e, "log", "ln"),
    },
    Rewrite {
        holds: |w| w.pair.log_to_log10,
        apply: |_, e| rename_call(e, "log", "log10"),
    },
    // A statistical call at its nominal (`W::nominal`).
    Rewrite {
        holds: |w| w.nominal,
        apply: |_, e| nominal(e),
    },
    Rewrite {
        holds: |w| w.pair.spell_clamp,
        apply: |_, e| spell_clamp(e),
    },
    // A Xyce subcircuit's declared `M`, renamed in its body (`W::m_rename`).
    Rewrite {
        holds: |w| w.m_param.is_some(),
        apply: |w, e| rename_word(e, "m", w.m_param.as_deref().expect("an M renamed")),
    },
    Rewrite {
        holds: |w| w.pair.nint_to_xyce,
        apply: |_, e| spell_nint(e, true),
    },
    Rewrite {
        holds: |w| w.pair.nint_to_ngspice,
        apply: |_, e| spell_nint(e, false),
    },
    // Xyce's value of a call outside its real domain, where ngspice
    // fails the analysis.
    Rewrite {
        holds: |w| w.pair.xyce_domains,
        apply: |_, e| xyce_domains(e),
    },
    // Xyce's `pow`, `pwr`, `pwrs` as powers, for the power row.
    Rewrite {
        holds: |w| w.pair.xyce_power,
        apply: |_, e| xyce_power_calls(e),
    },
    // A power chain, a power under a unary sign: the source's grouping in
    // parentheses where the target groups another way; a power the target
    // gives another value: respelled by the pair's law (`powers`). Last:
    // a row that spells a call as a power goes before it.
    Rewrite {
        holds: |w| {
            power_law(w) != Law::Keep
                || Grouping::of(&w.rd.t.lexicon_at(0).ops) != Grouping::of(&w.sp.lexicon.ops)
        },
        apply: |w, e| {
            let src = Grouping::of(&w.rd.t.lexicon_at(0).ops);
            let tgt = Grouping::of(&w.sp.lexicon.ops);
            group_powers(e, src, tgt, w.rd.b.caret_power, power_law(w))
        },
    },
];

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
        let v = match self.pair.quoted_expr && quoted {
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
        let renamed = self.pair.xyce_m_rename && taken("m");
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

    /// The parameters the deck's `X` lines pass to subcircuit `sc` that
    /// neither its head (`head`) nor a `.param` of its body declares, each
    /// with its default on the head, on ngspice from Xyce. Xyce sets such
    /// a parameter for the instance (7.10 runs of XYCE_CORPUS
    /// `Certification_Tests/BUG_1558/bug_1558_{1,3,4}.cir`: R = 100k, over
    /// the top-level `.param R=1k`); ngspice-45 stops on the line
    /// ("Mismatch: 0 formal but 1 actual params"). The default is the
    /// top-level `.param` of that name, which a line that passes other
    /// parameters reads on Xyce, else the first value passed. ngspice
    /// reads a body `.param` as a head default already. `M` is the
    /// multiplicity (RG `X`), never a parameter passed. An `X` line with
    /// no parameter at all computes on Xyce with what the line of `sc`
    /// before it passed (7.10 run), which no default says: refused
    /// (W0140) beside any line that passes one the head lacks.
    pub(super) fn undeclared(
        &mut self,
        sc: &Block,
        head: &[(String, String)],
    ) -> Vec<(String, String)> {
        if !self.pair.xyce_undeclared {
            return Vec::new();
        }
        let has =
            |ps: &[(String, String)], k: &str| ps.iter().any(|(p, _)| p.eq_ignore_ascii_case(k));
        let views = |ps: &[u32]| {
            ps.iter()
                .map(|&a| self.rd.param_view(a))
                .collect::<Vec<_>>()
        };
        let (local, global) = (views(&sc.locals), views(&self.deck.top.params));
        // ponytail: every `X` line of the deck read again per subcircuit;
        // one master → lines table built in `write` when a Xyce deck of
        // thousands of subcircuits is slow.
        let mut uses: Vec<(u32, Vec<(String, String)>)> = Vec::new();
        for b in self.blocks() {
            for &(_, n) in &b.insts {
                if let Card::Inst { master, params, .. } = self.rd.card_view(n)
                    && master.eq_ignore_ascii_case(&sc.name)
                {
                    uses.push((n, params));
                }
            }
        }
        let mut out: Vec<(String, String)> = Vec::new();
        let mut passed = false;
        for (k, v) in uses.iter().flat_map(|(_, params)| params) {
            if has(head, k) || v.is_empty() || k.eq_ignore_ascii_case("m") {
                continue;
            }
            passed = true;
            if has(&local, k) || has(&out, k) {
                continue;
            }
            let default = global.iter().find(|(g, _)| g.eq_ignore_ascii_case(k));
            out.push((k.clone(), default.map_or(v, |(_, d)| d).clone()));
        }
        for (n, params) in &uses {
            if passed && params.is_empty() {
                self.refuse(Code::W0140, self.rd.main(*n));
            }
        }
        debug_assert!(out.iter().all(|(k, _)| !has(head, k)), "the head has none");
        debug_assert!(!has(&out, "m"), "M is the multiplicity");
        out
    }

    /// The resistance an R model `m` gives an instance with no value,
    /// on Xyce from another dialect: ngspice's model `r` or `resistance`
    /// is the default resistance (manual §3.3.2), Xyce's model `R` a
    /// multiplier on the instance's 1k (RG 7.10 resistor; Xyce by hand:
    /// `.model my r r=2k` with `r1 2 0 my` is 2 MΩ), so the model leaves
    /// it out (`W::model`) and the instance takes it. `None` elsewhere, or
    /// for a model that also sets `rsh` (its geometry decides).
    pub(super) fn r_default(&self, letter: char, m: &str) -> Option<String> {
        if letter != 'R' || !self.pair.r_model_multiplies {
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

    /// `e` for another family: each of [`REWRITES`] that holds, in order,
    /// then its ternaries grouped as `read` groups them (a blank before
    /// each `:` on Xyce). As written for the source's own text family.
    pub(super) fn ternaries_as(&self, e: &str, read: Read) -> String {
        if !self.pair.foreign_text {
            return e.to_string();
        }
        let mut e = e.to_string();
        // ngspice's power calls differ between its B-source evaluator and
        // its `{}` one: spelled as powers here, where the evaluator is
        // known. The B-source one reads a B source (`read` is C's for an
        // ngspice deck there only, `cards::behavioural_card`) and every
        // expression with a probe (a behavioural R, C or L).
        if self.pair.abs_power_out {
            e = ngspice_power_calls(&e, read == Read::C || has_probe(&e));
        }
        for r in REWRITES.iter().filter(|r| (r.holds)(self)) {
            e = (r.apply)(self, &e);
        }
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

/// The law a power is respelled by between `w`'s source and target
/// (each `Pair` field carries its evidence).
fn power_law(w: &W<'_, '_>) -> Law {
    let p = &w.pair;
    match (p.abs_power_out, p.abs_power_in, p.xyce_power) {
        (true, _, _) => Law::Abs,
        (_, true, true) => Law::Real,
        (_, true, false) => Law::Signed,
        _ => Law::Keep,
    }
}

/// Whether `e` reads a node voltage or a branch current (`v(a)`,
/// `i(v1)`): ngspice's `{}` evaluator has neither, so its B-source
/// evaluator reads such an expression wherever it stands.
fn has_probe(e: &str) -> bool {
    let b = e.as_bytes();
    (1..b.len()).any(|i| {
        b[i] == b'('
            && matches!(b[i - 1], b'v' | b'V' | b'i' | b'I')
            && (i == 1 || !(b[i - 2].is_ascii_alphanumeric() || b[i - 2] == b'_'))
    })
}

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
