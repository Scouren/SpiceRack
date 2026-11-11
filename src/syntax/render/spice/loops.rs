//! The deck's `.step`s → ngspice's control-script loops: one `foreach`
//! per `.step` around every run, the first `.step` innermost, and before
//! the first run what each value sets: a parameter of the deck's top
//! (`alterparam`, then one `reset`), the temperature (`option temp=`), a
//! top-level R, C, L, V or I (`alter`). `reset` reads the deck again: it
//! keeps an `alterparam` and undoes an `option` and an `alter`, so the
//! parameters come first.
//!
//! Cites: ngspice manual §12.14.4.3 (a loop of analyses), §13.6.4
//! (`foreach`), §13.5.5 (`alterparam`, `reset`), §13.5.3 (`alter`); Xyce
//! RG 7.10 §2.1.36 (`.STEP`: the points; "the first .STEP line … will be
//! the innermost loop"; a device name is its primary parameter: R, C, L,
//! V, I); LTspice help `.step`; `python/keywords.rs` `Sweep::Log` (points
//! in all); DESIGN-control-epochs §3 (the loop's spelling), §6 commits 3
//! and 6 (ZERO-LEDGER P5-001 step 3). Run on ngspice-45, Xyce 7.10 and
//! LTspice 26.1.1 (`research/impl/evidence/P5-001-s3/`): the eight points
//! of three nested steps agree in value and order; LTspice adds the stop
//! value a grid misses and sorts a downward range ascending.
//!
//! Contents:
//! - `pub(super) struct Loop`
//! - `W::loops(&mut self, tb: &Tb) -> Vec<Loop>`,
//!   `W::loop_open(&mut self, loops: &[Loop])`,
//!   `W::loop_close(&mut self, loops: &[Loop])`

use super::W;
use super::bench::Tb;
use crate::syntax::ast::NONE;
use crate::syntax::behaviour::CardRules;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;

/// One `.step` as a loop of the control script: what each value sets and
/// the values, in run order. Its variable is `s<k>`, `k` its place among
/// the deck's loops in read order, from 1.
pub(super) struct Loop {
    set: Set,
    values: Vec<f64>,
}

/// The most points one `.step` unrolls to (ngspice-45 ran a `foreach` of
/// this many values on one line); a longer one is refused.
const STEP_POINTS_MAX: usize = 4096;

/// The element letters a `.step` of a bare device name steps the value
/// of (Xyce RG `.STEP`: the devices with a primary parameter).
const PRIMARY: [char; 5] = ['R', 'C', 'L', 'V', 'I'];

