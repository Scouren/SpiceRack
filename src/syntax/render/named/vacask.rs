//! The VACASK spine of the named writer: the same `name (nodes) master
//! k=v` statements as Spectre's, with VACASK's own deck around them. VACASK
//! has no anonymous instances, so every device module the deck uses is
//! loaded and named by a `model` line (`load "spice/resistor.osdi"`, `model
//! resistor sp_resistor`); names are quoted where they are no identifier and
//! lowercased for a SPICE-origin deck (VACASK is case-sensitive); numbers
//! carry no suffix (`M` is mega there, milli in SPICE); the testbench is one
//! `control` block (`vacask_cards` writes the instances, `vacask_bench`
//! the block).
//!
//! Every shape was probed against the binary by the old VACASK writer
//! (against `share/doc/vacask/{demo,test}/*.sim`);
//! VACASK hard-errors on an unknown parameter, so a name passed through is
//! a loud failure, never a substitution.
//!
//! Cites: VACASK `docs/input-identifiers.md` (quoting),
//! `lib/dfllexer.l` L536-L557 (`section`), P462-P509 (`@if`), P848-855
//! (`parameters`), `docs/cmd-analysis-mc.md`, `docs/cir-loading.md` (`load`).
//!
//! Contents:
//! - `W::vacask(&mut self)`, `W::expr(&self, s: &str) -> String`, `W::vval(&self, v: &Val) -> String`,
//!   `W::vacask_num(&self, x: &Num) -> String`, `W::inst_params(&self, params: &[(String, String)]) -> String`
//! - `fn num(v: f64) -> String`, `fn respell(s: &str) -> String`, `fn q(name: &str) -> String`

use std::collections::BTreeSet;

use super::W;
use crate::syntax::ast::NONE;
use crate::syntax::behaviour::CardRules;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::Construct;
use crate::syntax::render::read::Func;
use crate::syntax::render::read::{Block, Card, Comp, Num, Raw, Val, Visit};
use crate::syntax::resolve::Role;

