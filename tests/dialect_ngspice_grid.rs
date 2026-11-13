//! The function-grid law (R76): every function and operator ngspice's reader maps to the IR,
//! evaluated by ngspice-45 on a grid inside its domain (signs, zero, a large and a small
//! magnitude, halves, both argument orders) and by the IR's reference evaluator on what the
//! reader makes of the same text; each value compared at the raw file's precision. A point that
//! differs is a reading the IR does not mean.
//!
//! Oracle: ngspice-45 itself (`ngspice -b -r`, binary raw), on the decks `settle/grid.py`
//! generates: `grid_numparam.sp`, one `V` source per point valued `{f(args)}` (numparam);
//! `grid_beh_<f>.sp`, one `B` source per point reading its arguments from nodes (the B-source
//! parser); `grid_fold.sp`, the same `B` sources with literal arguments, and the B points past
//! the range where ngspice's B function is C's (`ptfuncs.c`: `sin(1e10)`, `exp(1000)`, `ln(0)`).
//! A behavioural point is compared to the companion model's rounding (`companion_slack`); the
//! points where its slope is infinite are in the folded grid only (`grid.py` `STEEP`).
//!
//! Contents: tests only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

/// The settle folder.
const SETTLE: &str = "tests/fixtures/dialect/ngspice/settle";

/// Points where ngspice and the IR disagree, `context expression`, sorted; strict both ways.
/// Every row waits for the team lead's ruling on the B reading (ngspice-45 `ptfuncs.c`, runs in
/// `grid_fold.sp`): `sin`/`cos` are `f(MODULUS(x, 2π))` and `tan` `tan(MODULUS(x, π))`, with
/// `MODULUS(N, L) = N − (int)(N/L)·L` (INT_MIN past 2³¹); `exp` is 1e99 above 227.9559242;
/// `ln`, `log`, `log10` are −1e99 at 0.
const KNOWN_GRID: &[&str] = &[
    "behavioural v = exp(v(a18))",
    "folded v = cos(-1000.0)",
    "folded v = cos(1000.0)",
    "folded v = cos(100000000000.0)",
    "folded v = exp(1000.0)",
    "folded v = exp(250)",
    "folded v = ln(0)",
    "folded v = log(0)",
    "folded v = log10(0)",
    "folded v = sin(-1000.0)",
    "folded v = sin(-20000000000.0)",
    "folded v = sin(1000.0)",
    "folded v = sin(10000000000.0)",
    "folded v = sin(20000000000.0)",
    "folded v = tan(-1000.0)",
    "folded v = tan(1000.0)",
    "folded v = tan(10000000000.0)",
];

/// The bindings of a grid deck: ngspice's default temperature (27 °C), each node at the voltage
/// ngspice solved for it; nothing else is bound.
struct Grid<'a> {
    design: &'a ir::Design,
    nodes: &'a HashMap<String, f64>,
}

impl ir::Bindings for Grid<'_> {
    fn param(&self, _: ir::Ref) -> Option<f64> {
        None
    }
    fn voltage(&self, net: ir::NameId, _: ir::NameId) -> Option<f64> {
        self.nodes
            .get(&self.design.str(net).to_ascii_lowercase())
            .copied()
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        None
    }
    fn time(&self) -> Option<f64> {
        None
    }
    fn temp(&self) -> Option<f64> {
        Some(27.0)
    }
    fn freq(&self) -> Option<f64> {
        None
    }
    fn tstep(&self) -> Option<f64> {
        None
    }
    fn tstop(&self) -> Option<f64> {
        None
    }
    fn tnom(&self) -> Option<f64> {
        None
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, _: &[f64]) -> Option<f64> {
        None
    }
}

// ---- private ----

/// Whether ngspice's `got` and the IR's `want` agree: at the binary raw file's 16 digits, plus
/// `slack` (NaN agrees with NaN; a zero's sign is not compared, ngspice prints none, R76).
fn same(got: f64, want: f64, slack: f64) -> bool {
    if got.is_nan() || want.is_nan() {
        return got.is_nan() && want.is_nan();
    }
    (got - want).abs() <= 1e-14 * want.abs().max(1e-300) + slack
}

