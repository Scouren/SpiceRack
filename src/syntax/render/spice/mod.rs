//! The SPICE-family writer (ngspice, ESPice, HSPICE, Xyce, LTspice, and
//! CDL in the xschem layout): one deck spine over the reader's plan, spelled by the target's
//! [`Spelling`] row and gated by [`action`]. The targets differ only in
//! table rows and in the `Spell`/`Lift`/`Refuse` answers.
//!
//! This file is the spine (title, loads, globals, functions, includes,
//! parameters and models, subcircuits, sections, the body, the testbench,
//! `.end`; the frame of title, `.end` and control blocks decided once, by
//! the file's role) and the deck-wide cards (loads, globals, functions);
//! `blocks` writes a block's cards (includes, parameters and models, a
//! subcircuit, a section, a body in place), `cards` elements and
//! waveforms, `bench` the testbench and analyses (`xyce_options` Xyce's
//! options, `ngspice_options` the others' and HSPICE's defaults, `xyce_cards`
//! Xyce's spellings of another dialect's cards, `xyce_models` its model cards,
//! `prints` the `.print` cards, `epochs` the script lines of a circuit
//! changed between analyses), `checks` the deck-wide
//! refusals (another family's text, `^`, statistical calls), `laplace` the
//! transfer function a Laplace lift reads, `ternary` a `?:` as the
//! target reads it,
//! `cdl` CDL's layout; the `statistics` lift (`render::draws`) and the name
//! map (`render::names`) are shared with the named writer.
//!
//! Where a change goes:
//! - what one target does with a construct: its capability cell
//!   (`Spell`/`Lift`/`Refuse`, `syntax::dialects`), read through `W::ask`;
//! - a form of the target alone: a `Behaviours` field, read as `self.b`;
//! - a rule between the source and the target (Xyce's model `R` on
//!   ngspice, HSPICE's 25 °C elsewhere): a field of `pair::Pair`, read as
//!   `self.pair`;
//! - an expression rewrite for another family: a row of
//!   `values::REWRITES`, its spelling in `ternary`;
//! - an element card: its kind's `*_card` in `cards`; a testbench card:
//!   its step of `bench::W::bench`; an analysis: its arm of
//!   `analyses::W::analysis`; a model card: `model_cards::W::model`.
//!
//! Cites: ngspice manual ch.2 (the deck), §2.4.3 (`.subckt`), §2.6, §2.7
//! (`.include`, `.lib`), §2.15 (`.if`); HSPICE `.END` (a library ends
//! without one: the old HSPICE grammar's L9); Xyce RG Table 2-36 (BSIM3
//! level 49).
//!
//! Contents:
//! - `pub(super) fn write(rd: &mut Rd, deck: &Deck, lang: Lang) -> (String, Vec<Diag>)`

mod analyses;
mod bench;
mod blocks;
mod cards;
pub(in crate::syntax::render) use cards::{pair_suffix, pairs};
mod cdl;
mod checks;
mod epochs;
mod globals;
mod initial;
mod laplace;
mod model_cards;
mod ngspice_options;
mod pair;
pub(in crate::syntax::render) mod parasitics;
mod prints;
mod sweeps;
mod ternary;
mod tstep;
mod values;
mod verilog;
mod waves;
mod xyce_cards;
mod xyce_models;
mod xyce_options;

use std::collections::{HashMap, HashSet};

use super::Numbers;
use super::capability::{Action, Alt, Construct, Rule};
use super::draws::{self, Layers};
use super::names;
use super::out::{Out, Space};
use super::read::{Deck, Func, Rd};
use super::role::{self, Frame};
use super::spelling::{FuncForm, Spelling};
use crate::syntax::Lang;
use crate::syntax::behaviour::{Behaviours, EndRule, LoadForm, NumberReading, Writer};
use crate::syntax::diag::{Code, Diag};
use crate::syntax::lang::{CONSTRUCT_COUNT, Cells};
use crate::syntax::resolve::{FileId, Role};

