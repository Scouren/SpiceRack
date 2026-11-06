//! The testbench → Spectre statements: the deck's testbench items folded in
//! read order (options merged, the last temperature winning), then
//! `options`, `ic`, `nodeset`, `save` (a `.save`'s and a `.print`'s
//! signals), the measures in a `simulator lang=spice` region, and the
//! analyses (`analyses.rs`), each `.step` a `sweep` block around them.
//!
//! Cites: Spectre Reference 19.1 `[REF]` pp.187-236 (`options`: `reltol`
//! #1, `vabstol` #3, `iabstol` #4, `temp` #6, `tnom` #7, `dcmaxiters` #56),
//! p.490 (`ic`), p.502 (`nodeset`), pp.517-518 (`save`), p.17, p.20
//! (`.measure`), p.64 (`dc`: `dev=`/`param=`), p.43 (`ac`), pp.416-417
//! (`tran`), pp.181-182 (`noise`), pp.438-439 (`xf`), p.524 (`sens`),
//! pp.387-389 (`sp`), pp.406-410 (`sweep`); `[CMP]` pp.323-325
//! (`fourier`), p.886 (`save`: nodes, `inst:index` terminal currents);
//! `[KUN]` B.2 (`simulator lang=`).
//!
//! Contents:
//! - `pub(super) struct Tb`, `pub(super) type Step`, `Tb::stepped(&self) -> Vec<String>`
//! - `W::bench(&mut self)`, `W::path(&self, s: &str) -> String`, `W::fold(&self) -> Tb`, `W::unwritten_outputs(&mut self, tb: &Tb)`
//! - `pub(super) enum Signal`, `fn signal_of(out: &str) -> Option<Signal<'_>>`
//! - `fn bare_node(s: &str) -> &str`

use super::W;
use crate::syntax::behaviour::{ModelSemantics, OptionsForm, Writer};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::Construct;
use crate::syntax::render::read::{Ana, Bench, BenchCard, Opt, Val};

/// The testbench, folded from its statements in read order.
#[derive(Default)]
pub(super) struct Tb {
    /// Each analysis, with its statement's first token.
    pub(super) analyses: Vec<(Ana, u32)>,
    /// The portable options, with their statement's first token.
    pub(super) options: Vec<(String, String, u32)>,
    /// VACASK's own options, values as written, with their statement's
    /// first token.
    pub(super) vacask: Vec<(String, String, u32)>,
    pub(super) temp: Option<f64>,
    pub(super) tnom: Option<f64>,
    pub(super) ics: Vec<(String, Val)>,
    pub(super) nodesets: Vec<(String, Val)>,
    pub(super) saves: Vec<String>,
    /// The first save card's first token.
    pub(super) save_tok: Option<u32>,
    /// Each `.print`'s outputs and first token.
    pub(super) prints: Vec<(Vec<String>, u32)>,
    /// The first token of each options card holding Xyce's own options,
    /// which neither target writes.
    pub(super) xyce: Vec<u32>,
    /// Each `ngbehavior=` option's first token (ngspice's mode, which
    /// neither target has).
    pub(super) ngbehavior: Vec<u32>,
    /// Each `.meas` line, with its statement's first token.
    pub(super) measures: Vec<(String, u32)>,
    /// Each `.step`'s sweep, name, numbers and first token.
    pub(super) steps: Vec<Step>,
    /// Each list `.step`'s name, values and first token.
    pub(super) step_lists: Vec<(String, Vec<f64>, u32)>,
    /// VACASK control statements kept as text.
    pub(super) extra: Vec<u32>,
    /// Each VACASK `save` naming a module's output variable (not written).
    pub(super) unsaved: Vec<u32>,
}

