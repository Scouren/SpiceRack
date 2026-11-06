//! Spectre analysis, `options`, `save`, `ic`/`nodeset` statements and
//! VACASK `control` blocks → [`BenchCard`] views, what each adds to the
//! testbench as the old lowering applied them; `None` when the IR could not
//! hold one; and [`Rd::bench_cards`], every testbench item's views.
//!
//! Cites: the old lowering's named analyses; Spectre Circuit Simulator
//! Reference `[REF]` analysis statements; VACASK `share/doc/vacask/demo`
//! (`analysis`, `sweep`, `options`).
//!
//! Contents:
//! - `fn spectre_refusal(target: &Behaviours, kind: &str) -> Code`
//! - `Rd::bench_cards(&self, b: Bench) -> Vec<BenchCard>`
//! - `Rd::spectre_analysis(&self, n: u32) -> Option<Ana>`
//! - `Rd::saves(&self, n: u32) -> Option<Vec<String>>`
//! - `Rd::node_vals(&self, n: u32) -> Option<Vec<(String, f64)>>`

use super::named_elements::unquoted;
use super::{Ana, Bench, BenchCard, Opt, Rd, Sweep};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::{Behaviours, NamedRules};
use crate::syntax::diag::Code;
use crate::syntax::token::Tag as T;

/// What `target` refuses a Spectre analysis of keyword `kind`
/// ([`Ana::Spectre`]) with: `W0015` when it has no analysis of the kind,
/// `W0001` (the statement kept as Spectre text) when it has one this
/// reader does not translate. ngspice: `.pz`, `.sp`, `.sens` (manual
/// ch.15; ngspice-45 runs no `.pss`); HSPICE: `.pz`, `.sens`, `.lin`,
/// `.hb`, an analysis's `sweep` and `monte=`; Xyce: `.hb`, `.lin`, `.sens`,
/// `.step`, `.sampling` (RG Table 6-1); LTspice: `.step`; VACASK: `hb`,
/// `sweep`, `acxf`, `mc` (`docs/analysis_types.md`).
pub(in crate::syntax::render) fn spectre_refusal(target: &Behaviours, kind: &str) -> Code {
    let has = target.spectre_analyses;
    match has.contains(&kind) {
        true => Code::W0001,
        false => Code::W0015,
    }
}

impl<'a> Rd<'a> {
    /// What testbench item `b` adds, in order; empty when the IR held
    /// nothing of it.
    pub(in crate::syntax::render) fn bench_cards(&self, b: Bench) -> Vec<BenchCard> {
        match b {
            Bench::Card(n) => self.bench_card(n).into_iter().collect(),
            Bench::Meas(k) => vec![BenchCard::Meas(self.line(k))],
            Bench::Save(n) => {
                let outs = self.saves(n).expect("a save the reader placed");
                vec![BenchCard::Saves(outs)]
            }
            Bench::Analysis(n) => self
                .spectre_analysis(n)
                .map(BenchCard::Ana)
                .into_iter()
                .collect(),
            Bench::Options(n) => vec![BenchCard::Options(self.spectre_options(n))],
            Bench::NodeVals(n) => self
                .node_vals(n)
                .map(|v| BenchCard::NodeVals(self.tag(n) == Tag::Ic, v))
                .into_iter()
                .collect(),
            Bench::Control(n) => self.vacask_control(n),
        }
    }

