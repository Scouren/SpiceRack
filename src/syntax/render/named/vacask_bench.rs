//! The testbench → VACASK's `control` block: one `options` line, the
//! stepped parameters as control variables swept ahead of each analysis,
//! and the analyses, initial conditions on each transient's line and
//! nodesets on every `analysis` line; a Spectre `montecarlo` as an `mc`
//! loop around its inner analysis.
//!
//! Cites: VACASK `share/doc/vacask/demo` (`analysis`, `sweep`, `options`,
//! `var`), `docs/cmd-analysis-mc.md` (`mc`; as the old VACASK writer
//! probed them); Spectre Reference 19.1 pp.165-174 (`montecarlo`).
//!
//! Contents:
//! - `W::control(&mut self, tb: &Tb)`

use super::W;
use super::bench::{Step, Tb, bare_node};
use super::lift::draw_scopes;
use super::vacask::{num, q, respell};
use crate::syntax::behaviour::{NamedRules, OptionTo};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::read::{Ana, Card, Comp, DcSweep, Sweep, Val};

impl W<'_, '_> {
    /// The `control` block: options, then per analysis the stepped sweeps
    /// and the analysis, its initial conditions on its `analysis` line.
    pub(super) fn control(&mut self, tb: &Tb) {
        self.unwritten_outputs(tb);
        self.line("");
        self.line("control");
        let mut opts = String::from("  options rawfile=\"binary\"");
        // With a MAXORD the order is the option's: VACASK's `trap` and
        // `gear2` are order 2 whatever `tran_maxord` says, `am` and `gear`
        // read it (`lib/coretran.cpp`, "Set up integration method"; ngspice
        // manual p.318: MAXORD=1 with the trapezoid is backward Euler).
        let ord = tb
            .options
            .iter()
            .any(|o| o.0.eq_ignore_ascii_case("maxord"));
        for (k, v, tok) in &tb.options {
            let k = k.to_ascii_lowercase();
            match self.b.option_map.iter().find(|o| o.0 == k).map(|o| o.1) {
                Some(OptionTo::Name(own)) => opts.push_str(&format!(" {own}={}", respell(v))),
                Some(OptionTo::Method) => match (v.to_ascii_lowercase().as_str(), ord) {
                    ("trap" | "trapezoidal", false) => opts.push_str(" tran_method=\"trap\""),
                    ("trap" | "trapezoidal", true) => opts.push_str(" tran_method=\"am\""),
                    ("gear" | "gear2", false) => opts.push_str(" tran_method=\"gear2\""),
                    ("gear" | "gear2", true) => opts.push_str(" tran_method=\"gear\""),
                    _ => {
                        self.ask(Construct::UnmappedOption, *tok);
                    }
                },
                // The target's own text, `{}` the value (`rshunt=1k` is
                // `gshunt=1/(1e3)`); a text that takes a value the card
                // does not give is no option of the target.
                Some(OptionTo::Spell(text)) if !text.contains("{}") || !v.is_empty() => {
                    opts.push(' ');
                    opts.push_str(&text.replace("{}", &respell(v)));
                }
                Some(OptionTo::Cosmetic) => self.note(Code::W0024, *tok),
                Some(OptionTo::Spell(_)) | None => {
                    self.ask(Construct::UnmappedOption, *tok);
                }
            }
        }
        // A MAXORD with no METHOD: SPICE's default trapezoid at that order.
        if ord && !opts.contains(" tran_method=") {
            opts.push_str(" tran_method=\"am\"");
        }
        for (k, v, _) in &tb.vacask {
            opts.push_str(&format!(" {k}={v}"));
        }
        if let Some(t) = tb.temp {
            opts.push_str(&format!(" temp={}", num(t)));
        }
        if let Some(t) = tb.tnom {
            opts.push_str(&format!(" tnom={}", num(t)));
        }
        self.line(&opts);
        if let Some((_, tok)) = tb.measures.first() {
            self.ask(Construct::Measure, *tok);
        }
        for (_, p, [start, ..], _) in &tb.steps {
            self.line(&format!("  var {}={}", p.to_ascii_lowercase(), num(*start)));
        }
        let mut sweeps: Vec<String> = tb.steps.iter().filter_map(|s| self.step_sweep(s)).collect();
        // A list `.step` is a `sweep … values=[…]` (`lib/answeep.cpp`
        // `setupValueSweep`); `temp` is the `temp` option's.
        for (p, v, _) in &tb.step_lists {
            let name = p.to_ascii_lowercase();
            let what = match name.as_str() {
                "temp" => "option=\"temp\"".to_string(),
                _ => {
                    self.line(&format!("  var {name}={}", num(v[0])));
                    format!("variable=\"{name}\"")
                }
            };
            sweeps.push(format!(
                "sweep step_{} {what} {}",
                sanitize(&name),
                values(v)
            ));
        }
        // `.ic` and a capacitor's `ic=` are a transient's: ngspice forces
        // them in a transient's operating point only (`.ic` beside `.op`
        // leaves the node free, run) and VACASK's `op` has no `ic`
        // ("Parameter 'ic' not found.": it skips the analysis). A nodeset
        // is every analysis's.
        let caps = self.cap_ics();
        let nodeset = match tb.nodesets.is_empty() {
            true => String::new(),
            false => self.node_list("nodeset", &tb.nodesets, &[]),
        };
        // `Construct::Save` is `Spell(Omit)`: VACASK saves every node. A
        // kept control line runs where it stood among the analyses (an
        // `elaborate` picks the circuit the next ones run on): each is
        // written before the analysis that followed it in the source.
        // `W::fold` records each kept line's place in read order.
        assert!(
            tb.extra.is_sorted_by_key(|e| e.1),
            "kept lines in read order"
        );
        assert!(tb.extra.iter().all(|e| e.1 <= tb.plan.runs.len()));
        let mut extra = tb.extra.iter().copied().peekable();
        let swept: Vec<String> = tb
            .plan
            .runs
            .iter()
            .flat_map(|a| match a {
                Ana::Dc(sw) => sw.iter().map(|s| s.0.clone()).collect(),
                Ana::DcAxes(axes) => axes.iter().map(|x| x.src.clone()).collect(),
                _ => vec![],
            })
            .collect();
        let alters = self.dc_alters(&swept);
        let mut dc_types = false;
        // Paired with `plan`'s own check. No reader opens a second epoch
        // yet: every run is on the netlist as written.
        tb.plan.check();
        assert!(tb.plan.single_epoch(), "a second epoch has no writer yet");
        for (idx, (a, tok)) in tb.plan.runs.iter().zip(&tb.plan.run_tok).enumerate() {
            while let Some((n, _)) = extra.next_if(|&(_, before)| before <= idx) {
                let l = self.rd.line(n);
                self.line(&format!("  {l}"));
            }
            // The DC analyses see a waveform source's DC value
            // (`W::dc_alters`), a transient its waveform.
            let tran = spectre_kind(a) == Some("tran")
                || matches!(a, Ana::MonteCarlo { inner: (_, k), .. } if k.eq_ignore_ascii_case("tran"));
            if !alters.is_empty() && tran == dc_types {
                dc_types = !tran;
                for line in alter_lines(&alters, dc_types) {
                    self.line(&format!("  {line}"));
                }
            }
            // A `sweep` binds to the next `analysis`: the stepped sweeps
            // are repeated ahead of each.
            for s in &sweeps {
                self.line(&format!("  {s}"));
            }
            let lines = match a {
                Ana::MonteCarlo { .. } => self.vacask_mc(idx, a, *tok, tb),
                a => self.vacask_analysis(idx, a, *tok),
            };
            let held = match tran {
                true => self.held_ics(a, &caps),
                false => vec![],
            };
            for l in lines {
                let on = l.trim_start().starts_with("analysis");
                let ic = match on && tran && !(tb.ics.is_empty() && held.is_empty()) {
                    true => self.node_list("ic", &tb.ics, &held),
                    false => String::new(),
                };
                let nodeset = if on { nodeset.as_str() } else { "" };
                self.line(&format!("  {l}{ic}{nodeset}"));
            }
        }
        for (n, _) in extra {
            let l = self.rd.line(n);
            self.line(&format!("  {l}"));
        }
        self.line("endc");
    }

