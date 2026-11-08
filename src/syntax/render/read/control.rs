//! A SPICE `.control` block's lines → what each adds to the deck: an
//! analysis the testbench reads, a `pre_osdi` load, or a script line left out
//! with a note (W0025). Each line is classified once ([`ControlLine`]) and
//! placed by one exhaustive match, so a new class of line is a variant, its
//! test in `Rd::control_line`, and its arm in `Rd::control`.
//!
//! This is the file `research/DESIGN-control-epochs.md` §4 names
//! (`render/read/control.rs`), and [`ControlLine`] is a strict subset of the
//! classes its step 4 (§6: the ngspice `.control` reader, the bounded
//! unroller, W0131) reads. That step adds `Alter` (`alter`, `altermod`,
//! `alterparam`, `reset`), `Loop` and `EndLoop` (`foreach`, `repeat`,
//! `end`), and `Option`, each pushed as the design's `BenchCard`. It also
//! splits [`ControlLine::Script`] into `NoEffect` (W0025: print, plot,
//! write, set) and `DataDependent` (W0131).
//!
//! Cites: ngspice manual ch.17 (the control language), `pre_osdi file` (an
//! OSDI load before the circuit is parsed); the old lowering's `.control`;
//! DESIGN-control-epochs §4, §6 step 4.
//!
//! Contents:
//! - `pub(super) enum ControlLine`
//! - `Rd::control(&self, n: u32, scope: Scope, acc: &mut Acc)`

use super::{Acc, Bench, Rd, Scope};
use crate::syntax::ast::Tag;
use crate::syntax::diag::{Code, Diag};

/// What one `.control` line is to the deck.
pub(super) enum ControlLine {
    /// An analysis the old SPICE grammar reads as its dot card.
    Analysis,
    /// `pre_osdi file` at the top: the file's path.
    PreOsdi(String),
    /// Any other line: the script's, not read (W0025). DESIGN-control-epochs
    /// step 4 splits it into its alter, loop, option, no-effect and
    /// data-dependent classes.
    Script,
}

impl Rd<'_> {
    /// `.control` block `n` (at `scope`): each line's analysis onto the
    /// testbench, its load onto the deck's loads, any other line a note.
    pub(super) fn control(&self, n: u32, scope: Scope, acc: &mut Acc) {
        assert_eq!(self.tag(n), Tag::Control, "a .control block");
        let (bench, notes) = (acc.bench.len(), acc.notes.len());
        for &c in self.body(n) {
            match self.control_line(c, scope) {
                ControlLine::Analysis => acc.bench.push(Bench::Card(c)),
                ControlLine::PreOsdi(path) => acc.control_loads.push(path),
                ControlLine::Script => acc.notes.push(Diag::new(Code::W0025, self.main(c), 0)),
            }
        }
        assert!(acc.bench.len() >= bench, "the testbench only grows");
        assert!(acc.notes.len() >= notes, "the notes only grow");
    }
}

// ---- private ----

impl Rd<'_> {
    /// What `.control` line `c` (in a block at `scope`) is.
    fn control_line(&self, c: u32, scope: Scope) -> ControlLine {
        let head = self.main(c);
        let last = self.line_last(head);
        if self.tag(c) == Tag::Analysis && self.testbench_peg(c) {
            return ControlLine::Analysis;
        }
        if scope == Scope::Top && self.t.text(head).eq_ignore_ascii_case("pre_osdi") && head < last
        {
            // ngspice manual, `pre_osdi file`: an OSDI load before the
            // circuit is parsed.
            let p = self.span(head + 1, last);
            return ControlLine::PreOsdi(p.trim().trim_matches('"').to_string());
        }
        ControlLine::Script
    }
}
