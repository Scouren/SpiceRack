//! A deck in, the oracle's verdict out: the shared way an integration test
//! renders a deck (`syntax::render::render_text`), runs it on a real
//! simulator, and checks it against one of four oracle kinds. Every new
//! test copies one template below; none re-derives a number the way the
//! writer does (AGENTS.md "Testing").
//!
//! The four templates, cheapest first:
//! - [`spec_example`] / [`spec_refusal`]: a manual clause's own example,
//!   written (each expected line present) or refused (exact code list).
//! - [`golden`]: written text compared byte for byte with a fixture that
//!   records its provenance (who checked it, against what) on line 1.
//! - [`closed_form`]: the written deck run on real simulators meets a
//!   number derived by hand.
//! - [`sim_pair`]: the source deck on its own simulator against the written
//!   deck on the target's, one vector at one point.
//!
//! Known failures: a named list compared with what failed, never a count
//! and never a skip (`tests/test_import_suite/main.rs` and
//! `tests/fixtures/spice_bench_known_failures.txt` are the pattern).
//!
//! Simulators run through the cross-sim harness (`tests/test_cross_sim/sim.rs`,
//! the implementer's Phase 1 file, included read-only), not
//! `spicerack::backend`, so the oracle shares no code with what it checks.
//! Only the private `harness` below calls into it (`sim::run`), so a change
//! to its signature breaks one function here. LTspice runs through the
//! backend (the harness's own LTspice is the cross-sim cell's). Spectre has no local binary: its
//! numbers are the iccad4 runs recorded before 2026-11-08 as pins
//! ([`pinned`], ledger P1-043); none can be added (ZERO D43), so a deck
//! with no pin takes the manual page or another simulator as its oracle.
//! `Dialect::Hspice` runs the HSPICE text on ESPice
//! `--tokenizer=hspice` (the ledger's "ESP" oracle); a real HSPICE number is
//! `pinned(Dialect::Hspice, …)`.
//!
//! Unsupported fails, never skips: a simulator missing from PATH fails the
//! test; run in `nix develop .#testing`.
//!
//! Use from a test crate `tests/<name>.rs` (a crate in a folder adds `../`):
//! ```ignore
//! #[path = "common/oracle.rs"]
//! mod oracle;
//! use oracle::{Tol, sim_pair};
//! ```
//!
//! Cites: no clause (helpers); the oracles are each test's.
//!
//! Contents:
//! - `pub struct Tol`, `Tol::within(self, got: f64, want: f64) -> bool`
//! - `pub enum At`
//! - `pub fn write(text: &str, from: impl Into<Lang>, to: Dialect) -> String`
//! - `pub fn written(text: &str, from: impl Into<Lang>, to: Dialect) -> Written`
//! - `pub fn refusal(text: &str, from: impl Into<Lang>, to: Dialect) -> Vec<Code>`
//! - `pub fn notes(w: &Written) -> Vec<Code>`
//! - `pub fn run(on: Dialect, text: &str) -> Vec<RawData>`
//! - `pub fn value(plots: &[RawData], node: &str, at: At) -> f64`
//! - `pub fn op(on: Dialect, text: &str, node: &str) -> f64`
//! - `pub fn pinned(on: Dialect, text: &str, node: &str, at: At) -> f64`
//! - `pub fn recorded(on: Dialect, name: &str, key: &str) -> f64`
//! - `pub fn measure(on: Dialect, text: &str, node: &str, at: At) -> f64`
//! - `pub fn sim_pair(from: Dialect, deck: &str, to: Dialect, node: &str, at: At, tol: Tol) -> (f64, f64)`
//! - `pub fn closed_form(from: Dialect, deck: &str, on: &[Dialect], node: &str, at: At, want: f64, tol: Tol)`
//! - `pub fn golden(name: &str, provenance: &str, got: &str)`
//! - `pub fn spec_example(cite: &str, text: &str, from: impl Into<Lang>, to: Dialect, want: &[&str]) -> String`
//! - `pub fn spec_refusal(cite: &str, text: &str, from: Dialect, to: Dialect, want: &[Code])`

#![allow(dead_code)]

#[path = "../test_cross_sim/sim.rs"]
mod sim;
#[path = "../test_cross_sim/vacask.rs"]
mod vacask;

use std::path::Path;

use spicerack::result::RawData;
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::{Written, render_text};
use spicerack::syntax::{Dialect, Lang};