    /// Analysis `idx`'s lines (`analysis <kind><idx+1> …`, a DC sweep's
    /// `sweep` lines first); empty when it is refused. One arm per kind: a
    /// new `Ana` kind is one arm here.
    fn vacask_analysis(&mut self, idx: usize, a: &Ana, tok: u32) -> Vec<String> {
        let name = |kind: &str| format!("{kind}{}", idx + 1);
        let mut out = Vec::new();
        match a {
            Ana::Op => out.push(format!("analysis {} op", name("op"))),
            Ana::Dc(sweeps) => {
                if sweeps.is_empty() {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                }
                for (i, (src, start, stop, step)) in sweeps.iter().enumerate().rev() {
                    let scale = match i {
                        0 => "vsweep".to_string(),
                        i => format!("sweep{i}"),
                    };
                    match sweep_line(&scale, src, *start, *stop, *step) {
                        Some(l) => out.push(l),
                        None => {
                            self.refuse(Code::W0015, tok);
                            return vec![];
                        }
                    }
                }
                out.push(format!("  analysis {} op", name("dc")));
            }
            // A log or list axis is its values (`DcSweep::values`), the
            // first axis innermost as for `Ana::Dc`.
            Ana::DcAxes(axes) => {
                if !matches!(self.ask(Construct::DcLogList, tok), Action::Native) {
                    return vec![];
                }
                for (i, ax) in axes.iter().enumerate().rev() {
                    let scale = match i {
                        0 => "vsweep".to_string(),
                        i => format!("sweep{i}"),
                    };
                    let line = match (&ax.sweep, ax.sweep.values()) {
                        (DcSweep::Lin(a, b, c), _) => sweep_line(&scale, &ax.src, *a, *b, *c),
                        (_, Some(v)) => Some(format!(
                            "sweep {scale} instance=\"{}\" parameter=\"dc\" {}",
                            ax.src.to_ascii_lowercase(),
                            values(&v)
                        )),
                        (_, None) => None,
                    };
                    let Some(line) = line else {
                        self.refuse(Code::W0015, tok);
                        return vec![];
                    };
                    out.push(line);
                }
                out.push(format!("  analysis {} op", name("dc")));
            }
            Ana::Ac(sw) => match freq_sweep(sw) {
                Some(f) => out.push(format!("analysis {} ac {f}", name("ac"))),
                None => {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                }
            },
            Ana::Tran {
                step,
                stop,
                start,
                max_step,
                uic,
                ..
            } => {
                // `start` is where the results start, SPICE's TSTART: the
                // circuit is integrated from 0 (`docs/cmd-analysis-tran.md`;
                // measured, first point 5e-4). SPICE's default TMAX is the
                // smaller of TSTEP and (TSTOP-TSTART)/50 (ngspice manual,
                // `.tran`).
                let from = start.unwrap_or(0.0);
                let native = self.rd.b.named_rules == NamedRules::Vacask;
                let tmax = max_step.or((!native).then(|| step.min((stop - from) / 50.0)));
                let mut s = format!(
                    "analysis {} tran step={} stop={}",
                    name("tran"),
                    num(*step),
                    num(*stop)
                );
                if from != 0.0 {
                    s.push_str(&format!(" start={}", num(from)));
                }
                if let Some(t) = tmax {
                    s.push_str(&format!(" maxstep={}", num(t)));
                }
                s.push_str(match uic {
                    true => " icmode=\"uic\"",
                    false => " icmode=\"op\"",
                });
                out.push(s);
            }
            // `out=` is a node to ground or a node pair
            // (`docs/cmd-analysis-noise.md`: "a two-element vector
            // specifies a node pair"; ZERO-LEDGER P4-018).
            Ana::Noise {
                output,
                reference,
                source,
                sweep,
                ..
            } => {
                let Some(f) = freq_sweep(sweep) else {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                };
                let node = bare_node(output).to_ascii_lowercase();
                let pair = match reference.as_str() {
                    "" | "0" => format!("\"{node}\""),
                    r => format!("[\"{node}\", \"{}\"]", r.to_ascii_lowercase()),
                };
                out.push(format!(
                    "analysis {} noise out={pair} in=\"{}\" {f}",
                    name("noise"),
                    source.to_ascii_lowercase()
                ));
            }
            Ana::Tf(output, _) => out.push(format!(
                "analysis {} dcxf out=\"{}\"",
                name("tf"),
                bare_node(output).to_ascii_lowercase()
            )),
            Ana::Sens(..) | Ana::Pz(..) | Ana::Four(..) => {
                self.refuse(Code::W0015, tok);
            }
            // `.sp` is VACASK's `acsp` over every port
            // (`docs/cmd-analysis-acsp.md`; equal to ngspice-45's `.sp` to
            // 12 digits on a 50/75 ohm two-port, ZERO-LEDGER P4-007).
            Ana::Sp(sw) => {
                if !matches!(self.ask(Construct::SParameter, tok), Action::Native) {
                    return vec![];
                }
                let Some(f) = freq_sweep(sw) else {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                };
                let ports = self.acsp_ports().join(", ");
                out.push(format!("analysis {} acsp ports=[{ports}] {f}", name("sp")));
            }
            Ana::MonteCarlo { .. } => unreachable!("written by vacask_mc"),
            Ana::Spectre { kind, .. } => {
                self.refuse(
                    crate::syntax::render::read::spectre_refusal(self.b, kind),
                    tok,
                );
            }
        }
        out
    }