    /// `name [(p n)] kind k=v`: the analyses the old IR held (the old
    /// lowering's `spectre_analysis`).
    pub(in crate::syntax::render) fn spectre_analysis(&self, n: u32) -> Option<Ana> {
        let h = self.ast.header(Node(n));
        let kind = self.text(h.master);
        let ns: Vec<String> = h.nodes.iter().map(|&k| self.name_of(k)).collect();
        let p = self.pairs(n);
        let num = |k: &str| self.get(&p, k).and_then(|v| self.number(v));
        let s = |k: &str| self.get(&p, k).map(|v| self.text(v));
        // `[REF]` p.43: `lin=` and `log=` count steps, so a point more than
        // the IR's total; `dec=` counts points per decade.
        let sweep = || -> Option<Sweep> {
            let (variation, n) = ["dec", "lin", "log"]
                .into_iter()
                .find_map(|v| num(v).map(|x| (v, x as u32)))?;
            Some(Sweep {
                variation,
                points: n.saturating_add(u32::from(variation != "dec")),
                start: num("start")?,
                stop: num("stop")?,
            })
        };
        Some(match (kind.as_str(), ns.as_slice()) {
            ("dc", []) if p.is_empty() => Ana::Op,
            ("dc", []) => {
                // `dev=` and `iprobe=` (below) name a source as the name
                // map spells its declaration (`in1` is `vin1`).
                let source = match (s("dev"), s("param")) {
                    (Some(dev), Some(param)) if param == "dc" => {
                        self.declared(n, &dev, &['V', 'I'])?
                    }
                    (None, Some(param)) => param,
                    _ => return None,
                };
                Ana::Dc(vec![(source, num("start")?, num("stop")?, num("step")?)])
            }
            ("ac", []) => Ana::Ac(sweep()?),
            ("tran", []) => Ana::Tran {
                step: num("step")?,
                stop: num("stop")?,
                start: num("start"),
                max_step: num("maxstep"),
                uic: s("skipdc").as_deref() == Some("yes"),
                startup: false,
            },
            ("montecarlo", []) if self.montecarlo(n).is_some() => self.montecarlo(n)?,
            ("noise", [out, reference]) => Ana::Noise {
                output: out.clone(),
                reference: reference.clone(),
                source: self.declared(n, &s("iprobe")?, &['V', 'I'])?,
                sweep: sweep()?,
                pps: None,
            },
            // `[CMP]` p.790: poles and zeros from the `iprobe` source to
            // the output terminals; ngspice's `.pz` takes the source's
            // nodes as its input, `vol` for a voltage source, `cur` for a
            // current one (manual §15.3.6).
            ("pz", [o1, o2]) if p.len() == 1 => {
                let src = s("iprobe")?;
                let scope = self.scope_of.get(&n).copied().unwrap_or(NONE);
                let &k = self.insts.get(&(scope, src))?;
                let super::Card::Source { letter, np, nm, .. } = self.named_inst(k)? else {
                    return None;
                };
                let tf = if letter == 'V' { "vol" } else { "cur" };
                Ana::Pz([np, nm, o1.clone(), o2.clone()], tf, "pz")
            }
            // `[CMP]` p.826: with no `ports`, every port is active, as
            // ngspice's `.sp` takes every `portnum` source.
            ("sp", [])
                if p.iter()
                    .all(|(k, _)| ["start", "stop", "dec", "lin", "log"].contains(&k.as_str())) =>
            {
                Ana::Sp(sweep()?)
            }
            // A form of the analyses above the IR does not read stays text;
            // any other analysis is the Spectre family's own.
            ("dc" | "ac" | "tran" | "noise" | "sp" | "pz", _) => return None,
            _ if self.text_behaviours(self.main(n)).named_rules == NamedRules::Spectre => {
                Ana::Spectre { kind, node: n }
            }
            _ => return None,
        })
    }

    /// A `save` statement's outputs: a node as written, `dev:p` of a
    /// voltage source `i(<source>)` as the name map spells it; `None` for
    /// another terminal current, which no SPICE output list names as such.
    pub(in crate::syntax::render) fn saves(&self, n: u32) -> Option<Vec<String>> {
        self.fields(n)
            .iter()
            .map(|&o| {
                let s = self.name_of(o);
                match s.split_once(':') {
                    Some((dev, "p")) => Some(format!("i({})", self.declared(n, dev, &['V'])?)),
                    Some(_) => None,
                    None => Some(s),
                }
            })
            .collect()
    }

