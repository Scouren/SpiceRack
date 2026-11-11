//! Spectre analysis, `options`, `save`, `ic`/`nodeset` statements →
//! [`BenchCard`] views (VACASK `control` blocks in `named_control`), what each adds to the
//! testbench as the old lowering applied them; `None` when the IR could not
//! hold one; and [`Rd::bench_cards`], every testbench item's views.
//!
//! Cites: the old lowering's named analyses; Spectre Circuit Simulator
//! Reference `[REF]` analysis statements; VACASK `share/doc/vacask/demo`
//! (`analysis`, `sweep`, `options`).
//!
//! Contents:
//! - `fn spectre_refusal(target: &Behaviours, kind: &str) -> Code`, `fn vacask_refusal(rd: &Rd, target: &Behaviours, tok: u32) -> Option<Code>`
//! - `fn nums(v: Vec<(String, f64)>) -> Vec<(String, Val)>`
//! - `Rd::bench_cards(&self, b: Bench) -> Vec<BenchCard>`
//! - `Rd::spectre_analysis(&self, n: u32) -> Option<Ana>`
//! - `Rd::saves(&self, n: u32) -> Option<Vec<String>>`
//! - `Rd::node_vals(&self, n: u32) -> Option<Vec<(String, f64)>>`

use super::{Ana, Bench, BenchCard, Opt, Rd, Sweep};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::{Behaviours, NamedRules};
use crate::syntax::diag::Code;

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

/// What `target` refuses the VACASK `analysis NAME KIND` statement at
/// token `tok` with, when its kind is one no SPICE card reads (VACASK docs
/// cmd-analysis-*.md: `dcinc`, `acxf`, `acstb`, `hb`, `hbac`, `hbnoise`,
/// `pss`, `pac`, `pnoise`, `trannoise`): `W0001` where the target has an
/// analysis of the kind (`Behaviours::vacask_analyses`), `W0015` where it
/// lacks one; `None` for any other statement.
pub(in crate::syntax::render) fn vacask_refusal(
    rd: &Rd,
    target: &Behaviours,
    tok: u32,
) -> Option<Code> {
    const KINDS: [&str; 10] = [
        "dcinc",
        "acxf",
        "acstb",
        "hb",
        "hbac",
        "hbnoise",
        "pss",
        "pac",
        "pnoise",
        "trannoise",
    ];
    let vacask = rd.text_behaviours(tok).named_rules == NamedRules::Vacask;
    let kind = rd.t.text(tok + 2);
    if !vacask || rd.t.text(tok) != "analysis" || !KINDS.contains(&kind) {
        return None;
    }
    Some(match target.vacask_analyses.contains(&kind) {
        true => Code::W0001,
        false => Code::W0015,
    })
}

/// Numbered node values as [`BenchCard::NodeVals`] holds them.
pub(super) fn nums(v: Vec<(String, f64)>) -> Vec<(String, super::Val)> {
    v.into_iter()
        .map(|(n, x)| (n, super::Val::Num(x)))
        .collect()
}

impl<'a> Rd<'a> {
    /// What testbench item `b` adds, in order; empty when the IR held
    /// nothing of it.
    pub(in crate::syntax::render) fn bench_cards(&self, b: Bench) -> Vec<BenchCard> {
        match b {
            Bench::Card(n) => self.bench_card(n).into_iter().collect(),
            Bench::Set(n, value) => vec![self.alterparam_as(n, value)],
            Bench::Meas(k) => vec![BenchCard::Meas(self.line(k))],
            Bench::Save(n) => {
                let outs = self.saves(n).expect("a save the reader placed");
                vec![BenchCard::Saves(outs)]
            }
            Bench::Analysis(n) => match self.spectre_sweep(n) {
                Some(cards) => cards,
                None => self
                    .spectre_analysis(n)
                    .map(BenchCard::Ana)
                    .into_iter()
                    .collect(),
            },
            Bench::Options(n) => vec![BenchCard::Options(self.spectre_options(n))],
            Bench::NodeVals(n) => self
                .node_vals(n)
                .map(|v| BenchCard::NodeVals(self.tag(n) == Tag::Ic, nums(v)))
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
        // `[REF]` 19.1 pp.43-44: `lin=` and `log=` count steps, so a point
        // more than the IR's total; `dec=` counts points per decade. The
        // limits are `start` and `stop`, or `center` and `span`. With
        // `dev=`, `mod=` or `param=` the statement sweeps that parameter
        // at the one frequency `freq=`: no frequency sweep, so it stays
        // text (ZERO-LEDGER P5-021 writes it).
        let sweep = || -> Option<Sweep> {
            let swept = |k: &&str| self.get(&p, k).is_some();
            if ["dev", "mod", "param"].iter().any(swept) {
                return None;
            }
            let (variation, n) = ["dec", "lin", "log"]
                .into_iter()
                .find_map(|v| self.get(&p, v).map(|x| (v, self.count(x))))?;
            // A count is a whole number from 1: `lin=-3` cast to a count
            // is 0 steps, a sweep the deck never wrote; it stays text.
            let n = n.filter(|n| *n >= 1)?;
            let (start, stop) = match (num("start"), num("stop"), num("center"), num("span")) {
                (Some(a), Some(b), None, None) => (a, b),
                (None, None, Some(c), Some(w)) => (c - w / 2.0, c + w / 2.0),
                _ => return None,
            };
            Some(Sweep {
                variation,
                points: n.checked_add(u32::from(variation != "dec"))?,
                start,
                stop,
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

    /// `name sweep param=p start= stop= step= | lin=N | values=[…] { one
    /// analysis }` over a netlist parameter (`[REF]` 19.1 pp.406-409:
    /// `lin=` counts steps): the `.step` and the analysis it repeats;
    /// `None` for any other sweep (a device's parameter, nested sweeps),
    /// which stays the Spectre statement.
    fn spectre_sweep(&self, n: u32) -> Option<Vec<BenchCard>> {
        let h = self.ast.header(Node(n));
        if self.text(h.master) != "sweep" || !h.nodes.is_empty() {
            return None;
        }
        let p = self.pairs(n);
        let known = ["param", "start", "stop", "step", "lin", "values"];
        if p.iter().any(|(k, _)| !known.contains(&k.as_str())) {
            return None;
        }
        let num = |k: &str| self.get(&p, k).and_then(|v| self.number(v));
        let param = self.get(&p, "param").map(|v| self.text(v))?;
        let &[inner] = self.body(n) else {
            return None;
        };
        let a = (self.tag(inner) == Tag::Analysis).then(|| self.spectre_analysis(inner))??;
        if matches!(a, Ana::Spectre { .. }) {
            return None;
        }
        let step = match (self.get(&p, "values"), num("step"), num("lin")) {
            (Some(v), None, None) => BenchCard::StepList {
                param,
                values: self
                    .velems(v)
                    .into_iter()
                    .map(|x| self.number(x))
                    .collect::<Option<_>>()?,
            },
            (None, step, lin) => {
                let (start, stop) = (num("start")?, num("stop")?);
                let step = match (step, lin) {
                    (Some(s), None) => s,
                    (None, Some(l)) if l >= 1.0 => (stop - start) / l,
                    _ => return None,
                };
                BenchCard::Step {
                    sweep: None,
                    param,
                    nums: [start, stop, step],
                }
            }
            _ => return None,
        };
        Some(vec![step, BenchCard::Ana(a)])
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
}
