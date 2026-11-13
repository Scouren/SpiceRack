//! The function-grid law (R76): every function VACASK's reader maps to the IR, evaluated by
//! VACASK on a grid (signs, zeros, a large and a small magnitude, integer and real arguments,
//! both argument orders) and by the IR's reference evaluator on what the reader makes of the
//! same text; each value compared at the raw file's precision. A function that differs anywhere
//! is a reading the IR does not mean (F9: `atan2`).
//!
//! Oracle: VACASK 36be560 itself, on decks `settle/grid.py` generates (points inside each
//! function's domain only): `grid_static.sim`, one parameter per point read by a source's `dc`
//! (the parameter evaluator); `grid_dyn_<f>.sim`, one behavioural source per point reading its
//! arguments from a node (the compiled expression); `grid_fold_<f>.sim`, the same with literal
//! arguments, which VACASK folds with its parameter evaluator.
//!
//! Contents: tests only.

use std::path::Path;

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

use super::settle::{raw, run};

/// The settle decks, relative to the crate root.
const DIR: &str = "tests/fixtures/dialect/vacask/settle";

/// Points where VACASK and the IR disagree, `context expression`, sorted; strict both ways.
///
/// Cited exception (R76a), all of them: `atan2(y, -0.0)` in the parameter evaluator, which
/// computes `atan(y/x)` and sees −∞ for x = −0 (`atan2(0.5, -0.0)` is −π/2, `atan2(-0.5, -0.0)`
/// 3π/2; run67) where the IR's expansion, which has no test for a zero's sign, gives π/2. A
/// signed zero on the function's branch cut, the one class the grid law lets stand.
const KNOWN_GRID: &[&str] = &[
    "folded atan2(-0.5, -0.0)",
    "folded atan2(-2.0, -0.0)",
    "folded atan2(-3, -0.0)",
    "folded atan2(0.5, -0.0)",
    "folded atan2(2.0, -0.0)",
    "folded atan2(3, -0.0)",
    "param (atan2(-0.5, -0.0))/2",
    "param (atan2(-2.0, -0.0))/2",
    "param (atan2(-3, -0.0))/2",
    "param (atan2(0.5, -0.0))/2",
    "param (atan2(2.0, -0.0))/2",
    "param (atan2(3, -0.0))/2",
    "param atan2(-0.5, -0.0)",
    "param atan2(-2.0, -0.0)",
    "param atan2(-3, -0.0)",
    "param atan2(0.5, -0.0)",
    "param atan2(2.0, -0.0)",
    "param atan2(3, -0.0)",
];

/// The bindings of a grid deck: VACASK's default temperature (27 °C: `$temp/$temp` is 1), the
/// node `one` at 1 V; nothing else is bound.
struct Grid;

impl ir::Bindings for Grid {
    fn param(&self, _: ir::Ref) -> Option<f64> {
        None
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        Some(1.0)
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

/// Whether VACASK's printed `got` and the IR's `want` agree at the raw file's 16 digits (NaN
/// agrees with NaN). A zero's sign is not compared: a node voltage never carries one (run74: none
/// of `grid_static.sim`'s 2856 values is printed `-0`, though C gives `ceil(-0.5)` and
/// `atan(-0.0)` as −0), so the oracle cannot see it.
fn same(got: f64, want: f64) -> bool {
    if got.is_nan() || want.is_nan() {
        return got.is_nan() && want.is_nan();
    }
    (got - want).abs() <= 1e-14 * want.abs().max(1e-300)
}

/// The deck `name` read by VACASK's reader, and its text.
fn read(name: &str) -> (ir::Design, String) {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR).join(name))
        .expect("a grid deck");
    let row = dialect::by_name("vacask").expect("the vacask row");
    let (mut d, mut diags) = (ir::Design::new(), ir::Diags::new());
    dialect::read(
        &Sources::one(name, &text),
        row,
        Policy::Lenient,
        &mut d,
        &mut diags,
    );
    (d, text)
}

/// The expression text the deck gives the value `key` (`g12=…` up to the next `h12=`, or the
/// `v=` of the source `b12`).
fn source_of(text: &str, key: &str) -> String {
    let k = key.trim_start_matches(['d', 'g', 'h']);
    let line = text
        .lines()
        .find(|l| l.contains(&format!(" {key}=")) || l.starts_with(&format!("b{k} ")))
        .unwrap_or_default();
    let (start, end) = match line.split_once(&format!(" {key}=")) {
        Some((_, rest)) => (rest, format!(" h{k}=")),
        None => (line.split_once("v=").map_or("", |(_, e)| e), String::new()),
    };
    match end.is_empty() {
        true => start.to_string(),
        false => start
            .split(end.as_str())
            .next()
            .unwrap_or_default()
            .to_string(),
    }
}

/// Every point where `deck`'s values differ, `context expression vacask=… ir=…`.
fn differences(deck: &str, context: &str) -> Vec<String> {
    let r = run("vacask", deck);
    let plot = raw(&r, "op1.raw");
    let (d, text) = read(deck);
    let column = |name: &str| {
        plot.variables
            .iter()
            .position(|v| v.name == name)
            .map(|c| plot.real_data[c][0])
    };
    let mut out = Vec::new();
    for (at, _) in d.reachable() {
        let (node, e) = match at.kind {
            ir::ItemKind::Param => {
                let p: ir::item::Param = d.row(at.index);
                (d.str(p.name).to_string(), p.value)
            }
            ir::ItemKind::Element => {
                let el: ir::Element = d.row(at.index);
                if el.kind != ir::ElemKind::Behav {
                    continue;
                }
                let name = d.str(el.name).trim_start_matches('b').to_string();
                (format!("d{name}"), d.behav(&el).expr)
            }
            _ => continue,
        };
        let want = ir::eval(&d, e, &Grid).unwrap_or(f64::NAN);
        // VACASK merges a node a source holds at a literal 0 into ground: no vector, and no
        // sign to compare.
        let Some(got) = column(&node) else {
            if want != 0.0 {
                out.push(format!(
                    "{context} {}: no vector, ir={want:e}",
                    source_of(&text, &node)
                ));
            }
            continue;
        };
        if !same(got, want) {
            out.push(format!(
                "{context} {} vacask={got:e} ir={want:e}",
                source_of(&text, &node)
            ));
        }
    }
    out
}

#[test]
fn every_function_the_reader_maps_means_what_vacask_computes() {
    let mut found = differences("grid_static.sim", "param");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR);
    let mut decks: Vec<String> = std::fs::read_dir(&dir)
        .expect("the settle directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| {
            (n.starts_with("grid_dyn_") || n.starts_with("grid_fold_")) && n.ends_with(".sim")
        })
        .collect();
    decks.sort();
    assert!(decks.len() >= 40, "{decks:?}");
    for deck in decks {
        let context = match deck.starts_with("grid_fold_") {
            true => "folded",
            false => "behavioural",
        };
        found.extend(differences(&deck, context));
    }
    let mut points: Vec<&str> = found
        .iter()
        .map(|f| f.split(" vacask=").next().unwrap_or(f))
        .collect();
    points.sort();
    assert_eq!(points, KNOWN_GRID, "\n{}", found.join("\n"));
}
