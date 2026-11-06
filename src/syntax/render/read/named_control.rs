//! A VACASK `control` block → [`BenchCard`] views: its options (those
//! SPICE names alike portable), its analyses with the sweeps before them,
//! its `save`s as SPICE output names, and which of its statements only
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
//! - `Rd::vacask_script(&self, head: u32) -> bool`
//! - `Rd::vacask_saves(&self, n: u32) -> Option<(Vec<String>, bool)>`

use super::named_elements::unquoted;
use super::{Ana, BenchCard, Opt, Rd, Sweep};
use crate::syntax::Dialect;
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::token::Tag as T;

impl<'a> Rd<'a> {
    /// A VACASK `control` block: its options and analyses, the rest kept as
    /// statements (the old lowering's `ControlBlock`).
    pub(super) fn vacask_control(&self, n: u32) -> Vec<BenchCard> {
        let mut out = Vec::new();
        let mut sweeps: Vec<u32> = Vec::new();
        let extra = |out: &mut Vec<BenchCard>, s: u32| out.push(BenchCard::Extra(s));
        for &st in self.body(n) {
            let head = self.ttag(self.main(st));
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

    /// Whether the VACASK statement at token `head` (its first) only steers a VACASK run and its
    /// scripts: the control commands `abort` (the error policy,
    /// `docs/cmd-errorhandling.md`), `print` (the console,
    /// `cmd-print.md`), `postprocess` (a program run after the analyses,
    /// `cmd-postprocess.md`), and an `embed` of a file no `load` or
    /// `include` of the deck reads (`input-embed.md`: the file is written
    /// for such a program).
    pub(in crate::syntax::render) fn vacask_script(&self, head: u32) -> bool {
        if self.d != Dialect::Vacask {
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
        if self.d != Dialect::Vacask || self.tag(n) != Tag::Save {
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
        let sweeps: Vec<(String, f64, f64, f64)> = sweeps
            .iter()
            .rev()
            .map(|&sw| {
                let p = self.pairs(sw);
                let num = |k: &str| self.get(&p, k).and_then(|v| self.number(v));
                let s = |k: &str| self.get(&p, k).and_then(|v| unquoted(&self.text(v)));
                let (src, from, to, pts) =
                    (s("instance")?, num("from")?, num("to")?, num("points")?);
                let src = self.declared(sw, &src, &['V', 'I'])?;
                (s("parameter").as_deref() == Some("dc"))
                    .then(|| (src, from, to, (to - from) / pts))
            })
            .collect::<Option<_>>()?;
        let kind = match self.ast.header(Node(st)).master {
            NONE => String::new(),
            w => self.text(w),
        };
        let freq = || -> Option<Sweep> {
            let mode = ["dec", "oct", "lin", "log"]
                .into_iter()
                .find(|m| s("mode").is_some_and(|x| x.eq_ignore_ascii_case(m)))?;
            let points = num("points")? as u32 + u32::from(mode == "lin");
            Some(Sweep {
                variation: mode,
                points,
                start: num("from")?,
                stop: num("to")?,
            })
        };
        let a = match kind.as_str() {
            "op" if !sweeps.is_empty() => Ana::Dc(sweeps),
            _ if !sweeps.is_empty() => return None,
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
                output: s("out")?,
                reference: "0".into(),
                source: self.declared(st, &s("in")?, &['V', 'I'])?,
                sweep: freq()?,
                pps: None,
            },
            "dcxf" => Ana::Tf(s("out")?, String::new()),
            _ => return None,
        };
        Some(vec![BenchCard::Ana(a)])
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

/// Numbered node values as [`BenchCard::NodeVals`] holds them.
fn nums(v: Vec<(String, f64)>) -> Vec<(String, super::Val)> {
    v.into_iter()
        .map(|(n, x)| (n, super::Val::Num(x)))
        .collect()
}