/// A tolerance: `|got - want| <= rel * max(|want|, floor)`. The test names
/// where `rel` comes from (the simulator's RELTOL, the closed form's own
/// error, the two simulators' constants); `floor` is the `want` below
/// which the bound stops shrinking (0: a zero `want` needs an exact zero).
#[derive(Clone, Copy, Debug)]
pub struct Tol {
    pub rel: f64,
    pub floor: f64,
}

/// Where in a plot [`value`] reads a vector.
#[derive(Clone, Copy, Debug)]
pub enum At {
    /// The first point: an operating point, a sweep's start.
    First,
    /// The last point: a transient's stop time, a sweep's end.
    Last,
    /// Time `t` of a transient, linear between its two neighbouring points.
    Time(f64),
}

/// CPU seconds a test deck may run: these decks are a few elements.
const SECS: u64 = 20;

impl Tol {
    /// Whether `got` meets `want` to this tolerance; false on a NaN.
    pub fn within(self, got: f64, want: f64) -> bool {
        assert!(
            self.rel >= 0.0 && self.floor >= 0.0,
            "a tolerance: {self:?}"
        );
        (got - want).abs() <= self.rel * want.abs().max(self.floor)
    }
}

/// `text`, read as `from` (a dialect, or a lexicon such as
/// `Lang::NGSPICE_GIVEN`: a deck the Python API built), written for `to`.
///
/// # Panics
/// On a refusal, with its messages.
pub fn write(text: &str, from: impl Into<Lang>, to: Dialect) -> String {
    written(text, from, to).text
}

/// [`write`] with the notes (`Written::notes`) the writer left.
///
/// # Panics
/// On a refusal, with its messages.
pub fn written(text: &str, from: impl Into<Lang>, to: Dialect) -> Written {
    render_text(text, from, "t", to).unwrap_or_else(|e| panic!("{to:?}: {}\n{text}", e.messages))
}

/// Every diagnostic code of the refusal of `text` (read as `from`) for
/// `to`, in the refusal's order (source position).
///
/// # Panics
/// When `to` writes the deck instead.
pub fn refusal(text: &str, from: impl Into<Lang>, to: Dialect) -> Vec<Code> {
    match render_text(text, from, "t", to) {
        Ok(w) => panic!("written, not refused:\n{}", w.text),
        Err(e) => e.diags.iter().map(|d| d.code).collect(),
    }
}

/// The codes of the notes `w` carries, in order.
pub fn notes(w: &Written) -> Vec<Code> {
    w.notes.diags.iter().map(|d| d.code).collect()
}

/// Every plot of `text` run on `on`'s simulator, the `constants` plot left
/// out. HSPICE text runs on ESPice `--tokenizer=hspice`.
///
/// # Panics
/// On a failed or timed-out run, and for Spectre, CDL and Verilog (no
/// local simulator: pin Spectre and check it with [`golden`]).
pub fn run(on: Dialect, text: &str) -> Vec<RawData> {
    let sim = match on {
        Dialect::Ngspice => sim::Sim::Ngspice,
        Dialect::Xyce => sim::Sim::Xyce,
        Dialect::Vacask => sim::Sim::Vacask,
        Dialect::Espice => sim::Sim::Espice,
        Dialect::Hspice => sim::Sim::EspiceHspice,
        Dialect::Ltspice => return ltspice(text),
        Dialect::Spectre | Dialect::Cdl | Dialect::Verilog => {
            panic!("no local {on:?} simulator: a Spectre number is `pinned` (P1-043)")
        }
    };
    harness(sim, text)
}

/// The vector `v(node)` (or `node`, case-folded) at `at`, from the first
/// plot of `plots` that holds it.
///
/// # Panics
/// When no plot holds it, when it is complex (an `.ac`: read it through a
/// `.meas` instead), or when `At::Time` is outside the plot's time axis.
pub fn value(plots: &[RawData], node: &str, at: At) -> f64 {
    let want = format!("v({node})");
    for p in plots {
        let Some(i) = p
            .variables
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(&want) || v.name.eq_ignore_ascii_case(node))
        else {
            continue;
        };
        assert!(!p.is_complex, "{want} is complex in '{}'", p.plot_name);
        let col = &p.real_data[i];
        assert!(!col.is_empty(), "{want}: no points");
        return match at {
            At::First => col[0],
            At::Last => col[col.len() - 1],
            At::Time(t) => interpolate(p, col, t),
        };
    }
    let names: Vec<_> = plots
        .iter()
        .flat_map(|p| &p.variables)
        .map(|v| &v.name)
        .collect();
    panic!("no {want} in {names:?}")
}

