//! Throughput of the IR's bulk paths in a release build: building a generated design of 10⁶
//! items, dumping it to IR text, parsing that text back and checking it (`rewrite/IR-IMPL.md`
//! §3, the numbers written into `rewrite/IR-PERF.md`), and the scaling gate that keeps them
//! linear. Run:
//! `cargo run --release --features gen --example throughput [items] [feature-set]`, where the
//! set is `kernel` (default) or `all`; or `… --example throughput scaling`, the merge-tier gate
//! that keeps every phase (build, dump, parse, check) linear over the full set: it compares
//! `SCALE_N` with `4 × SCALE_N` items and exits 1 when a phase's ratio exceeds `SCALE_BOUND`
//! (linear is 4, quadratic 16).
//!
//! What it compares, said on its first line: user-space instructions retired, counted by
//! `perf stat` on a child process per phase and size (`… throughput count <phase> <items>`, which
//! runs the phases up to `<phase>`; a phase's count is the difference from the phase before),
//! pinned to the performance cores on a hybrid CPU (`/sys/devices/cpu_core/cpus`, event
//! `cpu_core/instructions/u`; elsewhere `instructions:u`). Counts repeat to 0.02 % under any load
//! (measured at load average 46), where times did not (a linear phase read 8.74). Instructions
//! do not see a memory-bound superlinearity (cache or TLB misses, page faults, allocator
//! behaviour) nor time in the kernel: the time of each phase (fastest of `TIME_RUNS`) is printed
//! beside them, report only. Where the counter cannot be read (no `perf`, a stricter
//! `perf_event_paranoid`), the gate compares the fastest of `SCALE_RUNS` times instead, against
//! the same bound, and says so.
//!
//! Another crate's scaling example is counted the same way when it has the same two modes: a
//! `count <phase> <items>` mode that runs exactly the phases up to one and exits, and a driver
//! that runs it under `perf stat -x, -e <event>` (`taskset -c <cores>` first on a hybrid CPU) and
//! reads the count from the first CSV field of the stderr line naming the event.
//!
//! A phase's count here is a plain difference from the phase before; the earlier phases' run-to-run
//! variation (HashMap seeds) moves it about 1 % for dump, the smallest (3.7e8 under a 4.6e9 build).
//! When a phase is small enough for that to move its ratio (a stage of 1e7 under 1e10 did, in the
//! pipeline's example), run the phase k times in one child and take (count at 1 + k − count at 1) / k:
//! the reference form is `examples/pipeline_scaling.rs`' `count <from> <to> <stage> <n> <k>`
//! (`ir-next` 88bb3b06).
//!
//! Contents: none (a measurement binary).

use std::time::Instant;

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{Design, Diags, Feature, FeatureSet, dump, parse};

/// The scaling gate's smaller size; the larger is four times it.
const SCALE_N: u32 = 100_000;
/// Runs per size when the gate falls back to times; each phase keeps its fastest.
const SCALE_RUNS: u32 = 5;
/// Runs per size of the report-only times beside the counts.
const TIME_RUNS: u32 = 2;
/// The phases, in run order.
const PHASES: [&str; 4] = ["build", "dump", "parse", "check"];
/// The largest time ratio a phase may show between `4 × SCALE_N` and `SCALE_N` items. Linear is
/// 4 (about 4 to 5 with cache effects); quadratic is 16. Generous so as not to flake on a
/// loaded machine, tight enough that a quadratic step fails.
const SCALE_BOUND: f64 = 8.0;

/// One run's times (seconds) and sizes.
struct Phases {
    build: f64,
    dump: f64,
    parse: f64,
    relink: f64,
    check: f64,
    mb: f64,
    lines: usize,
    exprs: usize,
}

/// How the gate counts a child's user-space instructions: the event, and the cores it pins to.
struct Counter {
    event: String,
    cores: Option<String>,
}

