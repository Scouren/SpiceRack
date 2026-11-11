//! The perf suite's workloads (TESTING.md, "Performance"): the golden
//! workload in memory on one thread, and the raw-file reader on a generated
//! file. `benches/perf.py` runs them under `perf stat` and gates the counts
//! against `benches/perf.pins`.
//!
//!   perfbench tokenize|parse|render <sets> [target]
//!   perfbench rawgen <path> bin|ascii <variables> <points>
//!   perfbench raw <path> [reps]
//!
//! `<sets>` is `-` (every set) or a comma list of golden sets; `[target]` is
//! a dialect name or `-` (every target). Each run prints one line,
//! `bench <key>=<value> ...`; `allocs` counts every allocation and
//! reallocation in the measured region, exact and the same on every run.
//! With `PERFBENCH_CTL` and `PERFBENCH_ACK` naming the FIFOs of `perf stat
//! --control fifo:<ctl>,<ack> -D -1`, the counters run only inside the
//! measured region, so input loading (`inputs()`: every kit, ≈ 8e9
//! instructions) is left out.
//!
//! Contents: none (a bench binary; `main` is its entry).

#[path = "../src/bin/golden/inputs.rs"]
#[allow(dead_code)]
mod inputs;

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use spicerack::import::decode;
use spicerack::syntax::render;
use spicerack::syntax::token::Tag;
use spicerack::syntax::{Dialect, Lang, parse, tokenize};

const USAGE: &str = "usage: perfbench tokenize|parse|render <sets> [target] | \
                     perfbench rawgen <path> bin|ascii <variables> <points> | \
                     perfbench raw <path> [reps]";

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

/// The system allocator, counting each allocation and its bytes.
struct Counting;

// SAFETY: every call forwards to `System` with the caller's arguments.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size() as u64, Relaxed);
        // SAFETY: the caller's contract is `System.alloc`'s.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` through this allocator.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(n as u64, Relaxed);
        // SAFETY: `p` came from `System` through this allocator.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        [stage @ ("tokenize" | "parse" | "render"), sets, rest @ ..] if rest.len() <= 1 => {
            golden(stage, sets, rest.first().copied().unwrap_or("-"));
        }
        ["rawgen", path, form @ ("bin" | "ascii"), nv, np] => {
            let (Ok(nv), Ok(np)) = (nv.parse(), np.parse()) else {
                return usage();
            };
            std::fs::write(path, rawgen(*form == "bin", nv, np)).expect("write the raw file");
        }
        ["raw", path, rest @ ..] if rest.len() <= 1 => {
            let Ok(reps) = rest.first().map_or(Ok(5), |r| r.parse::<u32>()) else {
                return usage();
            };
            raw(path, reps);
        }
        _ => usage(),
    }
}

// ---- private ----