/// `v(node)` of `text`'s first plot run on `on`: its operating point.
pub fn op(on: Dialect, text: &str, node: &str) -> f64 {
    value(&run(on, text), node, At::First)
}

/// `v(node)` at `at` from the pin of the deck `text` on real `on` (Spectre
/// or HSPICE on iccad4, recorded before 2026-11-08; ledger P1-043). The pin
/// is `tests/<spectre|hspice>/pins/<key>.pin`, `<key>` the deck's FNV-1a 64
/// hash in 16 hex digits, so a writer change that moves one byte loses the
/// pin: no run can replace it (ZERO D43). Its first line is `provenance:
/// <tool version, host, date>`; each other line `<vector>
/// <first|last|t=<time>> <value>`.
///
/// # Panics
/// For a dialect with a local simulator, on a missing pin ("no recorded
/// pin": take the manual page or another simulator as the oracle, or list
/// the line under `UNRUN_SPECTRE_HSPICE`), or when the pin lacks the value.
pub fn pinned(on: Dialect, text: &str, node: &str, at: At) -> f64 {
    let key = format!("{:016x}", fnv1a(text.as_bytes()));
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(pin_dir(on))
        .join("pins")
        .join(format!("{key}.pin"));
    let pin = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "no recorded pin {}: use the manual-page or other-simulator oracle (D43), or list the line in UNRUN_SPECTRE_HSPICE\n{text}",
            path.display()
        )
    });
    let mut lines = pin.lines();
    let provenance = lines.next().and_then(|l| l.strip_prefix("provenance: "));
    assert!(
        provenance.is_some_and(|p| !p.trim().is_empty()),
        "{}: line 1 is `provenance: …`",
        path.display()
    );
    let want = format!("v({node})");
    for l in lines {
        let f: Vec<&str> = l.split_whitespace().collect();
        assert!(
            f.len() == 3,
            "{}: `{l}` is not `<vector> <at> <value>`",
            path.display()
        );
        let name = f[0].eq_ignore_ascii_case(node) || f[0].eq_ignore_ascii_case(&want);
        let here = match (at, f[1]) {
            (At::First, "first") | (At::Last, "last") => true,
            (At::Time(t), w) => w.strip_prefix("t=").and_then(|x| x.parse().ok()) == Some(t),
            _ => false,
        };
        if name && here {
            return f[2]
                .parse()
                .unwrap_or_else(|e| panic!("{}: `{l}`: {e}", path.display()));
        }
    }
    panic!("{}: no {want} at {at:?}", path.display())
}

/// The number `key` of the run named `name` that real `on` made on iccad4
/// before 2026-11-08 (Spectre or HSPICE; ZERO D43, tier a) of a deck whose
/// text was not kept, so no FNV names it: the pin
/// `tests/<spectre|hspice>/recorded/<name>.pin`. Its first line is
/// `provenance: <tool version, host, date, where the record is>`, its
/// second `deck: <what ran, as the record describes it>`; each other line
/// is `<key> <value>` or a `#` comment.
///
/// ```ignore
/// // Ledger P6-002: HSPICE's `.DISTO` HD2 of the recorded diode stage.
/// let hd2 = recorded(Dialect::Hspice, "disto", "hd2");
/// ```
///
/// # Panics
/// For a dialect with a local simulator, on a missing pin, or when the
/// pin lacks its provenance, its deck or the key.
pub fn recorded(on: Dialect, name: &str, key: &str) -> f64 {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(pin_dir(on))
        .join("recorded")
        .join(format!("{name}.pin"));
    let pin = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "no recorded run {}: use the manual-page or other-simulator oracle (D43), or list the line in UNRUN_SPECTRE_HSPICE",
            path.display()
        )
    });
    let mut lines = pin.lines();
    for head in ["provenance: ", "deck: "] {
        let said = lines.next().and_then(|l| l.strip_prefix(head));
        assert!(
            said.is_some_and(|s| !s.trim().is_empty()),
            "{}: a line `{head}…` is missing",
            path.display()
        );
    }
    for l in lines.filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = l.split_whitespace().collect();
        assert!(
            f.len() == 2,
            "{}: `{l}` is not `<key> <value>`",
            path.display()
        );
        if f[0] == key {
            return f[1]
                .parse()
                .unwrap_or_else(|e| panic!("{}: `{l}`: {e}", path.display()));
        }
    }
    panic!("{}: no {key}", path.display())
}

