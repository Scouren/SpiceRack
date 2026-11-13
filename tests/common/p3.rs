//! Law P3 on one pass (`tests/pass_laws.rs`, apart for its 500-line cap): the output uses only
//! the input's features, the pass's `requires` and the design's stated laws' members (R90a); a
//! function's, operator's or draw's static and dynamic members count as one (R65, A39). And
//! P3's self-tests.
//!
//! Contents: `pub fn p3_on(&Pass, ir::Design) -> Result<(), String>`

use spicerack::ir;
use spicerack::passes::Pass;

/// P3 on one design: what `pass` leaves uses nothing outside what the design used plus
/// `requires`, an expression feature compared without its form (R65, A39).
pub fn p3_on(pass: &Pass, mut design: ir::Design) -> Result<(), String> {
    // a stated law's member is the design's own (R90a): where a pass makes a carrier, the
    // variant is the one the design states, which no `requires` can name
    let stated = design.laws().features();
    let allowed = ir::features_used(&design)
        .union(pass.requires)
        .union(stated);
    let allowed: Vec<ir::Feature> = allowed.iter().map(static_member).collect();
    let mut diags = ir::Diags::new();
    (pass.run)(&mut design, &mut diags);
    let used = ir::features_used(&design);
    let extra: Vec<ir::Feature> = used
        .iter()
        .filter(|&f| !allowed.contains(&static_member(f)))
        .collect();
    match extra.is_empty() {
        true => Ok(()),
        false => Err(format!("it adds {extra:?}")),
    }
}

// ---- private ----

/// P3 counts a stated law's member as the design's own (R90a): a pass that makes a dynamic
/// `ln` (beside a static one) under a stated `log_zero_error` makes a carrier of the design's
/// variant, which no `requires` can name.
#[test]
fn p3_counts_a_stated_law_member_as_the_designs_own() {
    let text = "file \"d.sp\" deck\n  param a = ln(2)\n  element behav b1 (n 0) expr=(v(n) + 1)\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    design
        .builder()
        .set_laws(ir::Laws::of(&[ir::Law::LogZeroError]));
    p3_on(&stub("dyn_ln", ln_one), design).expect("the stated law's member is the design's");
}

/// P3's self-test (R65, A39): on `v(n) + 1` beside a static `2 * 3`, a pass that writes
/// `v(n) + 1 * 1` only moves `Mul` from its static into its dynamic member, which P3 passes; one
/// that writes `v(n) + (1 - 1)` adds `Sub`, which neither form used, and P3 catches it.
#[test]
fn p3_passes_a_changed_form_and_catches_a_new_operator() {
    let form_only = stub("form_only", mul_one);
    p3_on(&form_only, p3_design()).expect("a moved form is no new feature");
    let adds = stub("adds_sub", sub_one);
    let found = p3_on(&adds, p3_design()).expect_err("Sub is new in either form");
    assert!(found.contains("BinaryDyn(Sub)"), "{found}");
}

/// A function, operator or draw's static member; every other feature itself. A root's form is
/// a fact about the root (R51), so a pass that merges two roots or drops a caller moves what it
/// did not write between the two members (R65).
fn static_member(feature: ir::Feature) -> ir::Feature {
    match feature {
        ir::Feature::FuncDyn(f) => ir::Feature::Func(f),
        ir::Feature::BinaryDyn(op) => ir::Feature::Binary(op),
        ir::Feature::UnaryDyn(op) => ir::Feature::Unary(op),
        ir::Feature::DistDyn(d) => ir::Feature::Dist(d),
        other => other,
    }
}

/// A self-test pass of one rule, no `requires` and no notes.
fn stub(name: &'static str, run: fn(&mut ir::Design, &mut ir::Diags)) -> Pass {
    Pass {
        name,
        removes: ir::Feature::Item(ir::ItemKind::Comment),
        requires: ir::FeatureSet::EMPTY,
        notes: &[],
        run,
    }
}

/// P3's self-test design: a static `Mul` and a dynamic `Add` over a literal.
fn p3_design() -> ir::Design {
    let text = "file \"d.sp\" deck\n  param a = 2 * 3\n  element behav b1 (n 0) expr=(v(n) + 1)\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    design
}

/// Every literal of a dynamic root `x` becomes `x * x`.
fn mul_one(design: &mut ir::Design, _: &mut ir::Diags) {
    literal_to(design, ir::BinOp::Mul);
}

/// Every literal of a dynamic root `x` becomes `x - x`.
fn sub_one(design: &mut ir::Design, _: &mut ir::Diags) {
    literal_to(design, ir::BinOp::Sub);
}

/// Every literal of a dynamic root `x` becomes `ln(x)`.
fn ln_one(design: &mut ir::Design, _: &mut ir::Diags) {
    let changed = ir::rewrite_exprs_in(design, ir::Form::Dynamic, &mut |b, e| {
        let ir::Expr::Num(_) = b.design().expr(e) else {
            return None;
        };
        Some(b.call(ir::Func::Ln, &[e]))
    });
    assert_eq!(changed, 1, "one dynamic root holds a literal");
}

fn literal_to(design: &mut ir::Design, op: ir::BinOp) {
    let changed = ir::rewrite_exprs_in(design, ir::Form::Dynamic, &mut |b, e| {
        let ir::Expr::Num(_) = b.design().expr(e) else {
            return None;
        };
        Some(b.binary(op, e, e))
    });
    assert_eq!(changed, 1, "one dynamic root holds a literal");
}
