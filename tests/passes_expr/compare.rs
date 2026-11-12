//! How the expression passes' proofs compare: a pass run on generated designs or on a text, and
//! each root it changed evaluated before and after (`ir::eval_typed`) under the same bindings.
//!
//! Contents: `pub const ANY`, `pub const fn code`, `pub fn pass`, `pub fn keeps_generated_values`, `pub fn keeps_values_over`,
//! `pub fn keeps_boundary_values`, `pub fn notes`, `pub fn typed`, `pub fn roots`

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes;

use super::bindings::{Pair, Point, boundary};

/// Every tag pair of two operands.
pub const ANY: &[u8] = &[0, 1, 2, 3];

/// A moderate design (the pass laws' size).
const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

/// Seeds of the generated designs.
const SEEDS: std::ops::Range<u64> = 0..200;

/// The rules that stop reading some operands (their doc states it: R12, an operand's error is
/// not carried once the operand is no longer read).
const DROPS: &[&str] = &["sampling_draws_to_nominal"];

/// Bindings per generated design.
const TRIALS: u64 = 8;

/// A code of this group.
pub const fn code(n: u16) -> ir::Code {
    ir::Code::new(b'U', n)
}

/// The registered pass `name`.
pub fn pass(name: &str) -> &'static passes::Pass {
    passes::all()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("{name} is not registered"))
}

/// Runs `name` on generated designs and compares every root it changed, under [`TRIALS`]
/// bindings each; returns how many changed roots held a node `hit` accepts.
pub fn keeps_generated_values(name: &str, hit: fn(ir::Expr) -> bool, follow: bool) -> usize {
    keeps_values_over(name, hit, follow, ir::FeatureSet::ALL)
}

/// [`keeps_generated_values`] over designs generated from `features`.
pub fn keeps_values_over(
    name: &str,
    hit: fn(ir::Expr) -> bool,
    follow: bool,
    features: ir::FeatureSet,
) -> usize {
    let p = pass(name);
    let mut compared = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &features, SIZE);
        let before = roots(&design);
        let mut diags = ir::Diags::new();
        (p.run)(&mut design, &mut diags);
        let after = roots(&design);
        assert_eq!(before.len(), after.len(), "seed {seed}: the roots stay");
        for (&old, &new) in before.iter().zip(&after) {
            if old == new {
                continue;
            }
            compared += usize::from(holds(&design, old, hit));
            for trial in 0..TRIALS {
                let env = Point {
                    design: &design,
                    trial: seed * TRIALS + trial,
                    follow,
                    depth: std::cell::Cell::new(0),
                };
                let was = ir::eval_typed(&design, old, &env);
                if DROPS.contains(&name) && dropped(&design, was, new) {
                    continue;
                }
                let got = differ(&design, was, ir::eval_typed(&design, new, &env));
                assert!(got.is_none(), "{name} seed {seed} trial {trial}: {got:?}");
            }
        }
    }
    compared
}

/// Reads `text` (one deck whose params `x` and `y` are the operands), runs `name`, and compares
/// every root it changed with `x` and `y` bound to each pair of the boundary set, under each
/// tag pair of `tags` (bit 0: `x` Int, bit 1: `y` Int; a non-whole value stays Real). Returns
/// the points compared.
pub fn keeps_boundary_values(name: &str, text: &str, tags: &[u8]) -> usize {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let before = roots(&design);
    (pass(name).run)(&mut design, &mut diags);
    let after = roots(&design);
    let changed: Vec<(ir::ExprId, ir::ExprId)> = before
        .into_iter()
        .zip(after)
        .filter(|(a, b)| a != b)
        .collect();
    assert!(!changed.is_empty(), "{name}: the pass changed no root");
    let set = boundary();
    let mut points = 0;
    for &x in &set {
        for &y in &set {
            for &tags in tags {
                let env = Pair {
                    design: &design,
                    x: tagged(x, tags & 1 == 1),
                    y: tagged(y, tags & 2 == 2),
                };
                if env.x.int != (tags & 1 == 1) || env.y.int != (tags & 2 == 2) {
                    continue; // a non-whole value has no Int binding
                }
                for &(old, new) in &changed {
                    points += 1;
                    let got = differ(
                        &design,
                        ir::eval_typed(&design, old, &env),
                        ir::eval_typed(&design, new, &env),
                    );
                    assert!(got.is_none(), "{name} x={x:e} y={y:e} tags {tags}: {got:?}");
                }
            }
        }
    }
    points
}

