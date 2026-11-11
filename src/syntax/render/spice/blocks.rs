//! One block of the plan → its SPICE-family cards: a block's `.include`
//! and `.lib` cards, its parameters and models, a subcircuit
//! (`.subckt` … `.ends`), a library section (`.lib` … `.endl`), and a
//! body's elements, conditionals and kept text in place.
//!
//! Cites: ngspice manual §2.4.3 (`.subckt`), §2.6, §2.7 (`.include`,
//! `.lib`), §2.15 (`.if`); Xyce RG `.INCLUDE`; LTspice help (`Rser`).
//!
//! Contents:
//! - `W::file_cards(&mut self, b: &Block)`, `W::params_and_models(&mut self, b: &Block)`
//! - `W::subckt(&mut self, sc: &Block)`, `W::section(&mut self, s: &Section)`
//! - `W::body(&mut self, b: &Block, rser: bool)`, `W::coupled(&self, b: &Block) -> Vec<String>`,
//!   `W::raw_lines(&self, raw: &[Raw]) -> Vec<String>`

use super::{W, bench};
use crate::syntax::behaviour::{CardRules, IncludeScope, ParamForm};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::{Block, Card, Comp, Func, Raw, Section, Visit};

impl W<'_, '_> {
    /// A block's `.include` and `.lib` cards, in the block: SPICE scopes a
    /// `.lib` inside a `.subckt` to that subcircuit, so hoisting one
    /// changes which models it sees.
    pub(super) fn file_cards(&mut self, b: &Block) {
        // Xyce makes what a file inside a `.subckt` defines global (run
        // by hand), so another dialect's deck would lose its scope.
        let first = b.includes.first().or(b.libs.first());
        if self.b.include_scope == IncludeScope::Global
            && self.rd.b.include_scope != IncludeScope::Global
            && b.node != crate::syntax::ast::NONE
            && let Some(&n) = first
        {
            self.refuse(Code::W0046, self.rd.main(n));
        }
        // A path with a blank is quoted where the target writes paths
        // bare (ngspice §2.6 and Xyce RG `.INCLUDE` read `"…"`), or it
        // would be a path and a section.
        let quote = |p: &str| match self.sp.path_quote {
            "" if p.contains(char::is_whitespace) => "\"",
            q => q,
        };
        for &inc in &b.includes {
            let p = self.rd.include_path(inc);
            let q = quote(&p);
            self.o.token(&format!(".include {q}{p}{q}"), Space::Line);
        }
        for &lib in &b.libs {
            let (p, section) = self.rd.lib_view(lib);
            let q = quote(&p);
            if section.is_empty() {
                match self.ask(Construct::LibWholeFile, self.rd.main(lib)) {
                    Action::Native => self.o.token(&format!(".lib {q}{p}{q}"), Space::Line),
                    Action::Spell(Alt::Include) => {
                        self.o.token(&format!(".include {q}{p}{q}"), Space::Line)
                    }
                    _ => {}
                }
                continue;
            }
            self.o
                .token(&format!(".lib {q}{p}{q} {section}"), Space::Line);
        }
    }

    /// A block's `.param` cards and `.model` cards, as a deck's top and a
    /// library section hold them.
    pub(super) fn params_and_models(&mut self, b: &Block) {
        self.param_cards(&b.params);
        self.models(b);
    }

    pub(super) fn subckt(&mut self, sc: &Block) {
        // Subcircuits are written hoisted, one at a time (paired with the
        // reset at the end).
        assert_eq!(self.scope, crate::syntax::ast::NONE, "no subcircuit open");
        assert!(self.m_param.is_none(), "no subcircuit's M renamed");
        self.scope = sc.node;
        let mut head = self
            .rd
            .subckt_head(sc.node)
            .expect("the reader read the head");
        // A Xyce `X` line's parameter the head lacks (`W::undeclared`).
        let passed = self.undeclared(sc, &head.params);
        head.params.extend(passed);
        let mut line = format!(".subckt {}", sc.name);
        for p in &head.ports {
            line.push(' ');
            line.push_str(p);
        }
        let params = !head.params.is_empty() || !sc.params.is_empty();
        if let (true, Some(word)) = (params, self.sp.subckt_params) {
            line.push(' ');
            line.push_str(word);
        }
        self.m_param = self.m_rename(&head.params);
        let is_m = |k: &str| k.eq_ignore_ascii_case("m");
        for (k, v) in &head.params {
            let k = match &self.m_param {
                Some(m) if is_m(k) => m.as_str(),
                _ => k.as_str(),
            };
            line.push_str(&format!(" {k}={}", self.param_value(v)));
        }
        // A Spectre or VACASK subcircuit's `parameters` are its defaults.
        for &a in &sc.params {
            let (k, v) = self.rd.param_view(a);
            line.push_str(&format!(" {k}={}", self.param_value(&v)));
        }
        self.o.token(&line, Space::Line);
        self.file_cards(sc);
        let draws = self.draws.locals(sc.node).to_vec();
        self.param_lines(&draws);
        self.param_cards(&sc.locals);
        if let Some(&(Func::Param(n) | Func::Card(n) | Func::Real(n))) = sc.funcs.first()
            && self.ask(Construct::LocalFunc, self.rd.main(n)) == Action::Native
        {
            self.funcs(&sc.funcs);
        }
        self.models(sc);
        // A nested subcircuit, hoisted, takes the models around it.
        for (m, bin) in self.inherited_models(sc) {
            self.model(m, bin);
            self.o.token("", Space::Line);
        }
        self.body(
            sc,
            self.rd.b.card_rules == CardRules::Ltspice && !self.in_section,
        );
        self.o.token(&format!(".ends {}", sc.name), Space::Line);
        self.scope = crate::syntax::ast::NONE;
        self.m_param = None;
    }

