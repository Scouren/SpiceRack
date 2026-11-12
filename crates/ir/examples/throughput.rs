//! Throughput of the IR's three bulk paths in a release build: building a generated design of
//! 10⁶ items, dumping it to IR text, and parsing that text back (`rewrite/IR-IMPL.md` §3, the
//! number written into `rewrite/IR-PERF.md`). Run:
//! `cargo run --release --features gen --example throughput [items] [feature-set]`, where the
//! set is `kernel` (default) or `all`.
//!
//! Contents: none (a measurement binary).

use std::time::Instant;

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{Design, Diags, Feature, FeatureSet, dump, parse};

// ---- private ----

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let items: u32 = args
        .get(1)
        .map_or(1_000_000, |a| a.parse().expect("an item count"));
    let all = args.get(2).is_some_and(|a| a == "all");
    let set = if all { every_feature() } else { kernel_set() };
    let size = Size {
        items,
        scope_depth: 3,
        expr_depth: 4,
        files: 1,
    };

    let t = Instant::now();
    let d = generate(1, &set, size);
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
    let mb = text.len() as f64 / 1e6;
    let lines = text.lines().count();
    println!(
        "items {items}, set {}, text {mb:.1} MB, {lines} lines, {} expression rows",
        if all { "all" } else { "kernel" },
        d.expr_count()
    );
    println!("build {build:.3} s ({:.0} items/s)", items as f64 / build);
    println!("dump  {dumped:.3} s ({:.1} MB/s)", mb / dumped);
    println!(
        "parse {parsed:.3} s ({:.1} MB/s), of which a link is about {relink:.3} s",
        mb / parsed
    );
    println!("check {check:.3} s ({:.0} items/s)", items as f64 / check);
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
