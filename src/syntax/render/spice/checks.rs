//! The deck-wide refusals the SPICE-family writer makes before it writes: a
//! statement kept as another family's text (`ForeignText`), text that did
//! not parse for another dialect (W0032), a `^` the
//! target reads with the other meaning (`CaretReread`), a statistical call
//! the target has no function for (`StatisticalCall`), an LTspice
//! `.tran … startup` the target has no ramp for (`TranStartup`), an ngspice
//! then-branch ternary another family groups otherwise (W0034, W0035),
//! Xyce's `sdt` on ngspice (W0045). Each reads every
//! block the deck writes (definitions, sections, branches) and every
//! expression the old reader kept as text.
//!
//! Cites: ngspice manual §2.8.5, §2.11.5; LTspice help (`^` XOR,
//! `gauss(x)`); Xyce RG §2.1.18.
//!
//! Contents:
//! - `W::checks(&mut self)`, `W::unparsed(&mut self)`, `W::blocks(&self) -> Vec<&Block>`

use super::W;
use super::ternary::{Nest, ngspice_nest};
use crate::syntax::behaviour::{ExprEval, Writer};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::read::{Block, Card, Comp, Func, Num, Val};

/// The statistical functions (ngspice §2.11.5).
const STATISTICAL: [&str; 5] = ["agauss", "gauss", "aunif", "unif", "limit"];

/// A deck-wide expression check: where `holds`, the first expression of
/// the deck that `finds` matches gets `then`.
struct ExprCheck {
    holds: fn(&W<'_, '_>) -> bool,
    finds: fn(&str) -> bool,
    then: Then,
}

/// What an expression check records.
#[derive(Clone, Copy)]
enum Then {
    /// A refusal with this code.
    Refuse(Code),
    /// The target's cell for this construct (`W::ask`).
    Ask(Construct),
}

/// The deck-wide expression checks, in the order they are recorded. Adding
/// one (a call or an operator the target has no form for): a row, gated on
/// a `Pair` field or the target's cells, finding the call with [`calls`].
const EXPR_CHECKS: &[ExprCheck] = &[
    // Xyce's time integral: ngspice's B source has `ddt`, no integral
    // (manual §5.1).
    ExprCheck {
        holds: |w| w.b.expr_eval == ExprEval::Ngspice,
        finds: |e| calls(e).any(|f| f.eq_ignore_ascii_case("sdt")),
        then: Then::Refuse(Code::W0045),
    },
    // `^` read with the other meaning (power or XOR).
    ExprCheck {
        holds: |w| w.pair.caret_reread,
        finds: |e| e.contains('^'),
        then: Then::Ask(Construct::CaretReread),
    },
    ExprCheck {
        holds: |w| w.cells[Construct::StatisticalCall as usize] != Action::Native,
        finds: |e| calls(e).any(|f| STATISTICAL.iter().any(|s| s.eq_ignore_ascii_case(f))),
        then: Then::Ask(Construct::StatisticalCall),
    },
];

impl W<'_, '_> {
    /// Records the deck-wide refusals.
    pub(super) fn checks(&mut self) {
        let blocks = self.blocks();
        // Text kept from another family reaches only its own; its own
        // family copies it unchecked.
        let raws: Vec<u32> = blocks.iter().flat_map(|b| self.raws(b)).collect();
        let foreign = |tok: &u32| self.rd.text_behaviours(*tok).text_family != self.b.text_family;
        let (other, own): (Vec<u32>, Vec<u32>) = raws.iter().partition(|t| foreign(t));
        // A statement whose target lacks it is that statement's refusal,
        // the rest foreign text's (one note each kind); CDL leaves each
        // out, noted where it does (`cdl::unwritten`).
        let mut asked = Vec::new();
        let cdl = self.b.writer == Writer::Cdl;
        for &tok in other.iter().filter(|_| !cdl) {
            let c = self.rd.kept_construct(tok);
            if !asked.contains(&c) {
                asked.push(c);
                self.ask(c, tok);
            }
        }
        own.iter().for_each(|&tok| self.note(Code::W0027, tok));
        if let Some(tok) = self.rd.tran_startup(&self.deck.bench) {
            self.ask(Construct::TranStartup, tok);
        }
        // A waveform's values past its form's count are not written.
        for b in &blocks {
            for &c in &b.comps {
                if let Comp::Card(n) = c
                    && let Card::Source { wave: Some(w), .. } = self.rd.card_view(n)
                    && w.cut
                {
                    self.note(Code::W0026, self.rd.main(n));
                }
            }
        }
        let held: Vec<&ExprCheck> = EXPR_CHECKS.iter().filter(|c| (c.holds)(self)).collect();
        // Every expression the deck carries, built once for the checks
        // below that read it (the seam P9-012's AST walk replaces).
        let exprs = match self.pair.ngspice_ternaries || !held.is_empty() {
            true => self.expressions(&blocks),
            false => Vec::new(),
        };
        self.ternary_checks(&blocks, &exprs);
        // Each check that holds, at the first expression it finds.
        for c in held {
            if let Some((_, tok)) = exprs.iter().find(|(e, _)| (c.finds)(e)) {
                match c.then {
                    Then::Refuse(code) => self.refuse(code, *tok),
                    Then::Ask(construct) => {
                        self.ask(construct, *tok);
                    }
                }
            }
        }
    }

