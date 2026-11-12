//! The dialect pipeline's scaling gate, in a release build: for every `BUILTIN` row with both
//! sides, a generated flat deck in that row's own text, then per stage (`read` by the row,
//! `legalize` to each other row, `write` by that row) the time at N and at 4N elements, each the
//! fastest of `RUNS` runs (load can only inflate a time, so the minimum does not flake). A linear
//! stage measures a ratio of about 4 to 5; a quadratic one about 16. Exits 1 when a ratio exceeds
//! `BOUND`, 2 when a row cannot carry the deck (unsupported fails, never skips).
//!
//! The deck: N resistors in a chain on N + 1 nodes, a tenth of them naming one shared model, one
//! subcircuit instantiated N / 10 times, one source, an operating point. So the readers' name and
//! model lookups, the passes' walks and the writers' `admits` all see N rows. It starts as
//! ngspice text read by the ngspice reader (through the builder), and each row's source text is
//! that design legalized for the row and written by the row's writer.
//!
//! N is the constant `N` for every cell, so no deck outgrows what a stage that does not scale
//! builds in seconds. A fast stage is not timed in the noise: each (cell, stage) repeats its
//! stage `reps` times per timing, `reps` doubled until the stage takes at least `BASELINE_S` at N
//! (capped at `REPS_MAX`), and the same `reps` at 4N.
//!
//! Run: `cargo run --release --example pipeline_scaling`. One line per (cell, stage):
//! `<from>-><to> <stage> n=<N> reps=<k> <t(N)> s <t(4N)> s ratio <r>` and `ok` or `BREACH`.
//!
//! Cites: the IR crate's `examples/throughput.rs` (`scaling`), whose method this copies; the
//! census finding (ngspice → VACASK on a 246k-line powergrid deck: VACASK's `admits` and `write`
//! 4x per doubling).
//!
//! Contents: none (a measurement binary).

use std::collections::HashMap;
use std::time::Instant;

use spicerack::dialect::{self, BUILTIN, DialectRow, Policy, Sources};
use spicerack::ir;

/// Runs per size; each stage keeps its fastest.
const RUNS: u32 = 5;
/// The largest ratio of the time at 4N to the time at N: linear is 4 (4 to 5 with cache
/// effects), quadratic 16.
const BOUND: f64 = 8.0;
/// The least time a stage takes at N before it is measured, seconds.
const BASELINE_S: f64 = 0.05;
/// The smaller deck's element count; the larger is four times it.
const N: usize = 4_000;
/// The most repetitions of a stage per timing.
const REPS_MAX: u32 = 4_096;

/// A stage of the pipeline.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Read,
    Legalize,
    Write,
}

