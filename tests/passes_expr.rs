//! The meaning of the expression passes (`src/passes/expr/`): each keeps every value.
//!
//! Oracle: the reference evaluator (`ir::eval_typed`), never the pass's own arithmetic. For each
//! root a pass rewrites, the old tree and the new one (both stay in the design: rows are only
//! appended) are evaluated under the same bindings and must agree bit for bit and tag for tag,
//! or both be a domain error of the same kind. Points: every root of generated designs
//! (`ir::gen::generate`) holding the pass's feature, under bindings drawn from the boundary set
//! (±0, k+½, k+½±1 ulp, 2⁵²±1, 2⁵³, the domain edges, Int and Real tags) and random finite
//! values; plus each pair of the boundary set as the operands of the rewritten node. Bindings are
//! finite: a non-finite operand lies outside every function's domain (R12).
//!
//! Contents: tests only.

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes;

/// A moderate design (the pass laws' size).
const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

/// Every tag pair of two operands.
const ANY: &[u8] = &[0, 1, 2, 3];

/// Seeds of the generated designs.
const SEEDS: std::ops::Range<u64> = 0..200;

/// Bindings per generated design.
const TRIALS: u64 = 8;

/// Every read answers a value drawn by (trial, name) from the boundary set or a random finite
/// number; a whole value within 2⁵³ is Int on odd draws.
struct Point<'d> {
    design: &'d ir::Design,
    trial: u64,
    /// A resolved parameter read is its definition's value, evaluated (a head default for a
    /// formal), as the tag table assumes: the typed passes decide by it.
    follow: bool,
    depth: std::cell::Cell<u32>,
}

/// `x` and `y` bound to two values, the temperature 27, everything else 1.
struct Pair<'d> {
    design: &'d ir::Design,
    x: ir::Value,
    y: ir::Value,
}

impl ir::Bindings for Point<'_> {
    fn param(&self, p: ir::Ref) -> Option<f64> {
        self.param_value(p).map(|v| v.number)
    }
    fn param_value(&self, p: ir::Ref) -> Option<ir::Value> {
        if !self.follow || !p.is_resolved() {
            return Some(self.value(self.design.str(p.name)));
        }
        let def = self.design.def(p.def);
        let value = match (def.kind, def.item.kind) {
            (ir::DefKind::Param, ir::ItemKind::Param) => {
                self.design.row::<ir::item::Param>(def.item.index).value
            }
            (ir::DefKind::Param, ir::ItemKind::Subckt) => {
                let s: ir::item::Subckt = self.design.row(def.item.index);
                let given = self
                    .design
                    .params(s.params)
                    .iter()
                    .find(|q| q.0 == def.name);
                given.map_or(ir::ExprId::NONE, |q| q.1)
            }
            _ => return Some(ir::Value::real(self.value(self.design.str(p.name)).number)),
        };
        if value.is_none() || self.depth.get() > 64 {
            return None;
        }
        self.depth.set(self.depth.get() + 1);
        let v = ir::eval_typed(self.design, value, self).ok();
        self.depth.set(self.depth.get() - 1);
        v
    }
    fn voltage(&self, net: ir::NameId, _: ir::NameId) -> Option<f64> {
        Some(self.value(self.design.str(net)).number)
    }
    fn current(&self, element: ir::NameId) -> Option<f64> {
        Some(self.value(self.design.str(element)).number)
    }
    fn time(&self) -> Option<f64> {
        Some(self.value("$time").number)
    }
    fn temp(&self) -> Option<f64> {
        Some(self.value("$temp").number)
    }
    fn freq(&self) -> Option<f64> {
        Some(self.value("$freq").number)
    }
    fn tstep(&self) -> Option<f64> {
        Some(self.value("$tstep").number)
    }
    fn tstop(&self) -> Option<f64> {
        Some(self.value("$tstop").number)
    }
    fn tnom(&self) -> Option<f64> {
        Some(self.value("$tnom").number)
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, args: &[f64]) -> Option<f64> {
        Some(args[0])
    }
}

impl ir::Bindings for Pair<'_> {
    fn param(&self, p: ir::Ref) -> Option<f64> {
        self.param_value(p).map(|v| v.number)
    }
    fn param_value(&self, p: ir::Ref) -> Option<ir::Value> {
        Some(match self.design.str(p.name) {
            "x" => self.x,
            "y" => self.y,
            _ => ir::Value::real(1.0),
        })
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        Some(1.0)
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        Some(1.0)
    }
    fn time(&self) -> Option<f64> {
        Some(1.0)
    }
    fn temp(&self) -> Option<f64> {
        Some(27.0)
    }
    fn freq(&self) -> Option<f64> {
        Some(1.0)
    }
    fn tstep(&self) -> Option<f64> {
        Some(1.0)
    }
    fn tstop(&self) -> Option<f64> {
        Some(1.0)
    }
    fn tnom(&self) -> Option<f64> {
        Some(27.0)
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, args: &[f64]) -> Option<f64> {
        Some(args[0])
    }
}