    /// A Spectre `montecarlo` (`idx`, `a`) as VACASK's `mc` loop around
    /// the deck's analysis of its inner kind, named as the loop's own
    /// (`docs/cmd-analysis-mc.md` lines 7-23), `lh=0`: Spectre's default
    /// `sampling=standard` draws plainly (Reference 19.1 pp.165-174), and
    /// `seed` carries the seed, not Spectre's draws. Refused (`W0015`) with
    /// no analysis of that kind, and (`W0115`) where `variations` (default
    /// `process`, p.166) leaves out a scope the deck draws in, as `mc`
    /// varies every call.
    fn vacask_mc(&mut self, idx: usize, a: &Ana, tok: u32, tb: &Tb) -> Vec<String> {
        let Ana::MonteCarlo {
            name,
            runs,
            seed,
            variations,
            inner: (_, kind),
        } = a
        else {
            unreachable!("vacask_mc takes a montecarlo");
        };
        let (process, mismatch) = draw_scopes(self.rd, self.deck);
        let left_out = match variations.as_deref() {
            None | Some("process") => mismatch,
            Some("mismatch") => process,
            _ => false,
        };
        if left_out {
            self.refuse(Code::W0115, tok);
            return vec![];
        }
        let inner = tb
            .plan
            .runs
            .iter()
            .find(|x| spectre_kind(x) == Some(kind.to_ascii_lowercase().as_str()));
        let Some(inner) = inner else {
            self.refuse(Code::W0015, tok);
            return vec![];
        };
        let body = self.vacask_analysis(idx, inner, tok);
        if body.is_empty() {
            return body;
        }
        let seed = seed.map(|s| format!(" seed={s}")).unwrap_or_default();
        let mut out = vec![format!(
            "mc {} samples={runs}{seed} lh=0",
            name.to_ascii_lowercase()
        )];
        out.extend(body.into_iter().map(|l| format!("  {l}")));
        out.push("endmc".into());
        out
    }