/// The rounding a B source's operating point carries beyond its function's: ngspice solves the
/// companion `v = f(x0) + Σ ∂f/∂xᵢ·(xᵢ − x0ᵢ)` (`asrcload.c`), whose terms `∂f/∂xᵢ·xᵢ` cancel in
/// floating point, so the value is good to a few ulps of `Σ |xᵢ·∂f/∂xᵢ|` (run: B `cos(v(a))` at
/// 1000 is off by 5e-14, |x·sin x| = 827). The slopes are the IR's, by central difference.
fn companion_slack(
    d: &ir::Design,
    e: ir::ExprId,
    nodes: &HashMap<String, f64>,
    source: &str,
) -> f64 {
    let mut sum = 0.0;
    for arg in source.split("v(").skip(1) {
        let node = arg.split(')').next().unwrap_or_default();
        let x = nodes[node];
        if x == 0.0 {
            continue;
        }
        let h = 1e-6 * x.abs();
        let at = |dx: f64| {
            let mut moved = nodes.clone();
            moved.insert(node.to_string(), x + dx);
            let grid = Grid {
                design: d,
                nodes: &moved,
            };
            ir::eval(d, e, &grid).unwrap_or(f64::NAN)
        };
        sum += (x * (at(h) - at(-h)) / (2.0 * h)).abs();
    }
    4.0 * f64::EPSILON * sum
}

/// Every node voltage ngspice-45 solves for `deck` at its operating point, by node name.
fn solve(deck: &str) -> HashMap<String, f64> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ngspice_grid");
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    let raw = dir.join(deck.replace(".sp", ".raw"));
    let _ = std::fs::remove_file(&raw);
    let run = std::process::Command::new("ngspice")
        .arg("-b")
        .arg("-r")
        .arg(&raw)
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(SETTLE)
                .join(deck),
        )
        .output()
        .expect("ngspice runs (nix develop .#testing)");
    assert!(
        run.status.success(),
        "{deck}: ngspice failed\n{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let plot = spicerack::rawfile::parse_raw(&std::fs::read(&raw).expect("the raw file"))
        .expect("a raw file ngspice wrote");
    let mut out = HashMap::new();
    for (c, v) in plot.variables.iter().enumerate() {
        let name = v.name.to_ascii_lowercase();
        let name = name
            .strip_prefix("v(")
            .map_or(name.as_str(), |n| n.trim_end_matches(')'));
        out.insert(name.to_string(), plot.re(c)[0]);
    }
    out
}

/// Every point where `deck`'s values differ, `context expression ngspice=… ir=…`.
fn differences(deck: &str, context: &str) -> Vec<String> {
    let nodes = solve(deck);
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(SETTLE)
            .join(deck),
    )
    .expect("a grid deck");
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let (mut d, mut diags) = (ir::Design::new(), ir::Diags::new());
    dialect::read(
        &Sources::one(deck, &text),
        row,
        Policy::Lenient,
        &mut d,
        &mut diags,
    );
    let grid = Grid {
        design: &d,
        nodes: &nodes,
    };
    let mut out = Vec::new();
    let mut seen = 0usize;
    for (at, _) in d.reachable() {
        if at.kind != ir::ItemKind::Element {
            continue;
        }
        let el: ir::Element = d.row(at.index);
        let name = d.str(el.name).to_ascii_lowercase();
        let e = match el.kind {
            ir::ElemKind::VSource if name.starts_with("vg") => d.source(&el).dc,
            ir::ElemKind::Behav => d.behav(&el).expr,
            _ => continue,
        };
        seen += 1;
        let source = text
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with(&format!("{name} ")))
            .and_then(|l| l.splitn(4, ' ').nth(3))
            .unwrap_or_default();
        let want = ir::eval(&d, e, &grid).unwrap_or(f64::NAN);
        let got = nodes[&name[1..]];
        let slack = match context {
            "behavioural" => companion_slack(&d, e, &nodes, source),
            _ => 0.0,
        };
        if !same(got, want, slack) {
            out.push(format!("{context} {source} ngspice={got:e} ir={want:e}"));
        }
    }
    let points = text
        .lines()
        .filter(|l| l.starts_with(['b', 'v']) && !l.starts_with("va"));
    assert_eq!(
        seen,
        points.count(),
        "{deck}: every point is read to an element"
    );
    out
}

#[test]
fn every_function_the_reader_maps_means_what_ngspice_computes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SETTLE);
    let mut decks: Vec<String> = std::fs::read_dir(&dir)
        .expect("the settle folder")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.starts_with("grid_") && n.ends_with(".sp"))
        .collect();
    decks.sort();
    assert!(decks.len() >= 30, "{decks:?}");
    let mut found = Vec::new();
    for deck in &decks {
        let context = match deck.as_str() {
            "grid_numparam.sp" => "numparam",
            "grid_fold.sp" => "folded",
            _ => "behavioural",
        };
        found.extend(differences(deck, context));
    }
    let mut points: Vec<&str> = found
        .iter()
        .map(|f| f.split(" ngspice=").next().unwrap_or(f))
        .collect();
    points.sort();
    assert_eq!(points, KNOWN_GRID, "\n{}", found.join("\n"));
}
