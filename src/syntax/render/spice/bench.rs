//! The testbench → SPICE-family cards: the deck's testbench statements
//! folded in read order (options merged into one card, the last `.temp`
//! winning), then options, temperature, initial conditions, saves, prints,
//! measures, `.step`s and the analyses (`analyses.rs`), which an ngspice deck with a
//! `.meas` or `.four` runs from one `.control` block.
//!
//! Cites: ngspice manual §11.1-§11.3 (analyses), §15.4.3 (`.save`), §15.6
//! (`.meas`), §15.4.2 (`.four`, the `fourier` command), §17.5 (`-b -r`:
//! "No .measure possible in batch mode"); HSPICE `.probe`, `.option post`,
//! `.option delmax`, `START=`; Xyce RG pp.96-104 (`.options <pkg>`), p.155
//! (`.step`), Table 6-1; LTspice help (`.step param`, no `.print`); VACASK
//! `simulator/cmd.cpp` (the `control` commands).
//!
//! Contents:
//! - `fn is_four(line: &str) -> bool`, `fn is_continuation(line: &str) -> bool`
//! - `pub(super) struct Tb`, `Tb::runs(&self) -> bool`, `Tb::tran_uic(&self) -> bool`, `Tb::monte_carlo(&self) -> bool`,
//!   `Tb::measures_dc(&self) -> bool`
//! - `W::fold(&self) -> Tb`, `W::control_run(&self, tb: &Tb) -> Option<Vec<String>>`,
//!   `W::bench(&mut self, mut tb: Tb, control: Option<&[String]>)`
//! - `W::vacask_lines(&mut self, lines: &[(String, u32)])`

use super::W;
use crate::syntax::behaviour::{AnalysisForm, ControlBlock, OptionsForm, Writer};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::epochs::{Plan, plan};
use crate::syntax::render::read::{Ana, Bench, BenchCard, Opt, Val};

/// The testbench, folded from its statements in read order.
#[derive(Default)]
pub(super) struct Tb {
    /// The run plan: each analysis in run order, with its statement's
    /// first token and its epoch (`read::epochs`).
    pub(super) plan: Plan,
    /// The portable options, with their statement's first token.
    pub(super) portable: Vec<(String, String, u32)>,
    pub(super) xyce: Vec<(String, String)>,
    pub(super) temp: Option<f64>,
    pub(super) tnom: Option<f64>,
    ics: Vec<(String, Val)>,
    nodesets: Vec<(String, Val)>,
    pub(super) saves: Vec<String>,
    /// The first save card's first token.
    pub(super) save_tok: Option<u32>,
    /// Each `.print`: its analysis, its outputs and its first token.
    pub(super) prints: Vec<(String, Vec<String>, u32)>,
    /// The first token of each options card holding an option not
    /// written: VACASK's own, or Xyce's on another target.
    dropped: Vec<u32>,
    /// The first token of each options card holding a Xyce option that
    /// changes the solution (`xyce_options::changes_solution`), on a
    /// target other than Xyce: refused.
    refused: Vec<u32>,
    measures: Vec<String>,
    /// Each measure's statement's first token.
    meas_toks: Vec<u32>,
    /// Each `.step`'s sweep, name, numbers and first token.
    pub(super) steps: Vec<(Option<&'static str>, String, [f64; 3], u32)>,
    /// Each list `.step`'s name, values and first token.
    pub(super) step_lists: Vec<(String, Vec<f64>, u32)>,
    /// VACASK control statements kept as text: none is written
    /// (`W::vacask_lines`).
    pub(super) extra: Vec<u32>,
    /// Each VACASK `save` naming a module's output variable (not written).
    unsaved: Vec<u32>,
    /// `ngbehavior=`'s mode, with its statement's first token.
    pub(super) ngbehavior: Option<(String, u32)>,
    /// HSPICE's `.option delmax=` (`Opt::MaxStep`), the last given.
    step_cap: Option<f64>,
}

/// Control commands that start an analysis: a `.control` block issuing one
/// owns the run.
const RUN_COMMANDS: [&str; 5] = ["run", "op", "ac", "tran", "dc"];

impl Tb {
    /// Whether the deck runs an analysis.
    pub(super) fn runs(&self) -> bool {
        !self.plan.runs.is_empty()
    }