    /// A `.step`'s `sweep` line; `None` when it is refused (VACASK sweeps
    /// lin, dec or oct).
    fn step_sweep(&mut self, (sweep, param, [start, stop, step], tok): &Step) -> Option<String> {
        let name = param.to_ascii_lowercase();
        let scale = format!("step_{}", sanitize(&name));
        Some(match sweep {
            Some(mode @ ("dec" | "oct")) => format!(
                "sweep {scale} variable=\"{name}\" from={} to={} mode=\"{mode}\" points={}",
                num(*start),
                num(*stop),
                *step as u64
            ),
            Some("log") => {
                self.refuse(Code::W0016, *tok);
                return None;
            }
            _ => format!(
                "sweep {scale} variable=\"{name}\" from={} to={} step={}",
                num(*start),
                num(*stop),
                num(*step)
            ),
        })
    }
}

// ---- private ----

impl W<'_, '_> {
    /// ` ic={"n", v, …}`: each node and its value ([`W::vval`]), upstream
    /// VACASK's mixed list (36be560; a run of `ic={"out", 0.5}` starts out
    /// at 0.5 V). The fork's `["n"; v]` is its "unexpected ;", `["n", v]`
    /// its "Vector component type incompatibility". `more` are entries
    /// already spelled (`W::cap_ics`' node pairs), after the nodes.
    fn node_list(&self, key: &str, items: &[(String, Val)], more: &[String]) -> String {
        let nodes = items
            .iter()
            .map(|(n, v)| format!("\"{}\", {}", n.to_ascii_lowercase(), self.vval(v)));
        let body: Vec<String> = nodes.chain(more.iter().cloned()).collect();
        format!(" {key}={{{}}}", body.join(", "))
    }