/// The notes `name` leaves on the fixture `case` of its folder, by code.
pub fn notes(name: &str, case: &str) -> Vec<ir::Code> {
    let path = format!(
        "{}/tests/fixtures/passes/expr/{name}/{case}.ir",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect("the fixture");
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(&text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    (pass(name).run)(&mut design, &mut diags);
    diags.rows().iter().map(|d| d.code).collect()
}

/// Every root `name` changes keeps its value on generated designs, its parameters bound to
/// their definitions; returns the changed roots that held a node `hit` accepts.
pub fn typed(name: &str, hit: fn(ir::Expr) -> bool) -> usize {
    let n = keeps_generated_values(name, hit, true);
    eprintln!("{name}: {n} generated roots");
    n
}

/// Every expression root of every reachable item, in `map_exprs` order.
pub fn roots(design: &ir::Design) -> Vec<ir::ExprId> {
    let mut copy = design.clone();
    let mut out = Vec::new();
    let items: Vec<ir::ItemRef> = copy.reachable().into_iter().map(|(i, _)| i).collect();
    let mut editor = copy.editor();
    for item in items {
        editor.map_exprs(item, &mut |e, _| out.push(*e));
    }
    out
}

// ---- private ----

/// Whether `was` is an error raised at a row the new tree no longer reads.
fn dropped(design: &ir::Design, was: Result<ir::Value, ir::Domain>, new: ir::ExprId) -> bool {
    let at = match was {
        Ok(_) => return false,
        Err(
            ir::Domain::DivideByZero(e)
            | ir::Domain::OutOfDomain(e)
            | ir::Domain::Unbound(e)
            | ir::Domain::CallDepth(e)
            | ir::Domain::NotANumber(e),
        ) => e,
    };
    let mut stack = vec![new];
    for _ in 0..=design.expr_count() * 4 {
        let Some(e) = stack.pop() else { return true };
        if e == at {
            return false;
        }
        match design.expr(e) {
            ir::Expr::Call(_, l)
            | ir::Expr::UserCall(_, l)
            | ir::Expr::Draw(_, _, _, l)
            | ir::Expr::List(l) => stack.extend_from_slice(design.exprs(l)),
            ir::Expr::Unary(_, a) => stack.push(a),
            ir::Expr::Binary(_, a, b) => stack.extend([a, b]),
            ir::Expr::Ternary(c, a, b) => stack.extend([c, a, b]),
            _ => {}
        }
    }
    panic!("the walk outgrew the table")
}

/// Whether `e` may compute an Int (the operators of A12 K1's table that keep one).
fn int_capable(e: ir::Expr) -> bool {
    use ir::BinOp::{Add, DivTyped, Mul, Sub};
    use ir::Func::{Max, Min, SgnZeroPos, ToInt};
    matches!(
        e,
        ir::Expr::Binary(Add | Sub | Mul | DivTyped, ..)
            | ir::Expr::Unary(ir::UnOp::Neg, _)
            | ir::Expr::Call(Min | Max | SgnZeroPos | ToInt, _)
    )
}

/// Whether the tree under `root` holds a node `hit` accepts (bounded by the row count).
fn holds(design: &ir::Design, root: ir::ExprId, hit: fn(ir::Expr) -> bool) -> bool {
    let mut stack = vec![root];
    for _ in 0..=design.expr_count() * 4 {
        let Some(e) = stack.pop() else { return false };
        if e.is_none() {
            continue;
        }
        let x = design.expr(e);
        if hit(x) {
            return true;
        }
        match x {
            ir::Expr::Call(_, l)
            | ir::Expr::UserCall(_, l)
            | ir::Expr::Draw(_, _, _, l)
            | ir::Expr::List(l) => stack.extend_from_slice(design.exprs(l)),
            ir::Expr::Unary(_, a) => stack.push(a),
            ir::Expr::Binary(_, a, b) => stack.extend([a, b]),
            ir::Expr::Ternary(c, a, b) => stack.extend([c, a, b]),
            _ => {}
        }
    }
    panic!("the walk outgrew the table")
}

/// `None` when the two results agree, else the pair: the same bits, or the same domain error
/// kind; the same tag while `after` holds a typed division (only `DivTyped` reads a tag, A12
/// K1). Without one, an Int beyond 2⁵³ (a domain error, R12: not carried) may become a Real:
/// an error at a node that may compute an Int may become a value or another error.
fn differ(
    after: &ir::Design,
    a: Result<ir::Value, ir::Domain>,
    b: Result<ir::Value, ir::Domain>,
) -> Option<String> {
    let used = ir::features_used(after);
    let typed = used.contains(ir::Feature::Binary(ir::BinOp::DivTyped))
        || used.contains(ir::Feature::BinaryDyn(ir::BinOp::DivTyped));
    let same = match (a, b) {
        (Ok(x), Ok(y)) => x.number.to_bits() == y.number.to_bits() && (!typed || x.int == y.int),
        (Err(ir::Domain::OutOfDomain(e)), _) if !typed && int_capable(after.expr(e)) => true,
        // R12: `pow` of a negative base and a fractional exponent is outside its domain, and
        // `pow_to_powabs` is exact on the domain only (the one rule stated so)
        (Err(ir::Domain::OutOfDomain(e)), _)
            if matches!(after.expr(e), ir::Expr::Call(ir::Func::Pow, _)) =>
        {
            true
        }
        (Err(x), Err(y)) => std::mem::discriminant(&x) == std::mem::discriminant(&y),
        _ => false,
    };
    (!same).then(|| format!("{a:?} before, {b:?} after"))
}

/// `v` Int when asked and whole within 2⁵³, else Real.
fn tagged(v: f64, int: bool) -> ir::Value {
    match int && v.fract() == 0.0 && v.abs() <= 9_007_199_254_740_992.0 {
        true => ir::Value::int(v),
        false => ir::Value::real(v),
    }
}
