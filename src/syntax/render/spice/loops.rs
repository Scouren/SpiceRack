//! The deck's `.step`s → ngspice's control-script loops: one `foreach`
//! per `.step` around every run, the first `.step` innermost, and before
//! the first run what each value sets: a parameter of the deck's top
//! (`alterparam`, then one `reset`), the temperature (`option temp=`), a
//! top-level R, C, L, V or I (`alter`). `reset` reads the deck again: it
//! keeps an `alterparam` and undoes an `option` and an `alter`, so the
//! parameters come first. A `.dc` axis by decade, octave or list is a
//! loop of the same script: one `foreach` an axis around the one `dc`,
//! which holds the looped axis at the loop's value (`dc src $d $d 1`: a
//! sweep of one point; ngspice-45 puts the source back after it, run in
//! `research/impl/evidence/P5-001-s6/`).
//!
//! Cites: ngspice manual §12.14.4.3 (a loop of analyses), §13.6.4
//! (`foreach`), §13.5.5 (`alterparam`, `reset`), §13.5.3 (`alter`); Xyce
//! RG 7.10 §2.1.36 (`.STEP`: the points; "the first .STEP line … will be
//! the innermost loop"; a device name is its primary parameter: R, C, L,
//! V, I); LTspice help `.step`; `python/keywords.rs` `Sweep::Log` (points
//! in all); DESIGN-control-epochs §3 (the loop's spelling), §6 commits 3
//! and 6 (ZERO-LEDGER P5-001 step 3). Run on ngspice-45, Xyce 7.10 and
//! LTspice 26.1.1 (`research/impl/evidence/P5-001-s3/`): the eight points
//! of three nested steps agree in value and order. A range's points are
//! the reader's (`read/sweeps.rs::step_points`), LTspice's own grid too
//! (`ltspice_step`).
//!
//! Contents:
//! - `pub(super) struct Loop`
//! - `W::loops(&mut self, tb: &Tb) -> Vec<Loop>`,
//!   `W::loop_open(&mut self, loops: &[Loop])`,
//!   `W::loop_close(&mut self, loops: &[Loop])`,
//!   `W::dc_looped(&mut self, tb: &Tb) -> bool`,
//!   `W::dc_loops(&self, axes: &[DcAxis]) -> Option<String>`

use super::W;
use super::bench::Tb;
use crate::syntax::ast::NONE;
use crate::syntax::behaviour::CardRules;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::{Ana, DcAxis, DcSweep, STEP_POINTS_MAX, step_points};

/// One `.step` as a loop of the control script: what each value sets and
/// the values, in run order. Its variable is `s<k>`, `k` its place among
/// the deck's loops in read order, from 1.
pub(super) struct Loop {
    set: Set,
    values: Vec<f64>,
}

/// The element letters a `.step` of a bare device name steps the value
/// of (Xyce RG `.STEP`: the devices with a primary parameter).
const PRIMARY: [char; 5] = ['R', 'C', 'L', 'V', 'I'];

/// What a loop's value sets.
enum Set {
    /// A parameter of the deck's top: `alterparam`, then `reset`.
    Param(String),
    /// The circuit temperature: `option temp=`.
    Temp,
    /// A top-level element's value, by its written name: `alter`.
    Element(String),
}

impl W<'_, '_> {
    /// The deck's `.step`s as loops, in read order: those whose cell is
    /// `Spell(Alt::ControlScript)`, each with the values it unrolls to. A
    /// `.step` the script cannot run is refused (W0016) and left out: a
    /// name that is not `temp`, a parameter of the deck's top or a
    /// top-level R, C, L, V, I; numbers that are no grid, or one of more
    /// than [`STEP_POINTS_MAX`] points; a deck with a second circuit state
    /// (its `alterparam`s outlive a `reset`, so the next pass would start
    /// from the last state).
    pub(super) fn loops(&mut self, tb: &Tb) -> Vec<Loop> {
        let scripted = |c: Construct| self.cells[c as usize] == Action::Spell(Alt::ControlScript);
        // LTspice's `.step param` names parameters only.
        let lt = self.rd.b.card_rules == CardRules::Ltspice;
        let mut steps: Vec<(u32, &str, Option<Vec<f64>>)> = Vec::new();
        if scripted(Construct::Step) {
            let ranges = tb.steps.iter();
            steps.extend(ranges.map(|(sweep, name, nums, tok)| {
                let points = step_points(*sweep, *nums).map(|p| p.0);
                (*tok, name.as_str(), points)
            }));
        }
        if scripted(Construct::StepList) {
            let lists = tb.step_lists.iter();
            steps.extend(lists.map(|(name, v, tok)| (*tok, name.as_str(), Some(v.clone()))));
        }
        // Read order: a token's index grows along the deck.
        steps.sort_by_key(|s| s.0);
        let count = steps.len();
        let mut loops = Vec::with_capacity(count);
        for (tok, name, values) in steps {
            let values = values.filter(|v| !v.is_empty() && v.len() <= STEP_POINTS_MAX);
            let set = self.step_set(name, lt).filter(|_| tb.plan.single_epoch());
            match (set, values) {
                (Some(set), Some(values)) => loops.push(Loop { set, values }),
                _ => self.refuse(Code::W0016, tok),
            }
        }
        assert!(loops.len() <= count, "loops: one a `.step` at most");
        assert!(loops.iter().all(|l| !l.values.is_empty()), "loops: a point");
        loops
    }

