//! The dialect pipeline's scaling gate, in a release build: for every `BUILTIN` row with both
//! sides, a generated flat deck in that row's own text, then per stage (`read` by the row,
//! `legalize` to each other row, `write` by that row) the cost at N and at 4N elements. A linear
//! stage measures a ratio of about 4 to 5; a quadratic one about 16. Exits 1 when a ratio exceeds
//! `BOUND`, 2 when a row cannot carry the deck (unsupported fails, never skips).
//!
//! What it compares, said on its first line: user-space instructions retired, counted by
//! `perf stat` on a child process per stage and size (`… pipeline_scaling count <from> <to>
//! <stage> <n> <k>`, which runs the stages before `<stage>` in pipeline order, `source` first,
//! once and `<stage>` itself k times; a stage's count per run is the count at k = 1 + reps less
//! the count at k = 1, over reps, so the stages before it and their variation cancel), pinned to the performance cores on a
//! hybrid CPU (`/sys/devices/cpu_core/cpus`, event `cpu_core/instructions/u`; elsewhere
//! `instructions:u`). Counts repeat under load where times do not (the IR's gate read a linear
//! phase at 8.74 by time at load average 70). Instructions do not see a memory-bound
//! superlinearity (cache or TLB misses, page faults, the allocator) nor time in the kernel: the
//! time of each stage is printed beside them, report only (fastest of `TIME_RUNS`). Where the
//! counter cannot be read (no `perf`, a stricter `perf_event_paranoid`), the gate compares the
//! fastest of `RUNS` times instead, against the same bound, and says so.
//!
//! The deck: N resistors in a chain on N + 1 nodes, a tenth of them naming one shared model, one
//! subcircuit instantiated N / 10 times, one source, an operating point. So the readers' name and
//! model lookups, the passes' walks and the writers' `admits` all see N rows. It starts as
//! ngspice text read by the ngspice reader (through the builder), and each row's source text is
//! that design legalized for the row and written by the row's writer.
//!
//! A fast stage is not timed in the noise: each (cell, stage) repeats its stage `reps` times per
//! timing, `reps` doubled until the stage takes at least `BASELINE_S` at N (capped at
//! `REPS_MAX`), and the same `reps` at 4N.
//!
//! Run: `cargo run --release --example pipeline_scaling`. One line per (cell, stage):
//! `<from>-><to> <stage> n=<N> <count(N)> -> <count(4N)> instructions ratio <r>; time … (report
//! only)` and `ok` or `BREACH`.
//!
//! Cites: the IR crate's `examples/throughput.rs` (`scaling`, `Counter`, `run_up_to`), whose
//! method this copies; the census finding (ngspice → VACASK on a 246k-line powergrid deck:
//! VACASK's `admits` and `write` 4x per doubling).
//!
//! Contents: none (a measurement binary).

use std::collections::HashMap;
use std::time::Instant;

use spicerack::dialect::{self, BUILTIN, DialectRow, Policy, Sources};
use spicerack::ir;

/// Runs per size when the gate falls back to times; each stage keeps its fastest.
const RUNS: u32 = 5;
/// Runs per size of the report-only times beside the counts.
const TIME_RUNS: u32 = 2;
/// The largest ratio of the cost at 4N to the cost at N: linear is 4 (4 to 5 with cache
/// effects), quadratic 16.
const BOUND: f64 = 8.0;
/// The least time a stage takes at N before it is timed, seconds.
const BASELINE_S: f64 = 0.05;
/// The smaller deck's element count; the larger is four times it.
const N: usize = 4_000;
/// The most repetitions of a stage per timing.
const REPS_MAX: u32 = 4_096;
/// The stages in pipeline order, as `count` names them; `source` builds the row's text.
const STAGES: [&str; 4] = ["source", "read", "legalize", "write"];

/// A measured stage: an index into `STAGES`, never 0.
type Stage = usize;