/// `v(node)` at `at` of the deck `text` on `on`: a run where one is local,
/// the iccad4 pin for Spectre ([`pinned`]).
pub fn measure(on: Dialect, text: &str, node: &str, at: At) -> f64 {
    match on {
        Dialect::Spectre => pinned(on, text, node, at),
        _ => value(&run(on, text), node, at),
    }
}

/// Template: a real-simulator pair. `deck` (read as `from`) on `from`'s
/// simulator, and written for `to` on `to`'s, give the same `v(node)` at
/// `at`; returns `(source, target)`. The oracle is the source simulator,
/// so `tol` is what the two simulators agree to on a correct deck (name
/// its cause: RELTOL, their constants, the integrator).
///
/// ```ignore
/// // Ledger P3-030: Xyce ignores M on an independent source, ngspice multiplies.
/// #[test]
/// fn a_xyce_source_m_is_not_a_multiplier_on_ngspice() {
///     let deck = "t\nI1 0 a 1m M=2\nR1 a 0 1k\n.op\n.end\n";
///     sim_pair(Dialect::Xyce, deck, Dialect::Ngspice, "a", At::First, Tol { rel: 1e-6, floor: 0.0 });
/// }
/// ```
///
/// # Panics
/// When they differ, with both values and the written deck.
pub fn sim_pair(
    from: Dialect,
    deck: &str,
    to: Dialect,
    node: &str,
    at: At,
    tol: Tol,
) -> (f64, f64) {
    let target = write(deck, from, to);
    let want = measure(from, deck, node, at);
    let got = measure(to, &target, node, at);
    assert!(
        tol.within(got, want),
        "{to:?} v({node}) = {got}, {from:?} {want} ({tol:?})\n{target}"
    );
    (want, got)
}

/// Template: a closed form. `deck` (read as `from`) written for each of
/// `on` and run on its simulator gives `v(node) = want` at `at`. Put the
/// derivation in a comment above `want`; include `from` in `on` so the
/// source simulator vouches for the form too.
///
/// ```ignore
/// // 2 V over 1k/1k: out = 1 V.
/// closed_form(Dialect::Ngspice, DIVIDER, &[Dialect::Ngspice, Dialect::Xyce],
///             "out", At::First, 1.0, Tol { rel: 1e-9, floor: 1e-12 });
/// ```
///
/// # Panics
/// On the first simulator that misses `want`.
pub fn closed_form(
    from: Dialect,
    deck: &str,
    on: &[Dialect],
    node: &str,
    at: At,
    want: f64,
    tol: Tol,
) {
    assert!(!on.is_empty(), "a simulator to run");
    for &d in on {
        let text = if d == from {
            deck.to_string()
        } else {
            write(deck, from, d)
        };
        let got = measure(d, &text, node, at);
        assert!(
            tol.within(got, want),
            "{d:?}: v({node}) = {got}, want {want} ({tol:?})\n{text}"
        );
    }
}

/// Template: a golden with provenance. `got` equals the fixture
/// `tests/fixtures/golden/<name>`, whose first line is `provenance:
/// <provenance>` (who checked the text, against which manual page or pinned
/// run), then a blank line, then the text. `SPICERACK_BLESS=1` writes the
/// fixture; review its diff before committing. Name it `.txt`: a deck
/// extension (`.sp`, `.scs`, …) under `tests/` makes it a golden-gate input
/// (`src/bin/golden/inputs.rs`).
///
/// ```ignore
/// // Ledger P3-075: Spectre takes the portable method=gear.
/// golden("spectre/tran_method_gear.scs",
///        "Spectre Reference 19.1 p.841 #15 (method); no Spectre run",
///        &write(deck, Dialect::Ngspice, Dialect::Spectre));
/// ```
///
/// # Panics
/// On a difference, on a missing fixture (bless it), on an empty
/// provenance.
pub fn golden(name: &str, provenance: &str, got: &str) {
    assert!(
        !provenance.trim().is_empty(),
        "{name}: a golden names its provenance"
    );
    assert!(!provenance.contains('\n'), "{name}: provenance is one line");
    assert!(
        name.ends_with(".txt"),
        "{name}: a golden is .txt, never a deck extension"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden")
        .join(name);
    let body = format!("provenance: {provenance}\n\n{got}");
    if std::env::var_os("SPICERACK_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().expect("a directory")).expect("the golden directory");
        std::fs::write(&path, &body).expect("the golden written");
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}: bless it (SPICERACK_BLESS=1) and review",
            path.display()
        )
    });
    assert_eq!(
        body,
        want,
        "{} differs (SPICERACK_BLESS=1 rewrites it)",
        path.display()
    );
}