/// A device module the deck needs: VACASK's build of the SPICE device set
/// (`load "spice/<m>.osdi"` and `model <m> sp_<m>`), a built-in (a
/// `model` line only), or a module VACASK ships beside the SPICE set
/// (`load "<m>.osdi"` and `model <m> <m>`: `tline_ideal`,
/// `docs/dev-vacask.md`). The model is named after the module, as
/// upstream's own decks do (`model resistor resistor`, `test/test_op.sim`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Need {
    Sp(&'static str),
    Builtin(&'static str),
    Own(&'static str),
}

impl W<'_, '_> {
    /// The whole VACASK deck, ending in a line end.
    pub(super) fn vacask(&mut self) {
        let deck = self.deck;
        let blocks = self.blocks();
        let mut funcs = deck
            .funcs
            .iter()
            .chain(deck.sections.iter().flat_map(|s| &s.funcs));
        if let Some(&(Func::Param(n) | Func::Card(n) | Func::Real(n))) = funcs.next() {
            self.ask(Construct::Func, self.rd.main(n));
        }
        let needs: BTreeSet<Need> = blocks.iter().flat_map(|b| self.comp_needs(b)).collect();
        // A library declares each block's synthesised models in it: the
        // deck including it declares its own, and VACASK refuses a second
        // `model resistor` ("already exists") but scopes one in a subckt.
        let library = self.rd.role() == Role::Library;
        // Models first: they add loads too. A subcircuit's and a section's
        // models stay in it, where VACASK scopes them.
        let mut loads: BTreeSet<String> = BTreeSet::new();
        let top_models = self.model_lines(&deck.top, &mut loads);
        let def_models: Vec<Vec<String>> = deck
            .defs
            .iter()
            .map(|sc| self.model_lines(sc, &mut loads))
            .collect();
        // A branch's models load their modules too.
        for b in &blocks {
            for (_, c) in &b.conds {
                for (_, body) in &c.branches {
                    self.model_lines(body, &mut loads);
                }
            }
        }
        let section_models: Vec<Vec<Vec<String>>> = deck
            .sections
            .iter()
            .map(|s| {
                std::iter::once(&s.body)
                    .chain(&s.defs)
                    .map(|b| self.model_lines(b, &mut loads))
                    .collect()
            })
            .collect();
        for n in &needs {
            match n {
                Need::Sp(m) => {
                    loads.insert(format!("spice/{m}.osdi"));
                }
                Need::Own(m) => {
                    loads.insert(format!("{m}.osdi"));
                }
                Need::Builtin(_) => {}
            }
        }
        let (def_models, section_models) = match library {
            false => (def_models, section_models),
            true => {
                let own = |w: &Self, b: &Block, m: &[String]| {
                    let mut l = need_lines(&w.block_needs(b));
                    l.extend_from_slice(m);
                    l
                };
                let defs = deck.defs.iter().zip(&def_models);
                let defs = defs.map(|(b, m)| own(self, b, m)).collect();
                let secs = deck.sections.iter().zip(&section_models).map(|(s, ms)| {
                    let bs = std::iter::once(&s.body).chain(&s.defs);
                    bs.zip(ms).map(|(b, m)| own(self, b, m)).collect()
                });
                (defs, secs.collect())
            }
        };
        let top_needs = match library {
            true => self.block_needs(&deck.top),
            false => needs.clone(),
        };
        // VACASK compiles a `.va` itself (`docs/cir-loading.md`): a load
        // the deck reads is written as read, whatever its dialect
        // (Spectre's `ahdl_include "x.va"`). One given beside the text (the
        // API's `veriloga()`) goes through OpenVAF, which compiles it once
        // beside its source; VACASK would compile it again in every run's
        // directory.
        let read = deck.top.loads.iter().chain(&deck.top.own_loads);
        loads.extend(read.map(|&n| self.rd.unquote(self.rd.fields(n)[0])));
        self.unwritten_loads();
        match crate::veriloga::osdi_loads(&deck.given_loads) {
            Ok(l) => loads.extend(l),
            Err(_) => self.refuse(Code::W0004, 0),
        }

        let title = deck.top.name.replace(['\n', '\r'], " ");
        let title = match title.trim().is_empty() {
            true => "circuit".to_string(),
            false => title,
        };
        let frame = self.frame();
        if frame.title {
            self.line(&title);
            self.line("");
        }
        if frame.ground {
            self.line("ground 0");
            self.line("");
        }
        if !deck.globals.is_empty() {
            let gs: Vec<String> = deck.globals.iter().map(|g| q(g)).collect();
            self.line(&format!("global {}", gs.join(" ")));
        }
        for l in &loads {
            self.line(&format!("load \"{l}\""));
        }
        self.vacask_files(&deck.top, "");
        self.line("");
        for n in &top_needs {
            // A user model wearing a synthesised name would be a second
            // definition; VACASK takes the first and runs the wrong device.
            let auto = n.auto();
            let clash = blocks.iter().find_map(|b| {
                b.models.iter().find_map(|&(m, bin)| {
                    let name = self.rd.model_view(m, bin).name;
                    name.eq_ignore_ascii_case(auto).then(|| self.rd.main(m))
                })
            });
            if let Some(tok) = clash {
                self.refuse(Code::W0104, tok);
            }
            self.line(&n.line());
        }
        for l in &top_models {
            self.line(l);
        }
        self.line("");
        // `.step param` names become control variables, so they are no
        // netlist parameters.
        let tb = self.fold();
        let stepped = tb.stepped();
        let params = self.param_list(&deck.top.params, &stepped);
        if !params.is_empty() {
            self.line(&format!("parameters{params}"));
            self.line("");
        }
        for (sc, models) in deck.defs.iter().zip(&def_models) {
            self.vacask_subckt(sc, models);
            self.line("");
        }
        for (s, models) in deck.sections.iter().zip(&section_models) {
            self.line(&format!("section {}", q(&s.body.name)));
            self.vacask_files(&s.body, "  ");
            self.vacask_body(&s.body, &models[0]);
            for (sc, m) in s.defs.iter().zip(&models[1..]) {
                self.vacask_subckt(sc, m);
            }
            self.line("endsection");
            self.line("");
        }
        self.vacask_elements(&deck.top, "");
        if frame.control {
            self.control(&tb);
        }
    }
}

impl W<'_, '_> {
    /// An expression, lowercased for a SPICE-origin deck (SPICE names are
    /// case-blind, VACASK's are not) unless it is a number, then
    /// [`respell`]ed; a SPICE `'e'` is an expression, a VACASK one a quoted
    /// name, kept.
    pub(super) fn expr(&self, s: &str) -> String {
        if self.keep_case {
            return respell(s);
        }
        let s = super::undelimited(s);
        match super::spice_num(s.trim()) {
            Some(_) => respell(s),
            None => respell(&s.to_ascii_lowercase()),
        }
    }

    /// A value: a number through [`num`], an expression through
    /// [`W::expr`], raw text [`respell`]ed.
    pub(super) fn vval(&self, v: &Val) -> String {
        match v {
            Val::Num(x) => num(*x),
            Val::Expr(e) => self.expr(e),
            Val::Raw(t) => respell(t),
        }
    }

    /// A waveform value: [`num`], or a [`W::expr`].
    pub(super) fn vacask_num(&self, x: &Num) -> String {
        match x {
            Num::Num(v) => num(*v),
            Num::Expr(e) => self.expr(e),
        }
    }

    /// Instance parameters: `m`, SPICE's device multiplier, is the
    /// Verilog-A builtin `$mfactor`.
    pub(super) fn inst_params(&self, params: &[(String, String)]) -> String {
        let mut s = String::new();
        for (k, v) in params {
            let k = k.to_ascii_lowercase();
            let k = if k == "m" { "$mfactor" } else { &k };
            s.push_str(&format!(" {k}={}", self.expr(v)));
        }
        s
    }
}

/// A number VACASK cannot misread: no suffix (SPICE's are case-blind,
/// VACASK's not: `M` is milli in one and mega in the other), but where
/// a suffix keeps fewer digits
/// ([`crate::syntax::render::spelling::shortest`]: `10u`; lowercase but
/// `T`, `G`, and `meg` for mega).
pub(super) fn num(v: f64) -> String {
    match v == 0.0 {
        true => "0".to_string(),
        false => crate::syntax::render::spelling::shortest(v, format!("{v:e}"), "meg"),
    }
}

/// A value as text: a number respelled in VACASK's suffixes, anything else
/// (a name, an expression) without ngspice's `{}`.
pub(super) fn respell(s: &str) -> String {
    let t = s.trim();
    let t = t
        .strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .unwrap_or(t)
        .trim();
    match super::spice_num(t) {
        Some((_, spelled)) => spelled,
        None => t.to_string(),
    }
}

/// A name as a VACASK identifier (`docs/input-identifiers.md`):
/// `[a-zA-Z_$][0-9a-zA-Z_$]*` or a number without a leading zero, else
/// quoted, `''` for a quote; lowercased.
pub(super) fn q(name: &str) -> String {
    let ident = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    let lead = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$');
    let number =
        name.chars().all(|c| c.is_ascii_digit()) && (name == "0" || !name.starts_with('0'));
    let quoted = !(ident && (lead || number)) && !name.is_empty();
    let mut out = String::with_capacity(name.len() + 2);
    if quoted {
        out.push('\'');
    }
    for c in name.chars() {
        if quoted && c == '\'' {
            out.push('\'');
        }
        out.push(c.to_ascii_lowercase());
    }
    if quoted {
        out.push('\'');
    }
    out
}

// ---- private ----

impl<'r> W<'_, 'r> {
    /// The modules block `b`'s own elements need (its branches' not).
    fn comp_needs(&self, b: &Block) -> BTreeSet<Need> {
        let mut out = BTreeSet::new();
        for &c in &b.comps {
            if let Comp::Card(n) = c {
                let card = self.rd.card_view(n);
                out.extend(need(&card));
                // An RF port's series resistor (`vacask_card`).
                if matches!(card, Card::Source { port: Some(_), .. }) {
                    out.insert(Need::Sp("resistor"));
                }
                if self.rd.b.card_rules == CardRules::Ltspice {
                    out.extend(parasitic_needs(&card));
                }
            }
        }
        out
    }

    /// [`Self::comp_needs`] of `b` and of every branch inside it (a heap
    /// stack, as `read::Walk`).
    fn block_needs(&self, b: &Block) -> BTreeSet<Need> {
        let mut out = BTreeSet::new();
        let mut work = vec![b];
        while let Some(b) = work.pop() {
            out.extend(self.comp_needs(b));
            work.extend(
                b.conds
                    .iter()
                    .flat_map(|(_, c)| &c.branches)
                    .map(|(_, body)| body),
            );
        }
        out
    }
}

impl W<'_, '_> {
    /// Block `b`'s `include` and `include … section=` statements, each
    /// after `pad`.
    fn vacask_files(&mut self, b: &Block, pad: &str) {
        for (n, is_lib) in self.rd.file_order(b) {
            let l = match is_lib {
                true => self.lib_include(n),
                false => format!("include \"{}\"", self.rd.include_path(n)),
            };
            self.line(&format!("{pad}{l}"));
        }
    }

    /// `subckt name (ports)` … `ends`.
    fn vacask_subckt(&mut self, sc: &Block, models: &[String]) {
        let head = self
            .rd
            .subckt_head(sc.node)
            .expect("the reader read the head");
        let ports: Vec<String> = head.ports.iter().map(|p| q(p)).collect();
        self.line(&format!("subckt {} ({})", q(&sc.name), ports.join(" ")));
        // VACASK's `include` is textual (`lib/dfllexer.l`): inside the
        // subckt, what the file defines is local to it, as ngspice §2.7
        // scopes a `.lib` there.
        self.vacask_files(sc, "  ");
        self.vacask_body(sc, models);
        self.line("ends");
    }

    /// A block's models, `parameters` (its own, then a SPICE definition's
    /// `.param`s: one reading another cannot be overridden, P848-855),
    /// elements and text kept: a `subckt`'s or a `section`'s body.
    fn vacask_body(&mut self, b: &Block, models: &[String]) {
        self.vacask_preamble(b, models);
        self.vacask_elements(b, "  ");
    }

    /// [`Self::vacask_body`]'s statements before the elements.
    fn vacask_preamble(&mut self, b: &Block, models: &[String]) {
        for m in models {
            self.line(&format!("  {m}"));
        }
        let mut params = String::new();
        if b.node != NONE {
            let head = self
                .rd
                .subckt_head(b.node)
                .expect("the reader read the head");
            for (k, v) in &head.params {
                params.push_str(&format!(" {}={}", k.to_ascii_lowercase(), self.expr(v)));
            }
        }
        params.push_str(&self.param_list(&b.params, &[]));
        if !params.is_empty() {
            self.line(&format!("parameters{params}"));
        }
        // A subcircuit's mismatch draws come first among its locals.
        let draws = match b.node {
            NONE => &[][..],
            n => self.draws.locals(n),
        };
        let mut locals: String = draws
            .iter()
            .map(|(k, v)| format!(" {}={}", k.to_ascii_lowercase(), self.expr(v)))
            .collect();
        locals.push_str(&self.param_list(&b.locals, &[]));
        if !locals.is_empty() {
            self.line(&format!("parameters{locals}"));
        }
    }

    /// ` name=value …` for `params` (the names in `skip` left out), each
    /// after the draws the `statistics` lift puts before it and with the
    /// value it gives it (one layer: `p = agauss(…)`).
    fn param_list(&self, params: &[u32], skip: &[String]) -> String {
        let mut s = String::new();
        let mut put = |k: &str, v: &str| {
            if !skip.contains(&k.to_ascii_lowercase()) {
                s.push_str(&format!(" {}={}", k.to_ascii_lowercase(), self.expr(v)));
            }
        };
        for &a in params {
            for (k, v) in self.draws.before(a) {
                put(k, v);
            }
            let (k, v) = self.rd.param_view(a);
            put(&k, self.draws.value(a).unwrap_or(&v));
        }
        s
    }

    /// Elements, instances and conditionals in place, then the text kept,
    /// each after `pad` (a branch's after two blanks; a conditional's first
    /// line only): `@if c` … `@elseif c` … `@else` … `@end` (P462-P509), a
    /// branch holding instances and models only, its models' loads collected
    /// with every other model's (the walk is a heap stack: `read::Walk`).
    fn vacask_elements(&mut self, b: &Block, pad: &str) {
        let pad_of = |at: &Block| if std::ptr::eq(at, b) { pad } else { "  " };
        // The pad of the block the open chain sits in.
        let mut chain_pad = pad;
        for v in b.walk() {
            match v {
                Visit::Comp(at, Comp::Card(n)) | Visit::Inst(at, n) => {
                    if let Some(line) = self.vacask_card(n) {
                        self.line(&format!("{}{line}", pad_of(at)));
                    }
                }
                // `checks` refuses a kept element line, once.
                Visit::Comp(_, Comp::Raw(_)) => {}
                Visit::Cond(at, _) => chain_pad = pad_of(at),
                Visit::Branch(c, i) => {
                    let (head, body) = (c.branches[i].0, &c.branches[i].1);
                    if !body.params.is_empty() || !body.locals.is_empty() {
                        self.ask(Construct::BranchParam, self.rd.main(head));
                    }
                    let line = match (self.rd.condition(head), i) {
                        (Some(c), 0) => format!("{chain_pad}@if {}", self.expr(&c)),
                        (Some(c), _) => format!("@elseif {}", self.expr(&c)),
                        (None, _) => "@else".to_string(),
                    };
                    self.line(&line);
                    let models = self.model_lines(body, &mut BTreeSet::new());
                    self.vacask_preamble(body, &models);
                }
                Visit::EndCond => self.line("@end"),
                Visit::EndBlock(at) => {
                    let lines: Vec<String> = at
                        .raw
                        .iter()
                        .filter(|&&(Raw::Line(n) | Raw::Lines(n) | Raw::Joined(n))| {
                            !self.omitted(self.rd.main(n))
                        })
                        .flat_map(|&r| match r {
                            Raw::Line(n) => vec![self.rd.line(n)],
                            Raw::Lines(n) => self.rd.lines(n),
                            Raw::Joined(n) => vec![self.rd.lines(n).join("\n")],
                        })
                        .collect();
                    for l in lines {
                        self.line(&format!("{}{l}", pad_of(at)));
                    }
                }
            }
        }
    }
}

impl Need {
    /// The synthesised model's name, the module's.
    fn auto(&self) -> &'static str {
        match self {
            Need::Sp(m) | Need::Builtin(m) | Need::Own(m) => m,
        }
    }

    /// Its `model` line.
    fn line(&self) -> String {
        match self {
            Need::Sp(m) => format!("model {m} sp_{m}"),
            Need::Builtin(m) | Need::Own(m) => format!("model {m} {m}"),
        }
    }
}