fn usage() {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

/// The measured region: `perf stat` counts between `start` and `stop`.
struct Region {
    fifos: Option<(std::fs::File, BufReader<std::fs::File>)>,
    allocs: u64,
    bytes: u64,
    t0: Instant,
}

impl Region {
    fn start() -> Region {
        let fifos = match (
            std::env::var_os("PERFBENCH_CTL"),
            std::env::var_os("PERFBENCH_ACK"),
        ) {
            (Some(c), Some(a)) => {
                let ctl = std::fs::OpenOptions::new().write(true).open(c);
                let ack = std::fs::File::open(a);
                Some((
                    ctl.expect("open PERFBENCH_CTL"),
                    BufReader::new(ack.expect("open PERFBENCH_ACK")),
                ))
            }
            _ => None,
        };
        let mut r = Region {
            fifos,
            allocs: 0,
            bytes: 0,
            t0: Instant::now(),
        };
        r.send("enable");
        r.allocs = ALLOCS.load(Relaxed);
        r.bytes = BYTES.load(Relaxed);
        r.t0 = Instant::now();
        r
    }

    /// Ends the region: its allocations, their bytes, and the wall seconds.
    fn stop(mut self) -> (u64, u64, f64) {
        let dt = self.t0.elapsed().as_secs_f64();
        let counts = (
            ALLOCS.load(Relaxed) - self.allocs,
            BYTES.load(Relaxed) - self.bytes,
        );
        self.send("disable");
        (counts.0, counts.1, dt)
    }

    fn send(&mut self, cmd: &str) {
        let Some((ctl, ack)) = &mut self.fifos else {
            return;
        };
        writeln!(ctl, "{cmd}").expect("write the perf control FIFO");
        let mut line = String::new();
        ack.read_line(&mut line).expect("read the perf ack FIFO");
        // perf writes `ack\n\0`: the NUL leads the next read.
        let ack = line.trim_matches(|c: char| c == '\0' || c.is_whitespace());
        assert_eq!(ack, "ack", "perf acknowledges `{cmd}`");
    }
}

/// The golden workload: every input of `sets` tokenized, parsed, or
/// rendered for `target`, one thread, in memory.
fn golden(stage: &str, sets: &str, target: &str) {
    let mut manifest = Vec::new();
    let all = inputs::inputs(Path::new(env!("CARGO_MANIFEST_DIR")), &mut manifest);
    let absent: Vec<&String> = manifest
        .iter()
        .filter(|l| l.starts_with("absent"))
        .collect();
    assert!(
        absent.is_empty(),
        "run in `nix develop .#testing` with $IHP_PDK: {absent:?}"
    );
    let wanted = |set: &str| sets == "-" || sets.split(',').any(|s| s == set);
    let srcs: Vec<_> = all
        .iter()
        .filter(|i| wanted(i.set))
        .map(|i| {
            let bytes = std::fs::read(&i.path).expect("read an input");
            let (s, n) = inputs::source(&decode(&bytes), i.form, &i.path);
            // Loaded here, outside the measured region, as the walk did.
            let kit = Lang::from(i.dialect).lexicon().map(|_| i.load_kit());
            (i, s, n, kit)
        })
        .collect();
    assert!(!srcs.is_empty(), "no input in sets `{sets}`");
    let targets: Vec<Dialect> = Dialect::ALL
        .into_iter()
        .filter(|d| target == "-" || d.name() == target)
        .collect();
    assert!(!targets.is_empty(), "no target `{target}`");
    let in_bytes: usize = srcs.iter().map(|s| s.1.len()).sum();
    let statements: usize = srcs
        .iter()
        .filter_map(|(i, s, _, _)| {
            let lex = Lang::from(i.dialect).lexicon()?;
            let t = tokenize(s, lex);
            Some(
                t.tags()
                    .iter()
                    .filter(|t| **t == Tag::EndOfStatement)
                    .count(),
            )
        })
        .sum();
    let (mut tokens, mut nodes, mut out_bytes) = (0usize, 0usize, 0usize);
    let region = Region::start();
    for (i, s, n, kit) in &srcs {
        let Some(lex) = Lang::from(i.dialect).lexicon() else {
            continue;
        };
        match stage {
            "tokenize" => tokens += tokenize(s, lex).len(),
            "parse" => {
                let t = tokenize(s, lex);
                tokens += t.len();
                nodes += parse(&t, lex).len();
            }
            _ => {
                for &d in &targets {
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match kit {
                        Some((k, f)) => render::render_kit(k, *f, n, d),
                        None => render::render_text(s, i.dialect, n, d),
                    }));
                    if let Ok(Ok(w)) = r {
                        out_bytes += w.text.len();
                    }
                }
            }
        }
    }
    let (allocs, bytes, secs) = region.stop();
    let per = (statements * targets.len()).max(1) as f64;
    println!(
        "bench {stage} sets={sets} target={target} files={} in_bytes={in_bytes} \
         statements={statements} targets={} tokens={tokens} nodes={nodes} \
         out_bytes={out_bytes} allocs={allocs} alloc_bytes={bytes} \
         allocs_per_stmt_target={:.1} mb_per_s={:.1} secs={secs:.3}",
        srcs.len(),
        targets.len(),
        allocs as f64 / per,
        in_bytes as f64 / secs / 1e6,
    );
}

/// A raw file of `nv` real variables (`time` first) and `np` points, binary
/// (point-major little-endian f64s) or ASCII, every value distinct.
fn rawgen(binary: bool, nv: usize, np: usize) -> Vec<u8> {
    assert!(nv >= 1 && np >= 1, "a plot has a variable and a point");
    let mut out = format!(
        "Title: perfbench\nDate: -\nPlotname: Transient Analysis\nFlags: real\n\
         No. Variables: {nv}\nNo. Points: {np}\nVariables:\n\t0\ttime\ttime\n"
    );
    for v in 1..nv {
        out.push_str(&format!("\t{v}\tv(n{v})\tvoltage\n"));
    }
    out.push_str(if binary { "Binary:\n" } else { "Values:\n" });
    let mut out = out.into_bytes();
    let value = |p: usize, v: usize| p as f64 * 1e-9 + v as f64 * 1e-3;
    for p in 0..np {
        if !binary {
            out.extend_from_slice(format!("{p}").as_bytes());
        }
        for v in 0..nv {
            if binary {
                out.extend_from_slice(&value(p, v).to_le_bytes());
            } else {
                out.extend_from_slice(format!("\t{:e}\n", value(p, v)).as_bytes());
            }
        }
    }
    out
}

/// Parses the raw file at `path` `reps` times in the measured region.
fn raw(path: &str, reps: u32) {
    let data = std::fs::read(path).expect("read the raw file");
    let mut best = f64::MAX;
    let mut doubles = 0usize;
    let region = Region::start();
    for _ in 0..reps {
        let t = Instant::now();
        let plots = spicerack::rawfile::parse_raw_all(&data).expect("a well-formed raw file");
        best = best.min(t.elapsed().as_secs_f64());
        doubles = plots
            .iter()
            .map(|p| p.real_data.iter().map(Vec::len).sum::<usize>())
            .sum();
        std::hint::black_box(&plots);
    }
    let (allocs, bytes, secs) = region.stop();
    println!(
        "bench raw file={path} in_bytes={} reps={reps} doubles={doubles} allocs={allocs} \
         alloc_bytes={bytes} best_secs={best:.4} mb_per_s={:.0} secs={secs:.3}",
        data.len(),
        data.len() as f64 / best / 1e6,
    );
}
