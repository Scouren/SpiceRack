//! Throughput of the IR's bulk paths in a release build: building a generated design of 10⁶
//! items, dumping it to IR text, parsing that text back and checking it (`rewrite/IR-IMPL.md`
//! §3, the numbers written into `rewrite/IR-PERF.md`), and the scaling gate that keeps them
//! linear. Run:
//! `cargo run --release --features gen --example throughput [items] [feature-set]`, where the
//! set is `kernel` (default) or `all`; or `… --example throughput scaling`, which measures every
//! phase over the full set at `SCALE_N` and `4 × SCALE_N` items, each the fastest of
//! `SCALE_RUNS` runs (load can only inflate a time, so the minimum does not flake), and exits 1
//! when a phase's time ratio exceeds `SCALE_BOUND` (merge tier: a linear phase measures about 4
//! to 5 here, the cache included; a quadratic one about 16).
//!
//! Contents: none (a measurement binary).

use std::time::Instant;

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{Design, Diags, Feature, FeatureSet, dump, parse};

/// The scaling gate's smaller size; the larger is four times it.
const SCALE_N: u32 = 100_000;
/// Runs per size in the scaling gate; each phase keeps its fastest.
const SCALE_RUNS: u32 = 5;
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

// ---- private ----

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "scaling") {
        std::process::exit(scaling());
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

/// The scaling gate: each phase over the full set at `SCALE_N` and `4 × SCALE_N` (the fastest
/// of `SCALE_RUNS` runs each: a minimum is what a loaded machine cannot inflate, so the gate does
/// not flake); prints the ratios and returns 1 when one exceeds `SCALE_BOUND`.
fn scaling() -> i32 {
    let set = every_feature();
    let best = |n: u32| {
        let mut t = [f64::INFINITY; 4];
        for _ in 0..SCALE_RUNS {
            let p = measure(n, &set);
            for (k, s) in [p.build, p.dump, p.parse, p.check].into_iter().enumerate() {
                t[k] = t[k].min(s);
            }
        }
        t
    };
    let (small, large) = (best(SCALE_N), best(4 * SCALE_N));
    let mut fail = 0;
    for (k, name) in ["build", "dump", "parse", "check"].iter().enumerate() {
        let ratio = large[k] / small[k];
        let verdict = if ratio > SCALE_BOUND { "FAIL" } else { "ok" };
        println!(
            "{name:5} {:.3} s -> {:.3} s at {}x items, fastest of {SCALE_RUNS} runs: ratio {ratio:.2} (bound {SCALE_BOUND}) {verdict}",
            small[k], large[k], 4
        );
        if ratio > SCALE_BOUND {
            fail = 1;
        }
    }
    fail
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
        exprs: d.expr_count(),
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
