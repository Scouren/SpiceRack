//! The copying rules' size bound through `legalize` (`src/passes/expr/mod.rs`, "The size
//! bound"): a tower of each copying rule, and a mixed one, 24 deep.
//!
//! Contents: tests only.

use spicerack::ir;

use super::compare;

// ---- private ----

#[test]
fn a_tower_of_copying_rules_stays_bounded_through_legalize() {
    // ROOT_PATHS_MAX of src/passes/expr/nodes.rs: no copying rule makes a root spell more
    const TOWER_PATHS_MAX: u64 = 8192;
    // oracle: the root's size counted by paths after legalize (what a writer will spell),
    // against TOWER_PATHS_MAX, and the design's row count (memory) against a fixed bound; each
    // tower is 24 deep, and a mixed tower nests every copying rule
    let one = |f: &str, inner: &str| match f {
        "pow_signed" => format!("pow_signed({inner}, 3)"),
        "pwl" => format!("pwl({inner}, 0, 0, 1, 1)"),
        "call" => format!("@f({inner})"),
        _ => format!("{f}({inner})"),
    };
    let funcs = [
        ("sqr", ir::Func::Sqr),
        ("pow_signed", ir::Func::PowSigned),
        ("round_half_even", ir::Func::RoundHalfEven),
        ("round_half_away", ir::Func::RoundHalfAway),
        ("sgn", ir::Func::Sgn),
        ("step_half", ir::Func::StepHalf),
        ("ramp", ir::Func::Ramp),
        ("exp_lin14", ir::Func::ExpLin14),
        ("pwl", ir::Func::Pwl),
    ];
    let mut target = ir::FeatureSet::ALL;
    for (_, f) in funcs {
        for g in [ir::Feature::Func(f), ir::Feature::FuncDyn(f)] {
            target = target.minus(ir::FeatureSet::EMPTY.with(g));
        }
    }
    for g in [
        ir::Feature::Expr(ir::ExprTag::UserCall),
        ir::Feature::Item(ir::ItemKind::Func),
    ] {
        target = target.minus(ir::FeatureSet::EMPTY.with(g));
    }
    let names: Vec<&str> = funcs.iter().map(|f| f.0).chain(["call"]).collect();
    let mut towers: Vec<(String, String)> = names
        .iter()
        .map(|&f| {
            (
                f.to_string(),
                (0..24).fold("x".to_string(), |e, _| one(f, &e)),
            )
        })
        .collect();
    let mixed = (0..24).fold("x".to_string(), |e, k| one(names[k % names.len()], &e));
    towers.push(("mixed".to_string(), mixed));
    for (name, body) in towers {
        let text = format!(
            "file \"p.sp\" deck\n  param x = 0.3\n  func f(u) = u * u + u\n  param r = {body}\n"
        );
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(&text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "{name}: {:?}", diags.rows());
        spicerack::passes::legalize(&mut design, &target, &mut diags);
        let size = compare::roots(&design)
            .iter()
            .flat_map(|(_, r)| r.iter().map(|&e| compare::paths(&design, e)))
            .max()
            .unwrap_or(0);
        eprintln!(
            "{name}: the largest root spells {size} rows, the design holds {}",
            design.expr_count_all()
        );
        assert!(
            size <= TOWER_PATHS_MAX,
            "{name}: a root of {size} rows by paths"
        );
        assert!(
            design.expr_count_all() <= 1 << 20,
            "{name}: {} rows",
            design.expr_count_all()
        );
    }
}
