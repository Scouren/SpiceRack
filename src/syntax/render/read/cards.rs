//! SPICE dot cards → where the plan places them: includes and `.lib`
//! calls, `.global`, `.model`, `.param`, `.func`, a `.subckt`'s head, and the
//! testbench cards (analyses, options, outputs, `.control` commands), as
//! the old lowering placed them, and their views; each card first checked
//! against its rule in the old SPICE grammar (a card no rule reads is `Raw`
//! or `Unparsed` by its word, `Rd::refused`).
//!
//! Cites: the old lowering;
//! the old SPICE grammar's `dotcard` and its overlays (`Include`, `Lib`, `Model`,
//! `Params`, `Func`, `Options`, `Temp`, `Ic`, `Save`, `Out`, `Print`, `Op`
//! … `Four`, ngspice `Sp`, HSPICE `Tran`/`Probe`, LTspice `Tran`/`Lib`/
//! `Step`/`Params_lt`, Xyce `Include`/`Options`/`Step`); Xyce RG p.96.
//!
//! Contents:
//! - `pub(in crate::syntax::render) struct Head`, `struct Model`
//! - `Rd::dot_card(&mut self, n: u32, here: &mut Block, scope: Scope, inside: bool, acc: &mut Acc)`
//! - `Rd::subckt_head(&self, n: u32) -> Option<Head>`, `Rd::model_head(&self, n: u32) -> Option<Node>`
//! - `Rd::model_view(&self, n: u32) -> Model`, `Rd::include_path(&self, n: u32) -> String`,
//!   `Rd::lib_view(&self, n: u32) -> (String, String)`, `Rd::param_view(&self, a: u32) -> (String, String)`,
//!   `Rd::file_order(&self, b: &Block) -> Vec<(u32, bool)>`,
//!   `Rd::global_param(&self, a: u32) -> bool`, `Rd::dot_rule(&self, word: &str) -> Option<DotRule>`,
//!   `Rd::is_word_item(&self, n: u32) -> bool`,
//!   `Rd::param_winners(&self, deck: &mut Deck, to: &Behaviours)`

use std::collections::HashMap;

use super::{Acc, Block, Deck, Func, Kept, Rd, Scope};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::{Behaviours, CardRules, DotRule, NetlistRules};
use crate::syntax::token::Tag as T;

/// A `.subckt` head as the old grammar read it: its ports and parameter
/// defaults (each its text, trimmed).
pub(in crate::syntax::render) struct Head {
    pub(in crate::syntax::render) ports: Vec<String>,
    pub(in crate::syntax::render) params: Vec<(String, String)>,
}

/// A `.model` card as the old reader kept it.
pub(in crate::syntax::render) struct Model {
    pub(in crate::syntax::render) name: String,
    pub(in crate::syntax::render) kind: String,
    /// Each `k=v` (a flag's value empty), a matrix joined by blanks.
    pub(in crate::syntax::render) params: Vec<(String, String)>,
}