    /// Text that did not parse is the source's own: its dialect gets it
    /// back as written, another target refuses it (W0032) unless the
    /// statement holding it was left out (a drop note at its first token,
    /// or the reader's `P0010`/`P0011` for a line its simulator rejects).
    /// Asked once the deck is written, when its drops are noted.
    pub(super) fn unparsed(&mut self) {
        if !self.pair.foreign_text {
            return;
        }
        let drops = [Code::W0021, Code::W0023, Code::W0024, Code::W0028];
        let mut dropped: Vec<(u32, u32)> = self
            .diags
            .iter()
            .filter(|d| drops.contains(&d.code))
            .map(|d| (d.token, self.rd.line_last(d.token)))
            .collect();
        // A `TABLE` source and Xyce's B POLY are read from their text
        // (`Rd::table_source`, `Rd::b_poly`), so a parse note inside either
        // is no unread text.
        let blocks = self.blocks();
        let comps = blocks.iter().flat_map(|b| &b.comps);
        dropped.extend(comps.filter_map(|c| match *c {
            Comp::Card(n)
                if matches!(self.rd.card_view(n), Card::Table { .. } | Card::Poly { .. }) =>
            {
                let t = self.rd.main(n);
                Some((t, self.rd.line_last(t)))
            }
            _ => None,
        }));
        // A line its own simulator rejects was left out by the reader
        // (`Rd::rejected`, its note at the line's first token): text
        // inside it is no unread text.
        let rejected = [Code::P0010, Code::P0011];
        let notes = self.deck.notes.iter();
        let left_out = notes.filter(|d| rejected.contains(&d.code));
        dropped.extend(left_out.map(|d| (d.token, self.rd.line_last(d.token))));
        let unread = [Code::P0002, Code::P0003, Code::P0008];
        let bad = self.rd.ast.diags().iter().find(|d| {
            unread.contains(&d.code) && !dropped.iter().any(|&(a, b)| (a..=b).contains(&d.token))
        });
        if let Some(d) = bad {
            self.refuse(Code::W0032, d.token);
        }
    }
}

impl<'r> W<'_, 'r> {
    /// Every block the deck writes from: the top, its definitions, each
    /// section's body and definitions, and the branches inside each.
    pub(super) fn blocks(&self) -> Vec<&'r Block> {
        let deck = self.deck;
        let sections = deck
            .sections
            .iter()
            .flat_map(|s| std::iter::once(&s.body).chain(&s.defs));
        let mut out = Vec::new();
        let mut work: Vec<&Block> = std::iter::once(&deck.top)
            .chain(&deck.defs)
            .chain(sections)
            .collect();
        work.reverse();
        while let Some(b) = work.pop() {
            out.push(b);
            let branches = b.conds.iter().flat_map(|(_, c)| &c.branches);
            let mut inner: Vec<&Block> = branches.map(|(_, body)| body).collect();
            inner.reverse();
            work.extend(inner);
        }
        out
    }
}

// ---- private ----