/// The writer's state for one deck.
struct W<'a, 'r> {
    rd: &'r Rd<'a>,
    deck: &'r Deck,
    sp: &'static Spelling,
    /// What the target does with each construct.
    cells: &'static Cells,
    /// Per construct: the definition's own code a cell raises.
    details: &'static [u16; CONSTRUCT_COUNT],
    /// The target's behaviours.
    b: &'static Behaviours,
    /// The rules between the source's behaviours and the target's.
    pair: pair::Pair,
    /// The deck's numbers were read with ngspice's arithmetic.
    x: bool,
    o: Out,
    /// The deck's diagnostics: refusals and notes.
    diags: Vec<Diag>,
    /// The `statistics` lift's fixups.
    draws: draws::Draws,
    /// The element names, letters added.
    names: names::Names,
    /// The subcircuit being written (its node; `NONE` at the top): the
    /// scope a reference is looked up in.
    scope: u32,
    /// The deck runs its `.four` cards as `fourier` commands in its
    /// `.control` block, so the cards are left out.
    drop_four: bool,
    /// A section's definitions are being written.
    in_section: bool,
    /// The name a Xyce subcircuit's `M` parameter is written under while
    /// its body is (`W::subckt`).
    m_param: Option<String>,
    /// The source evaluates a statistical function to its nominal (HSPICE
    /// or Xyce outside a Monte Carlo run) where the target draws one
    /// (`ternary::nominal`).
    nominal: bool,
    /// The source applies a device `IC=` in its operating point where the
    /// target does only under UIC (a Xyce deck on ngspice; `W::device_ic`).
    dcop_ic: bool,
    /// The `.ic` node values the devices' `IC=` become (`W::device_ic`).
    device_ics: Vec<(String, String)>,
    /// Xyce: each model card's refusal, judged once (`xyce_cards`), by
    /// its file (`None`: this one) and card; and the kit files judged.
    xyce_verdicts: HashMap<(Option<FileId>, u32), (Option<Code>, xyce_models::Shape)>,
    xyce_files: HashSet<FileId>,
}

