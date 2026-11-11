//! A VACASK `control` block → [`BenchCard`] views: its options (those
//! SPICE names alike portable), its analyses with the sweeps before them,
//! its `save`s as SPICE output names, its `alter`s of a top-level
//! parameter (`BenchCard::Alter`), and which of its statements only
//! steer a VACASK run (`Rd::vacask_script`); every other statement is kept
//! as `BenchCard::Extra`.
//!
//! Cites: the old lowering's `ControlBlock`; VACASK 36be560 `docs/`
//! (`cmd-overview.md`, `cmd-options-*.md`, `cmd-save.md`,
//! `cmd-errorhandling.md`, `cmd-print.md`, `cmd-postprocess.md`,
//! `input-embed.md`).
//!
//! Contents:
//! - `Rd::vacask_control(&self, n: u32) -> Vec<BenchCard>`
//! - `Rd::vacask_promoted(&self, stmts: &[u32]) -> HashSet<String>`
//! - `Rd::vacask_script(&self, head: u32) -> bool`
//! - `Rd::vacask_saves(&self, n: u32) -> Option<(Vec<String>, bool)>`

use std::collections::HashSet;

use super::named_bench::nums;
use super::named_elements::unquoted;
use super::{Ana, BenchCard, DcAxis, DcSweep, Opt, Rd, Sweep};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::NamedRules;
use crate::syntax::token::Tag as T;

impl<'a> Rd<'a> {
    /// A VACASK `control` block: its options and analyses, the rest kept as
    /// statements (the old lowering's `ControlBlock`).
    pub(super) fn vacask_control(&self, n: u32) -> Vec<BenchCard> {
        let mut out = Vec::new();
        let mut sweeps: Vec<u32> = Vec::new();
        let extra = |out: &mut Vec<BenchCard>, s: u32| out.push(BenchCard::Extra(s));
        let promoting = self
            .sole_elaborate(n)
            .filter(|(_, names)| names.iter().all(|x| self.promoted.contains(x)))
            .map(|(st, _)| st);
        for &st in self.body(n) {
            let head = self.ttag(self.main(st));
            // The one `elaborate` the reader read as the top.
            if promoting == Some(st) {
                continue;
            }
            if head == T::KwSweep {
                sweeps.push(st);
                continue;
            }
            if head == T::KwAnalysis {
                match self.vacask_analysis(&sweeps, st) {
                    Some(cards) => out.extend(cards),
                    None => {
                        for &s in sweeps.iter().chain([&st]) {
                            extra(&mut out, s);
                        }
                    }
                }
                sweeps.clear();
                continue;
            }
            for &s in &sweeps {
                extra(&mut out, s);
            }
            sweeps.clear();
            if let Some(alters) = self.vacask_alter(st) {
                out.extend(alters);
                continue;
            }
            match self.vacask_options(st) {
                Some(o) => out.push(BenchCard::Options(o)),
                None => extra(&mut out, st),
            }
        }
        for &s in &sweeps {
            extra(&mut out, s);
        }
        out
    }

    /// The subcircuits a VACASK deck's control block makes toplevel
    /// circuits beside the default one (`docs/cmd-elaboration.md`:
    /// `elaborate circuit("sub1", …)` promotes each, its models and
    /// definitions in the global scope), read as the top itself: when the
    /// block's only `elaborate` comes before its first analysis and names
    /// top-level subcircuits with no terminals. Empty otherwise (a deck that
    /// elaborates twice runs two circuits, `Rd::kept_construct`).
    pub(super) fn vacask_promoted(&self, stmts: &[u32]) -> HashSet<String> {
        if self.b.named_rules != NamedRules::Vacask {
            return HashSet::new();
        }
        let control = stmts.iter().copied().find(|&s| self.tag(s) == Tag::Control);
        let Some((_, names)) = control.and_then(|c| self.sole_elaborate(c)) else {
            return HashSet::new();
        };
        let portless = |name: &str| {
            stmts.iter().any(|&s| {
                self.tag(s) == Tag::Subckt
                    && self.fields(s).len() == 1
                    && self.name_of(self.fields(s)[0]) == name
            })
        };
        match names.iter().all(|n| portless(n)) {
            true => names.into_iter().collect(),
            false => HashSet::new(),
        }
    }