impl Rd<'_> {
    /// A dot card at `scope` (`inside` a `.subckt`).
    pub(super) fn dot_card(
        &mut self,
        n: u32,
        here: &mut Block,
        scope: Scope,
        inside: bool,
        acc: &mut Acc,
    ) {
        // quirk: the old overlays' `dialect` rule (the old LTspice grammar's
        // `Raw_lt`, the old Xyce grammar's `Raw_xy`) is tried before the
        // shared dot cards.
        let word = self.head_kind(n);
        if self.dialect_card(n, &word, scope, acc) {
            return;
        }
        if self.dialect_raw(&word) {
            return self.keep(n, here, Kept::Raw);
        }
        let done = match self.tag(n) {
            Tag::Include => self.include(n, here),
            Tag::LibCall => self.lib_call(n, here),
            Tag::Global => self.global(n, acc),
            Tag::Model => self.model(n, here),
            Tag::Param => self.param_card(n, here, inside, acc),
            Tag::Func => self.func(n, here, inside, acc),
            Tag::Card => Err(self.refused(n)),
            _ if scope != Scope::Top => match self.testbench_peg(n) {
                true => Err(Kept::Raw),
                false => Err(self.refused(n)),
            },
            _ => match (self.testbench_peg(n), self.testbench_ok(n)) {
                (true, true) => {
                    acc.bench.push(super::Bench::Card(n));
                    Ok(())
                }
                (true, false) => Err(Kept::Raw),
                (false, _) => Err(self.refused(n)),
            },
        };
        if let Err(k) = done {
            self.keep(n, here, k);
        }
    }

    /// The head of `.subckt name ports [params:] k=v`, when the old SPICE grammar's
    /// `Subckt` reads it.
    pub(in crate::syntax::render) fn subckt_head(&self, n: u32) -> Option<Head> {
        let items = self.fields(n);
        if self.named() {
            items.first()?;
            return Some(Head {
                ports: items[1..].iter().map(|&p| self.name_of(p)).collect(),
                params: vec![],
            });
        }
        let (&name, rest) = items.split_first()?;
        if !self.is_word_item(name) {
            return None;
        }
        let mut ports = Vec::new();
        let mut params = Vec::new();
        let mut k = 0;
        while k < rest.len() && self.is_word_item(rest[k]) && !self.is_params_colon(rest[k]) {
            ports.push(self.name_of(rest[k]));
            k += 1;
        }
        if k < rest.len() && self.is_params_colon(rest[k]) {
            k += 1;
        }
        for &a in &rest[k..] {
            let v = self.ast.rhs(Node(a));
            if self.tag(a) != Tag::Assign
                || !self.is_word_item(self.ast.lhs(Node(a)))
                || !self.val_ok(v)
            {
                return None;
            }
            params.push((self.key(a), self.etext(v).trim().to_string()));
        }
        Some(Head { ports, params })
    }

    /// A `.model` card's name item, when the old SPICE grammar's `Model` reads the card.
    pub(in crate::syntax::render) fn model_head(&self, n: u32) -> Option<Node> {
        let items = self.fields(n);
        let [name, kind, rest @ ..] = items else {
            return None;
        };
        if !self.is_word_item(*name) || !self.is_word_item(*kind) {
            return None;
        }
        // quirk: the old SPICE grammar's `mbody`: parentheses and commas
        // separate, nothing else.
        let mut prev = self.last(*kind);
        for &k in rest {
            if !(prev + 1..self.first(k))
                .all(|x| matches!(self.ttag(x), T::Comma | T::LParen | T::RParen))
            {
                return None;
            }
            prev = self.last(k);
        }
        if !(prev + 1..=self.line_last(self.main(n)))
            .all(|x| matches!(self.ttag(x), T::Comma | T::LParen | T::RParen))
        {
            return None;
        }
        let xyce = self.b.card_rules == CardRules::Xyce;
        let mval = |v: u32| match self.tag(v) {
            Tag::Word | Tag::Str => true,
            Tag::Delimited => self.expr_ok(v) || (xyce && self.ttag(self.main(v)) == T::LBrace),
            _ => false,
        };
        let ok = rest.iter().all(|&k| match self.tag(k) {
            Tag::Word => true,
            Tag::Assign => {
                let v = self.ast.rhs(Node(k));
                v != NONE
                    && match self.tag(v) {
                        Tag::Seq => {
                            let parts = self.ast.list(self.ast.lhs(Node(v)));
                            mval(parts[0]) && parts[1..].iter().all(|&p| self.is_word_item(p))
                        }
                        _ => mval(v),
                    }
            }
            _ => false,
        });
        ok.then_some(Node(*name))
    }

    /// A `.model` card's view (an `ako:` card as the model it derives,
    /// `Rd::derived`; Xyce's BSIM3 level 8 or 9 read as 49), or a Spectre
    /// or VACASK model's, its bin `bin`'s (`NONE`: none).
    pub(in crate::syntax::render) fn model_view(&self, n: u32, bin: u32) -> Model {
        if self.named() {
            return self.named_model_view(n, bin);
        }
        let items = self.fields(n);
        let mut m = Model {
            name: self.text(items[0]),
            kind: self.text(items[1]),
            params: self.params_of(&items[2..], true),
        };
        self.derived(&mut m);
        if self.b.card_rules == CardRules::Xyce
            && matches!(m.kind.to_ascii_lowercase().as_str(), "nmos" | "pmos")
        {
            // Xyce's BSIM3 is level 9 (RG Table 2-36): the IR keeps 49.
            for (k, v) in &mut m.params {
                if k.eq_ignore_ascii_case("level") && matches!(v.parse::<f64>(), Ok(8.0 | 9.0)) {
                    *v = "49".into();
                }
            }
        }
        m
    }

    /// An `.include` card's path (Xyce `.incl` takes the rest of the card).
    pub(in crate::syntax::render) fn include_path(&self, n: u32) -> String {
        if self.named() {
            return self.unquote(self.fields(n)[0]);
        }
        let items = self.fields(n);
        let first = items[0];
        if self.b.card_rules == CardRules::Xyce && self.head_kind(n) == "incl" {
            // quirk: the old Xyce grammar's `Path_xy`: a quoted path alone,
            // else all of the rest of the card.
            match items {
                [p] if self.tag(*p) == Tag::Str => self.path_of(*p),
                _ => self.span(self.main(first), self.line_last(self.main(n))),
            }
        } else if self.b.card_rules == CardRules::Ltspice && items.len() > 1 {
            // LTspice help, `.INCLUDE <filename>`: the rest of the card.
            self.rest_path(n)
        } else {
            self.path_of(first)
        }
    }

    /// A `.lib path section` card's path and section (LTspice's whole-file
    /// `.lib path`: an empty section).
    pub(in crate::syntax::render) fn lib_view(&self, n: u32) -> (String, String) {
        if self.named() {
            let sec = self
                .get(&self.pairs(n), "section")
                .expect("an include with a section");
            return (self.unquote(self.fields(n)[0]), self.text(sec));
        }
        match self.fields(n) {
            [p] => (self.path_of(*p), String::new()),
            // LTspice has no sections: the rest of the card is the file,
            // blanks and all (LTspice help, `.LIB <filename>`; the
            // corpus's `.lib C:\users\brian\My Documents\…\standard.dio`).
            [_, ..] if self.b.card_rules == CardRules::Ltspice => {
                (self.rest_path(n), String::new())
            }
            [p, s, ..] => (self.path_of(*p), self.text(*s)),
            [] => unreachable!("lib_call read it"),
        }
    }

    /// Block `b`'s `.include` and `.lib` cards in statement order, each
    /// with whether it is a `.lib` call. A file read later may set again
    /// what an earlier one set (ngspice-45: the later `.param` wins), so a
    /// writer keeps the deck's order.
    pub(in crate::syntax::render) fn file_order(&self, b: &Block) -> Vec<(u32, bool)> {
        let includes = b.includes.iter().map(|&n| (n, false));
        let mut cards: Vec<(u32, bool)> =
            includes.chain(b.libs.iter().map(|&n| (n, true))).collect();
        // A statement's main token is its place in the source.
        cards.sort_by_key(|&(n, _)| self.main(n));
        cards
    }

    /// Whether the `.param` assignment `a` is an Xyce `.global_param`'s:
    /// Xyce runs a constant instance `temp={…}` from a `.param` at the
    /// swept temperature, from a `.global_param` at its value
    /// (Xyce_Regression BUG_1203_SON/dot_step.cir, run both ways), so
    /// Xyce writes it back as `.global_param`.
    pub(in crate::syntax::render) fn global_param(&self, a: u32) -> bool {
        self.global_params.contains(&a)
    }

    /// A `.param` assignment's name and value text.
    pub(in crate::syntax::render) fn param_view(&self, a: u32) -> (String, String) {
        (
            self.text(self.ast.lhs(Node(a))),
            self.vtext(self.ast.rhs(Node(a))),
        )
    }

    /// Leaves, of the definitions one block gives one parameter name, the
    /// one the source reads, when `to` would read another: the writers
    /// write `deck`'s parameters as they stand. Xyce keeps the first
    /// definition (7.10 on `.param rx=1k`, `.param rx=3k`: 1 kΩ), ngspice
    /// and HSPICE the last (ngspice-45: 3 kΩ, and a parameter between the
    /// two reads the last; HSPICE User Guide: Simulation and Analysis
    /// E-2010.12 p.340); LTspice, VACASK and Spectre refuse a second one
    /// (Spectre Reference 19.1, option `redefinedparams`; under `warning`
    /// it keeps the last). A deck for the source's own rules, or between
    /// two that keep the last, is left as read.
    pub(in crate::syntax::render) fn param_winners(&self, deck: &mut Deck, to: &Behaviours) {
        let (from, to) = (self.b.netlist_rules, to.netlist_rules);
        let last = |r: NetlistRules| matches!(r, NetlistRules::Ngspice | NetlistRules::Hspice);
        if from == to || (last(from) && last(to)) {
            return;
        }
        // ponytail: two named languages share `NetlistRules::Spice`, so
        // Spectre → VACASK keeps a redefinition Spectre only reads under
        // `redefinedparams=warning`; a named rule set of its own when one
        // such deck turns up. A name defined in an `.if` branch and again
        // outside it is two blocks' and is left alone.
        let first = from == NetlistRules::Xyce;
        let mut work: Vec<&mut Block> = vec![&mut deck.top];
        work.extend(deck.defs.iter_mut());
        for s in &mut deck.sections {
            work.push(&mut s.body);
            work.extend(s.defs.iter_mut());
        }
        let mut blocks = 0;
        while let Some(b) = work.pop() {
            blocks += 1;
            assert!(blocks <= self.ast.len(), "a block is a node: the walk ends");
            let Block {
                params,
                locals,
                conds,
                ..
            } = b;
            self.keep_winners(params, first);
            self.keep_winners(locals, first);
            for (_, c) in conds {
                work.extend(c.branches.iter_mut().map(|(_, body)| body));
            }
        }
    }

    /// The reader's rule for the file's own dot card `word` (lowercased,
    /// without its dot), when its language declares one.
    pub(super) fn dot_rule(&self, word: &str) -> Option<DotRule> {
        self.b.dot_rules.iter().find(|r| r.0 == word).map(|r| r.1)
    }

    /// Whether `n` is one word.
    pub(super) fn is_word_item(&self, n: u32) -> bool {
        n != NONE && self.tag(n) == Tag::Word
    }
}