/// The `model` lines of `needs`.
fn need_lines(needs: &BTreeSet<Need>) -> Vec<String> {
    needs.iter().map(Need::line).collect()
}

/// The modules an LTspice card's parasitics need (`W::parasitics`):
/// `Rser`/`Rpar` a resistor, `Lser` an inductor, `Cpar` a capacitor.
fn parasitic_needs(c: &Card) -> Vec<Need> {
    let params = match c {
        Card::Rcl {
            letter: 'C' | 'L',
            params,
            ..
        } => params,
        Card::Source { parasitics, .. } => parasitics,
        _ => return vec![],
    };
    let module = |k: &str| match k.to_ascii_lowercase().as_str() {
        "rser" | "rpar" => Some(Need::Sp("resistor")),
        "lser" => Some(Need::Sp("inductor")),
        "cpar" => Some(Need::Sp("capacitor")),
        _ => None,
    };
    params.iter().filter_map(|(k, _)| module(k)).collect()
}

/// The module element `c` needs, if any.
fn need(c: &Card) -> Option<Need> {
    Some(match c {
        // An element naming its model is that model's instance.
        Card::Rcl { model: Some(_), .. } => return None,
        Card::Rcl { letter: 'R', .. } => Need::Sp("resistor"),
        Card::Rcl { letter: 'C', .. } => Need::Sp("capacitor"),
        Card::Rcl { .. } => Need::Sp("inductor"),
        Card::Coupling { .. } => Need::Builtin("mutual"),
        Card::Source { letter: 'V', .. } => Need::Builtin("vsource"),
        Card::Source { .. } => Need::Builtin("isource"),
        Card::Linear { letter: 'E', .. } => Need::Builtin("vcvs"),
        Card::Linear { letter: 'G', .. } => Need::Builtin("vccs"),
        Card::Linear { letter: 'F', .. } => Need::Builtin("cccs"),
        Card::Linear { .. } => Need::Builtin("ccvs"),
        Card::Line { .. } => Need::Own("tline_ideal"),
        _ => return None,
    })
}