/// The slack of a point count, in steps: a grid that ends this close to
/// its stop value reaches it (`0 1 0.1` is eleven points in doubles).
const GRID_SLACK: f64 = 1e-9;

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
        // LTspice's `.step param` names parameters only, and its grid
        // takes the stop value in.
        let lt = self.rd.b.card_rules == CardRules::Ltspice;
        let mut steps: Vec<(u32, &str, Option<Vec<f64>>)> = Vec::new();
        if scripted(Construct::Step) {
            let ranges = tb.steps.iter();
            steps.extend(
                ranges.map(|(sweep, name, nums, tok)| {
                    (*tok, name.as_str(), points(*sweep, *nums, lt))
                }),
            );
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

/// The points of a ranged `.step`: `lin` (or no keyword) from `start` by
/// `by` while not past `stop`; `dec` and `oct`, `by` points a decade or an
/// octave from `start` while not past `stop`; `log`, `by` points in all
/// from `start` to `stop`. A range that runs away from its stop value is
/// its start alone (Xyce RG `.STEP`). `closed` is LTspice's rule (run):
/// the stop value follows a grid that misses it, a downward range steps
/// down by `|by|`, and its points ascend. `None` when the numbers are no
/// grid (a zero step, a logarithmic range not above zero or of no point)
/// or the grid has [`STEP_POINTS_MAX`] steps or more.
fn points(sweep: Option<&str>, [start, stop, by]: [f64; 3], closed: bool) -> Option<Vec<f64>> {
    // A logarithmic range's steps: `by` of them a factor `base`.
    let log =
        |base: f64| (start > 0.0 && stop > 0.0 && by >= 1.0).then(|| (stop / start).log(base) * by);
    let down = closed && stop < start;
    let d = match (closed, down) {
        (false, _) => by,
        (true, false) => by.abs(),
        (true, true) => -by.abs(),
    };
    // The steps from `start` to `stop`, and a logarithmic grid's ratio.
    let (span, ratio) = match sweep {
        None | Some("lin") => ((stop - start) / d, None),
        Some("dec") => (log(10.0)?, Some(10.0_f64.powf(1.0 / by))),
        Some("oct") => (log(2.0)?, Some(2.0_f64.powf(1.0 / by))),
        Some("log") => {
            log(10.0)?;
            (by - 1.0, Some((stop / start).powf(1.0 / (by - 1.0))))
        }
        Some(_) => return None,
    };
    if !(span.is_finite() && span < STEP_POINTS_MAX as f64) {
        return None;
    }
    let steps = (span + GRID_SLACK).floor().max(0.0);
    let at = |i: f64| match ratio {
        None => start + d * i,
        Some(r) => start * r.powf(i),
    };
    let mut out: Vec<f64> = (0..=steps as u32).map(|i| at(f64::from(i))).collect();
    if closed && span - steps > GRID_SLACK {
        out.push(stop);
    }
    if down {
        out.reverse();
    }
    assert!(!out.is_empty(), "points: the start value at least");
    assert!(out.len() <= STEP_POINTS_MAX + 1, "points: under the cap");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::points;

    /// Oracle: the values Xyce 7.10 and LTspice 26.1.1 stepped through,
    /// each run by hand (research/impl/evidence/P5-001-s3/RESULTS.txt,
    /// §2 and §3), and for `log` the API's sentence (`Sweep::Log`: N
    /// points in all, log-spaced: 10^(3 + 0.5 k)).
    #[test]
    fn a_range_unrolls_to_the_points_its_simulator_steps_through() {
        let near = |got: Option<Vec<f64>>, want: &[f64]| {
            let got = got.expect("a grid");
            assert_eq!(got.len(), want.len(), "{got:?} for {want:?}");
            for (g, w) in got.iter().zip(want) {
                assert!((g - w).abs() <= 1e-8 * w.abs(), "{got:?} for {want:?}");
            }
        };
        // Xyce: x1c.cir, x4.cir, x3.cir, x5.cir.
        near(points(None, [1e3, 1e4, 4e3], false), &[1e3, 5e3, 9e3]);
        let dec = [1e3, 3.16227766e3, 1e4, 3.16227766e4];
        near(points(Some("dec"), [1e3, 5e4, 2.0], false), &dec);
        let oct = [
            1e3,
            1.414213562e3,
            2e3,
            2.828427125e3,
            4e3,
            5.656854249e3,
            8e3,
        ];
        near(points(Some("oct"), [1e3, 8e3, 2.0], false), &oct);
        near(
            points(Some("lin"), [3e3, 1e3, -1e3], false),
            &[3e3, 2e3, 1e3],
        );
        // Xyce RG `.STEP`: a step away from the stop value is the start alone.
        near(points(None, [3e3, 1e3, 1e3], false), &[3e3]);
        // LTspice: l1.cir, l4.cir, l5.cir.
        near(points(None, [1e3, 1e4, 4e3], true), &[1e3, 5e3, 9e3, 1e4]);
        let dec = [1e3, 3.16227766e3, 1e4, 3.16227766e4, 5e4];
        near(points(Some("dec"), [1e3, 5e4, 2.0], true), &dec);
        near(points(None, [3e3, 1e3, 1e3], true), &[1e3, 2e3, 3e3]);
        // Five points from 1k to 100k.
        let log = [1e3, 3.16227766e3, 1e4, 3.16227766e4, 1e5];
        near(points(Some("log"), [1e3, 1e5, 5.0], false), &log);
        // No grid.
        assert_eq!(points(None, [0.0, 1.0, 0.0], false), None);
        assert_eq!(points(Some("dec"), [0.0, 1.0, 2.0], false), None);
        assert_eq!(points(None, [0.0, 1e9, 1.0], false), None);
    }
}