// ---- private ----

impl Rd<'_> {
    fn is_params_colon(&self, n: u32) -> bool {
        self.tag(n) == Tag::Word && self.text(n).eq_ignore_ascii_case("params:")
    }

    /// A dot card its language reads its own way, ahead of the shared dot
    /// cards (word `word`, at `scope`), by its `dot_rule` row
    /// (`Behaviours::dot_rules`): `true` when this placed it.
    fn dialect_card(&self, n: u32, word: &str, scope: Scope, acc: &mut Acc) -> bool {
        let (bench, notes) = (acc.bench.len(), acc.notes.len());
        let placed = match self.dot_rule(word) {
            Some(DotRule::Note(code)) if self.fields(n).is_empty() => {
                acc.notes
                    .push(crate::syntax::diag::Diag::new(code, self.main(n), 0));
                true
            }
            Some(DotRule::SParams) if scope == Scope::Top && self.testbench_peg(n) => {
                acc.bench.push(super::Bench::Card(n));
                true
            }
            Some(DotRule::Preprocess) => self.preprocess(n, acc),
            Some(DotRule::Note(_) | DotRule::SParams) | None => false,
        };
        let grew = acc.bench.len() + acc.notes.len() - (bench + notes);
        assert!(grew <= usize::from(placed), "placed once, or left as read");
        placed
    }

    /// The text of card `n` from its first field to the line's end, a path
    /// with blanks in it.
    fn rest_path(&self, n: u32) -> String {
        let first = self.main(self.fields(n)[0]);
        let t = self.span(first, self.line_last(self.main(n)));
        let t = t.trim();
        match t.as_bytes() {
            [b'"', .., b'"'] => t[1..t.len() - 1].to_string(),
            _ => t.to_string(),
        }
    }

    /// A path item without its quotes (the old lowering's `path`).
    fn path_of(&self, p: u32) -> String {
        let t = self.text(p);
        let p = t.trim();
        match p.as_bytes() {
            [b'"', .., b'"'] | [b'\'', .., b'\''] => p[1..p.len() - 1].to_string(),
            _ => p.to_string(),
        }
    }

    /// `.include path` (Xyce `.incl` takes the rest of the card).
    fn include(&mut self, n: u32, here: &mut Block) -> Result<(), Kept> {
        if self.fields(n).is_empty() {
            return Err(self.refused(n));
        }
        here.includes.push(n);
        Ok(())
    }

    /// `.lib path section` (LTspice: the section optional).
    fn lib_call(&mut self, n: u32, here: &mut Block) -> Result<(), Kept> {
        match self.fields(n) {
            [_] if self.b.card_rules == CardRules::Ltspice => {}
            [_, s, ..] if self.tag(*s) == Tag::Word || self.b.card_rules == CardRules::Ltspice => {}
            _ => return Err(self.refused(n)),
        }
        here.libs.push(n);
        Ok(())
    }

    /// `.global a b`.
    fn global(&mut self, n: u32, acc: &mut Acc) -> Result<(), Kept> {
        let items = self.fields(n);
        if !items.iter().all(|&k| self.is_word_item(k)) {
            return Err(self.refused(n));
        }
        for &k in items {
            let g = self.name_of(k);
            if !acc.globals.contains(&g) {
                acc.globals.push(g);
            }
        }
        Ok(())
    }

    /// `.model name kind params`.
    fn model(&mut self, n: u32, here: &mut Block) -> Result<(), Kept> {
        let Some(_) = self.model_head(n) else {
            return Err(self.refused(n));
        };
        here.models.push((n, NONE));
        Ok(())
    }

    /// `m` with an `ako:` card resolved. PSpice's and LTspice's
    /// `.model new ako:base type (k=v …)` ("a kind of"; LTspice also
    /// `ako: base`) is the model `base` with the listed parameters
    /// changed: the base card's type and parameters, each card's own over
    /// them along a chain of `ako:` cards (ngspice-45 in its PSPICE mode
    /// and LTspice 26.1.1 give the card and the model written out the same
    /// operating point). Left as read, for the writer to refuse (W0142),
    /// when no card of this file is the base, the chain is a circle, or
    /// the card's first word is no `k=v` and not the base's type.
    fn derived(&self, m: &mut Model) {
        // An `ako:` card's base name, lowercased, and where its own words
        // start: `ako:base` (0), `ako: base` (1, the base the first word).
        let ako = |kind: &str, params: &[(String, String)]| -> Option<(String, usize)> {
            let (head, base) = kind.split_at_checked(4)?;
            if !head.eq_ignore_ascii_case("ako:") {
                return None;
            }
            if !base.is_empty() {
                return Some((base.to_ascii_lowercase(), 0));
            }
            let (word, v) = params.first()?;
            v.is_empty().then(|| (word.to_ascii_lowercase(), 1))
        };
        if ako(&m.kind, &m.params).is_none() {
            return;
        }
        let (mut kind, mut params) = (m.kind.clone(), m.params.clone());
        // Each card's own words, from this one up to the model the chain
        // derives; a circle of `ako:` cards ends at the bound.
        let mut own: Vec<Vec<(String, String)>> = Vec::new();
        while let Some((base, at)) = ako(&kind, &params) {
            let card = self.models.get(&base);
            let Some(&card) = card.filter(|_| own.len() < self.models.len()) else {
                return;
            };
            own.push(params.split_off(at));
            let items = self.fields(card);
            (kind, params) = (self.text(items[1]), self.params_of(&items[2..], true));
        }
        assert!(!own.is_empty(), "an ako card has its base");
        assert!(own.len() <= self.models.len(), "a chain holds a card once");
        // The base's parameters, each card's own over them, the nearest
        // card's last. A card's first word that is no `k=v` is its type:
        // the base's.
        for mut over in own.into_iter().rev() {
            if over.first().is_some_and(|(_, v)| v.is_empty()) {
                if !over[0].0.eq_ignore_ascii_case(&kind) {
                    return;
                }
                over.remove(0);
            }
            for (k, v) in over {
                match params.iter_mut().find(|(b, _)| b.eq_ignore_ascii_case(&k)) {
                    Some(p) => p.1 = v,
                    None => params.push((k, v)),
                }
            }
        }
        (m.kind, m.params) = (kind, params);
    }

    /// quirk: whether `a` is a `.param` function definition the old reader
    /// read (the old SPICE grammar's `FuncDef`: a body that is delimited or
    /// one blank- and comma-free run).
    fn funcdef_ok(&self, a: u32) -> bool {
        let g = self.ast.lhs(Node(a));
        let v = self.ast.rhs(Node(a));
        if self.tag(g) != Tag::Group || !self.func_head_ok(g) || v == NONE {
            return false;
        }
        if self.tag(v) == Tag::Delimited {
            return self.expr_ok(v);
        }
        let (a, b) = (self.first(v), self.last(v));
        (a..b).all(|k| self.t.end(k) == self.t.start(k + 1))
            && (a..=b).all(|k| self.ttag(k) != T::Comma)
    }

    /// `.param a=1 f(x)=…` (the old SPICE grammar's `Params`).
    fn param_card(
        &mut self,
        n: u32,
        here: &mut Block,
        inside: bool,
        acc: &mut Acc,
    ) -> Result<(), Kept> {
        if self.b.card_rules == CardRules::Xyce && self.global_swept(n) {
            return Err(Kept::Raw);
        }
        let items = self.fields(n).to_vec();
        let ngspice = self.b.card_rules == CardRules::Ngspice;
        let lt_params = self.b.card_rules == CardRules::Ltspice && self.head_kind(n) == "params";
        let mut assigns = Vec::new();
        for (k, &it) in items.iter().enumerate() {
            let plus = ngspice
                && self.tag(it) == Tag::Word
                && self.text(it) == "+"
                && k > 0
                && items
                    .get(k + 1)
                    .is_some_and(|&x| self.tag(x) == Tag::Assign);
            if plus {
                continue;
            }
            if self.tag(it) != Tag::Assign {
                return Err(Kept::Unparsed);
            }
            let key = self.ast.lhs(Node(it));
            let ok = match self.tag(key) {
                Tag::Group => self.funcdef_ok(it),
                _ => {
                    self.is_word_item(key) && self.val_ok(self.ast.rhs(Node(it)))
                        || self.val_p_ok(it)
                }
            };
            if !ok {
                return Err(Kept::Unparsed);
            }
            assigns.push(it);
        }
        if lt_params && assigns.is_empty() {
            return Err(Kept::Unparsed);
        }
        let funcs = assigns
            .iter()
            .any(|&a| self.tag(self.ast.lhs(Node(a))) == Tag::Group);
        if inside && funcs {
            return Err(Kept::Raw);
        }
        for a in assigns {
            if self.tag(self.ast.lhs(Node(a))) == Tag::Group {
                acc.funcs.push(Func::Param(a));
            } else if inside {
                here.locals.push(a);
            } else {
                if self.b.card_rules == CardRules::Xyce && self.head_kind(n) == "global_param" {
                    self.global_params.insert(a);
                }
                here.params.push(a);
            }
        }
        Ok(())
    }

    /// quirk: the old ngspice grammar's `Val_p`: a braced group, an operator, an
    /// expression (`{1+x} + x*AGAUSS(0,1,1)`).
    fn val_p_ok(&self, a: u32) -> bool {
        let v = self.ast.rhs(Node(a));
        self.b.card_rules == CardRules::Ngspice
            && v != NONE
            && self.tag(v) == Tag::Binary
            && self.ttag(self.first(v)) == T::LBrace
    }

    /// Keeps, of the definitions `params` gives one name, the source's:
    /// its `first`, else its last; the others' order is kept. A SPICE
    /// name is read caselessly, a named language's as written.
    fn keep_winners(&self, params: &mut Vec<u32>, first: bool) {
        let fold = !self.named();
        let names: Vec<String> = params
            .iter()
            .map(|&a| self.text(self.ast.lhs(Node(a))))
            .map(|n| if fold { n.to_ascii_lowercase() } else { n })
            .collect();
        let mut winner: HashMap<&str, usize> = HashMap::with_capacity(names.len());
        for (k, name) in names.iter().map(String::as_str).enumerate() {
            if first {
                winner.entry(name).or_insert(k);
            } else {
                winner.insert(name, k);
            }
        }
        let mut k = 0;
        params.retain(|_| {
            k += 1;
            winner[names[k - 1].as_str()] == k - 1
        });
        assert_eq!(k, names.len(), "every definition was weighed once");
        assert_eq!(params.len(), winner.len(), "one definition a name is left");
    }
}