/// A `.step`: its sweep keyword, parameter, start, stop and step, and its
/// statement's first token.
pub(super) type Step = (Option<&'static str>, String, [f64; 3], u32);

/// A SPICE output as a signal Spectre saves (`[CMP]` p.886: a node, or a
/// component's terminal current).
pub(super) enum Signal<'s> {
    /// A voltage's nodes, ground left out (`v(a,b)`, `vdb(a)`: the vector
    /// saved, its view the post-processor's).
    Nodes(Vec<&'s str>),
    /// A device's current (`i(r1)`, `im(r1)`).
    Current(&'s str),
}

/// SPICE's options and Spectre's of the same effect (`[REF]` pp.187-236,
/// by number): `reltol` #1, `vabstol` #3, `iabstol` #4, `chargeabstol` #5,
/// `temp` #6, `tnom` #7, `dcmaxiters` #56 (ngspice's `itl1`, Xyce's `max_iterations`), `scale`
/// #90, `gmin` #128, `pivrel` #181, `pivabs` #182 (SPICE's `pivtol`).
const SPECTRE_OPTIONS: &[(&str, &str)] = &[
    ("reltol", "reltol"),
    ("vntol", "vabstol"),
    ("abstol", "iabstol"),
    ("chgtol", "chargeabstol"),
    ("temp", "temp"),
    ("tnom", "tnom"),
    ("itl1", "dcmaxiters"),
    ("max_iterations", "dcmaxiters"),
    ("scale", "scale"),
    ("gmin", "gmin"),
    ("pivrel", "pivrel"),
    ("pivtol", "pivabs"),
];

impl Tb {
    /// The parameters the `.step`s sweep, list ones too, lowered.
    pub(super) fn stepped(&self) -> Vec<String> {
        let lists = self.step_lists.iter().map(|s| &s.0);
        let steps = self.steps.iter().map(|s| &s.1);
        steps.chain(lists).map(|p| p.to_ascii_lowercase()).collect()
    }
}

impl W<'_, '_> {
    /// The testbench's statements.
    pub(super) fn bench(&mut self) {
        let tb = self.fold();
        self.unwritten_outputs(&tb);
        // VACASK's own options are not Spectre's.
        let mut vacask: Vec<u32> = tb.vacask.iter().map(|o| o.2).collect();
        vacask.dedup();
        for tok in vacask {
            self.note(Code::W0024, tok);
        }
        // `[REF]` p.236: `o1 options scale=1.2`; options take `name=value`
        // only. A SPICE option is written where Spectre has one of the same
        // effect ([`SPECTRE_OPTIONS`]), its number respelled (SPICE
        // `VNTOL=1M` is milli); any other (a flag such as `ACCT`, `itl4`,
        // `method`) is not written: noted.
        let mut opts = Vec::new();
        for (k, v, tok) in &tb.options {
            let k = k.to_ascii_lowercase();
            let spectre = match self.rd.b.options_form {
                OptionsForm::Spectre => Some(k.as_str()),
                _ => SPECTRE_OPTIONS.iter().find(|o| o.0 == k).map(|o| o.1),
            };
            match spectre {
                Some(name) if !v.is_empty() => opts.push(format!("{name}={}", self.value_text(v))),
                _ => self.note(Code::W0024, *tok),
            }
        }
        if !opts.is_empty() {
            self.line(&format!("myopts options {}", opts.join(" ")));
        }
        if let Some(t) = tb.temp {
            self.line(&format!("mytemp options temp={}", self.num(t)));
        }
        if let Some(t) = tb.tnom {
            self.line(&format!("mytnom options tnom={}", self.num(t)));
        }
        for (word, vals) in [("ic", &tb.ics), ("nodeset", &tb.nodesets)] {
            for (node, v) in vals {
                let node = bare_node(&self.signal(node)).to_string();
                // An expression as cards.rs's `val` writes one.
                let v = match v {
                    Val::Num(x) => self.num(*x),
                    Val::Expr(e) | Val::Raw(e) => e.clone(),
                };
                self.line(&format!("{word} {node}={v}"));
            }
        }
        for s in &tb.saves {
            let words = match signal_of(s) {
                Some(sig) => self.save_words(&sig),
                None => bare_node(&self.signal(s)).to_string(),
            };
            if !words.is_empty() {
                self.line(&format!("save {words}"));
            }
        }
        // A `.print`'s signals are saved; its view of them (`vdb`, a
        // difference) is the post-processor's.
        for (outs, _) in &tb.prints {
            let words: Vec<String> = outs
                .iter()
                .filter_map(|o| signal_of(o))
                .map(|sig| self.save_words(&sig))
                .filter(|w| !w.is_empty())
                .collect();
            if !words.is_empty() {
                self.line(&format!("save {}", words.join(" ")));
            }
        }
        // `.measure` is SPICE syntax: fenced by language switches.
        for (m, tok) in &tb.measures {
            self.ask(Construct::Measure, *tok);
            let body = m.trim();
            let body = match body.starts_with('.') {
                true => body.to_string(),
                false => format!(".measure {body}"),
            };
            self.line("simulator lang=spice");
            self.line(&body);
            self.line("simulator lang=spectre");
        }
        // A VACASK control statement: a script line left out, any other
        // refused (no Spectre statement says it).
        for &n in &tb.extra {
            let tok = self.rd.main(n);
            self.ask(self.rd.kept_construct(tok), tok);
        }
        self.line("");
        let mut block: Vec<String> = Vec::new();
        for (a, tok) in &tb.analyses {
            if let Some(s) = self.analysis(a, *tok) {
                block.extend(s.split('\n').map(str::to_string));
            }
        }
        // One `sweep` block per step parameter, the first outermost; a
        // list `.step` is `sweep param=p values=[…]` (`[REF]` pp.406-409),
        // inside the others.
        let lists = tb
            .step_lists
            .iter()
            .enumerate()
            .rev()
            .map(|(i, (p, v, _))| {
                let k = tb.steps.len() + i + 1;
                format!(
                    "sweep{k}_{} sweep param={p} {} {{",
                    suffix(p),
                    self.values(v)
                )
            });
        let lists: Vec<String> = lists.collect();
        for head in lists {
            let mut wrapped = vec![head];
            wrapped.extend(block.into_iter().map(|l| format!("  {l}")));
            wrapped.push("}".into());
            block = wrapped;
        }
        for (idx, step) in tb.steps.iter().enumerate().rev() {
            let Some(head) = self.sweep_head(idx, step) else {
                continue;
            };
            let mut wrapped = vec![head];
            wrapped.extend(block.into_iter().map(|l| format!("  {l}")));
            wrapped.push("}".into());
            block = wrapped;
        }
        for l in &block {
            self.line(l);
        }
    }
}

impl W<'_, '_> {
    /// Notes what the testbench holds that neither target writes: each
    /// `.print`, Xyce's own options, ngspice's mode, the save list where the target saves
    /// A node in a signal position (`save`, `ic`, `nodeset`), its
    /// hierarchy kept: each instance on the path and the node escaped,
    /// joined by `.` (`[CMP]` p.886: `OpAmp1.comp`); Xyce's separator
    /// `:` (`v(X1:n)`, the corpus's Xyce decks) is Spectre's `.`.
    pub(super) fn path(&self, s: &str) -> String {
        let sep = self.rd.b.hier_sep;
        let mut segs: Vec<&str> = s.trim().split(sep).collect();
        let node = segs.pop().map_or(String::new(), |n| self.node(n));
        segs.iter()
            .map(|i| self.dev(i))
            .chain([node])
            .collect::<Vec<_>>()
            .join(".")
    }

    /// every node.
    pub(super) fn unwritten_outputs(&mut self, tb: &Tb) {
        for (outs, tok) in &tb.prints {
            self.ask(Construct::Print, *tok);
            // VACASK's raw file holds every node voltage and source current.
            let unwritten = outs.iter().any(|o| match (signal_of(o), self.b.writer) {
                (None, _) => true,
                (Some(Signal::Current(dev)), Writer::Vacask) => !dev.starts_with(['v', 'V']),
                (Some(_), _) => false,
            });
            if unwritten {
                self.note(Code::W0023, *tok);
            }
        }
        for &tok in &tb.xyce {
            self.note(Code::W0024, tok);
        }
        for &tok in &tb.unsaved {
            self.note(Code::W0023, tok);
        }
        for &tok in &tb.ngbehavior {
            self.ask(Construct::NgBehavior, tok);
        }
        for (_, _, tok) in &tb.step_lists {
            self.ask(Construct::StepList, *tok);
        }
        if let Some(tok) = tb.save_tok {
            self.ask(Construct::Save, tok);
        }
    }

    /// The deck's testbench items, folded in read order.
    pub(super) fn fold(&self) -> Tb {
        let mut tb = Tb::default();
        for &b in &self.deck.bench {
            let tok = match b {
                Bench::Card(n)
                | Bench::Meas(n)
                | Bench::Save(n)
                | Bench::Analysis(n)
                | Bench::Options(n)
                | Bench::NodeVals(n)
                | Bench::Control(n) => self.rd.main(n),
            };
            for card in self.rd.bench_cards(b) {
                match card {
                    BenchCard::Ana(a) => tb.analyses.push((a, tok)),
                    BenchCard::Options(opts) => {
                        for o in opts {
                            match o {
                                // A bare `parser` read as ngspice is Xyce's
                                // option package (sky130's combined kit:
                                // `.options parser scale=1.0u`); ngspice-45
                                // takes it as an unused flag.
                                Opt::Portable(k, v)
                                    if self.rd.b.model_semantics == ModelSemantics::Ngspice
                                        && v.is_empty()
                                        && k.eq_ignore_ascii_case("parser") =>
                                {
                                    if !tb.xyce.contains(&tok) {
                                        tb.xyce.push(tok);
                                    }
                                }
                                // The last value wins, as in ngspice: neither
                                // target takes a name twice.
                                Opt::Portable(k, v) => {
                                    tb.options.retain(|o| !o.0.eq_ignore_ascii_case(&k));
                                    tb.options.push((k, v, tok));
                                }
                                Opt::Temp(t) => tb.temp = Some(t),
                                Opt::Tnom(t) => tb.tnom = Some(t),
                                Opt::Vacask(k, v) => tb.vacask.push((k, v, tok)),
                                Opt::NgBehavior(_) => tb.ngbehavior.push(tok),
                                Opt::Xyce(..) => {
                                    if !tb.xyce.contains(&tok) {
                                        tb.xyce.push(tok);
                                    }
                                }
                            }
                        }
                    }
                    BenchCard::Saves(s) => {
                        tb.save_tok.get_or_insert(tok);
                        tb.saves.extend(s);
                    }
                    BenchCard::Step { sweep, param, nums } => {
                        tb.steps.push((sweep, param, nums, tok))
                    }
                    BenchCard::StepList { param, values } => {
                        tb.step_lists.push((param, values, tok))
                    }
                    BenchCard::Temp(t) => tb.temp = Some(t),
                    BenchCard::NodeVals(true, v) => tb.ics.extend(v),
                    BenchCard::NodeVals(false, v) => tb.nodesets.extend(v),
                    BenchCard::Print(_, outs) => tb.prints.push((outs, tok)),
                    BenchCard::Meas(m) => tb.measures.push((m, tok)),
                    // A VACASK `save` is a save list (VACASK's own writer
                    // copies the statement).
                    BenchCard::Extra(n) => match self.rd.vacask_saves(n) {
                        Some((s, dropped)) if self.b.writer != Writer::Vacask => {
                            tb.save_tok.get_or_insert(self.rd.main(n));
                            tb.saves.extend(s);
                            tb.unsaved.extend(dropped.then(|| self.rd.main(n)));
                        }
                        _ => tb.extra.push(n),
                    },
                }
            }
        }
        tb
    }
}

/// A SPICE signal (`V(out)`, `i(v1)`) as the bare node or device name;
/// other text trimmed (`[REF]` p.502: `nodeset 7=0 out=1`).
pub(super) fn bare_node(s: &str) -> &str {
    let t = s.trim();
    let inner = ["V(", "v(", "I(", "i("]
        .into_iter()
        .find_map(|p| t.strip_prefix(p))
        .and_then(|r| r.strip_suffix(')'));
    inner.map_or(t, str::trim)
}

/// `out` as a [`Signal`]; `None` for an output with no signal form (an
/// expression, an option such as `format=`, a device parameter).
pub(super) fn signal_of(out: &str) -> Option<Signal<'_>> {
    let (w, r) = out.trim().split_once('(')?;
    let args = r.strip_suffix(')')?;
    let parts: Vec<&str> = args.split(',').map(str::trim).collect();
    if parts.iter().any(|p| p.is_empty() || p.contains(['(', ')'])) {
        return None;
    }
    match w.to_ascii_lowercase().as_str() {
        "v" | "vm" | "vr" | "vi" | "vp" | "vph" | "vdb" if parts.len() <= 2 => Some(Signal::Nodes(
            parts.into_iter().filter(|&p| p != "0").collect(),
        )),
        "i" | "im" | "ir" | "ii" | "ip" | "iph" | "idb" if parts.len() == 1 => {
            Some(Signal::Current(parts[0]))
        }
        _ => None,
    }
}

// ---- private ----

impl W<'_, '_> {
    /// `sig`'s words in a Spectre `save`: its nodes, or the current into
    /// its device's first terminal (`r1:1`, `[CMP]` p.886: a terminal by
    /// index; SPICE's `i(r1)` flows into the first node).
    fn save_words(&self, sig: &Signal) -> String {
        match sig {
            Signal::Nodes(nodes) => nodes
                .iter()
                .map(|n| self.path(n))
                .collect::<Vec<_>>()
                .join(" "),
            Signal::Current(dev) => format!("{}:1", self.inst_ref(dev)),
        }
    }

    /// A signal (`v(x)`, `i(x)`, bare `x`), the name inside its wrapper in
    /// Spectre's spelling.
    fn signal(&self, s: &str) -> String {
        let t = s.trim();
        let wrapped = t
            .split_once('(')
            .filter(|(w, r)| matches!(*w, "v" | "V" | "i" | "I") && r.ends_with(')'));
        match wrapped {
            Some((w, r)) => format!("{w}({})", self.path(&r[..r.len() - 1])),
            None => self.path(t),
        }
    }

    /// `sweep<k>_<name> sweep param=p start= stop= step= {` for `.step`
    /// `idx`; `None` when it is refused.
    fn sweep_head(
        &mut self,
        idx: usize,
        (sweep, param, [start, stop, step], tok): &Step,
    ) -> Option<String> {
        let suffix = suffix(param);
        let by = match sweep {
            Some("dec") => "dec",
            Some("log") => "log",
            Some("oct") => {
                self.refuse(Code::W0016, *tok);
                return None;
            }
            _ => "step",
        };
        Some(format!(
            "sweep{}_{suffix} sweep param={param} start={} stop={} {by}={} {{",
            idx + 1,
            self.num(*start),
            self.num(*stop),
            self.num(*step)
        ))
    }
}

/// A sweep's name suffix: `param` lowered, punctuation as `_`.
fn suffix(param: &str) -> String {
    param
        .chars()
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c.to_ascii_lowercase(),
            false => '_',
        })
        .collect()
}