/// The source texts built so far, by (row, N): each is built once (a writer that does not
/// scale makes them slow to build).
type Texts = HashMap<(&'static str, usize), String>;

/// How the gate counts a child's user-space instructions: the event, and the cores it pins to.
struct Counter {
    event: String,
    cores: Option<String>,
}

// ---- private ----

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "count") {
        let row = |k: usize| {
            args.get(k)
                .and_then(|a| dialect::by_name(a))
                .expect("count <from> <to> <stage> <n> <reps>")
        };
        let stage = STAGES
            .iter()
            .position(|s| args.get(4).is_some_and(|a| a == s))
            .expect("a stage of STAGES");
        let n: usize = args.get(5).and_then(|a| a.parse().ok()).expect("a size");
        let reps: u32 = args
            .get(6)
            .and_then(|a| a.parse().ok())
            .expect("a run count");
        run_up_to(row(2), row(3), stage, n, reps);
        return;
    }
    let rows: Vec<&DialectRow> = BUILTIN
        .iter()
        .filter(|r| r.read.is_some() && r.write.is_some())
        .collect();
    assert!(!rows.is_empty(), "a row with both sides");
    let counter = Counter::find();
    match &counter {
        Ok(c) => println!(
            "measure: instructions ({}), time beside it is report only",
            c.describe()
        ),
        Err(why) => {
            println!("measure: time, fastest of {RUNS} runs (no instruction counter: {why})")
        }
    }
    let counter = counter.ok();
    let mut breach = false;
    let mut texts = HashMap::new();
    for &from in &rows {
        breach |= report(&counter, &mut texts, from, from, 1);
        for &to in rows.iter().filter(|t| t.name != from.name) {
            breach |= report(&counter, &mut texts, from, to, 2);
            breach |= report(&counter, &mut texts, from, to, 3);
        }
    }
    std::process::exit(i32::from(breach));
}

impl Counter {
    /// The counter this machine can read, proved by counting `true`; why not otherwise.
    fn find() -> Result<Counter, String> {
        let cores = std::fs::read_to_string("/sys/devices/cpu_core/cpus")
            .ok()
            .map(|s| s.trim().to_owned());
        let event = if cores.is_some() {
            "cpu_core/instructions/u"
        } else {
            "instructions:u"
        };
        let c = Counter {
            event: event.to_owned(),
            cores,
        };
        match c.count(&["true"]) {
            Ok(n) if n > 0 => Ok(c),
            Ok(_) => Err(format!("{event} counted nothing")),
            Err(e) => Err(e),
        }
    }

    fn describe(&self) -> String {
        match &self.cores {
            Some(c) => format!("{} on cpus {c}", self.event),
            None => self.event.clone(),
        }
    }

    /// `stage`'s own instructions on the cell at `n`, per run: the count with `reps` more runs
    /// of it than a count with one, divided by `reps` (the stages before it, run once in both,
    /// cancel, and so does their variation).
    fn stage(&self, from: &DialectRow, to: &DialectRow, stage: Stage, n: usize, reps: u32) -> f64 {
        assert!(stage > 0, "source is setup, never measured");
        assert!(reps > 0, "at least one extra run");
        let me = std::env::current_exe().expect("this binary");
        let me = me.to_str().expect("a UTF-8 path");
        let n = n.to_string();
        let upto = |k: u32| {
            self.count(&[
                me,
                "count",
                from.name,
                to.name,
                STAGES[stage],
                &n,
                &k.to_string(),
            ])
            .unwrap_or_else(|e| panic!("{}: {e}", STAGES[stage]))
        };
        let (once, more) = (upto(1), upto(1 + reps));
        assert!(more > once, "a stage retires instructions");
        (more - once) as f64 / f64::from(reps)
    }

