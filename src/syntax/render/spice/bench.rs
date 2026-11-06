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
//! - `pub(super) struct Tb`, `W::fold(&self) -> Tb`
//! - `W::control_run(&self) -> Option<Vec<String>>`, `W::bench(&mut self, control: Option<&[String]>)`, `W::runs(&self) -> bool`
//! - `W::vacask_lines(&mut self, lines: &[(String, u32)])`

use super::W;
use crate::syntax::Lang;
use crate::syntax::behaviour::{AnalysisForm, ControlBlock, OptionsForm};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::{Ana, Bench, BenchCard, Opt};

/// The testbench, folded from its statements in read order.
#[derive(Default)]
pub(super) struct Tb {
    /// Each analysis, with its statement's first token.
    analyses: Vec<(Ana, u32)>,
    /// The portable options, with their statement's first token.
    pub(super) portable: Vec<(String, String, u32)>,
    pub(super) xyce: Vec<(String, String)>,
    pub(super) temp: Option<f64>,
    pub(super) tnom: Option<f64>,
    ics: Vec<(String, f64)>,
    nodesets: Vec<(String, f64)>,
    saves: Vec<String>,
    /// The first save card's first token.
    save_tok: Option<u32>,
    /// Each `.print`: its analysis, its outputs and its first token.
    prints: Vec<(String, Vec<String>, u32)>,
    /// The first token of each options card holding an option not
    /// written: VACASK's own, or Xyce's on another target.
    dropped: Vec<u32>,
    measures: Vec<String>,
    /// Each `.step`'s sweep, name, numbers and first token.
    steps: Vec<(Option<&'static str>, String, [f64; 3], u32)>,
    /// Each list `.step`'s name, values and first token.
    step_lists: Vec<(String, Vec<f64>, u32)>,
    /// VACASK control lines kept as text, with their statement's first
    /// token: none is written (`W::vacask_lines`).
    pub(super) extra: Vec<(String, u32)>,
    /// `ngbehavior=`'s mode, with its statement's first token.
    pub(super) ngbehavior: Option<(String, u32)>,
}

/// Control commands that start an analysis: a `.control` block issuing one
/// owns the run.
const RUN_COMMANDS: [&str; 5] = ["run", "op", "ac", "tran", "dc"];

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
    pub(super) fn control_run(&self) -> Option<Vec<String>> {
        if self.b.control_block != ControlBlock::Ngspice {
            return None;
        }
        let tb = self.fold();
        let mut lines: Vec<String> = Vec::new();
        for b in self.deck.defs.iter().chain(std::iter::once(&self.deck.top)) {
            lines.extend(self.raw_lines(&b.raw));
        }
        let raw = || lines.iter().map(String::as_str);
        let mut fourier = fourier_commands(raw());
        for (a, _) in &tb.analyses {
            if let Ana::Four(f, outs) = a {
                let mut cmd = format!("fourier {}", Lang::NGSPICE.spelling().number(*f, self.x));
                outs.iter().for_each(|o| cmd.push_str(&format!(" {o}")));
                fourier.push(cmd);
            }
        }
        if (tb.measures.is_empty() && !raw().any(is_meas) && fourier.is_empty())
            || runs_in_control(raw())
        {
            return None;
        }
        tb.analyses
            .iter()
            .any(|(a, _)| matches!(a, Ana::Ac(_) | Ana::Tran { .. } | Ana::Dc(_)))
            .then_some(fourier)
    }