    /// A Spectre `ic`/`nodeset`'s node values; `None` when one is no
    /// number (the old lowering's `named_testbench`).
    pub(in crate::syntax::render) fn node_vals(&self, n: u32) -> Option<Vec<(String, f64)>> {
        self.fields(n)
            .iter()
            .map(|&k| {
                (self.tag(k) == Tag::Assign).then_some(())?;
                Some((self.key(k), self.number(self.ast.rhs(Node(k)))?))
            })
            .collect()
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// `name montecarlo numruns=N [seed=S] [variations=v] { inner kind }`
    /// (`[REF]` p.174, `variations` p.166) over one analysis statement with
    /// no nodes and no parameters (the Python API's `inner_analysis` and
    /// `inner_type`); `None` for any other form, which stays text.
    fn montecarlo(&self, n: u32) -> Option<Ana> {
        let p = self.pairs(n);
        let known = ["numruns", "seed", "variations"];
        if p.iter().any(|(k, _)| !known.contains(&k.as_str())) {
            return None;
        }
        let whole = |k: &str| {
            let x = self.number(self.get(&p, k)?)?;
            (x >= 0.0 && x.fract() == 0.0 && x <= u32::MAX as f64).then_some(x as u64)
        };
        let runs = u32::try_from(whole("numruns")?).ok()?;
        let seed = match self.get(&p, "seed") {
            Some(_) => Some(whole("seed")?),
            None => None,
        };
        let variations = match self.get(&p, "variations").map(|v| self.text(v)) {
            Some(v) if ["process", "mismatch", "all"].contains(&v.as_str()) => Some(v),
            Some(_) => return None,
            None => None,
        };
        let &[inner] = self.body(n) else {
            return None;
        };
        if self.tag(inner) != Tag::Analysis {
            return None;
        }
        let (h, ih) = (self.ast.header(Node(n)), self.ast.header(Node(inner)));
        if !ih.nodes.is_empty() || !self.fields(inner).is_empty() {
            return None;
        }
        Some(Ana::MonteCarlo {
            name: self.text(h.name),
            runs,
            seed,
            variations,
            inner: (self.text(ih.name), self.text(ih.master)),
        })
    }

    /// `name options k=v` (the old lowering's `spectre_options`).
    fn spectre_options(&self, n: u32) -> Vec<Opt> {
        let mut out = Vec::new();
        for (k, v) in self.pairs(n) {
            out.push(match (k.as_str(), self.number(v)) {
                ("temp", Some(x)) => Opt::Temp(x),
                ("tnom", Some(x)) => Opt::Tnom(x),
                _ => {
                    let k = match k.as_str() {
                        "iabstol" => "abstol",
                        "vabstol" => "vntol",
                        "dcmaxiters" => "max_iterations",
                        other => other,
                    };
                    Opt::Portable(k.to_string(), self.vtext(v))
                }
            });
        }
        out
    }

    /// A VACASK `control` block: its options and analyses, the rest kept as
    /// lines (the old lowering's `ControlBlock`).
    fn vacask_control(&self, n: u32) -> Vec<BenchCard> {
        let mut out = Vec::new();
        let mut sweeps: Vec<u32> = Vec::new();
        let extra = |out: &mut Vec<BenchCard>, s: u32| out.push(BenchCard::Extra(self.line(s)));
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

    /// A VACASK `options` statement in a `control` block (the old
    /// lowering's `vacask_control`, `Ctl`).
    fn vacask_options(&self, st: u32) -> Option<Vec<Opt>> {
        if self.tag(st) != Tag::Options {
            return None;
        }
        let mut out = Vec::new();
        for (k, v) in self.pairs(st) {
            match (k.as_str(), self.number(v)) {
                ("rawfile", _) => {}
                ("temp", Some(x)) => out.push(Opt::Temp(x)),
                ("tnom", Some(x)) => out.push(Opt::Tnom(x)),
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
                    BenchCard::NodeVals(true, ic),
                    BenchCard::NodeVals(false, ns),
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