    /// Whether a transient of the deck has UIC.
    pub(super) fn tran_uic(&self) -> bool {
        self.plan
            .runs
            .iter()
            .any(|a| matches!(a, Ana::Tran { uic: true, .. }))
    }

    /// Whether the deck runs a Monte Carlo analysis.
    pub(super) fn monte_carlo(&self) -> bool {
        self.plan
            .runs
            .iter()
            .any(|a| matches!(a, Ana::MonteCarlo { .. }))
    }

    /// Whether a measure of the deck reads a DC sweep (`.meas dc`).
    pub(super) fn measures_dc(&self) -> bool {
        self.measures
            .iter()
            .any(|m| measure_kind(m).eq_ignore_ascii_case("dc"))
    }
}

/// True if `line` is a `.four` card (any case).
pub(super) fn is_four(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|w| w.eq_ignore_ascii_case(".four"))
}

/// True if `line` continues the card above it (`+ ...`).
pub(super) fn is_continuation(line: &str) -> bool {
    line.trim_start().starts_with('+')
}

impl W<'_, '_> {
    /// Whether an ngspice deck runs its analyses from a `.control` block,
    /// and the `fourier` commands to run there, one per `.four`.
    ///
    /// ngspice refuses `.measure` and ignores `.four` under `-b -r`, so a
    /// deck with either (a card or a raw line) runs every analysis from one
    /// block, in card order, where `.four` is the `fourier` command. `None`
    /// for other targets, when nothing needs a block, the deck has no
    /// `.ac`/`.tran`/`.dc` for a `.measure` to read, or a raw control block
    /// already runs the deck.
    pub(super) fn control_run(&self, tb: &Tb) -> Option<Vec<String>> {
        if self.b.control_block != ControlBlock::Ngspice {
            return None;
        }
        let mut lines: Vec<String> = Vec::new();
        for b in self.deck.defs.iter().chain(std::iter::once(&self.deck.top)) {
            lines.extend(self.raw_lines(&b.raw));
        }
        let raw = || lines.iter().map(String::as_str);
        let mut fourier = fourier_commands(raw());
        for a in &tb.plan.runs {
            if let Ana::Four(f, outs) = a {
                let mut cmd = format!("fourier {}", self.sp.number(*f, self.x));
                outs.iter().for_each(|o| cmd.push_str(&format!(" {o}")));
                fourier.push(cmd);
            }
        }
        if (tb.measures.is_empty() && !raw().any(is_meas) && fourier.is_empty())
            || runs_in_control(raw())
        {
            return None;
        }
        tb.plan
            .runs
            .iter()
            .any(|a| matches!(a, Ana::Ac(_) | Ana::Tran { .. } | Ana::Dc(_)))
            .then_some(fourier)
    }

    /// The deck's testbench statements, folded in read order: once per
    /// deck, by the spine (`spice::write`), which hands it to every step.
    pub(super) fn fold(&self) -> Tb {
        let mut tb = Tb::default();
        // The plan's cards (the analyses) in read order: `plan` builds
        // from them.
        let mut script: Vec<(BenchCard, u32)> = Vec::new();
        for &b in &self.deck.bench {
            let tok = match b {
                Bench::Card(n)
                | Bench::Set(n, _)
                | Bench::Meas(n)
                | Bench::Save(n)
                | Bench::Analysis(n)
                | Bench::Options(n)
                | Bench::NodeVals(n)
                | Bench::Control(n) => self.rd.main(n),
            };
            for card in self.rd.bench_cards(b) {
                self.fold_one(&mut tb, &mut script, card, tok);
            }
        }
        tb.plan = plan(script);
        // The cap is every transient's, wherever its card stands.
        if let Some(cap) = tb.step_cap {
            tb.plan.runs.iter_mut().for_each(|a| a.cap_step(cap));
        }
        tb
    }

    /// The testbench's cards, from `tb` ([`W::fold`]'s): the testbench
    /// spine, one step a line.
    pub(super) fn bench(&mut self, mut tb: Tb, control: Option<&[String]>) {
        self.respell_refs(&mut tb);
        let runs = tb.runs();
        self.hspice_defaults(&mut tb, runs);
        for &tok in &tb.dropped {
            self.note(Code::W0024, tok);
        }
        for &tok in &tb.refused {
            self.refuse(Code::W0017, tok);
        }
        for &tok in &tb.unsaved {
            self.note(Code::W0023, tok);
        }
        self.options_cards(&tb);
        self.node_values(&tb);
        self.saves(&tb);
        self.prints(&tb.prints, &tb.plan.runs);
        self.measures(&tb);
        self.steps(&tb);
        self.step_lists(&tb);
        self.state_steps();
        self.vacask_lines(&tb.extra);
        self.run_analyses(&tb, control);
        self.alter_blocks();
    }