/// The deck `rd` read, written for `target`, and its diagnostics: the
/// refusals when any construct has no meaning-keeping form there (the text
/// is then not the deck), the notes of what is kept or left out. The `Lift`
/// rules record their fixups in `rd` before anything is written.
pub(super) fn write(rd: &mut Rd, deck: &Deck, lang: Lang) -> (String, Vec<Diag>) {
    let (sp, cells, b) = (lang.spelling(), lang.cells(), lang.behaviours());
    let mut diags = Vec::new();
    // LTspice's `.tran … startup` on a target with no such start: the
    // top's DC sources are read with its ramp from here on.
    if lang.action(Construct::TranStartup) == Action::Lift(Rule::StartupRamp)
        && let Some(tok) = rd.ramp_startup(deck)
    {
        diags.push(Diag::new(Code::W0037, tok, 0));
    }
    let draws = match lang.action(Construct::Statistics) {
        Action::Lift(Rule::StatisticsDraws) => {
            draws::lift(rd, deck, Layers::Two).unwrap_or_else(|d| {
                diags.push(d);
                draws::Draws::default()
            })
        }
        _ => draws::Draws::default(),
    };
    let rd: &Rd = rd;
    let mut w = W {
        rd,
        deck,
        sp,
        cells,
        details: &lang.row().details,
        b,
        pair: pair::Pair::of(rd.b, b),
        // CDL's cards are the old layout's ngspice cards,
        // which spells numbers without ngspice's arithmetic.
        x: rd.b.numbers == NumberReading::Ngspice
            && b.writer != Writer::Cdl
            && deck.numbers == Numbers::Read,
        o: Out::new(4096),
        diags,
        draws,
        names: names::Names::new(rd, deck, |_, c| cards::renamed(rd, cells, c)),
        scope: crate::syntax::ast::NONE,
        drop_four: false,
        in_section: false,
        m_param: None,
        nominal: false,
        dcop_ic: false,
        device_ics: Vec::new(),
        xyce_verdicts: HashMap::new(),
        xyce_files: HashSet::new(),
    };
    // The testbench, folded once: every step below reads it.
    let tb = w.fold();
    w.dcop_ic = w.pair.device_ic_in_op && !tb.tran_uic();
    w.nominal = w.pair.stat_nominal && !tb.monte_carlo();
    // Verilog evaluates nothing, so no deck-wide check refuses it.
    if b.writer == Writer::Verilog {
        w.verilog();
        return (w.o.finish(), w.diags);
    }
    w.checks();
    if b.writer == Writer::Cdl {
        if !tb.plan.single_epoch() {
            w.scripted(&tb.plan);
        }
        w.cdl(&tb.extra);
        w.unparsed();
        return (w.o.finish(), w.diags);
    }
    let frame = w.frame(&tb);
    let control = if frame.control {
        w.control_run(&tb)
    } else {
        None
    };
    w.drop_four = control.is_some();
    if frame.title {
        w.o.token(&format!("{} {}", sp.comment, deck.top.name), Space::Line);
    }
    // ngspice reads its mode before the deck: the backend lifts this line.
    if let Some((mode, tok)) = &tb.ngbehavior
        && w.ask(Construct::NgBehavior, *tok) == Action::Spell(Alt::NgBehaviorLine)
    {
        w.o.token(&format!("{}{mode}", super::NGBEHAVIOR), Space::Line);
    }
    w.loads(frame.control);
    w.globals();
    w.funcs(&deck.funcs);
    w.file_cards(&deck.top);
    w.params_and_models(&deck.top);
    for sc in &deck.defs {
        w.o.token("", Space::Line);
        w.subckt(sc);
    }
    for s in &deck.sections {
        w.o.token("", Space::Line);
        w.section(s);
    }
    w.body(&deck.top, true);
    w.bench(tb, control.as_deref());
    match frame.end {
        true => w.o.token(".end", Space::None),
        // The deck ends on its last card's line end.
        false => w.o.pop(),
    }
    w.unparsed();
    let text = w.o.finish();
    let text = match w.pair.xyce_globals {
        true => globals::xyce_globals(&text),
        false => text,
    };
    (text, w.diags)
}

// ---- private ----

impl W<'_, '_> {
    /// What the deck is written in. Every file is framed as a deck (title,
    /// `.end`, control blocks), a library too: the role is a guess from the
    /// text (`resolve::role` takes a Python circuit with no analysis yet,
    /// or a deck running only analyses kept as text, Xyce's `.hb`, for a
    /// library), and the frame costs a library nothing where it is
    /// included: a title line is a `*` comment, ngspice-45 and Xyce 7.10
    /// read an included `.end` as nothing and ngspice runs an included
    /// `.control` block (each run by hand). HSPICE's `.END` ends the run
    /// even in an included file, so there a file that runs no analysis (a
    /// library rendered alone, or sources alone) ends without one.
    fn frame(&self, tb: &bench::Tb) -> Frame {
        let mut f = role::frame(Role::Deck);
        if self.b.end_rule == EndRule::WhenRuns {
            f.end = tb.runs();
        }
        f
    }

    /// `c`'s action for the target; a refusal, or the note of a drop or a
    /// copy, is recorded at `token`.
    fn ask(&mut self, c: Construct, token: u32) -> Action {
        let a = self.cells[c as usize];
        if let Action::Refuse(code) | Action::Spell(Alt::Omit(code) | Alt::Copy(code)) = a {
            self.diags
                .push(Diag::new(code, token, self.details[c as usize]));
        }
        a
    }

    fn refuse(&mut self, code: Code, token: u32) {
        self.diags.push(Diag::new(code, token, 0));
    }

    /// A note at `token`: something kept or left out, the deck still
    /// written.
    fn note(&mut self, code: Code, token: u32) {
        self.diags.push(Diag::new(code, token, 0));
    }