    /// The top's RF ports in `portnum` order, each the pair `acsp` names
    /// (`docs/cmd-analysis-acsp.md`, "Port definition"): its source and
    /// the series resistor `vacask_card` writes for it (`r<decl>_ser`).
    // ponytail: the top's ports only; a port inside a subcircuit has no
    // name here (ngspice numbers the flattened deck's).
    fn acsp_ports(&self) -> Vec<String> {
        let mut ports: Vec<(u32, String)> = Vec::new();
        for &c in &self.deck.top.comps {
            let Comp::Card(n) = c else { continue };
            let Card::Source {
                port: Some((k, _)), ..
            } = self.rd.card_view(n)
            else {
                continue;
            };
            let decl = self.names.decl(n);
            assert!(
                decl.starts_with(['V', 'v']),
                "an RF port is a voltage source"
            );
            let r = q(&format!("{decl}_ser"));
            ports.push((k, format!("\"v{}\", \"r{r}\"", q(&decl[1..]))));
        }
        ports.sort();
        ports.into_iter().map(|(_, p)| p).collect()
    }
}

/// The Spectre analysis keyword of `a` (`[REF]`: an operating point is a
/// `dc` with no sweep, a transfer function `xf`); `None` for a Monte Carlo.
fn spectre_kind(a: &Ana) -> Option<&'static str> {
    Some(match a {
        Ana::Op | Ana::Dc(_) | Ana::DcAxes(_) => "dc",
        Ana::Ac(_) => "ac",
        Ana::Sp(_) => "sp",
        Ana::Tran { .. } => "tran",
        Ana::Noise { .. } => "noise",
        Ana::Tf(..) => "xf",
        Ana::Sens(..) => "sens",
        Ana::Pz(..) => "pz",
        Ana::Four(..) => "fourier",
        Ana::MonteCarlo { .. } | Ana::Spectre { .. } => return None,
    })
}

/// `alter instance("v1", …) type="dc"` (`docs/cmd-alter.md`), or with
/// `dc` false each source's own waveform `type` back.
fn alter_lines(alters: &[(String, &str)], dc: bool) -> Vec<String> {
    if dc {
        let names: Vec<String> = alters.iter().map(|(n, _)| format!("\"{n}\"")).collect();
        return vec![format!("alter instance({}) type=\"dc\"", names.join(", "))];
    }
    alters
        .iter()
        .map(|(n, t)| format!("alter instance(\"{n}\") type=\"{t}\""))
        .collect()
}

/// A DC sweep of instance `src`'s `dc` from `start` by `step` over the
/// whole intervals that fit (its last point `stop` within rounding);
/// `None` for a zero step or a count out of range.
fn sweep_line(scale: &str, src: &str, start: f64, stop: f64, step: f64) -> Option<String> {
    if step == 0.0 {
        return None;
    }
    let span = ((stop - start) / step).abs();
    let n = (span + 1e-9 * span.max(1.0)).floor();
    // `points=0` is one point, `from` (`lib/answeep.cpp`
    // `setupLinearSweep`: `end = points+1`): SPICE's `.dc v1 5 5 1`.
    if n.is_nan() || n > 1e7 {
        return None;
    }
    let last = start + (stop - start).signum() * n * step.abs();
    let last = match (last - stop).abs() <= 1e-9 * step.abs() {
        true => stop,
        false => last,
    };
    // SPICE's `.dc temp` is the `temp` option's sweep (`docs/cmd-sweep.md`).
    let what = match src.eq_ignore_ascii_case("temp") {
        true => "option=\"temp\"".to_string(),
        false => format!("instance=\"{}\" parameter=\"dc\"", src.to_ascii_lowercase()),
    };
    Some(format!(
        "sweep {scale} {what} from={} to={} mode=\"lin\" points={}",
        num(start),
        num(last),
        n as u64
    ))
}

/// `from= to= mode="dec" points=n` (a `lin` sweep counts intervals, not
/// points; one point is `points=0`, `lib/answeep.cpp` `setupLinearSweep`);
/// `None` for `log` or no point.
fn freq_sweep(sw: &Sweep) -> Option<String> {
    let n = match sw.variation {
        "dec" | "oct" => sw.points,
        "lin" => sw.points.saturating_sub(1),
        _ => return None,
    };
    let one = sw.variation == "lin" && sw.points == 1;
    (n != 0 || one).then(|| {
        format!(
            "from={} to={} mode=\"{}\" points={n}",
            num(sw.start),
            num(sw.stop),
            sw.variation
        )
    })
}

/// `values=[v1, v2, …]` in VACASK's numbers.
fn values(v: &[f64]) -> String {
    let v: Vec<String> = v.iter().map(|&x| num(x)).collect();
    format!("values=[{}]", v.join(", "))
}

/// VACASK identifiers carry no punctuation.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c.to_ascii_lowercase(),
            false => '_',
        })
        .collect()
}