// ---- private ----

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "scaling") {
        std::process::exit(scaling());
    }
    if args.get(1).is_some_and(|a| a == "count") {
        let phase = PHASES
            .iter()
            .position(|p| args.get(2).is_some_and(|a| a == p))
            .expect("count <build|dump|parse|check> <items>");
        let items: u32 = args
            .get(3)
            .and_then(|a| a.parse().ok())
            .expect("an item count");
        run_up_to(phase, items, &every_feature());
        return;
    }
    let items: u32 = args
        .get(1)
        .map_or(1_000_000, |a| a.parse().expect("an item count"));
    let all = args.get(2).is_some_and(|a| a == "all");
    let set = if all { every_feature() } else { kernel_set() };
    let p = measure(items, &set);
    println!(
        "items {items}, set {}, text {:.1} MB, {} lines, {} expression rows",
        if all { "all" } else { "kernel" },
        p.mb,
        p.lines,
        p.exprs
    );
    println!(
        "build {:.3} s ({:.0} items/s)",
        p.build,
        items as f64 / p.build
    );
    println!("dump  {:.3} s ({:.1} MB/s)", p.dump, p.mb / p.dump);
    println!(
        "parse {:.3} s ({:.1} MB/s), of which a link is about {:.3} s",
        p.parse,
        p.mb / p.parse,
        p.relink
    );
    println!(
        "check {:.3} s ({:.0} items/s)",
        p.check,
        items as f64 / p.check
    );
}

/// The scaling gate (module header): instruction counts when `perf` can read them, else times;
/// prints which on its first line, then each phase's ratio; returns 1 when one exceeds
/// `SCALE_BOUND`.
fn scaling() -> i32 {
    let set = every_feature();
    let best = |n: u32, runs: u32| {
        let mut t = [f64::INFINITY; 4];
        for _ in 0..runs {
            let p = measure(n, &set);
            for (k, s) in [p.build, p.dump, p.parse, p.check].into_iter().enumerate() {
                t[k] = t[k].min(s);
            }
        }
        t
    };
    let counter = Counter::find();
    let ratios: [f64; 4] = match &counter {
        Ok(c) => {
            println!(
                "measure: instructions ({}), time beside it is report only",
                c.describe()
            );
            let (small, large) = (c.phases(SCALE_N), c.phases(4 * SCALE_N));
            let (ts, tl) = (best(SCALE_N, TIME_RUNS), best(4 * SCALE_N, TIME_RUNS));
            std::array::from_fn(|k| {
                let ratio = large[k] as f64 / small[k] as f64;
                println!(
                    "{:5} {:.3e} -> {:.3e} instructions at 4x items: ratio {ratio:.2} (bound {SCALE_BOUND}); time {:.3} s -> {:.3} s, ratio {:.2} (report only)",
                    PHASES[k],
                    small[k] as f64,
                    large[k] as f64,
                    ts[k],
                    tl[k],
                    tl[k] / ts[k]
                );
                ratio
            })
        }
        Err(why) => {
            println!("measure: time, fastest of {SCALE_RUNS} runs (no instruction counter: {why})");
            let (small, large) = (best(SCALE_N, SCALE_RUNS), best(4 * SCALE_N, SCALE_RUNS));
            std::array::from_fn(|k| {
                let ratio = large[k] / small[k];
                println!(
                    "{:5} {:.3} s -> {:.3} s at 4x items: ratio {ratio:.2} (bound {SCALE_BOUND})",
                    PHASES[k], small[k], large[k]
                );
                ratio
            })
        }
    };
    let failed: Vec<&str> = (0..4)
        .filter(|&k| ratios[k] > SCALE_BOUND)
        .map(|k| PHASES[k])
        .collect();
    println!(
        "scaling {}",
        if failed.is_empty() {
            "ok".to_owned()
        } else {
            format!("FAIL: {}", failed.join(" "))
        }
    );
    i32::from(!failed.is_empty())
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

    /// Each phase's own instructions at `items`: the count up to it minus the count up to the
    /// phase before.
    fn phases(&self, items: u32) -> [u64; 4] {
        let me = std::env::current_exe().expect("this binary");
        let me = me.to_str().expect("a UTF-8 path");
        let n = items.to_string();
        let upto: Vec<u64> = PHASES
            .iter()
            .map(|p| {
                self.count(&[me, "count", p, &n])
                    .unwrap_or_else(|e| panic!("{p}: {e}"))
            })
            .collect();
        std::array::from_fn(|k| upto[k] - if k == 0 { 0 } else { upto[k - 1] })
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
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().find(|l| l.contains(self.event.as_str()));
        let field = line.and_then(|l| l.split(',').next()).unwrap_or("");
        field
            .parse()
            .map_err(|_| format!("perf printed no count: {}", err.trim()))
    }
}