    /// The instructions `cmd` retires in user space, read from `perf stat -x,`'s stderr.
    fn count(&self, cmd: &[&str]) -> Result<u64, String> {
        let mut c = match &self.cores {
            Some(cores) => {
                let mut c = std::process::Command::new("taskset");
                c.args(["-c", cores, "perf"]);
                c
            }
            None => std::process::Command::new("perf"),
        };
        c.args(["stat", "-x,", "-e", &self.event]).args(cmd);
        let out = c.output().map_err(|e| format!("perf: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "exit {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().find(|l| l.contains(self.event.as_str()));
        let field = line.and_then(|l| l.split(',').next()).unwrap_or("");
        field
            .parse()
            .map_err(|_| format!("perf printed no count: {}", err.trim()))
    }
}

/// Runs the stages before `stage` (an index into `STAGES`) once and `stage` itself `reps` times
/// on the cell at `n`, and nothing else: the unit `Counter::stage` subtracts. Exits 2 as the
/// gate does on a refusal.
fn run_up_to(from: &DialectRow, to: &DialectRow, stage: Stage, n: usize, reps: u32) {
    let times = |s: Stage| if s == stage { reps } else { 1 };
    let text = source(from, n);
    if stage == 0 {
        return;
    }
    let mut design = ir::Design::new();
    for _ in 0..times(1) {
        design = ir::Design::new();
        let mut diags = ir::Diags::new();
        dialect::read(
            &Sources::one("deck", &text),
            from,
            Policy::Lenient,
            &mut design,
            &mut diags,
        );
        refused(&diags, from, "read");
    }
    if stage == 1 {
        return;
    }
    let read = design;
    let mut design = read.clone();
    for _ in 0..times(2) {
        // a fresh copy per repetition: legalize works in place
        design = read.clone();
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, to, &mut diags);
        refused(&diags, to, "legalize");
    }
    if stage == 2 {
        return;
    }
    for _ in 0..times(3) {
        let out = dialect::write(&design, design.entry(), to, &mut ir::Diags::new());
        assert!(!out.is_empty(), "a written deck");
    }
}

/// Measures one (cell, stage) and prints its line; whether it breaches.
fn report(
    counter: &Option<Counter>,
    texts: &mut Texts,
    from: &DialectRow,
    to: &DialectRow,
    stage: Stage,
) -> bool {
    let runs = if counter.is_some() { TIME_RUNS } else { RUNS };
    let mut reps = 1;
    // bounded: reps doubles up to REPS_MAX
    let base = loop {
        let t = fastest(texts, from, to, stage, N, reps, runs);
        if t >= BASELINE_S || reps >= REPS_MAX {
            break t;
        }
        reps *= 2;
    };
    let big = fastest(texts, from, to, stage, 4 * N, reps, runs);
    let time = format!("{base:.4} s -> {big:.4} s (reps={reps})");
    let (ratio, line) = match counter {
        Some(c) => {
            let small = c.stage(from, to, stage, N, reps);
            let large = c.stage(from, to, stage, 4 * N, reps);
            let ratio = large / small;
            let line = format!(
                "{small:.3e} -> {large:.3e} instructions ratio {ratio:.2}; time {time} ratio {:.2} (report only)",
                big / base
            );
            (ratio, line)
        }
        None => (big / base, format!("{time} ratio {:.2}", big / base)),
    };
    let verdict = if ratio > BOUND { "BREACH" } else { "ok" };
    println!(
        "{}->{} {} n={N} {line} {verdict}",
        from.name, to.name, STAGES[stage]
    );
    ratio > BOUND
}

/// The fastest of `runs` timings of `reps` runs of `stage` on the deck of `n` elements in
/// `from`'s text.
fn fastest(
    texts: &mut Texts,
    from: &DialectRow,
    to: &DialectRow,
    stage: Stage,
    n: usize,
    reps: u32,
    runs: u32,
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
    for _ in 0..runs {
        let mut spent = 0.0;
        for _ in 0..reps {
            // legalize works in place: its copy is made before the clock starts
            let mut copy = match stage {
                2 => read_design.clone(),
                _ => ir::Design::new(),
            };
            let at = Instant::now();
            match stage {
                1 => drop(read()),
                2 => dialect::legalize(&mut copy, to, &mut ir::Diags::new()),
                3 => {
                    let out = dialect::write(&legal, legal.entry(), to, &mut ir::Diags::new());
                    assert!(!out.is_empty(), "a written deck");
                }
                _ => unreachable!("stage {stage} is not measured"),
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
