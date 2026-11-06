//! The deck-wide refusals the SPICE-family writer makes before it writes: a
//! statement kept as another family's text (`ForeignText`), text that did
//! not parse for another dialect (W0032), a `^` the
//! target reads with the other meaning (`CaretReread`), a statistical call
//! the target has no function for (`StatisticalCall`), an ngspice
//! then-branch ternary another family groups otherwise (W0034, W0035),
//! Xyce's `sdt` on ngspice (W0045). Each reads every
//! block the deck writes (definitions, sections, branches) and every
//! expression the old reader kept as text.
//!
//! Cites: ngspice manual §2.8.5, §2.11.5; LTspice help (`^` XOR,
//! `gauss(x)`); Xyce RG §2.1.18.
//!
//! Contents:
//! - `W::checks(&mut self)`, `W::unparsed(&mut self)`

use super::W;
use super::ternary::{Nest, ngspice_nest};
use crate::syntax::Dialect;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::read::{Block, Card, Comp, Func, Num, Val};

/// The statistical functions (ngspice §2.11.5).
const STATISTICAL: [&str; 5] = ["agauss", "gauss", "aunif", "unif", "limit"];

impl W<'_, '_> {
    /// Records the deck-wide refusals.
    pub(super) fn checks(&mut self) {
        let blocks = self.blocks();
        // Text kept from another family reaches only its own; its own
        // family copies it unchecked.
        let raws: Vec<u32> = blocks.iter().flat_map(|b| self.raws(b)).collect();
        let foreign = |tok: &u32| self.rd.text_dialect(*tok).family() != self.d.family();
        let (other, own): (Vec<u32>, Vec<u32>) = raws.iter().partition(|t| foreign(t));
        // CDL leaves each out, noted where it does (`cdl::unwritten`).
        if let Some(&tok) = other.first()
            && self.d != Dialect::Cdl
        {
            self.ask(Construct::ForeignText, tok);
        }
        own.iter().for_each(|&tok| self.note(Code::W0027, tok));
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
        self.ternary_checks(&blocks);
        // Xyce's time integral: ngspice's B source has `ddt`, no integral
        // (manual §5.1).
        if self.d.family() == Dialect::Ngspice {
            let exprs = self.expressions(&blocks);
            let sdt = |f: &str| f.eq_ignore_ascii_case("sdt");
            if let Some((_, tok)) = exprs.iter().find(|(e, _)| calls(e).any(sdt)) {
                self.refuse(Code::W0045, *tok);
            }
        }
        let caret = caret_means_power(self.rd.d) != caret_means_power(self.d);
        let stat = crate::syntax::render::capability::action(self.d, Construct::StatisticalCall)
            != Action::Native;
        if !caret && !stat {
            return;
        }
        let exprs = self.expressions(&blocks);
        if caret && let Some((_, tok)) = exprs.iter().find(|(e, _)| e.contains('^')) {
            self.ask(Construct::CaretReread, *tok);
        }
        let is_stat = |f: &str| STATISTICAL.iter().any(|s| s.eq_ignore_ascii_case(f));
        if stat && let Some((_, tok)) = exprs.iter().find(|(e, _)| calls(e).any(is_stat)) {
            self.ask(Construct::StatisticalCall, *tok);
        }
    }

    /// Text that did not parse is the source's own: its dialect gets it
    /// back as written, another target refuses it (W0032) unless the
    /// statement holding it was left out (a drop note at its first token).
    /// Asked once the deck is written, when its drops are noted.
    pub(super) fn unparsed(&mut self) {
        if self.rd.d.family() == self.d.family() {
            return;
        }
        let drops = [Code::W0021, Code::W0023, Code::W0024, Code::W0028];
        let dropped: Vec<(u32, u32)> = self
            .diags
            .iter()
            .filter(|d| drops.contains(&d.code))
            .map(|d| (d.token, self.rd.line_last(d.token)))
            .collect();
        let unread = [Code::P0002, Code::P0003, Code::P0008];
        let bad = self.rd.ast.diags().iter().find(|d| {
            unread.contains(&d.code) && !dropped.iter().any(|&(a, b)| (a..=b).contains(&d.token))
        });
        if let Some(d) = bad {
            self.refuse(Code::W0032, d.token);
        }
    }
}

// ---- private ----

impl<'r> W<'_, 'r> {
    /// An ngspice deck's then-branch ternary on another family
    /// (`ternary`): in a `{}` value, written as ngspice's evaluator reads
    /// it (W0034), or refused when nested deeper (W0035); in a function,
    /// refused, as ngspice reads its body by C's grammar in a B source
    /// and its own way in a `{}` value. A B source's is C's (no code).
    fn ternary_checks(&mut self, blocks: &[&Block]) {
        if self.rd.d.family() != Dialect::Ngspice || self.d.family() == Dialect::Ngspice {
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
        let exprs = self.expressions(blocks);
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

    /// Every block the deck writes from: the top, its definitions, each
    /// section's body and definitions, and the branches inside each.
    fn blocks(&self) -> Vec<&'r Block> {
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
        Card::Behavioural { expr, .. } => out.push((expr, tok)),
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

/// `^` means power in ngspice, HSPICE, ESPice and Spectre, but XOR in
/// LTspice, Xyce and VACASK.
fn caret_means_power(d: Dialect) -> bool {
    !matches!(d, Dialect::Ltspice | Dialect::Xyce | Dialect::Vacask)
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