/// Runs the phases up to `phase` (an index into `PHASES`) at `items` and nothing else: the unit
/// `Counter::phases` subtracts.
fn run_up_to(phase: usize, items: u32, set: &FeatureSet) {
    let size = Size {
        items,
        scope_depth: 3,
        expr_depth: 4,
        files: 1,
    };
    let d = generate(1, set, size);
    if phase == 0 {
        return;
    }
    let mut text = String::new();
    for f in d.files() {
        dump(&d, f, &mut text);
    }
    if phase == 1 {
        return;
    }
    let mut back = Design::new();
    let mut diags = Diags::new();
    parse(&text, &mut back.builder(), &mut diags);
    if phase == 2 {
        return;
    }
    let mut checked = Diags::new();
    spicerack_ir::check(&back, &mut checked);
    assert!(checked.is_empty(), "the generated design is valid");
}

/// Builds, dumps, parses, links and checks one generated design of `items` items, asserting
/// it is valid and reads back equal.
fn measure(items: u32, set: &FeatureSet) -> Phases {
    let size = Size {
        items,
        scope_depth: 3,
        expr_depth: 4,
        files: 1,
    };
    let t = Instant::now();
    let d = generate(1, set, size);
    let build = t.elapsed().as_secs_f64();

    let t = Instant::now();
    let mut text = String::new();
    for f in d.files() {
        dump(&d, f, &mut text);
    }
    let dumped = t.elapsed().as_secs_f64();

    let t = Instant::now();
    let mut back = Design::new();
    let mut diags = Diags::new();
    parse(&text, &mut back.builder(), &mut diags);
    let parsed = t.elapsed().as_secs_f64();

    let t = Instant::now();
    let mut linked = back.clone();
    spicerack_ir::link(&mut linked, &mut Diags::new());
    let relink = t.elapsed().as_secs_f64();

    let t = Instant::now();
    let mut checked = Diags::new();
    spicerack_ir::check(&back, &mut checked);
    let check = t.elapsed().as_secs_f64();
    assert!(checked.is_empty(), "the generated design is valid");
    assert!(diags.is_empty(), "the dump parses");
    assert!(back.eq_ignoring_spans(&d), "and reads back equal");
    Phases {
        build,
        dump: dumped,
        parse: parsed,
        relink,
        check,
        mb: text.len() as f64 / 1e6,
        lines: text.lines().count(),
        exprs: d.expr_count_all(),
    }
}

fn kernel_set() -> FeatureSet {
    use spicerack_ir::ItemKind;
    every_feature_where(|f| match f {
        Feature::Item(k) => (k as u8) <= ItemKind::Func as u8,
        Feature::Elem(_)
        | Feature::Wave(_)
        | Feature::Analysis(_)
        | Feature::Circuit(_)
        | Feature::Run(_)
        | Feature::Option(_) => false,
        _ => true,
    })
}

fn every_feature() -> FeatureSet {
    every_feature_where(|_| true)
}

fn every_feature_where(keep: impl Fn(Feature) -> bool) -> FeatureSet {
    (0..Feature::COUNT)
        .map(Feature::from_index)
        .filter(|f| keep(*f))
        .fold(FeatureSet::EMPTY, |s, f| s.with(f))
}