// ---- private ----

/// A code of this group.
const fn code(n: u16) -> ir::Code {
    ir::Code::new(b'U', n)
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

impl Point<'_> {
    fn value(&self, name: &str) -> ir::Value {
        let mut h = self.trial.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
        for b in name.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3);
        }
        h ^= h >> 29;
        let set = boundary();
        let number = match h % 3 {
            0 => (h >> 8) as f64 / (1u64 << 52) as f64 * 8.0 - 4.0,
            _ => set[(h >> 8) as usize % set.len()],
        };
        let whole = number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_992.0;
        match whole && (h >> 40) & 1 == 1 {
            true => ir::Value::int(number),
            false => ir::Value::real(number),
        }
    }
}

/// The boundary set: zeros, halves and their neighbours, 2⁵² ± 1, 2⁵³, the ends of the doubles.
fn boundary() -> Vec<f64> {
    let p52 = 4_503_599_627_370_496.0f64;
    let mut out = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        2.0,
        3.0,
        -3.0,
        0.5,
        -0.5,
        1.5,
        2.5,
        -2.5,
        2.5f64.next_up(),
        2.5f64.next_down(),
        0.5f64.next_down(),
        -(0.5f64.next_down()),
        1.0 / 3.0,
        p52 - 1.0,
        p52,
        p52 + 1.0,
        -(p52 + 1.0),
        2.0 * p52,
        1e308,
        f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
        -1e-300,
    ];
    out.extend([1.0f64, -1.0].map(|s| s * (1.0f64).next_up()));
    out
}

/// The registered pass `name`.
fn pass(name: &str) -> &'static passes::Pass {
    passes::all()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("{name} is not registered"))
}

/// Every expression root of every reachable item, in `map_exprs` order.
fn roots(design: &ir::Design) -> Vec<ir::ExprId> {
    let mut copy = design.clone();
    let mut out = Vec::new();
    let items: Vec<ir::ItemRef> = copy.reachable().into_iter().map(|(i, _)| i).collect();
    let mut editor = copy.editor();
    for item in items {
        editor.map_exprs(item, &mut |e, _| out.push(*e));
    }
    out
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
        (Err(x), Err(y)) => std::mem::discriminant(&x) == std::mem::discriminant(&y),
        _ => false,
    };
    (!same).then(|| format!("{a:?} before, {b:?} after"))
}