/// The source texts built so far, by (row, N): each is built once (a writer that does not
/// scale makes them slow to build).
type Texts = HashMap<(&'static str, usize), String>;

// ---- private ----

fn main() {
    let rows: Vec<&DialectRow> = BUILTIN
        .iter()
        .filter(|r| r.read.is_some() && r.write.is_some())
        .collect();
    assert!(!rows.is_empty(), "a row with both sides");
    let mut breach = false;
    let mut texts = HashMap::new();
    for &from in &rows {
        breach |= report(&mut texts, from, from, Stage::Read);
        for &to in rows.iter().filter(|t| t.name != from.name) {
            breach |= report(&mut texts, from, to, Stage::Legalize);
            breach |= report(&mut texts, from, to, Stage::Write);
        }
    }
    std::process::exit(i32::from(breach));
}

/// Measures one (cell, stage) and prints its line; whether it breaches.
fn report(texts: &mut Texts, from: &DialectRow, to: &DialectRow, stage: Stage) -> bool {
    let mut reps = 1;
    // bounded: reps doubles up to REPS_MAX
    let base = loop {
        let t = fastest(texts, from, to, stage, N, reps);
        if t >= BASELINE_S || reps >= REPS_MAX {
            break t;
        }
        reps *= 2;
    };
    let big = fastest(texts, from, to, stage, 4 * N, reps);
    let ratio = big / base;
    let name = match stage {
        Stage::Read => "read",
        Stage::Legalize => "legalize",
        Stage::Write => "write",
    };
    let verdict = if ratio > BOUND { "BREACH" } else { "ok" };
    println!(
        "{}->{} {name} n={N} reps={reps} {base:.4} s {big:.4} s ratio {ratio:.2} {verdict}",
        from.name, to.name
    );
    ratio > BOUND
}

/// The fastest of `RUNS` timings of `reps` runs of `stage` on the deck of `n` elements in
/// `from`'s text.
fn fastest(
    texts: &mut Texts,
    from: &DialectRow,
    to: &DialectRow,
    stage: Stage,
    n: usize,
    reps: u32,
) -> f64 {
    let text = texts
        .entry((from.name, n))
        .or_insert_with(|| source(from, n))
        .clone();
    let src = Sources::one("deck", &text);
    let read = || {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        dialect::read(&src, from, Policy::Lenient, &mut design, &mut diags);
        refused(&diags, from, "read");
        design
    };
    let read_design = read();
    let mut legal = read_design.clone();
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut legal, to, &mut diags);
    refused(&diags, to, "legalize");
    let mut best = f64::INFINITY;
    for _ in 0..RUNS {
        let mut spent = 0.0;
        for _ in 0..reps {
            // legalize works in place: its copy is made before the clock starts
            let mut copy = match stage {
                Stage::Legalize => read_design.clone(),
                Stage::Read | Stage::Write => ir::Design::new(),
            };
            let at = Instant::now();
            match stage {
                Stage::Read => drop(read()),
                Stage::Legalize => dialect::legalize(&mut copy, to, &mut ir::Diags::new()),
                Stage::Write => {
                    let out = dialect::write(&legal, legal.entry(), to, &mut ir::Diags::new());
                    assert!(!out.is_empty(), "a written deck");
                }
            }
            spent += at.elapsed().as_secs_f64();
        }
        best = best.min(spent);
    }
    best
}

/// The deck of `n` elements in `row`'s text: the ngspice deck read, legalized for `row` and
/// written by it.
fn source(row: &DialectRow, n: usize) -> String {
    let ng = dialect::by_name("ngspice").expect("the ngspice row");
    let text = ngspice_deck(n);
    if row.name == ng.name {
        return text;
    }
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one("deck", &text),
        ng,
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    refused(&diags, ng, "read");
    dialect::legalize(&mut design, row, &mut diags);
    refused(&diags, row, "legalize");
    dialect::write(&design, design.entry(), row, &mut diags)
}

/// The generated deck in ngspice text (module header).
fn ngspice_deck(n: usize) -> String {
    let mut out = String::with_capacity(n * 24);
    out.push_str("scaling deck\n.model rm r tc1=0\n.subckt cell a b\nr1 a b 1k\n.ends\n");
    out.push_str("v1 n0 0 dc 1\n");
    for i in 0..n {
        match i % 10 {
            0 => out.push_str(&format!("r{i} n{i} n{} 1k rm\n", i + 1)),
            _ => out.push_str(&format!("r{i} n{i} n{} 1k\n", i + 1)),
        }
    }
    for j in 0..n / 10 {
        out.push_str(&format!("x{j} n{} 0 cell\n", j * 10));
    }
    out.push_str(".op\n.end\n");
    out
}

/// Exits 2 when `diags` refuses: the row cannot carry the deck, which is no measurement.
fn refused(diags: &ir::Diags, row: &DialectRow, step: &str) {
    let codes: Vec<String> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .map(|d| d.code.to_string())
        .collect();
    if !codes.is_empty() {
        eprintln!("{} {step} refuses the deck: {codes:?}", row.name);
        std::process::exit(2);
    }
}