    /// VACASK control statements the reader kept as text: a script line
    /// (`abort`, `print`, `postprocess`: `Rd::vacask_script`) left out
    /// (W0120), any other (`alter`, `var`, `clear`, `elaborate`, an
    /// analysis kept as text) changes what runs and is refused as VACASK's
    /// text; CDL writes neither (W0028).
    pub(super) fn vacask_lines(&mut self, lines: &[u32]) {
        for &n in lines {
            let tok = self.rd.main(n);
            match self.b.writer {
                Writer::Cdl => self.note(Code::W0028, tok),
                Writer::Cards | Writer::Verilog | Writer::Spectre | Writer::Vacask => {
                    match crate::syntax::render::read::vacask_refusal(self.rd, self.b, tok) {
                        Some(c) => self.refuse(c, tok),
                        None => {
                            self.ask(self.rd.kept_construct(tok), tok);
                        }
                    }
                }
            }
        }
    }
}

// ---- private ----

impl W<'_, '_> {
    /// The options card(s) (Xyce's per package, `xyce_options`; the others'
    /// one `.options`, `ngspice_options`), then `.temp` and `.options tnom`.
    fn options_cards(&mut self, tb: &Tb) {
        if self.b.options_form == OptionsForm::XycePackages {
            self.xyce_drops(tb);
            for l in self.xyce_options(tb) {
                self.o.token(&l, Space::Line);
            }
        } else {
            if let Some(line) = self.options_line(tb) {
                self.o.token(&line, Space::Line);
            }
            // A temperature list sets the temperature of every run (Xyce
            // RG `.STEP`), and HSPICE writes it as its own `.temp` below.
            let listed = self.cells[Construct::StepList as usize] == Action::Spell(Alt::TempList)
                && tb
                    .step_lists
                    .iter()
                    .any(|(p, ..)| p.eq_ignore_ascii_case("temp"));
            if let Some(t) = tb.temp.filter(|_| !listed) {
                let t = self.sp.decimal(t, self.x);
                self.o.token(&format!(".temp {t}"), Space::Line);
            }
            if let Some(t) = tb.tnom {
                let t = self.sp.decimal(t, self.x);
                self.o.token(&format!(".options tnom={t}"), Space::Line);
            }
        }
    }

    /// The `.ic` and `.nodeset` cards, then the `.ic` a device `IC=` became
    /// (`initial`) where the deck sets none for its node.
    fn node_values(&mut self, tb: &Tb) {
        for (card, vals) in [(".ic", &tb.ics), (".nodeset", &tb.nodesets)] {
            for (n, v) in vals {
                let v = match v {
                    Val::Num(x) => self.sp.decimal(*x, self.x),
                    // Xyce, ngspice, HSPICE and LTspice take an expression
                    // in their delimiters here (Xyce RG `.IC`, ngspice
                    // manual `.ic`, HSPICE `.IC`; ngspice-45 and Xyce 7.10
                    // ran `.ic v(1)={V1}`).
                    Val::Expr(e) => format!("{}{e}{}", self.sp.expr.0, self.sp.expr.1),
                    Val::Raw(t) => t.clone(),
                };
                self.o.token(&format!("{card} V({n})={v}"), Space::Line);
            }
        }
        for (n, v) in std::mem::take(&mut self.device_ics) {
            if !tb.ics.iter().any(|(m, _)| m.eq_ignore_ascii_case(&n)) {
                self.o.token(&format!(".ic V({n})={v}"), Space::Line);
            }
        }
    }

    /// The `.meas` cards; another dialect's measure ngspice-45 cannot read
    /// refused (W0048).
    fn measures(&mut self, tb: &Tb) {
        for (m, &tok) in tb.measures.iter().zip(&tb.meas_toks) {
            if self.pair.foreign_measures && !ngspice_measures(m) {
                self.refuse(Code::W0048, tok);
                continue;
            }
            // `.MEASURE TRAN ...` is already a card, in any case.
            let card = m.get(..5).is_some_and(|p| p.eq_ignore_ascii_case(".meas"));
            let line = format!("{}{m}", if card { "" } else { ".meas " });
            let line = self.xyce_measure(line);
            self.o.token(&line, Space::Line);
        }
    }