/// Runs `name` on generated designs and compares every root it changed, under [`TRIALS`]
/// bindings each; returns how many changed roots held a node `hit` accepts.
fn keeps_generated_values(name: &str, hit: fn(ir::Expr) -> bool, follow: bool) -> usize {
    let p = pass(name);
    let mut compared = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
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
                let got = differ(
                    &design,
                    ir::eval_typed(&design, old, &env),
                    ir::eval_typed(&design, new, &env),
                );
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
fn keeps_boundary_values(name: &str, text: &str, tags: &[u8]) -> usize {
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

/// `v` Int when asked and whole within 2⁵³, else Real.
fn tagged(v: f64, int: bool) -> ir::Value {
    match int && v.fract() == 0.0 && v.abs() <= 9_007_199_254_740_992.0 {
        true => ir::Value::int(v),
        false => ir::Value::real(v),
    }
}

fn is_powabs(e: ir::Expr) -> bool {
    matches!(e, ir::Expr::Call(ir::Func::PowAbs, _))
}

/// The notes `name` leaves on the fixture `case` of its folder, by code.
fn notes(name: &str, case: &str) -> Vec<ir::Code> {
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
fn typed(name: &str, hit: fn(ir::Expr) -> bool) -> usize {
    let n = keeps_generated_values(name, hit, true);
    eprintln!("{name}: {n} generated roots");
    n
}

#[test]
fn powabs_to_pow_keeps_every_value() {
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = pow_abs(x, y)\n";
    let points = keeps_boundary_values("powabs_to_pow", text, ANY);
    let generated = keeps_generated_values("powabs_to_pow", is_powabs, false);
    assert!(generated > 0, "no generated root held pow_abs");
    eprintln!("powabs_to_pow: {points} boundary points, {generated} generated roots");
}

#[test]
fn powabs_to_pow_dyn_keeps_every_value() {
    // `$temp` makes the root dynamic (class Temp, R14); `0 * $temp` adds nothing to y
    let text =
        "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = pow_abs(x, y + 0 * $temp)\n";
    let points = keeps_boundary_values("powabs_to_pow_dyn", text, ANY);
    let generated = keeps_generated_values("powabs_to_pow_dyn", is_powabs, false);
    assert!(generated > 0, "no generated root held a dynamic pow_abs");
    eprintln!("powabs_to_pow_dyn: {points} boundary points, {generated} generated roots");
}

#[test]
fn div_to_div_typed_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Binary(ir::BinOp::Div, ..));
    for (x, y, tags) in [
        ("1i", "2i", &[3u8][..]),
        ("1", "2i", &[2]),
        ("1", "1", &[0]),
    ] {
        let text = format!(
            "file \"p.sp\" deck\n  param x = {x}\n  param y = {y}\n  param r = x / y\n  element behav b1 (n 0) expr=(x / y + v(n))\n"
        );
        for name in ["div_to_div_typed", "div_to_div_typed_dyn"] {
            let points = keeps_boundary_values(name, &text, tags);
            eprintln!("{name} ({x}, {y}): {points} boundary points");
        }
    }
    assert!(typed("div_to_div_typed", hit) > 0);
    assert!(typed("div_to_div_typed_dyn", hit) > 0);
}

#[test]
fn div_typed_to_div_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Binary(ir::BinOp::DivTyped, ..));
    for (x, y, tags) in [
        ("1i", "2i", &[3u8][..]),
        ("1", "2i", &[2]),
        ("1i", "1", &[1]),
    ] {
        let text = format!(
            "file \"p.sp\" deck\n  param x = {x}\n  param y = {y}\n  param r = x /: y\n  element behav b1 (n 0) expr=(x /: y + v(n))\n"
        );
        for name in ["div_typed_to_div", "div_typed_to_div_dyn"] {
            let points = keeps_boundary_values(name, &text, tags);
            eprintln!("{name} ({x}, {y}): {points} boundary points");
        }
    }
    assert!(typed("div_typed_to_div", hit) > 0);
    assert!(typed("div_typed_to_div_dyn", hit) > 0);
}

#[test]
fn a_division_of_mixed_operand_types_is_noted_and_so_is_every_int_quotient() {
    let got = notes("div_typed_to_div", "mixed");
    assert!(
        !got.is_empty() && got.iter().all(|&c| c == code(101)),
        "{got:?}"
    );
}

#[test]
fn toint_to_trunc_keeps_every_value_and_waits_for_the_typed_divisions() {
    let hit = |e| matches!(e, ir::Expr::Call(ir::Func::ToInt, _));
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = to_int(x) + to_int(y)\n  element behav b1 (n 0) expr=(to_int(x) + v(n))\n";
    for name in ["toint_to_trunc", "toint_to_trunc_dyn"] {
        eprintln!(
            "{name}: {} boundary points",
            keeps_boundary_values(name, text, &[0])
        );
    }
    assert!(typed("toint_to_trunc", hit) > 0);
    assert!(typed("toint_to_trunc_dyn", hit) > 0);
    assert_eq!(notes("toint_to_trunc", "typed_division_left"), [code(103)]);
}

#[test]
fn toreal_to_mul_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Call(ir::Func::ToReal, _));
    for (x, tags) in [("1i", &[1u8, 3][..]), ("1", &[0, 2])] {
        let text = format!(
            "file \"p.sp\" deck\n  param x = {x}\n  param y = 1\n  param r = to_real(x) /: 2i\n  element behav b1 (n 0) expr=(to_real(x) /: 2i + v(n))\n"
        );
        for name in ["toreal_to_mul", "toreal_to_mul_dyn"] {
            let points = keeps_boundary_values(name, &text, tags);
            eprintln!("{name} ({x}): {points} boundary points");
        }
    }
    assert!(typed("toreal_to_mul", hit) > 0);
    assert!(typed("toreal_to_mul_dyn", hit) > 0);
}

#[test]
fn sgnzeropos_to_ternary_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Call(ir::Func::SgnZeroPos, _));
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = sgn_zero_pos(x) /: 2i\n  element behav b1 (n 0) expr=(sgn_zero_pos(x) /: 2i + v(n))\n";
    for name in ["sgnzeropos_to_ternary", "sgnzeropos_to_ternary_dyn"] {
        eprintln!(
            "{name}: {} boundary points",
            keeps_boundary_values(name, text, &[0, 2])
        );
    }
    assert!(typed("sgnzeropos_to_ternary", hit) > 0);
    assert!(typed("sgnzeropos_to_ternary_dyn", hit) > 0);
    assert_eq!(notes("sgnzeropos_to_ternary", "typed"), [code(103)]);
}
