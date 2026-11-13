//! How the expression passes' proofs compare: a pass run on generated designs or on a text, and
//! each root it changed evaluated before and after (`ir::eval_typed`) under the same bindings.
//!
//! Contents: `pub const ANY`, `pub const fn code`, `pub fn pass`, `pub fn keeps_generated_values`, `pub fn keeps_values_over`,
//! `pub fn keeps_boundary_values`, `pub fn notes`, `pub fn typed`, `pub fn roots`, `pub fn pairs`, `pub fn paths`, `pub fn keeps_folded`,
//! `pub fn keeps_every_root_value`

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

/// The rules after which an operand is evaluated only where it is read (`inline_funcs`: ngspice's
/// textual rule), so an error the original raised need not be raised (R12).
const LAZY: &[&str] = &["inline_funcs"];

/// The rules that drop a tag no operator reads (their doc states which nodes read one): their
/// changed roots are compared by value only.
const VALUE_ONLY: &[&str] = &["int_literal_to_plain"];

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
        for (old, new) in pairs(&before, &after) {
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
                // R12: the copy reads an argument where the body does; the call bound every
                // argument first, so its error need not be the copy's
                if LAZY.contains(&name) && was.is_err() {
                    continue;
                }
                let now = ir::eval_typed(&design, new, &env);
                let got = differ(&design, was, now, !VALUE_ONLY.contains(&name));
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
    let changed: Vec<(ir::ExprId, ir::ExprId)> = pairs(&before, &after)
        .into_iter()
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
                        !VALUE_ONLY.contains(&name),
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
pub fn roots(design: &ir::Design) -> Vec<(ir::ItemRef, Vec<ir::ExprId>)> {
    let mut copy = design.clone();
    let items: Vec<ir::ItemRef> = copy.reachable().into_iter().map(|(i, _)| i).collect();
    let mut editor = copy.editor();
    let mut out = Vec::new();
    for item in items {
        let mut row = Vec::new();
        editor.map_exprs(item, &mut |e, _| row.push(*e));
        out.push((item, row));
    }
    out
}

/// The (before, after) root pairs of the items reachable in both (a pass may take items out:
/// `inline_funcs` drops the functions it inlined), field by field.
pub fn pairs(
    before: &[(ir::ItemRef, Vec<ir::ExprId>)],
    after: &[(ir::ItemRef, Vec<ir::ExprId>)],
) -> Vec<(ir::ExprId, ir::ExprId)> {
    let mut out = Vec::new();
    for (item, new) in after {
        let Some((_, old)) = before.iter().find(|(i, _)| i == item) else {
            continue;
        };
        assert_eq!(old.len(), new.len(), "an item keeps its fields");
        out.extend(old.iter().copied().zip(new.iter().copied()));
    }
    out
}

/// The rows under `e` counted by paths (what a writer spells), saturating; each row visited
/// once.
pub fn paths(design: &ir::Design, e: ir::ExprId) -> u64 {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![e];
    while let Some(r) = stack.pop() {
        if r.is_none() || !seen.insert(r) {
            continue;
        }
        rows.push(r);
        stack.extend(operands(design, r));
    }
    rows.sort_unstable();
    let mut size = std::collections::HashMap::new();
    for &r in &rows {
        let below = operands(design, r)
            .iter()
            .fold(1u64, |n, k| n.saturating_add(size[k]));
        size.insert(r, below);
    }
    size[&e]
}

/// The points `name` folded in `text` (0 when it left the operator), each compared.
pub fn keeps_folded(name: &str, text: &str) -> usize {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    let before = ir::features_used(&design);
    let mut copy = design.clone();
    (pass(name).run)(&mut copy, &mut diags);
    match ir::features_used(&copy) == before {
        true => 0,
        false => keeps_boundary_values(name, text, &[0]),
    }
}

/// Runs `name` on generated designs and compares every root, changed or not, by value: the
/// old root evaluated in a copy of the design before the pass, the new one in the design after
/// (a pass that rewrites a definition's value changes what an unchanged root reads), under the
/// same [`TRIALS`] bindings, parameters bound to their definitions. Tags are not compared (a
/// rule that drops a tag no operator reads); an Int result out of range before may be a value
/// after (R12). Returns how many roots changed.
pub fn keeps_every_root_value(name: &str) -> usize {
    let p = pass(name);
    let mut changed = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        let before = design.clone();
        let mut diags = ir::Diags::new();
        (p.run)(&mut design, &mut diags);
        for (old, new) in pairs(&roots(&before), &roots(&design)) {
            assert_eq!(
                old.is_none(),
                new.is_none(),
                "{name}: a field kept given or absent"
            );
            if old.is_none() {
                continue;
            }
            changed += usize::from(old != new);
            for trial in 0..TRIALS {
                let at = |d| Point {
                    design: d,
                    trial: seed * TRIALS + trial,
                    follow: true,
                    depth: std::cell::Cell::new(0),
                };
                let was = ir::eval_typed(&before, old, &at(&before));
                let now = ir::eval_typed(&design, new, &at(&design));
                let same = match (was, now) {
                    (Ok(x), Ok(y)) => x.number.to_bits() == y.number.to_bits(),
                    // R12: an Int result out of range is outside the domain, so what the
                    // widened tree does there (a value, or its own error) is not carried
                    (Err(ir::Domain::OutOfDomain(e)), _) => {
                        int_capable(before.expr(e))
                            || matches!(now, Err(ir::Domain::OutOfDomain(_)))
                    }
                    (Err(x), Err(y)) => std::mem::discriminant(&x) == std::mem::discriminant(&y),
                    _ => false,
                };
                assert!(
                    same,
                    "{name} seed {seed} trial {trial}: {was:?} before, {now:?} after"
                );
            }
        }
    }
    changed
}

// ---- private ----

/// The operands of `e`.
fn operands(design: &ir::Design, e: ir::ExprId) -> Vec<ir::ExprId> {
    match design.expr(e) {
        ir::Expr::Call(_, l)
        | ir::Expr::UserCall(_, l)
        | ir::Expr::Draw(_, _, _, l)
        | ir::Expr::List(l) => design.exprs(l).to_vec(),
        ir::Expr::Unary(_, a) => vec![a],
        ir::Expr::Binary(_, a, b) => vec![a, b],
        ir::Expr::Ternary(c, a, b) => vec![c, a, b],
        _ => Vec::new(),
    }
}

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
    let mut seen = std::collections::HashSet::new();
    for _ in 0..=2 * design.expr_count() + 1 {
        let Some(e) = stack.pop() else { return true };
        if e == at {
            return false;
        }
        if !seen.insert(e) {
            continue;
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
    let mut seen = std::collections::HashSet::new();
    for _ in 0..=2 * design.expr_count() + 1 {
        let Some(e) = stack.pop() else { return false };
        if e.is_none() || !seen.insert(e) {
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
    tags: bool,
) -> Option<String> {
    let used = ir::features_used(after);
    let typed = tags && used.contains(ir::Feature::Binary(ir::BinOp::DivTyped))
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