    /// The loops' opening lines: each `foreach`, the last `.step`
    /// outermost, then what the values set, before the first run.
    pub(super) fn loop_open(&mut self, loops: &[Loop]) {
        // Paired with `loops`' own check.
        assert!(
            loops.iter().all(|l| !l.values.is_empty()),
            "loop_open: a point"
        );
        for (k, l) in loops.iter().enumerate().rev() {
            let vals: Vec<String> = l
                .values
                .iter()
                .map(|v| self.sp.decimal(*v, self.x))
                .collect();
            let line = format!("foreach s{} {}", k + 1, vals.join(" "));
            self.o.token(&line, Space::Line);
        }
        let (mut params, mut rest) = (Vec::new(), Vec::new());
        for (k, l) in loops.iter().enumerate() {
            let v = k + 1;
            match &l.set {
                Set::Param(p) => params.push(format!("alterparam {p}=$s{v}")),
                Set::Temp => rest.push(format!("option temp=$s{v}")),
                Set::Element(e) => rest.push(format!("alter {e}=$s{v}")),
            }
        }
        let reset = (!params.is_empty()).then(|| "reset".to_string());
        assert_eq!(params.len() + rest.len(), loops.len(), "a line a loop");
        for line in params.iter().chain(&reset).chain(&rest) {
            self.o.token(line, Space::Line);
        }
    }

    /// The loops' closing lines: one `end` each.
    pub(super) fn loop_close(&mut self, loops: &[Loop]) {
        for _ in loops {
            self.o.token("end", Space::Line);
        }
    }

    /// Whether a `.dc` of the plan runs as loops of the control script (a
    /// log or list axis where the `DcLogList` cell is
    /// `Spell(Alt::ControlScript)`): the deck's analyses then run from
    /// the block. Beside a `.meas dc`, which would read one pass of the
    /// loops and not the sweep, each such `.dc` the script could run is
    /// refused (W0038).
    pub(super) fn dc_looped(&mut self, tb: &Tb) -> bool {
        if self.cells[Construct::DcLogList as usize] != Action::Spell(Alt::ControlScript) {
            return false;
        }
        let measured = tb.measures_dc();
        let mut looped = false;
        for (a, tok) in tb.plan.runs.iter().zip(&tb.plan.run_tok) {
            let Ana::DcAxes(axes) = a else { continue };
            looped = true;
            if measured && self.dc_loops(axes).is_some() {
                self.refuse(Code::W0038, *tok);
            }
        }
        looped
    }

    /// A `.dc` with a log or list axis as the control script runs it:
    /// that axis and every axis outside it is a `foreach` over its values
    /// (`DcSweep::values`; a linear one unrolled by [`step_points`]), the last
    /// axis outermost, around one `dc` in which a looped axis stands at
    /// the loop's value (`src $d<k> $d<k> 1`: one point, and ngspice puts
    /// the source back after the run); a linear axis inside keeps its
    /// range. The points come in the sweep's own order, one plot a pass.
    /// `None` where the script has no such run: more than two axes
    /// (ngspice's `dc` takes two sources), a looped axis of no value or
    /// more than [`STEP_POINTS_MAX`], or one over a parameter of the
    /// deck's top (`dc` sweeps a source, a resistor or `temp`).
    pub(super) fn dc_loops(&self, axes: &[DcAxis]) -> Option<String> {
        assert!(!axes.is_empty(), "dc_loops: an axis");
        if axes.len() > 2 {
            return None;
        }
        let num = |v: f64| self.sp.number(v, self.x);
        let (mut open, mut dc) = (Vec::new(), String::from("dc"));
        for (k, ax) in axes.iter().enumerate() {
            let values = match ax.sweep {
                DcSweep::Lin(a, b, c) if open.is_empty() => {
                    dc.push_str(&format!(" {} {} {} {}", ax.src, num(a), num(b), num(c)));
                    continue;
                }
                DcSweep::Lin(a, b, c) => step_points(None, [a, b, c]).map(|p| p.0),
                DcSweep::Log(_) | DcSweep::List(_) => ax.sweep.values(),
            };
            let values = values.filter(|v| !v.is_empty() && v.len() <= STEP_POINTS_MAX)?;
            if matches!(self.step_set(&ax.src, true), Some(Set::Param(_))) {
                return None;
            }
            let d = k + 1;
            let vals: Vec<String> = values.iter().map(|v| self.sp.decimal(*v, self.x)).collect();
            open.push(format!("foreach d{d} {}", vals.join(" ")));
            dc.push_str(&format!(" {} $d{d} $d{d} 1", ax.src));
        }
        // Paired with the reader: an all-linear `.dc` is `Ana::Dc`.
        assert!(!open.is_empty(), "dc_loops: a log or list axis");
        let ends = open.len();
        open.reverse();
        open.push(dc);
        open.extend(std::iter::repeat_n("end".to_string(), ends));
        Some(open.join("\n"))
    }
}

// ---- private ----

impl W<'_, '_> {
    /// What a `.step` of `name` sets: the temperature, a parameter the
    /// deck's top declares, else (not from LTspice, `lt`) a top-level R,
    /// C, L, V or I; `None` for any other name (a device or model
    /// parameter `dev:par`, a parameter of an included file).
    fn step_set(&self, name: &str, lt: bool) -> Option<Set> {
        if name.eq_ignore_ascii_case("temp") {
            return Some(Set::Temp);
        }
        let declared = |&a: &u32| self.rd.param_view(a).0.eq_ignore_ascii_case(name);
        if self.deck.top.params.iter().any(declared) {
            return Some(Set::Param(name.to_string()));
        }
        let element = |l: &char| self.names.reference(NONE, name, *l);
        let found = PRIMARY.iter().filter(|_| !lt).find_map(element);
        found.map(|w| Set::Element(w.to_string()))
    }
}