    /// `.lib <name>` … `.endl <name>`, the body written as a deck's top is.
    pub(super) fn section(&mut self, s: &Section) {
        let name = &s.body.name;
        if self.ask(Construct::LibSection, 0) != Action::Native {
            return;
        }
        self.o.token(&format!(".lib {name}"), Space::Line);
        self.file_cards(&s.body);
        self.funcs(&s.funcs);
        self.params_and_models(&s.body);
        self.in_section = true;
        for sc in &s.defs {
            self.subckt(sc);
        }
        self.in_section = false;
        self.body(&s.body, false);
        self.o.token(&format!(".endl {name}"), Space::Line);
    }

    /// Elements, instances and conditionals in place, then the text kept;
    /// each branch of `.if` … `.endif` as its models, then its elements
    /// (the walk is a heap stack: `read::Walk`). With `rser` (an LTspice
    /// deck's top or definition), an inductor of `b` itself carries
    /// LTspice's implicit `Rser=1m` unless a `K` couples it.
    pub(super) fn body(&mut self, b: &Block, rser: bool) {
        let coupled = match rser && self.rd.b.card_rules == CardRules::Ltspice {
            true => Some(self.coupled(b)),
            false => None,
        };
        let mut walk = b.walk();
        while let Some(v) = walk.next() {
            match v {
                Visit::Comp(at, Comp::Card(n)) => {
                    let own = std::ptr::eq(at, b);
                    self.card(n, coupled.as_deref().filter(|_| own));
                }
                Visit::Comp(_, Comp::Raw(n)) => {
                    let line = self.rd.line(n);
                    self.o.token(&line, Space::None);
                }
                Visit::Inst(_, n) => self.card(n, None),
                Visit::Cond(_, c) => {
                    let first = c.branches[0].0;
                    if self.ask(Construct::Conditional, self.rd.main(first)) != Action::Native {
                        walk.prune();
                    }
                    continue;
                }
                Visit::Branch(c, i) => {
                    let (head, body) = (c.branches[i].0, &c.branches[i].1);
                    if !body.params.is_empty() || !body.locals.is_empty() {
                        self.ask(Construct::BranchParam, self.rd.main(head));
                    }
                    let line = match (self.rd.condition(head), i) {
                        (Some(c), 0) => format!(".if ({c})"),
                        (Some(c), _) => format!(".elseif ({c})"),
                        (None, _) => ".else".to_string(),
                    };
                    self.o.token(&line, Space::Line);
                    self.file_cards(body);
                    self.models(body);
                    continue;
                }
                Visit::EndCond => {
                    self.o.token(".endif", Space::Line);
                    continue;
                }
                Visit::EndBlock(at) => {
                    let lines = self.raw_lines(&at.raw);
                    self.write_raw(&lines);
                    continue;
                }
            }
            self.o.token("", Space::Line);
        }
    }

    /// The inductors a `K` card of `b` couples, by name after the letter.
    pub(super) fn coupled(&self, b: &Block) -> Vec<String> {
        let mut out = Vec::new();
        for &c in &b.comps {
            if let Comp::Card(n) = c
                && let Card::Coupling { ls, .. } = self.rd.card_view(n)
            {
                out.extend(ls);
            }
        }
        out
    }

    /// The text of `raw`, one string per line; a VACASK script line
    /// (`Rd::vacask_script`, noted by `checks`) is left out, but CDL's
    /// layout copies it as it copies all kept text.
    pub(super) fn raw_lines(&self, raw: &[Raw]) -> Vec<String> {
        let mut out = Vec::new();
        for &r in raw {
            let (Raw::Line(n) | Raw::Lines(n) | Raw::Joined(n)) = r;
            let omit = matches!(
                self.cells[Construct::VacaskScript as usize],
                Action::Spell(Alt::Omit(_))
            );
            if omit && self.rd.vacask_script(self.rd.main(n)) {
                continue;
            }
            match r {
                Raw::Line(n) => out.push(self.rd.line(n)),
                Raw::Lines(n) => out.extend(self.rd.lines(n)),
                Raw::Joined(n) => out.push(self.rd.lines(n).join("\n")),
            }
        }
        out
    }
}

// ---- private ----

impl W<'_, '_> {
    /// `.param name=value`, one card each, in order, with the draws the
    /// `statistics` lift puts before each and the value it gives one.
    fn param_cards(&mut self, params: &[u32]) {
        for &a in params {
            let (name, mut v) = self.rd.param_view(a);
            let draws = self.draws.before(a).to_vec();
            self.param_lines(&draws);
            if let Some(d) = self.draws.value(a) {
                v = d.to_string();
            }
            let v = self.spelled(&v);
            let kw = match self.b.param_form == ParamForm::Xyce && self.rd.global_param(a) {
                true => ".global_param",
                false => ".param",
            };
            self.o.token(&format!("{kw} {name}={v}"), Space::Line);
        }
    }

    /// `.param name=value` for parameters a `Lift` rule adds.
    fn param_lines(&mut self, params: &[(String, String)]) {
        for (name, v) in params {
            let v = self.spelled(v);
            self.o.token(&format!(".param {name}={v}"), Space::Line);
        }
    }

    /// Raw lines, one each; with `drop_four`, every `.four` card and its
    /// `+` continuation lines are left out (the deck runs them as
    /// `fourier` commands in its `.control` block instead).
    fn write_raw(&mut self, lines: &[String]) {
        let mut four = false;
        for l in lines {
            if !bench::is_continuation(l) {
                four = self.drop_four && bench::is_four(l);
            }
            if !four {
                self.o.token(l, Space::Line);
            }
        }
    }
}
