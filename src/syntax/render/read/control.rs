//! A SPICE `.control` block's lines → what each adds to the deck: an
//! analysis the testbench reads, a `pre_osdi` load, a top-level parameter
//! set for the runs after it (`alterparam` at its `reset`: a
//! [`BenchCard::Alter`], the next circuit state of the run plan), a line
//! that changes no run left out with a note (W0025), or a line that
//! changes the circuit or the script's flow and is not read, which
//! refuses the deck (W0160). Each line is classified once
//! ([`ControlLine`]) and placed by one exhaustive match, so a new class of
//! line is a variant, its test in `Rd::control_line`, and its arm in
//! `Rd::control`.
//!
//! This is the file `research/DESIGN-control-epochs.md` §4 names
//! (`render/read/control.rs`) and its §6 step 4 (ZERO-LEDGER P5-001). The
//! rows that read more of the script each take lines out of
//! [`ControlLine::Unread`]: `alter`, `altermod` and `option` (P5-021), the
//! loops and the logic over constants (P5-088), a script that reads its
//! own results (P4-096, W0131).
//!
//! Cites: ngspice-45 manual ch.13 (the control language): §13.5.5
//! `alterparam` ("a subsequent call to reset is required for the
//! parameter value change to become effective"), §13.5.65 `reset`,
//! §13.5.69 `run`, §13.5.73 `set`, §13.7 (the variables); `pre_osdi file`
//! (an OSDI load before the circuit is parsed). Run on ngspice-45
//! (`research/impl/evidence/P5-001-s4/RESULTS.txt`): an `alterparam`
//! changes nothing before its `reset`; `set temp=`, `option` and `alter`
//! change the next run; under `-b` a dot-card analysis runs after the
//! script, and at each `run`.
//!
//! Contents:
//! - `pub(super) enum ControlLine`
//! - `Rd::control(&self, n: u32, scope: Scope, acc: &mut Acc)`
//! - `Rd::alterparam(&self, c: u32) -> Option<BenchCard>`

use super::{Acc, Bench, BenchCard, Rd, Scope};
use crate::syntax::ast::Tag;
use crate::syntax::diag::{Code, Diag};

/// What one `.control` line is to the deck.
pub(super) enum ControlLine {
    /// An analysis the old SPICE grammar reads as its dot card.
    Analysis,
    /// `pre_osdi file` at the top: the file's path.
    PreOsdi(String),
    /// `alterparam name = number`: a top-level parameter's value from the
    /// next `reset` on ([`Rd::alterparam`]).
    Alter,
    /// `reset`: the circuit read again, with every `alterparam` so far.
    Reset,
    /// A line that changes no run (it shows, writes or post-processes
    /// results, or sets how they are shown), or a command read as a dot
    /// card that the card grammar does not read (`meas`): left out with a
    /// note (W0025).
    Skipped,
    /// A line that changes the circuit or the script's flow and is not
    /// read (`alter`, `option`, `set temp=`, `foreach`, `source` …; an
    /// `alterparam` of a subcircuit's parameter or to an expression): the
    /// deck is refused (W0160).
    Unread,
}

/// The commands that only show, write or post-process what the analyses
/// gave (ngspice manual §13.5), lowercased, a blank between two. `run` is one only while the
/// script changes no circuit ([`Rd::control`]). Any other command that is
/// not read is taken to change the circuit ([`ControlLine::Unread`]).
const SHOWS: &str = "asciiplot compose cutout define destroy display echo fft gnuplot hardcopy \
    help history iplot let linearize listing plot print psd quit run rusage save setplot setscale \
    settype show showmod spec status sysinfo undefine unlet version where wrdata write wrnodev wrs2p";

/// The `set` variables that only steer how results are shown or written
/// (ngspice manual §13.7), lowercased, a blank between two; a `color<N>` and a `hcopy…` one
/// besides. A `set` of any other word may be a simulator variable (`set
/// temp=127` changes the next run: run on ngspice-45), so it is unread.
const SHOWN_VARS: &str = "appendwrite askquit brief curplottitle filetype gridstyle height \
    noaskquit nobreak nolegend nomoremode nopadding noprintscale numdgt plotstyle pointchars units \
    width wr_singlescale wr_vecnames xbrushwidth xgridwidth";