/// Template: a spec example that is written. The example of manual clause
/// `cite`, read as `from` and written for `to`, holds each of `want` as a
/// whole line; returns the text for further checks.
///
/// ```ignore
/// spec_example("LTspice help, .TRAN", "t\nR1 a 0 1k\nV1 a 0 1\n.tran 750u startup\n.end\n",
///              Dialect::Ltspice, Dialect::Ltspice, &[".tran 0 750u startup"]);
/// ```
///
/// # Panics
/// On a refusal, or on a line of `want` the text lacks.
pub fn spec_example(
    cite: &str,
    text: &str,
    from: impl Into<Lang>,
    to: Dialect,
    want: &[&str],
) -> String {
    assert!(
        !cite.trim().is_empty() && !want.is_empty(),
        "a clause and its lines"
    );
    let w = write(text, from, to);
    let missing: Vec<&&str> = want
        .iter()
        .filter(|l| !w.lines().any(|x| x == **l))
        .collect();
    assert!(missing.is_empty(), "{cite}: {to:?} lacks {missing:?}\n{w}");
    w
}

/// Template: a spec example that is refused. The example of clause `cite`
/// (the target's manual saying it lacks the construct), read as `from`, is
/// refused by `to` with exactly the codes `want`, in order.
///
/// ```ignore
/// spec_refusal("Xyce RG 7.10 §2.1.20: no .disto", DISTO_DECK,
///              Dialect::Ngspice, Dialect::Xyce, &[Code::W0015]);
/// ```
///
/// # Panics
/// When `to` writes it, or the codes differ.
pub fn spec_refusal(cite: &str, text: &str, from: Dialect, to: Dialect, want: &[Code]) {
    assert!(
        !cite.trim().is_empty(),
        "a refusal cites the target's clause"
    );
    assert_eq!(refusal(text, from, to), want, "{cite}: {to:?}\n{text}");
}

// ---- private ----

/// `text` on the cross-sim harness's `sim`: every plot. The one call into
/// `tests/test_cross_sim/sim.rs` (the implementer's file): a change to
/// `sim::run` breaks here only.
fn harness(sim: sim::Sim, text: &str) -> Vec<RawData> {
    match sim::run(sim, Path::new("/nonexistent/t.cir"), text, &[], SECS) {
        sim::Ran::Plots(p) => p,
        sim::Ran::Timeout => panic!("{sim:?}: past {SECS} s\n{text}"),
        sim::Ran::Failed(e) => panic!("{sim:?}: {e}\n{text}"),
    }
}

/// FNV-1a, 64 bit: a pin's key, stable across Rust releases (unlike
/// `DefaultHasher`).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The directory under `tests/` of `on`'s pins: Spectre's or HSPICE's, the
/// two simulators with no binary here.
fn pin_dir(on: Dialect) -> &'static str {
    match on {
        Dialect::Spectre => "spectre",
        Dialect::Hspice => "hspice",
        _ => panic!("{on:?} runs here: `run` it, a pin is for Spectre and HSPICE"),
    }
}

/// `text` on LTspice, through `spicerack::backend`, as a user's run goes
/// (the harness's own LTspice is the cross-sim cell's): every plot.
#[cfg(feature = "backend")]
fn ltspice(text: &str) -> Vec<RawData> {
    Dialect::Ltspice
        .run_all(text)
        .unwrap_or_else(|e| panic!("ltspice: {e}\n{text}"))
}

#[cfg(not(feature = "backend"))]
fn ltspice(_: &str) -> Vec<RawData> {
    panic!("LTspice runs through spicerack::backend: build with the `backend` feature")
}

/// `col` at time `t`, linear between the two points of the plot's time
/// axis (its variable 0) around `t`.
fn interpolate(p: &RawData, col: &[f64], t: f64) -> f64 {
    let axis = &p.real_data[0];
    assert!(
        p.variables[0].name.eq_ignore_ascii_case("time"),
        "At::Time on '{}', whose axis is {}",
        p.plot_name,
        p.variables[0].name
    );
    assert!(
        axis.len() == col.len() && axis.len() >= 2,
        "a time axis of two points"
    );
    let at = axis.partition_point(|&x| x < t);
    assert!(at > 0 || axis[0] == t, "t = {t} before {}", axis[0]);
    assert!(at < axis.len(), "t = {t} after {}", axis[axis.len() - 1]);
    if axis[at] == t {
        return col[at];
    }
    let (t0, t1) = (axis[at - 1], axis[at]);
    col[at - 1] + (col[at] - col[at - 1]) * (t - t0) / (t1 - t0)
}