    /// The `.step` sweeps (Xyce's `.step <param>`, the others' `.step
    /// param`); Spectre's total-count `log` refused (W0016).
    fn steps(&mut self, tb: &Tb) {
        for (sweep, param, [a, b, c], tok) in &tb.steps {
            if self.ask(Construct::Step, *tok) != Action::Native {
                continue;
            }
            // Spectre's total-count `log`: Xyce's `.step` sweeps lin, dec,
            // oct or a list (RG p.155), LTspice's lin, oct, dec or a list.
            if *sweep == Some("log") {
                self.refuse(Code::W0016, *tok);
                continue;
            }
            let line = match (self.b.analysis_form, sweep) {
                (AnalysisForm::Xyce, Some(s)) => format!(".step {s} {param} {a} {b} {c}"),
                (AnalysisForm::Xyce, None) => format!(".step {param} {a} {b} {c}"),
                (_, Some(s)) => format!(".step {s} param {param} {a} {b} {c}"),
                (_, None) => format!(".step param {param} {a} {b} {c}"),
            };
            self.o.token(&line, Space::Line);
        }
    }

    /// The list `.step`s; HSPICE's temperature list as `.temp t1 t2 …`.
    fn step_lists(&mut self, tb: &Tb) {
        for (param, values, tok) in &tb.step_lists {
            let act = self.ask(Construct::StepList, *tok);
            let vals: Vec<String> = values.iter().map(|v| self.sp.number(*v, self.x)).collect();
            let temp = param.eq_ignore_ascii_case("temp");
            // HSPICE UG `.TEMP t1 t2 …`: the analyses at each temperature.
            if act == Action::Spell(Alt::TempList) {
                match temp {
                    true => self
                        .o
                        .token(&format!(".temp {}", vals.join(" ")), Space::Line),
                    false => self.refuse(Code::W0016, *tok),
                }
                continue;
            }
            if act != Action::Native {
                continue;
            }
            // Xyce RG `.STEP <name> LIST`; LTspice help `.step
            // param <name> list`, `temp` bare.
            let name = match (self.b.analysis_form, param.eq_ignore_ascii_case("temp")) {
                (AnalysisForm::Ltspice, false) => format!("param {param}"),
                _ => param.clone(),
            };
            self.o.token(
                &format!(".step {name} list {}", vals.join(" ")),
                Space::Line,
            );
        }
    }

    /// The analyses: from one `.control` block with `control`'s `fourier`
    /// commands ([`W::control_run`]), else one card each.
    fn run_analyses(&mut self, tb: &Tb, control: Option<&[String]>) {
        // Paired with `W::control_run`, which decides the block.
        if control.is_some() {
            assert_eq!(self.b.control_block, ControlBlock::Ngspice);
        }
        // Paired with `plan`'s own check.
        tb.plan.check();
        // A second circuit state is the target's `Construct::Epochs` cell:
        // ngspice's control script, which then runs every analysis (in
        // the block `control_run` asked for, or one of its own).
        let scripted = !tb.plan.single_epoch() && self.scripted(&tb.plan);
        // The `.step`s the target's cells spell as loops of that script
        // (`loops.rs`): they too run every analysis from the block, and so
        // does a `.dc` whose log or list axis is such a loop.
        let loops = self.loops(tb);
        let dc_looped = self.dc_looped(tb);
        let control = match control {
            None if scripted || !loops.is_empty() || dc_looped => Some(&[] as &[String]),
            given => given,
        };
        let each = || tb.plan.runs.iter().zip(&tb.plan.run_tok);
        match control {
            Some(fourier) => {
                // `fourier` reads the current plot: it follows the first
                // transient (at the end of the block when there is none).
                let at = tb
                    .plan
                    .runs
                    .iter()
                    .position(|a| matches!(a, Ana::Tran { .. }));
                self.o.token(".control", Space::Line);
                self.loop_open(&loops);
                for (i, (a, tok)) in each().enumerate() {
                    if scripted {
                        self.alter_lines(&tb.plan, i);
                    }
                    // As a command, `.four` would run the transient again.
                    if !matches!(a, Ana::Four(..)) {
                        let card = self.analysis(a, *tok);
                        self.o.token(card.trim_start_matches('.'), Space::Line);
                    }
                    if at == Some(i) {
                        fourier.iter().for_each(|f| self.o.token(f, Space::Line));
                    }
                }
                if at.is_none() {
                    fourier.iter().for_each(|f| self.o.token(f, Space::Line));
                }
                self.loop_close(&loops);
                self.o.token(".endc", Space::Line);
            }
            None => {
                // LTspice runs one analysis a deck: the cards follow the
                // line `simulation::run_each` splits at (ZERO P1-007).
                let runs = tb.plan.runs.iter().filter(|a| !matches!(a, Ana::Four(..)));
                if self.b.analysis_form == AnalysisForm::Ltspice && runs.count() > 1 {
                    self.o
                        .token(crate::syntax::render::LTSPICE_EACH, Space::Line);
                }
                for (a, tok) in each() {
                    let card = self.analysis(a, *tok);
                    self.o.token(&card, Space::Line);
                }
            }
        }
    }
    /// `tb`'s outputs (saves, prints, measures) with each
    /// `i(name)` of a renamed element spelled as its declaration
    /// (`names::Names::refs`); the testbench is the top's.
    fn respell_refs(&self, tb: &mut Tb) {
        let top = crate::syntax::ast::NONE;
        let refs = |v: &mut Vec<String>| v.iter_mut().for_each(|s| *s = self.names.refs(top, s));
        refs(&mut tb.saves);
        refs(&mut tb.measures);
        tb.prints.iter_mut().for_each(|(_, outs, _)| refs(outs));
    }