impl Rd<'_> {
    /// `.control` block `n` (at `scope`): each line's analysis onto the
    /// testbench, its load onto the deck's loads, each `alterparam` onto
    /// the testbench at the `reset` that applies it, any other line a
    /// note or a refusal ([`ControlLine`]).
    pub(super) fn control(&self, n: u32, scope: Scope, acc: &mut Acc) {
        assert_eq!(self.tag(n), Tag::Control, "a .control block");
        let (bench, notes) = (acc.bench.len(), acc.notes.len());
        // ngspice -b runs the deck's dot-card analyses after the script,
        // on the state it left, and at each `run` (run on ngspice-45):
        // beside a state change, a card read before the block or a `run`
        // is not in the plan's order.
        let dotted =
            |k: u32| self.tag(k) == Tag::Analysis && self.t.text(self.main(k)).starts_with('.');
        let mut beside = acc
            .bench
            .iter()
            .any(|b| matches!(b, Bench::Card(k) if dotted(*k)));
        // The `alterparam` lines waiting for their `reset`, and the first
        // `reset` that applied one.
        let mut pending: Vec<u32> = Vec::new();
        let mut applied: Option<u32> = None;
        for &c in self.body(n) {
            let tok = self.main(c);
            match self.control_line(c, scope) {
                ControlLine::Analysis => acc.bench.push(Bench::Card(c)),
                ControlLine::PreOsdi(path) => acc.control_loads.push(path),
                ControlLine::Alter => pending.push(c),
                ControlLine::Reset if !pending.is_empty() => {
                    applied.get_or_insert(tok);
                    acc.bench.extend(pending.drain(..).map(Bench::Card));
                }
                ControlLine::Reset | ControlLine::Skipped => {
                    beside |= self.t.text(tok).eq_ignore_ascii_case("run");
                    acc.notes.push(Diag::new(Code::W0025, tok, 0));
                }
                ControlLine::Unread => acc.notes.push(Diag::new(Code::W0160, tok, 0)),
            }
        }
        // An `alterparam` no `reset` followed changed no run.
        for c in pending {
            acc.notes.push(Diag::new(Code::W0025, self.main(c), 0));
        }
        if let Some(tok) = applied.filter(|_| beside) {
            acc.notes.push(Diag::new(Code::W0160, tok, 0));
        }
        assert!(acc.bench.len() >= bench, "the testbench only grows");
        assert!(acc.notes.len() >= notes, "the notes only grow");
    }

    /// `.control` line `c` as the parameter it sets: `alterparam name =
    /// number`, the top-level form (ngspice manual §13.5.5). `None` for
    /// any other line, for the subcircuit form (`alterparam sub name =
    /// v`) and for a value that is not one finite number.
    pub(super) fn alterparam(&self, c: u32) -> Option<BenchCard> {
        let head = self.main(c);
        let last = self.line_last(head);
        assert!(head <= last, "a line has a token");
        let word = self.t.text(head);
        if self.tag(c) != Tag::Line || head == last || !word.eq_ignore_ascii_case("alterparam") {
            return None;
        }
        let rest = self.span(head + 1, last);
        let (name, value) = rest.split_once('=')?;
        let name = name.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            return None;
        }
        let value = self
            .number_text(value.trim(), self.t.lexicon_at(head))
            .filter(|v| v.is_finite())?;
        Some(BenchCard::Alter {
            key: name.to_string(),
            value,
            node: c,
        })
    }
}

// ---- private ----

impl Rd<'_> {
    /// What `.control` line `c` (in a block at `scope`) is.
    fn control_line(&self, c: u32, scope: Scope) -> ControlLine {
        let head = self.main(c);
        let last = self.line_last(head);
        assert!(head <= last, "a line has a token");
        if self.tag(c) != Tag::Line {
            // A command read as its dot card: an analysis the card
            // grammar reads, else left out as before (`meas`).
            return match self.tag(c) == Tag::Analysis && self.testbench_peg(c) {
                true => ControlLine::Analysis,
                false => ControlLine::Skipped,
            };
        }
        let word = self.t.text(head).to_ascii_lowercase();
        match word.as_str() {
            // ngspice manual, `pre_osdi file`: an OSDI load before the
            // circuit is parsed.
            "pre_osdi" if scope == Scope::Top && head < last => {
                let p = self.span(head + 1, last);
                ControlLine::PreOsdi(p.trim().trim_matches('"').to_string())
            }
            "pre_osdi" => ControlLine::Skipped,
            "alterparam" if self.alterparam(c).is_some() => ControlLine::Alter,
            "reset" if head == last => ControlLine::Reset,
            "set" | "unset" if self.shows(head, last) => ControlLine::Skipped,
            // `let @dev[p] = …` names a device's parameter.
            "let" if head < last && self.t.text(head + 1).starts_with('@') => ControlLine::Unread,
            w if SHOWS.split(' ').any(|k| k == w) => ControlLine::Skipped,
            _ => ControlLine::Unread,
        }
    }

    /// Whether the `set` or `unset` line from token `head` to `last` only
    /// steers how results are shown or written: no word (`set` lists the
    /// variables), or every variable one of [`SHOWN_VARS`], a `color<N>`
    /// or a `hcopy…` one.
    fn shows(&self, head: u32, last: u32) -> bool {
        assert!(head <= last, "a line has a token");
        if head == last {
            return true;
        }
        let rest = self.span(head + 1, last).to_ascii_lowercase();
        let spaced = rest.replace('=', " = ");
        let mut words = spaced.split_whitespace().peekable();
        while let Some(var) = words.next() {
            let shown = SHOWN_VARS.split(' ').any(|k| k == var)
                || var.starts_with("color")
                || var.starts_with("hcopy");
            if !shown {
                return false;
            }
            // ponytail: a value is one word; a list or a quoted string
            // with blanks reads as an unknown variable (refused). Widen
            // when a deck needs one.
            if words.next_if_eq(&"=").is_some() {
                words.next();
            }
        }
        true
    }
}