    /// The deck's testbench statements, folded in read order.
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
                self.fold_one(&mut tb, card, tok);
            }
        }
        tb
    }

    /// Whether the deck runs an analysis.
    pub(super) fn runs(&self) -> bool {
        !self.fold().analyses.is_empty()
    }

    /// The testbench's cards.
    pub(super) fn bench(&mut self, control: Option<&[String]>) {
        let mut tb = self.fold();
        self.respell_refs(&mut tb);
        for &tok in &tb.dropped {
            self.note(Code::W0024, tok);
        }
        if self.b.options_form == OptionsForm::XycePackages {
            for l in self.xyce_options(&tb) {
                self.o.token(&l, Space::Line);
            }
        } else {
            let opts: Vec<String> = tb
                .portable
                .iter()
                .map(|(k, v, _)| {
                    let k = if k == "max_iterations" { "ITL1" } else { k };
                    if v.is_empty() {
                        k.to_string()
                    } else {
                        format!("{k}={v}")
                    }
                })
                .collect();
            if !opts.is_empty() {
                self.o
                    .token(&format!(".options {}", opts.join(" ")), Space::Line);
            }
            if let Some(t) = tb.temp {
                let t = self.sp.decimal(t, self.x);
                self.o.token(&format!(".temp {t}"), Space::Line);
            }
            if let Some(t) = tb.tnom {
                let t = self.sp.decimal(t, self.x);
                self.o.token(&format!(".options tnom={t}"), Space::Line);
            }
        }
        for (n, v) in &tb.ics {
            let v = self.sp.decimal(*v, self.x);
            self.o.token(&format!(".ic V({n})={v}"), Space::Line);
        }
        for (n, v) in &tb.nodesets {
            let v = self.sp.decimal(*v, self.x);
            self.o.token(&format!(".nodeset V({n})={v}"), Space::Line);
        }
        self.saves(&tb);
        for (kind, outs, tok) in &tb.prints {
            if self.ask(Construct::Print, *tok) == Action::Native {
                let sep = if kind.is_empty() { "" } else { " " };
                let line = format!(".print{sep}{kind} {}", outs.join(" "));
                self.o.token(&line, Space::Line);
            }
        }
        for m in &tb.measures {
            // `.MEASURE TRAN ...` is already a card, in any case.
            let card = m.get(..5).is_some_and(|p| p.eq_ignore_ascii_case(".meas"));
            let line = format!("{}{m}", if card { "" } else { ".meas " });
            self.o.token(&line, Space::Line);
        }
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
        for (param, values, tok) in &tb.step_lists {
            if self.ask(Construct::StepList, *tok) != Action::Native {
                continue;
            }
            // Xyce RG `.STEP <name> LIST`; LTspice help `.step
            // param <name> list`, `temp` bare.
            let vals: Vec<String> = values.iter().map(|v| self.sp.number(*v, self.x)).collect();
            let name = match (self.b.analysis_form, param.eq_ignore_ascii_case("temp")) {
                (AnalysisForm::Ltspice, false) => format!("param {param}"),
                _ => param.clone(),
            };
            self.o.token(
                &format!(".step {name} list {}", vals.join(" ")),
                Space::Line,
            );
        }
        self.vacask_lines(&tb.extra);
        match control {
            Some(fourier) => {
                // `fourier` reads the current plot: it follows the first
                // transient (at the end of the block when there is none).
                let at = tb
                    .analyses
                    .iter()
                    .position(|(a, _)| matches!(a, Ana::Tran { .. }));
                self.o.token(".control", Space::Line);
                for (i, (a, tok)) in tb.analyses.iter().enumerate() {
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
                self.o.token(".endc", Space::Line);
            }
            None => {
                for (a, tok) in &tb.analyses {
                    let card = self.analysis(a, *tok);
                    self.o.token(&card, Space::Line);
                }
            }
        }
    }

    /// VACASK control lines the reader kept as text (VACASK
    /// `simulator/cmd.cpp`): `abort`, `print` and `postprocess` steer the
    /// script and `save` the outputs, so each is left out (W0025, W0023);
    /// another (`alter`, `var`, `clear`, `elaborate`, an analysis kept as
    /// text) changes what runs and is refused as VACASK's text.
    pub(super) fn vacask_lines(&mut self, lines: &[(String, u32)]) {
        for (l, tok) in lines {
            let head = l.split(|c: char| c.is_whitespace() || c == '(').next();
            match head.unwrap_or("") {
                "abort" | "print" | "postprocess" => self.note(Code::W0025, *tok),
                "save" => self.note(Code::W0023, *tok),
                _ => {
                    self.ask(Construct::ForeignText, *tok);
                }
            }
        }
    }
}

// ---- private ----

impl W<'_, '_> {
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

    /// One testbench card into `tb`.
    fn fold_one(&self, tb: &mut Tb, card: BenchCard, tok: u32) {
        match card {
            BenchCard::Ana(a) => tb.analyses.push((a, tok)),
            BenchCard::Options(opts) => {
                for o in opts {
                    match o {
                        Opt::Portable(k, v) => tb.portable.push((k, v, tok)),
                        Opt::Xyce(k, v) => {
                            if self.b.options_form != OptionsForm::XycePackages
                                && !tb.dropped.contains(&tok)
                            {
                                tb.dropped.push(tok);
                            }
                            tb.xyce.push((k, v));
                        }
                        Opt::Temp(t) => tb.temp = Some(t),
                        Opt::Tnom(t) => tb.tnom = Some(t),
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
            BenchCard::Meas(m) => tb.measures.push(m),
            BenchCard::Extra(l) => tb.extra.push((l, tok)),
        }
    }

    /// The save list. A noise run produces `onoise_spectrum`, not node
    /// voltages, so a node list can never name it: ngspice then saves all
    /// (and the names beside it), the others none. HSPICE's `.save` saves
    /// simulator state; `.probe` selects outputs. Xyce's rawfile holds
    /// every vector.
    fn saves(&mut self, tb: &Tb) {
        let noise = tb
            .analyses
            .iter()
            .any(|(a, _)| matches!(a, Ana::Noise { .. }));
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