    /// One testbench card into `tb`; a plan card (an analysis, an alter)
    /// onto `script`, with its token.
    fn fold_one(&self, tb: &mut Tb, script: &mut Vec<(BenchCard, u32)>, card: BenchCard, tok: u32) {
        match card {
            c @ BenchCard::Ana(_) => script.push((c, tok)),
            // A plan card at its own statement's token: a control block's
            // cards share the block's.
            BenchCard::Alter { key, value, node } => {
                let tok = self.rd.main(node);
                script.push((BenchCard::Alter { key, value, node }, tok));
            }
            BenchCard::Options(opts) => {
                for o in opts {
                    match o {
                        Opt::Portable(k, v) => tb.portable.push((k, v, tok)),
                        Opt::Xyce(k, v) => {
                            if self.b.options_form != OptionsForm::XycePackages {
                                // One only Xyce solves with is refused, any
                                // other left out with a note.
                                let cards = match super::xyce_options::changes_solution(&k, &v) {
                                    true => &mut tb.refused,
                                    false => &mut tb.dropped,
                                };
                                if !cards.contains(&tok) {
                                    cards.push(tok);
                                }
                            }
                            tb.xyce.push((k, v));
                        }
                        Opt::Temp(t) => tb.temp = Some(t),
                        Opt::Tnom(t) => tb.tnom = Some(t),
                        Opt::MaxStep(x) => tb.step_cap = Some(x),
                        Opt::Vacask(..) => {
                            if !tb.dropped.contains(&tok) {
                                tb.dropped.push(tok);
                            }
                        }
                        Opt::NgBehavior(mode) => tb.ngbehavior = Some((mode, tok)),
                    }
                }
            }
            BenchCard::Saves(s) => {
                tb.save_tok.get_or_insert(tok);
                tb.saves.extend(s);
            }
            BenchCard::Step { sweep, param, nums } => tb.steps.push((sweep, param, nums, tok)),
            BenchCard::StepList { param, values } => tb.step_lists.push((param, values, tok)),
            BenchCard::Temp(t) => tb.temp = Some(t),
            BenchCard::NodeVals(true, v) => tb.ics.extend(v),
            BenchCard::NodeVals(false, v) => tb.nodesets.extend(v),
            BenchCard::Print(k, outs) => tb.prints.push((k, outs, tok)),
            BenchCard::Meas(m) => {
                tb.measures.push(m);
                tb.meas_toks.push(tok);
            }
            // A VACASK `save` is a save list; any other kept control
            // statement is asked about where the testbench is written.
            BenchCard::Extra(n) => match self.rd.vacask_saves(n) {
                Some((s, dropped)) => {
                    tb.save_tok.get_or_insert(self.rd.main(n));
                    tb.saves.extend(s);
                    tb.unsaved.extend(dropped.then(|| self.rd.main(n)));
                }
                None => tb.extra.push(n),
            },
        }
    }

