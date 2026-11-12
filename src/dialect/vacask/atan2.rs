//! VACASK's `atan2(y, x)` → the IR expression of what VACASK computes (R76, finding F9): not C's
//! `atan2`, so never `Func::Atan2`. Settle atan2 (VACASK 36be560):
//! - in a parameter and a control `var` (the parameter evaluator, written out here): `atan(y/x)`, then `+ π`
//!   when `y < 0 ≤ x` and `- π` when `y < 0` and `x < 0`; so `atan2(1, -1)` is -0.785 (C:
//!   2.356), `atan2(0, -1)` is -0 (C: π), `atan2(-1, 0)` is π/2 (C: -π/2), `atan2(0, 0)` NaN;
//!   the arguments are real (`atan2(0, -1)` with integers is -0 too);
//! - in a behavioural source (the dynamic form): C's `atan2` (settle atan2_beh_origin: 0 at the
//!   origin), but for `y = -0` with `x < 0` (π; C: -π), a signed zero on the branch cut: R76a
//!   keeps it `FuncDyn(Atan2)`, the grid law citing the point.
//!
//! Each argument is written once per place it is read (no row shares a child); a call whose
//! argument holds a draw is refused by the caller (`ATAN2_DRAW`: copies would draw twice).
//!
//! Contents: `DRAW`, `CODES`, `atan2`, `literal_only`, `holds_draw` (crate-private)

use super::expr_read::names_read;
use super::number;
use super::read::Lower;
use super::tables;
use crate::ir;

/// An `atan2` whose argument holds a draw (R76: its expansion reads each argument more than once).
pub(super) const DRAW: ir::Code = ir::Code::new(b'P', 2015);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[(
    DRAW,
    ir::Info {
        title: "an atan2 whose argument holds a draw, kept as its text",
        cite: "R76; settle atan2 (VACASK's atan2 is no IR function: the reader writes it out)",
        explain: "VACASK's atan2 is read as an expression over atan, a division and conditions, which reads each argument more than once; a draw there would be drawn once per copy, so the statement is kept as text.",
        kind: ir::Kind::Note,
    },
)];

/// The expression the static `atan2(y, x)` means (a parameter, a control statement).
/// `arg(l, which)` builds a fresh copy of argument `which` (0: `y`, 1: `x`).
pub(super) fn atan2(
    l: &mut Lower<'_, '_, '_>,
    arg: &dyn Fn(&mut Lower<'_, '_, '_>, usize) -> ir::ExprId,
) -> ir::ExprId {
    let lit = |l: &mut Lower<'_, '_, '_>, text: &str| {
        l.b.literal(number::lex(text).expect("a literal").literal)
    };
    // `atan(y * 1. / x)`: VACASK's quotient is real whatever the arguments' types.
    let quotient = |l: &mut Lower<'_, '_, '_>| {
        let (y, x) = (arg(l, 0), arg(l, 1));
        let one = lit(l, "1.");
        let real = l.b.binary(ir::BinOp::Mul, y, one);
        let q = l.b.binary(ir::BinOp::DivTyped, real, x);
        l.b.call(ir::Func::Atan, &[q])
    };
    // At `x = 0` VACASK's quotient is ±inf and its atan ±π/2 by `y`'s sign (the IR's division
    // by zero is a domain error, so the axis is stated): `x == 0 ? (y > 0 ? π/2 : y < 0 ? -π/2
    // : atan(y/x)) : atan(y/x)`; a `-0` for `x` flips VACASK's sign (a signed zero on the
    // discontinuity, R76a's cited exception), the origin stays `0/0` (NaN on VACASK, a domain
    // error in the IR).
    let base = |l: &mut Lower<'_, '_, '_>| {
        let x = arg(l, 1);
        let zero = lit(l, "0");
        let on_axis = l.b.binary(ir::BinOp::Eq, x, zero);
        let y = arg(l, 0);
        let zero = lit(l, "0");
        let up = l.b.binary(ir::BinOp::Gt, y, zero);
        let y = arg(l, 0);
        let zero = lit(l, "0");
        let down = l.b.binary(ir::BinOp::Lt, y, zero);
        let half = lit(l, "1.5707963267948966");
        let half2 = lit(l, "1.5707963267948966");
        let minus_half = l.b.unary(ir::UnOp::Neg, half2);
        let origin = quotient(l);
        let below = l.b.ternary(down, minus_half, origin);
        let axis = l.b.ternary(up, half, below);
        let off = quotient(l);
        l.b.ternary(on_axis, axis, off)
    };
    let shifted = |l: &mut Lower<'_, '_, '_>, op: ir::BinOp| {
        let a = base(l);
        let pi = lit(l, "3.141592653589793");
        l.b.binary(op, a, pi)
    };
    let test = |l: &mut Lower<'_, '_, '_>, which: usize, op: ir::BinOp| {
        let v = arg(l, which);
        let zero = lit(l, "0");
        l.b.binary(op, v, zero)
    };
    // `cond ? atan(…) + π : atan(…) - π`.
    let either = |l: &mut Lower<'_, '_, '_>, which: usize, op: ir::BinOp| {
        let c = test(l, which, op);
        let plus = shifted(l, ir::BinOp::Add);
        let minus = shifted(l, ir::BinOp::Sub);
        l.b.ternary(c, plus, minus)
    };
    // y < 0 ? (x >= 0 ? a + π : a - π) : a
    let c = test(l, 0, ir::BinOp::Lt);
    let turned = either(l, 1, ir::BinOp::Ge);
    let a = base(l);
    l.b.ternary(c, turned, a)
}

/// Whether the expression at CST node `node` reads no name but a function's (`ddt` and `idt`
/// read the circuit): a behavioural source's expression of literals only, which VACASK folds
/// with its parameter evaluator (settle atan2_forms: `atan2(0.5, -0.5)` there is -0.785).
pub(super) fn literal_only(l: &Lower<'_, '_, '_>, node: u32) -> bool {
    let reads = |n: &String| tables::function(n).is_none() || matches!(n.as_str(), "ddt" | "idt");
    !names_read(l, node).iter().any(reads)
}

/// Whether any of the CST nodes `args` reads a draw (then [`DRAW`]: copies would draw twice).
pub(super) fn holds_draw(l: &Lower<'_, '_, '_>, args: &[u32]) -> bool {
    let draws = |&a: &u32| names_read(l, a).iter().any(|n| tables::draw(n).is_some());
    args.iter().any(draws)
}