impl<'r> W<'_, 'r> {
    /// An ngspice deck's then-branch ternary on another family
    /// (`ternary`): in a `{}` value, written as ngspice's evaluator reads
    /// it (W0034), or refused when nested deeper (W0035); in a function,
    /// refused, as ngspice reads its body by C's grammar in a B source
    /// and its own way in a `{}` value. A B source's is C's (no code).
    fn ternary_checks(&mut self, blocks: &[&Block], exprs: &[(String, u32)]) {
        if !self.pair.ngspice_ternaries {
            return;
        }
        let deck = self.deck;
        let funcs = deck.funcs.len() + deck.sections.iter().map(|s| s.funcs.len()).sum::<usize>();
        let mut b_exprs: Vec<(String, u32)> = Vec::new();
        for b in blocks {
            for &c in &b.comps {
                if let Comp::Card(n) = c
                    && let Card::Behavioural { expr, .. } = self.rd.card_view(n)
                {
                    b_exprs.push((expr, self.rd.main(n)));
                }
            }
        }
        for (i, (e, tok)) in exprs.iter().enumerate() {
            match (i < funcs, ngspice_nest(e)) {
                (_, Nest::Flat) => {}
                (true, _) | (false, Nest::Deep) => self.refuse(Code::W0035, *tok),
                (false, Nest::One) if !b_exprs.contains(&(e.clone(), *tok)) => {
                    self.note(Code::W0034, *tok)
                }
                (false, Nest::One) => {}
            }
        }
    }

    /// A function's first token.
    fn func_token(&self, f: Func) -> u32 {
        let (Func::Param(n) | Func::Card(n) | Func::Real(n)) = f;
        self.rd.main(n)
    }

    /// The first `statistics` statement's first token, where the draws it
    /// lowers to anchor; 0 when the blocks hold none.
    fn statistics_token(&self, blocks: &[&Block]) -> u32 {
        let st = blocks.iter().find_map(|b| b.statistics.first());
        st.map_or(0, |&n| self.rd.main(n))
    }

    /// The first token of each of block `b`'s statements kept as text: its
    /// lines, then its elements.
    fn raws(&self, b: &Block) -> Vec<u32> {
        let lines = b.raw.iter().map(|&r| match r {
            crate::syntax::render::read::Raw::Line(n)
            | crate::syntax::render::read::Raw::Lines(n)
            | crate::syntax::render::read::Raw::Joined(n) => n,
        });
        let comps = b.comps.iter().filter_map(|c| match c {
            Comp::Raw(n) => Some(*n),
            Comp::Card(_) => None,
        });
        lines.chain(comps).map(|n| self.rd.main(n)).collect()
    }

    /// Every expression the deck's cards carry as text: functions,
    /// parameters, model, instance and element values, and the draws the
    /// `statistics` lift adds. Text kept as lines is not: it reaches its
    /// own dialect only.
    fn expressions(&self, blocks: &[&Block]) -> Vec<(String, u32)> {
        let deck = self.deck;
        let funcs = deck
            .funcs
            .iter()
            .chain(deck.sections.iter().flat_map(|s| &s.funcs));
        let mut out: Vec<(String, u32)> = funcs
            .map(|&f: &Func| (self.rd.func_view(f).2, self.func_token(f)))
            .collect();
        for b in blocks {
            if b.node != crate::syntax::ast::NONE {
                let head = self
                    .rd
                    .subckt_head(b.node)
                    .expect("the reader read the head");
                let tok = self.rd.main(b.node);
                out.extend(head.params.into_iter().map(|(_, v)| (v, tok)));
            }
            for &p in b.params.iter().chain(&b.locals) {
                out.push((self.rd.param_view(p).1, self.rd.main(p)));
            }
            for &(m, bin) in &b.models {
                let tok = self.rd.main(m);
                let params = self.rd.model_view(m, bin).params;
                out.extend(params.into_iter().map(|(_, v)| (v, tok)));
            }
            for &(_, i) in &b.insts {
                if let Card::Inst { params, .. } = self.rd.card_view(i) {
                    let tok = self.rd.main(i);
                    out.extend(params.into_iter().map(|(_, v)| (v, tok)));
                }
            }
            for &c in &b.comps {
                if let Comp::Card(n) = c {
                    card_exprs(self.rd.card_view(n), self.rd.main(n), &mut out);
                }
            }
        }
        // The `statistics` lift's draws, as the old writer checked the
        // lowered deck.
        let at = self.statistics_token(blocks);
        out.extend(self.draws.texts().map(|t| (t.to_string(), at)));
        out
    }
}

/// The expressions one element card carries (the old IR's
/// `Component::exprs`): its parameters, its waveform's expressions, an
/// expression value, a behavioural source's or charge's expression.
fn card_exprs(c: Card, tok: u32, out: &mut Vec<(String, u32)>) {
    let expr = |v: Val| match v {
        Val::Expr(e) => Some((e, tok)),
        Val::Num(_) | Val::Raw(_) => None,
    };
    let at = |(_, v): (String, String)| (v, tok);
    match c {
        Card::Rcl { value, params, .. } => {
            out.extend(params.into_iter().map(at));
            out.extend(expr(value));
        }
        Card::Source { value, wave, .. } => {
            if let Some(w) = wave {
                let nums = w.args.into_iter().chain(w.repeat).chain(w.delay);
                out.extend(nums.filter_map(|n| match n {
                    Num::Expr(e) => Some((e, tok)),
                    Num::Num(_) => None,
                }));
            }
            out.extend(expr(value));
        }
        Card::Behavioural { expr, .. } | Card::Table { expr, .. } => out.push((expr, tok)),
        Card::Charge { charge, params, .. } => {
            out.push((charge, tok));
            out.extend(params.into_iter().map(at));
        }
        Card::Semi { params, .. } => out.extend(params.into_iter().map(at)),
        Card::Coupling { .. }
        | Card::Linear { .. }
        | Card::Poly { .. }
        | Card::Switch { .. }
        | Card::Line { .. }
        | Card::Inst { .. } => {}
    }
}

/// The names `e` calls: each identifier followed by `(`.
// ponytail: a scan over the kept text, as the old writer's `calls`; the
// expression tree's `Call` nodes when the reader keeps nodes, not text.
fn calls(e: &str) -> impl Iterator<Item = &str> {
    let b = e.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    (0..b.len()).filter_map(move |i| {
        let starts = b[i].is_ascii_alphabetic() && (i == 0 || !ident(b[i - 1]) && b[i - 1] != b'.');
        if !starts {
            return None;
        }
        let end = i + b[i..].iter().take_while(|&&c| ident(c)).count();
        let open = end
            + b[end..]
                .iter()
                .take_while(|c| c.is_ascii_whitespace())
                .count();
        (b.get(open) == Some(&b'(')).then(|| &e[i..end])
    })
}