    /// The save list. A noise run produces `onoise_spectrum`, not node
    /// voltages, so a node list can never name it: ngspice then saves all
    /// (and the names beside it), the others none. HSPICE's `.save` saves
    /// simulator state; `.probe` selects outputs. Xyce's rawfile holds
    /// every vector.
    fn saves(&mut self, tb: &Tb) {
        let noise = tb.plan.runs.iter().any(|a| matches!(a, Ana::Noise { .. }));
        let ngspice = matches!(
            self.b.analysis_form,
            AnalysisForm::Ngspice | AnalysisForm::Espice
        );
        // Asked here, not through `ask`: a deck with no save card has no
        // token to note an omission at.
        let written = match self.cells[Construct::Save as usize] {
            Action::Spell(Alt::Omit(_)) => tb.saves.is_empty(),
            Action::Spell(Alt::Probe) if !noise => {
                for s in &tb.saves {
                    let line = match s.as_str() {
                        "all" => ".option post".to_string(),
                        s => format!(".probe {s}"),
                    };
                    self.o.token(&line, Space::Line);
                }
                true
            }
            _ if ngspice && noise => {
                let mut line = String::from(".save all");
                for s in tb.saves.iter().filter(|s| *s != "all") {
                    line.push_str(&format!(" {s}"));
                }
                self.o.token(&line, Space::Line);
                true
            }
            _ if !tb.saves.is_empty() && !noise => {
                self.o
                    .token(&format!(".save {}", tb.saves.join(" ")), Space::Line);
                true
            }
            _ => tb.saves.is_empty(),
        };
        if let Some(tok) = tb.save_tok.filter(|_| !written) {
            self.note(Code::W0023, tok);
        }
    }
}

/// True if a `.control`/`.endc` block among `lines` issues a run command.
fn runs_in_control<'a>(lines: impl IntoIterator<Item = &'a str>) -> bool {
    let mut inside = false;
    for line in lines {
        let word = line.split_whitespace().next().unwrap_or("");
        if word.eq_ignore_ascii_case(".control") {
            inside = true;
        } else if word.eq_ignore_ascii_case(".endc") {
            inside = false;
        } else if inside && RUN_COMMANDS.iter().any(|c| word.eq_ignore_ascii_case(c)) {
            return true;
        }
    }
    false
}

/// True if `line` is a `.meas`/`.measure` card (any case).
fn is_meas(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .and_then(|w| w.get(..5))
        .is_some_and(|w| w.eq_ignore_ascii_case(".meas"))
}

/// The `.four` cards among `lines`, each joined with its `+` continuation
/// lines, as `fourier` commands.
fn fourier_commands<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut open = false;
    for l in lines {
        if is_continuation(l) {
            if let (true, Some(cmd)) = (open, out.last_mut()) {
                l.trim_start()[1..]
                    .split_whitespace()
                    .for_each(|t| cmd.push_str(&format!(" {t}")));
            }
            continue;
        }
        open = is_four(l);
        if open {
            let mut cmd = String::from("fourier");
            l.split_whitespace()
                .skip(1)
                .for_each(|t| cmd.push_str(&format!(" {t}")));
            out.push(cmd);
        }
    }
    out
}

/// The analysis measure `m` reads: the word after `.meas`/`.measure` when
/// `m` is a whole card, else its first word; empty when it has none.
fn measure_kind(m: &str) -> &str {
    let mut words = m.split_whitespace();
    let first = words.next().unwrap_or("");
    match first
        .get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case(".meas"))
    {
        true => words.next().unwrap_or(""),
        false => first,
    }
}

/// Whether ngspice-45's `.meas` reads measure `m`: an analysis it
/// measures (manual §15.4: `.meas {DC|AC|TRAN|SP}`; `.meas noise` is
/// "unrecognized analysis type 'noise'", run by hand) and no Xyce-only
/// qualifier (`RFC_LEVEL`, Xyce RG `.MEASURE`).
fn ngspice_measures(m: &str) -> bool {
    let kind = measure_kind(m);
    let known = ["tran", "ac", "dc", "sp"]
        .iter()
        .any(|k| kind.eq_ignore_ascii_case(k));
    known && !m.to_ascii_lowercase().contains("rfc_level")
}
