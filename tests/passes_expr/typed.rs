//! The typed arithmetic's proofs (`div_typed_to_div`, `int_literal_to_plain`) and the R65 row
//! check, with the oracles of `tests/passes_expr.rs` (`ir::eval_typed`, `passes::legal`).
//!
//! Contents: tests only.

use spicerack::ir;
use spicerack::passes;

use super::compare::{ANY, code, keeps_boundary_values, keeps_every_root_value, notes};

// ---- private ----

/// Each row rewrites its own form (R65): a static-only deck for the plain row, a behavioural
/// one for the `_dyn` row, at the boundary set under each tag pair; generated designs, every
/// root by value (a quotient one row leaves is read by the other form's roots, which are
/// compared too); and the chain: a static Int quotient a dynamic Int division reads waits for
/// the dynamic row, then goes (`legalize` for a target with neither form).
#[test]
fn div_typed_to_div_keeps_every_value() {
    for (x, y, tags) in [
        ("1i", "2i", &[3u8][..]),
        ("1", "2i", &[2]),
        ("1i", "1", &[1]),
    ] {
        let head = format!("file \"p.sp\" deck\n  param x = {x}\n  param y = {y}\n");
        let decks = [
            ("div_typed_to_div", format!("{head}  param r = x /: y\n")),
            (
                "div_typed_to_div_dyn",
                format!("{head}  element behav b1 (n 0) expr=(x /: y + v(n))\n"),
            ),
        ];
        for (name, text) in decks {
            let points = keeps_boundary_values(name, &text, tags);
            eprintln!("{name} ({x}, {y}): {points} boundary points");
        }
    }
    for name in ["div_typed_to_div", "div_typed_to_div_dyn"] {
        let changed = keeps_every_root_value(name);
        eprintln!("{name}: {changed} generated roots changed");
        assert!(changed > 0, "{name}");
    }
    let text = "file \"p.sp\" deck\n  title \"t\"\n  param q = 7i /: 2i\n  element behav b1 (n 0) expr=(q /: 2i + v(n))\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    let typed = ir::FeatureSet::EMPTY
        .with(ir::Feature::Binary(ir::BinOp::DivTyped))
        .with(ir::Feature::BinaryDyn(ir::BinOp::DivTyped));
    passes::legalize(&mut design, &ir::FeatureSet::ALL.minus(typed), &mut diags);
    let mut out = String::new();
    ir::dump(&design, design.entry(), &mut out);
    assert_eq!(
        out,
        "file \"p.sp\" deck\n  title \"t\"\n  param q = trunc(7i / 2i)\n  element behav b1 (n 0) expr=(trunc(q / 2i) + v(n))\n"
    );
}

#[test]
fn a_division_of_mixed_operand_types_is_noted_and_so_is_every_int_quotient() {
    let got = notes("div_typed_to_div", "mixed");
    assert!(
        !got.is_empty() && got.iter().all(|&c| c == code(101)),
        "{got:?}"
    );
}

/// R65: a one-form row's `requires` name what it writes in its own form. So every plain row
/// (the static form) is runnable for a target with no dynamic member at all, and the typed
/// division's plain row for ngspice's own set (`div_typed_to_div` never ran there while it
/// required `FuncDyn(Trunc)`: corpus `demo/natures/ddt.sim` line 22), and the dynamic rows
/// once `trunc` has a rule there. Oracle: `passes::legal`.
#[test]
fn every_plain_row_is_runnable_without_the_dynamic_members() {
    let mut dynamic = ir::FeatureSet::EMPTY;
    for f in (0..ir::Feature::COUNT).map(ir::Feature::from_index) {
        if matches!(
            f,
            ir::Feature::FuncDyn(_)
                | ir::Feature::BinaryDyn(_)
                | ir::Feature::UnaryDyn(_)
                | ir::Feature::DistDyn(_)
        ) {
            dynamic = dynamic.with(f);
        }
    }
    let group = passes::GROUPS
        .iter()
        .find(|g| g.name == "expr")
        .expect("the group");
    let all: Vec<&passes::Pass> = passes::all().collect();
    let mut blocked = Vec::new();
    for p in group.passes.iter().filter(|p| {
        let twin = format!("{}_dyn", p.name);
        group.passes.iter().any(|q| q.name == twin)
    }) {
        let removes = ir::FeatureSet::EMPTY.with(p.removes);
        let legal = passes::legal(&all, &ir::FeatureSet::ALL.minus(dynamic).minus(removes));
        let missing: Vec<ir::Feature> = p.requires.minus(legal).iter().collect();
        if !missing.is_empty() {
            blocked.push((p.name, missing));
        }
    }
    assert!(
        blocked.is_empty(),
        "plain rows needing a dynamic member: {blocked:?}"
    );
    let ngspice = passes::legal(&all, &spicerack::dialect::ngspice::FEATURES);
    // and the dynamic ones through `trunc_to_ternary_dyn` (ngspice's B source has no trunc)
    for f in [
        ir::Feature::Binary(ir::BinOp::DivTyped),
        ir::Feature::BinaryDyn(ir::BinOp::DivTyped),
        ir::Feature::FuncDyn(ir::Func::ToInt),
        ir::Feature::FuncDyn(ir::Func::Trunc),
    ] {
        assert!(ngspice.contains(f), "{f:?} is legal for ngspice");
    }
}