    /// Model loads that must precede any card instantiating them:
    /// ngspice's `pre_osdi` control block (when the frame has `control`),
    /// ESPice's and HSPICE's `.hdl` (they compile Verilog-A themselves and
    /// load no OSDI binaries).
    fn loads(&mut self, control: bool) {
        let deck = self.deck;
        let path = |n: u32| self.rd.unquote(self.rd.fields(n)[0]);
        // Each top load's path and the token a diagnostic on it is anchored
        // at: the read ones', then those given beside the text (token 0).
        let read = deck.top.loads.iter().map(|&n| (path(n), self.rd.main(n)));
        let given = deck.given_loads.iter().map(|p| (p.clone(), 0));
        let top: Vec<(String, u32)> = read.chain(given).collect();
        let paths: Vec<String> = top.iter().map(|(p, _)| p.clone()).collect();
        let tok = top.first().map_or(0, |l| l.1);
        let source = crate::veriloga::is_source;
        match self.b.load_form {
            LoadForm::Ngspice if control && !top.is_empty() => {
                match crate::veriloga::osdi_loads(&paths) {
                    Ok(loads) => {
                        self.o.token(".control", Space::Line);
                        for p in &loads {
                            self.o.token(&format!("pre_osdi {p}"), Space::Line);
                        }
                        self.o.token(".endc", Space::Line);
                    }
                    Err(_) => self.refuse(Code::W0004, tok),
                }
            }
            LoadForm::Espice => {
                let defs = deck.defs.iter().flat_map(|s| &s.loads);
                let defs = defs.map(|&n| (path(n), self.rd.main(n)));
                let mut seen: Vec<String> = Vec::new();
                for (l, tok) in top.iter().cloned().chain(defs) {
                    if !source(&l) {
                        self.ask(Construct::OsdiLoad, tok);
                    } else if !seen.contains(&l) {
                        self.o.token(&format!(".hdl \"{l}\""), Space::Line);
                        seen.push(l);
                    }
                }
            }
            LoadForm::Xyce if !top.is_empty() => {
                self.ask(Construct::VerilogALoad, tok);
            }
            LoadForm::Hspice => {
                for (l, _) in &top {
                    if !source(l) {
                        self.ask(Construct::OsdiLoad, tok);
                    } else {
                        self.o.token(&format!(".hdl \"{l}\""), Space::Line);
                    }
                }
            }
            LoadForm::Ltspice => {
                for (l, tok) in &top {
                    let c = match source(l) {
                        true => Construct::VerilogALoad,
                        false => Construct::OsdiLoad,
                    };
                    self.ask(c, *tok);
                }
            }
            LoadForm::Ngspice | LoadForm::Xyce | LoadForm::None => {}
        }
        // ESPice loads a subcircuit's too (above); the others the top's.
        if self.b.load_form != LoadForm::Espice {
            for &n in deck.defs.iter().flat_map(|s| &s.loads) {
                self.note(Code::W0022, self.rd.main(n));
            }
        }
    }

    fn globals(&mut self) {
        if !self.deck.globals.is_empty() {
            let line = format!(".global {}", self.deck.globals.join(" "));
            self.o.token(&line, Space::Line);
        }
    }

    /// The functions, one card each (HSPICE spells one as a parameter).
    fn funcs(&mut self, funcs: &[Func]) {
        for &f in funcs {
            let (name, args, body) = self.rd.func_view(f);
            let (args, body) = (args.join(", "), self.ternaries(&body));
            let tok = match f {
                Func::Param(a) | Func::Card(a) | Func::Real(a) => self.rd.main(a),
            };
            let line = match self.ask(Construct::Func, tok) {
                Action::Spell(Alt::FuncParam) => {
                    debug_assert_eq!(self.sp.func, FuncForm::Param);
                    format!(".param {name}({args})='{body}'")
                }
                _ => format!(".func {name}({args}) {{{body}}}"),
            };
            self.o.token(&line, Space::Line);
        }
    }
}