    /// Whether the VACASK statement at token `head` (its first) only steers a VACASK run and its
    /// scripts: the control commands `abort` (the error policy,
    /// `docs/cmd-errorhandling.md`), `print` (the console,
    /// `cmd-print.md`), `postprocess` (a program run after the analyses,
    /// `cmd-postprocess.md`), and an `embed` of a file no `load` or
    /// `include` of the deck reads (`input-embed.md`: the file is written
    /// for such a program).
    pub(in crate::syntax::render) fn vacask_script(&self, head: u32) -> bool {
        if self.b.named_rules != NamedRules::Vacask {
            return false;
        }
        match self.ttag(head) {
            // `embed "file" <<<MARK`: the file is the next token.
            T::KwEmbed => {
                let file = self.t.text(head + 1).trim().trim_matches('"');
                head < self.line_last(head) && !self.reads_file(file)
            }
            _ => matches!(self.t.text(head), "abort" | "print" | "postprocess"),
        }
    }

    /// A VACASK `save` statement's outputs as SPICE spells them
    /// (`docs/cmd-analysis-tran.md` "Save directives"): `v(n)` and
    /// `v(a,b)` as written, `i(inst)` of a voltage source as the name map
    /// spells it, `default` and `full` every node (`all`); and whether it
    /// names a `p(inst,var)`, left out: a module's output variable, whose
    /// name and meaning are the module's (VACASK's `resistor` gives one
    /// parallel instance's `i`, ngspice's `@r1[i]` all of them). `None`
    /// for any other output.
    pub(in crate::syntax::render) fn vacask_saves(&self, n: u32) -> Option<(Vec<String>, bool)> {
        if self.b.named_rules != NamedRules::Vacask || self.tag(n) != Tag::Save {
            return None;
        }
        let head = self.main(n);
        let last = self.line_last(head);
        let text = (head < last).then(|| self.span(head + 1, last))?;
        let mut out = Vec::new();
        let (mut all, mut dropped) = (false, false);
        for item in save_items(&text)? {
            let (word, args) = match item.split_once('(') {
                Some((w, rest)) => (w, rest.strip_suffix(')')?.split(',').collect::<Vec<_>>()),
                None => (item.as_str(), vec![]),
            };
            match (word, args.as_slice()) {
                ("default" | "full", []) => all = true,
                ("v", [_] | [_, _]) => out.push(item.clone()),
                ("i", [inst]) => out.push(format!("i({})", self.declared(n, inst, &['V'])?)),
                ("p", [_, _]) => dropped = true,
                _ => return None,
            }
        }
        if all && !out.is_empty() {
            out.insert(0, "all".into());
        }
        Some((out, dropped))
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// Control block `n`'s `elaborate circuit[("a", …)]` and the names it
    /// promotes, when it is the block's only `elaborate` and comes before
    /// its first `analysis`; `None` otherwise or for another form
    /// (`topdef=`, `elaborate changes`).
    fn sole_elaborate(&self, n: u32) -> Option<(u32, Vec<String>)> {
        let is = |s: u32, w: &str| self.t.text(self.main(s)) == w;
        let body = self.body(n);
        let mut els = body.iter().copied().filter(|&s| is(s, "elaborate"));
        let (st, None) = (els.next()?, els.next()) else {
            return None;
        };
        let first_analysis = body
            .iter()
            .position(|&s| self.ttag(self.main(s)) == T::KwAnalysis);
        let at = body.iter().position(|&s| s == st)?;
        if first_analysis.is_some_and(|a| a < at) {
            return None;
        }
        let rest = self.line(st);
        let rest = rest
            .strip_prefix("elaborate")?
            .trim()
            .strip_prefix("circuit")?
            .trim();
        if rest.is_empty() {
            return Some((st, vec![]));
        }
        let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
        let names = inner.split(',').map(|a| unquoted(a.trim()));
        Some((st, names.collect::<Option<Vec<_>>>()?))
    }

    /// Whether a `load` or `include` anywhere in the file names `file`.
    fn reads_file(&self, file: &str) -> bool {
        let mut work: Vec<u32> = self.ast.fields(self.ast.root()).to_vec();
        while let Some(s) = work.pop() {
            if matches!(
                self.tag(s),
                Tag::Subckt | Tag::Section | Tag::Library | Tag::If | Tag::Else
            ) {
                work.extend(self.body(s));
                work.extend(self.ast.next(Node(s)).map(|nx| nx.0));
                continue;
            }
            let names = matches!(self.ttag(self.main(s)), T::KwLoad | T::KwInclude)
                && self
                    .fields(s)
                    .first()
                    .is_some_and(|&f| self.unquote(f) == file);
            if names {
                return true;
            }
        }
        false
    }

    /// `alter instance("__topinst__") k=v …` (`docs/cmd-alter.md`; the
    /// default toplevel instance, `cmd-elaboration.md`): one card per
    /// top-level parameter it sets, when every value is a number. `None`
    /// for any other statement, kept as text: an alter of another instance
    /// or of a model (VACASK drops it when a later parameter change
    /// rebuilds the circuit: ZERO-LEDGER P5-050), an expression value.
    fn vacask_alter(&self, st: u32) -> Option<Vec<BenchCard>> {
        let line = self.line(st);
        let target = line.strip_prefix("alter")?.trim_start();
        let target = target.strip_prefix("instance")?.trim_start();
        let (names, _) = target.strip_prefix('(')?.split_once(')')?;
        if unquoted(names.trim()).as_deref() != Some("__topinst__") {
            return None;
        }
        // The fields after `alter`: the word `instance`, its one name
        // (a string), then `k=v` items only.
        let sets = self.pairs(st);
        let others = self.fields(st).len() - sets.len();
        if sets.is_empty() || others != 2 {
            return None;
        }
        let cards = sets.into_iter().map(|(key, v)| {
            let value = self.number(v)?;
            Some(BenchCard::Alter {
                key,
                value,
                node: st,
            })
        });
        cards.collect()
    }

    /// A VACASK `options` statement in a `control` block (the old
    /// lowering's `vacask_control`, `Ctl`): an option SPICE names alike
    /// with the same quantity (`docs/cmd-options-tol.md`,
    /// `cmd-options-temp.md`, `cmd-options-special.md`: `scale`) and a
    /// `tran_method` SPICE has (`cmd-options-tran.md`: "trap", "am2" the
    /// trapezoidal rule; "gear2", "bdf2" second-order Gear, ngspice's
    /// `method=gear` at its default MAXORD 2) are portable, the rest
    /// VACASK's own.
    fn vacask_options(&self, st: u32) -> Option<Vec<Opt>> {
        const SPICE_ALIKE: [&str; 6] = ["reltol", "abstol", "vntol", "chgtol", "gmin", "scale"];
        if self.tag(st) != Tag::Options {
            return None;
        }
        let mut out = Vec::new();
        for (k, v) in self.pairs(st) {
            let method = match k.as_str() {
                "tran_method" => unquoted(&self.text(v)),
                _ => None,
            };
            match (k.as_str(), self.number(v), method.as_deref()) {
                ("rawfile", ..) => {}
                ("temp", Some(x), _) => out.push(Opt::Temp(x)),
                ("tnom", Some(x), _) => out.push(Opt::Tnom(x)),
                // As a number: VACASK's `M` is mega, SPICE's milli.
                (k, Some(x), _) if SPICE_ALIKE.contains(&k) => {
                    out.push(Opt::Portable(k.into(), format!("{x:e}")))
                }
                (_, _, Some("trap" | "am2")) => {
                    out.push(Opt::Portable("method".into(), "trap".into()))
                }
                (_, _, Some("gear2" | "bdf2")) => {
                    out.push(Opt::Portable("method".into(), "gear".into()))
                }
                _ => out.push(Opt::Vacask(k, self.text(v))),
            }
        }
        Some(out)
    }

    /// `sweep …` lines and the `analysis NAME KIND k=v` they belong to
    /// (the old lowering's `vacask_control`, `Ana`): the analysis and the
    /// initial conditions it sets; `None` when the IR had no such analysis.
    fn vacask_analysis(&self, sweeps: &[u32], st: u32) -> Option<Vec<BenchCard>> {
        let p = self.pairs(st);
        let num = |k: &str| self.get(&p, k).and_then(|v| self.number(v));
        let s = |k: &str| self.get(&p, k).and_then(|v| unquoted(&self.text(v)));
        // The last sweep is the inner one, as `.dc`'s first source.
        let axes: Vec<DcAxis> = sweeps
            .iter()
            .rev()
            .map(|&sw| self.vacask_sweep(sw))
            .collect::<Option<_>>()?;
        let kind = match self.ast.header(Node(st)).master {
            NONE => String::new(),
            w => self.text(w),
        };
        // A node list of one or two strings (`out=["o"]`, `["o", "r"]`,
        // cmd-analysis-noise.md), or one string.
        let nodes = |k: &str| -> Option<(String, String)> {
            let v = self.get(&p, k)?;
            let items: Vec<String> = self
                .velems(v)
                .into_iter()
                .map(|x| unquoted(&self.text(x)))
                .collect::<Option<_>>()?;
            match items.as_slice() {
                [o] => Some((o.clone(), "0".into())),
                [o, r] => Some((o.clone(), r.clone())),
                _ => None,
            }
        };
        let freq = || -> Option<Sweep> {
            // `values=[f]` (cmd-analysis-ac.md): one frequency, SPICE's
            // one-point linear sweep.
            if let Some(v) = self.get(&p, "values") {
                let [f] = self.velems(v)[..] else {
                    return None;
                };
                let f = self.number(f)?;
                return Some(Sweep {
                    variation: "lin",
                    points: 1,
                    start: f,
                    stop: f,
                });
            }
            let mode = ["dec", "oct", "lin", "log"]
                .into_iter()
                .find(|m| s("mode").is_some_and(|x| x.eq_ignore_ascii_case(m)))?;
            let points = intervals(num("points")?)?.checked_add(u32::from(mode == "lin"))?;
            Some(Sweep {
                variation: mode,
                points,
                start: num("from")?,
                stop: num("to")?,
            })
        };
        let a = match kind.as_str() {
            "op" if !axes.is_empty() => {
                let lin = axes.iter().map(|a| match a.sweep {
                    DcSweep::Lin(from, to, step) => Some((a.src.clone(), from, to, step)),
                    DcSweep::Log(_) | DcSweep::List(_) => None,
                });
                match lin.collect::<Option<Vec<_>>>() {
                    Some(dc) => Ana::Dc(dc),
                    None => Ana::DcAxes(axes),
                }
            }
            _ if !axes.is_empty() => return None,
            "op" => Ana::Op,
            "ac" => Ana::Ac(freq()?),
            "tran" => {
                let ic = match self.get(&p, "ic") {
                    Some(v) => self.node_list(v)?,
                    None => vec![],
                };
                let ns = match self.get(&p, "nodeset") {
                    Some(v) => self.node_list(v)?,
                    None => vec![],
                };
                let a = Ana::Tran {
                    step: num("step")?,
                    stop: num("stop")?,
                    start: None,
                    max_step: num("maxstep"),
                    uic: s("icmode").as_deref() == Some("uic"),
                    startup: false,
                };
                return Some(vec![
                    BenchCard::NodeVals(true, nums(ic)),
                    BenchCard::NodeVals(false, nums(ns)),
                    BenchCard::Ana(a),
                ]);
            }
            "noise" => Ana::Noise {
                output: nodes("out")?.0,
                reference: nodes("out")?.1,
                source: self.declared(st, &s("in")?, &['V', 'I'])?,
                sweep: freq()?,
                pps: None,
            },
            "dcxf" => Ana::Tf(s("out")?, String::new()),
            _ => return None,
        };
        Some(vec![BenchCard::Ana(a)])
    }

    /// `sweep name instance="src" parameter="dc"` (or `option="temp"`)
    /// over `from to step`, `from to mode="lin" points=N` (N intervals),
    /// `mode="dec"|"oct" points=N` (per decade or octave) or `values=[…]`
    /// (`docs/cmd-sweep.md`); `None` for any other sweep.
    fn vacask_sweep(&self, sw: u32) -> Option<DcAxis> {
        let p = self.pairs(sw);
        let num = |k: &str| self.get(&p, k).and_then(|v| self.number(v));
        let s = |k: &str| self.get(&p, k).and_then(|v| unquoted(&self.text(v)));
        let src = match (s("instance"), s("parameter"), s("option")) {
            (Some(i), Some(dc), None) if dc == "dc" => self.declared(sw, &i, &['V', 'I'])?,
            // A resistor's `r`: SPICE's `.dc` sweeps a resistor too
            // (ngspice manual §15.3.2).
            (Some(i), Some(r), None) if r == "r" => self.resistor(sw, &i)?,
            (None, None, Some(t)) if t == "temp" => "temp".into(),
            _ => return None,
        };
        let known = [
            "instance",
            "parameter",
            "option",
            "from",
            "to",
            "step",
            "mode",
            "points",
        ];
        if p.iter()
            .any(|(k, _)| !known.contains(&k.as_str()) && k != "values")
        {
            return None;
        }
        let sweep = match (s("mode").as_deref(), self.get(&p, "values")) {
            (_, Some(v)) => DcSweep::List(
                self.velems(v)
                    .into_iter()
                    .map(|x| self.number(x))
                    .collect::<Option<_>>()?,
            ),
            (None, None) => DcSweep::Lin(num("from")?, num("to")?, num("step")?),
            (Some("lin"), None) => {
                let (from, to) = (num("from")?, num("to")?);
                DcSweep::Lin(from, to, (to - from) / num("points")?)
            }
            (Some(m @ ("dec" | "oct")), None) => DcSweep::Log(Sweep {
                variation: if m == "dec" { "dec" } else { "oct" },
                points: intervals(num("points")?)?,
                start: num("from")?,
                stop: num("to")?,
            }),
            (Some(_), None) => return None,
        };
        Some(DcAxis { src, sweep })
    }

    /// The SPICE name of resistor `name` (referenced in statement `n`'s
    /// scope): `r` and its name; `None` for any other instance.
    fn resistor(&self, n: u32, name: &str) -> Option<String> {
        let scope = self.scope_of.get(&n).copied().unwrap_or(NONE);
        let &k = self.insts.get(&(scope, name.to_string()))?;
        match self.named_inst(k)? {
            super::Card::Rcl {
                letter: 'R', name, ..
            } => Some(format!("r{name}")),
            _ => None,
        }
    }

    /// `ic=["n"; v; ...]` (or `{"n", v, ...}`) → node values (the old
    /// lowering's `node_list`).
    fn node_list(&self, v: u32) -> Option<Vec<(String, f64)>> {
        let items: Vec<u32> = match self.tag(v) {
            Tag::Vector | Tag::List => self.ast.list(self.ast.rhs(Node(v)) + 1).to_vec(),
            _ => vec![v],
        };
        items
            .chunks(2)
            .map(|c| Some((unquoted(&self.text(*c.first()?))?, self.number(*c.get(1)?)?)))
            .collect()
    }
}

/// A `save` statement's items, blanks dropped inside each item's
/// parentheses and one group around them all opened (`save ( default
/// p(r1, i) )`); `None` when a parenthesis is unbalanced.
fn save_items(text: &str) -> Option<Vec<String>> {
    let mut t = text.trim();
    if let Some(inner) = t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        t = inner;
    }
    let (mut items, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in t.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    items.push(std::mem::take(&mut cur));
                }
                continue;
            }
            c if c.is_whitespace() => continue,
            _ => {}
        }
        if depth < 0 {
            return None;
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        items.push(cur);
    }
    (depth == 0).then_some(items)
}

/// A `points=` value as a count, its fraction dropped as VACASK drops it
/// (36be560: `points=2.5` runs 2 intervals); `None` below 0 (VACASK:
/// "Number of intervals … must be nonnegative") and above `u32::MAX`,
/// where a cast would saturate.
fn intervals(v: f64) -> Option<u32> {
    (0.0..=f64::from(u32::MAX)).contains(&v).then_some(v as u32)
}