/// Oracle: `eval_typed` by value, never the rule's arithmetic: every root of generated
/// designs before and after (parameters bound to their definitions, so a literal in a
/// definition reaches its readers); each root the rule changes in a deck at the boundary set,
/// `x` and `y` of every tag (its `q` is a typed division the rule must leave: a rule that wrote
/// its literals Real would change it, and it would be compared); and the notes on the fixture
/// whose tags are read (a typed division, a body under `to_int`, a bit operator, `to_real`).
#[test]
fn int_literal_to_plain_keeps_every_value_and_leaves_what_a_tag_reads() {
    let changed = keeps_every_root_value("int_literal_to_plain");
    eprintln!("int_literal_to_plain: {changed} generated roots changed");
    assert!(
        changed > 0,
        "no generated root held an unread integer literal"
    );
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = x * 3i + y - 2i\n  param s = min(x, 4i) * -y\n  param t = x > 1i ? y : 0i\n  param q = (x * 2i + 1i) /: 3i\n";
    let points = keeps_boundary_values("int_literal_to_plain", text, ANY);
    eprintln!("int_literal_to_plain: {points} boundary points");
    assert_eq!(notes("int_literal_to_plain", "read"), [code(118); 6]);
}

/// The census's `r=1.0/7` (VACASK corpus `demo/natures/ddt.sim:22`): legalized for ngspice,
/// the typed division goes first and then the integer literal, in one legalize. The rule read
/// the dead row of the division it replaced (rows are appended, the old ones stay) as a reader.
#[test]
fn a_division_gone_leaves_its_integer_literal_free_in_the_same_legalize() {
    let text = "file \"p.sp\" deck\n  title \"t\"\n  element resistor r1 (a 0) value=(1.0 /: 7i)\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    passes::legalize(
        &mut design,
        &spicerack::dialect::ngspice::FEATURES,
        &mut diags,
    );
    let left = ir::features_used(&design).minus(spicerack::dialect::ngspice::FEATURES);
    assert!(
        left.is_empty(),
        "left: {:?} {:?}",
        left.iter().collect::<Vec<_>>(),
        diags.rows()
    );
    let mut out = String::new();
    ir::dump(&design, design.entry(), &mut out);
    assert!(out.contains("value=(1.0 / 7)"), "{out}");
}

/// The census's integer quotient (VACASK corpus `test/test_hb3.sim:8`, `rth=(3000-25)/60`, 49
/// on VACASK): legalized for ngspice it is `trunc((3000 - 25) / 60)` with plain literals, 49 by
/// the reference evaluator, and ngspice writes `trunc` as numparam's `int` (ngspice-45:
/// `int((3000-25)/60)` gives 49 and `int(-2.5)` -2, `trunc(…)` is "Formula() error"); run:
/// the written deck's resistor of `rth` ohms passes 1/49 A at 1 V.
#[test]
fn an_integer_quotient_reaches_ngspice_as_its_truncation() {
    let text = "file \"p.sp\" deck\n  title \"t\"\n  param rth = (3000i - 25i) /: 60i\n  element vsource v1 (a 0) dc=1\n  element resistor r1 (a 0) value=rth\n  analysis op\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let ngspice = spicerack::dialect::ngspice::FEATURES;
    passes::legalize(&mut design, &ngspice, &mut diags);
    let left: Vec<ir::Feature> = ir::features_used(&design).minus(ngspice).iter().collect();
    assert!(left.is_empty(), "left: {left:?}");
    let mut out = String::new();
    ir::dump(&design, design.entry(), &mut out);
    assert!(out.contains("param rth = trunc((3000 - 25) / 60)"), "{out}");
    let row = spicerack::dialect::by_name("ngspice").expect("the ngspice row");
    let deck = spicerack::dialect::write(&design, design.entry(), row, &mut diags);
    let deck = deck.replace(".end", ".control\nop\nprint i(v1)\n.endc\n.end");
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("deck.cir");
    std::fs::write(&path, &deck).expect("the deck");
    let run = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&path)
        .output()
        .expect("ngspice runs (nix develop .#testing provides it; unsupported fails)");
    let printed = String::from_utf8_lossy(&run.stdout).into_owned();
    let line = printed
        .lines()
        .find(|l| l.contains("i(v1) = "))
        .unwrap_or_else(|| panic!("{deck}\n{printed}"));
    let amps: f64 = line
        .split('=')
        .nth(1)
        .expect("a value")
        .trim()
        .parse()
        .expect("a number");
    // `print` shows six significant digits: the tolerance is its rounding
    assert!((amps * 49.0 + 1.0).abs() < 1e-5, "{amps}: {deck}");
}
