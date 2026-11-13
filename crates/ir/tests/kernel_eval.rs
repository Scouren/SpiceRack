//! The reference evaluator: every `Func`, `BinOp` and `UnOp` at hand-worked points, each stated
//! domain edge, each native-meaning pair where its members differ, short-circuiting, user calls,
//! and every binding.
//!
//! Oracle: the value derived by hand in each case's comment (spec example), never computed by
//! calling the function another way. Transcendental results are compared within 1e-12 relative
//! (the literal in the test is the value's first 17 digits); everything else exactly.
//!
//! Contents: none (tests only).

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2};

use spicerack_ir::{
    BinOp, Bindings, Builder, Design, Dist, Domain, DrawPer, DrawWhen, ExprId, Func, NameId, Ref,
    Role, Span, UnOp, eval,
};

/// Bindings with fixed values: every read answers a number derived from what it names, or
/// `None` when the test asks for an unbound context.
struct Fixed {
    bound: bool,
}

const BOUND: Fixed = Fixed { bound: true };

impl Bindings for Fixed {
    fn param(&self, param: Ref) -> Option<f64> {
        self.bound.then_some(10.0 + param.name.0 as f64)
    }
    fn voltage(&self, net: NameId, reference: NameId) -> Option<f64> {
        let r = if reference.is_none() { 0.0 } else { 0.5 };
        self.bound.then_some(net.0 as f64 - r)
    }
    fn current(&self, element: NameId) -> Option<f64> {
        self.bound.then_some(-(element.0 as f64))
    }
    fn time(&self) -> Option<f64> {
        self.bound.then_some(1e-9)
    }
    fn temp(&self) -> Option<f64> {
        self.bound.then_some(27.0)
    }
    fn freq(&self) -> Option<f64> {
        self.bound.then_some(1e6)
    }
    fn tstep(&self) -> Option<f64> {
        self.bound.then_some(1e-12)
    }
    fn tstop(&self) -> Option<f64> {
        self.bound.then_some(1e-6)
    }
    fn tnom(&self) -> Option<f64> {
        self.bound.then_some(25.0)
    }
    fn draw(&self, dist: Dist, when: DrawWhen, per: DrawPer, args: &[f64]) -> Option<f64> {
        // the nominal: the first argument, marked by the enums so the test sees they arrived
        let mark = dist as u8 as f64 * 100.0 + when as u8 as f64 * 10.0 + per as u8 as f64;
        self.bound.then_some(args[0] + mark)
    }
}

// ---- private ----

/// Builds one expression in a fresh design and evaluates it.
fn ev(build: impl FnOnce(&mut Builder<'_>) -> ExprId) -> Result<f64, Domain> {
    let mut d = Design::new();
    let e = build(&mut d.builder());
    eval(&d, e, &BOUND)
}

fn call(f: Func, args: &[f64]) -> Result<f64, Domain> {
    ev(|b| {
        let xs: Vec<ExprId> = args.iter().map(|&x| b.num(x)).collect();
        b.call(f, &xs)
    })
}

fn bin(op: BinOp, x: f64, y: f64) -> Result<f64, Domain> {
    ev(|b| {
        let (x, y) = (b.num(x), b.num(y));
        b.binary(op, x, y)
    })
}

fn un(op: UnOp, x: f64) -> Result<f64, Domain> {
    ev(|b| {
        let x = b.num(x);
        b.unary(op, x)
    })
}

fn near(got: Result<f64, Domain>, want: f64) {
    let v = got.unwrap_or_else(|e| panic!("want {want}, got {e:?}"));
    let tol = 1e-12 * want.abs().max(1e-300);
    assert!(
        (v - want).abs() <= tol || (want == 0.0 && v.abs() < 1e-15),
        "want {want}, got {v}"
    );
}

fn exact(got: Result<f64, Domain>, want: f64) {
    assert_eq!(got, Ok(want));
}

fn out_of_domain(got: Result<f64, Domain>) {
    assert!(
        matches!(got, Err(Domain::OutOfDomain(_))),
        "want OutOfDomain, got {got:?}"
    );
}

#[test]
fn power_laws_differ_at_a_negative_base() {
    exact(call(Func::Pow, &[-2.0, 3.0]), -8.0); // (−1)³·2³
    out_of_domain(call(Func::Pow, &[-8.0, 1.0 / 3.0])); // fractional power of a negative
    out_of_domain(call(Func::Pow, &[0.0, -1.0])); // 0 to a negative power
    near(call(Func::Pow, &[2.0, 0.5]), SQRT_2); // √2
    exact(call(Func::PowAbs, &[-2.0, 3.0]), 8.0); // |−2|³
    near(call(Func::PowAbs, &[-8.0, 1.0 / 3.0]), 2.0); // |−8|^(1/3)
    out_of_domain(call(Func::PowAbs, &[0.0, -1.0]));
    exact(call(Func::PowRe, &[-2.0, 3.0]), -8.0); // equals Pow where Pow is defined
    near(call(Func::PowRe, &[-8.0, 1.0 / 3.0]), 1.0); // 8^(1/3)·cos(π/3) = 2 · 0.5
    near(call(Func::PowRe, &[-4.0, 0.5]), 0.0); // 2·cos(π/2)
}

#[test]
fn logs_roots_and_exponentials() {
    exact(call(Func::Abs, &[-2.0]), 2.0);
    exact(call(Func::Sqrt, &[4.0]), 2.0);
    out_of_domain(call(Func::Sqrt, &[-1.0]));
    exact(call(Func::Exp, &[0.0]), 1.0);
    out_of_domain(call(Func::Exp, &[1000.0])); // overflows: not finite
    near(call(Func::ExpLin14, &[14.0]), 1_202_604.284_164_776_8); // e¹⁴
    near(call(Func::ExpLin14, &[15.0]), 2_405_208.568_329_553_6); // e¹⁴·(1 + 15 − 14) = 2e¹⁴
    near(call(Func::Exp, &[15.0]), 3_269_017.372_472_110_6); // e¹⁵: the pair differs past 14
    exact(call(Func::Ln, &[1.0]), 0.0);
    out_of_domain(call(Func::Ln, &[0.0]));
    out_of_domain(call(Func::Ln, &[-1.0]));
    near(call(Func::Log10, &[1000.0]), 3.0);
    out_of_domain(call(Func::Log10, &[-1.0]));
    out_of_domain(call(Func::Log10, &[0.0])); // never ln|x| or -inf
    exact(call(Func::Sqrt, &[0.0]), 0.0); // the edge is in the domain
}

#[test]
fn roundings_and_steps() {
    exact(call(Func::Floor, &[-1.5]), -2.0);
    exact(call(Func::Ceil, &[-1.5]), -1.0);
    exact(call(Func::Trunc, &[-1.5]), -1.0);
    exact(call(Func::RoundHalfAway, &[2.5]), 3.0);
    exact(call(Func::RoundHalfAway, &[-2.5]), -3.0);
    exact(call(Func::RoundHalfEven, &[2.5]), 2.0);
    exact(call(Func::RoundHalfEven, &[3.5]), 4.0);
    exact(call(Func::RoundHalfEven, &[-2.5]), -2.0);
    exact(call(Func::Sgn, &[-3.0]), -1.0);
    exact(call(Func::Sgn, &[0.0]), 0.0);
    exact(call(Func::Sgn, &[0.1]), 1.0);
    exact(call(Func::Ramp, &[-1.0]), 0.0);
    exact(call(Func::Ramp, &[2.0]), 2.0);
    exact(call(Func::Min, &[1.0, 2.0]), 1.0);
    exact(call(Func::Max, &[1.0, 2.0]), 2.0);
    exact(call(Func::Min, &[3.0, 2.5]), 2.5); // mixed: the value unchanged (its type: kernel_typed)
    exact(call(Func::Clamp, &[5.0, 0.0, 2.0]), 2.0);
    exact(call(Func::Clamp, &[-5.0, 0.0, 2.0]), 0.0);
    out_of_domain(call(Func::Clamp, &[1.0, 2.0, 0.0])); // lo > hi
    exact(call(Func::Fmod, &[7.0, 3.0]), 1.0);
    exact(call(Func::Fmod, &[-7.0, 3.0]), -1.0); // the dividend's sign
    assert!(matches!(
        call(Func::Fmod, &[1.0, 0.0]),
        Err(Domain::DivideByZero(_))
    ));
    exact(call(Func::Hypot, &[3.0, 4.0]), 5.0);
}

#[test]
fn trigonometry_and_its_domains() {
    exact(call(Func::Sin, &[0.0]), 0.0);
    exact(call(Func::Cos, &[0.0]), 1.0);
    exact(call(Func::Tan, &[0.0]), 0.0);
    near(call(Func::Asin, &[1.0]), FRAC_PI_2);
    out_of_domain(call(Func::Asin, &[1.5]));
    exact(call(Func::Acos, &[1.0]), 0.0);
    out_of_domain(call(Func::Acos, &[-1.5]));
    near(call(Func::Atan, &[1.0]), FRAC_PI_4);
    near(call(Func::Atan2, &[1.0, -1.0]), 3.0 * FRAC_PI_4); // second quadrant
    exact(call(Func::Atan2, &[0.0, 0.0]), 0.0);
    near(call(Func::Atan2, &[0.0, -1.0]), PI);
    // R76: the negative x axis and the third quadrant (VACASK's atan2 differs on the first:
    // tests/fixtures/funcs/atan2_vacask.out prints -0 for atan2(0, -1), -0.785398 for (1, -1)).
    // A literal -0 is the one zero (`b.num(-0.0)` is 0); a computed one, `-(0)`, keeps its sign
    let at_neg_zero = |x: f64| {
        ev(|b| {
            let zero = b.num(0.0);
            let y = b.unary(UnOp::Neg, zero);
            let x = b.num(x);
            b.call(Func::Atan2, &[y, x])
        })
    };
    near(call(Func::Atan2, &[-0.0, -1.0]), PI); // the literal: +0
    near(at_neg_zero(-1.0), -PI);
    near(call(Func::Atan2, &[-1.0, -1.0]), -3.0 * FRAC_PI_4);
    let below = at_neg_zero(1.0).unwrap();
    assert!(
        below == 0.0 && below.is_sign_negative(),
        "atan2(-0, 1) is -0, got {below}"
    );
    exact(call(Func::Sinh, &[0.0]), 0.0);
    exact(call(Func::Cosh, &[0.0]), 1.0);
    exact(call(Func::Tanh, &[0.0]), 0.0);
    exact(call(Func::Asinh, &[0.0]), 0.0);
    exact(call(Func::Acosh, &[1.0]), 0.0);
    out_of_domain(call(Func::Acosh, &[0.5]));
    exact(call(Func::Atanh, &[0.0]), 0.0);
    out_of_domain(call(Func::Atanh, &[1.0]));
}

#[test]
fn pwl_interpolates_and_holds_its_ends() {
    let pts = [1.0, 10.0, 2.0, 20.0];
    let at = |x: f64| call(Func::Pwl, &[&[x][..], &pts].concat());
    exact(at(1.5), 15.0); // halfway between (1, 10) and (2, 20)
    exact(at(0.0), 10.0); // held below the first point
    exact(at(3.0), 20.0); // held above the last
    exact(at(2.0), 20.0);
    out_of_domain(call(Func::Pwl, &[1.5, 2.0, 10.0, 1.0, 20.0])); // x not increasing
    out_of_domain(call(Func::Pwl, &[1.5, 1.0, 10.0, 2.0])); // a point without its y
}

#[test]
fn the_circuit_readers_need_bindings() {
    assert!(
        matches!(call(Func::Ddt, &[1.0]), Err(Domain::Unbound(_))),
        "no time history"
    );
    assert!(matches!(call(Func::Idt, &[1.0]), Err(Domain::Unbound(_))));
}

#[test]
fn binary_operators() {
    exact(bin(BinOp::Add, 7.0, 2.0), 9.0);
    exact(bin(BinOp::Sub, 7.0, 2.0), 5.0);
    exact(bin(BinOp::Mul, 7.0, 2.0), 14.0);
    exact(bin(BinOp::Div, 7.0, 2.0), 3.5);
    assert!(matches!(
        bin(BinOp::Div, 1.0, 0.0),
        Err(Domain::DivideByZero(_))
    ));
    exact(bin(BinOp::Eq, 2.0, 2.0), 1.0);
    exact(bin(BinOp::Ne, 2.0, 2.0), 0.0);
    exact(bin(BinOp::Lt, 1.0, 2.0), 1.0);
    exact(bin(BinOp::Le, 2.0, 2.0), 1.0);
    exact(bin(BinOp::Gt, 1.0, 2.0), 0.0);
    exact(bin(BinOp::Ge, 2.0, 2.0), 1.0);
    exact(bin(BinOp::And, 2.0, 0.0), 0.0);
    exact(bin(BinOp::And, 2.0, 3.0), 1.0); // non-zero is true, the result is 0 or 1
    exact(bin(BinOp::Or, 0.0, 0.0), 0.0);
    exact(bin(BinOp::Or, 0.0, -2.0), 1.0);
    exact(bin(BinOp::BitAnd, 6.0, 3.0), 2.0); // 110 & 011
    exact(bin(BinOp::BitOr, 6.0, 3.0), 7.0);
    exact(bin(BinOp::BitXor, 6.0, 3.0), 5.0);
    out_of_domain(bin(BinOp::BitAnd, 1.5, 1.0));
    exact(bin(BinOp::Shl, 1.0, 4.0), 16.0);
    exact(bin(BinOp::Shr, -16.0, 2.0), -4.0); // arithmetic
    out_of_domain(bin(BinOp::Shl, 1.0, 64.0));
}

#[test]
fn unary_operators() {
    exact(un(UnOp::Neg, 2.0), -2.0);
    exact(un(UnOp::Not, 0.0), 1.0);
    exact(un(UnOp::Not, 3.0), 0.0);
    exact(un(UnOp::BitNot, 0.0), -1.0); // !0 in two's complement
    out_of_domain(un(UnOp::BitNot, 0.5));
}

#[test]
fn only_the_chosen_branch_is_evaluated() {
    // 1 ? 2 : ln(-1) = 2; 0 ? ln(-1) : 3 = 3; And/Or skip a right side that would fail
    let ternary = |c: f64| {
        ev(|b| {
            let (c, two, m1, three) = (b.num(c), b.num(2.0), b.num(-1.0), b.num(3.0));
            let bad = b.call(Func::Ln, &[m1]);
            let first = b.ternary(c, two, bad);
            let zero = b.num(0.0);
            let second = b.ternary(zero, bad, three);
            b.binary(BinOp::Add, first, second)
        })
    };
    exact(ternary(1.0), 5.0);
    assert!(
        matches!(ternary(0.0), Err(Domain::OutOfDomain(_))),
        "0 picks ln(-1)"
    );
    let skip = |op: BinOp, left: f64| {
        ev(|b| {
            let (l, one, zero) = (b.num(left), b.num(1.0), b.num(0.0));
            let bad = b.binary(BinOp::Div, one, zero);
            b.binary(op, l, bad)
        })
    };
    exact(skip(BinOp::And, 0.0), 0.0);
    exact(skip(BinOp::Or, 1.0), 1.0);
    assert!(matches!(
        skip(BinOp::And, 1.0),
        Err(Domain::DivideByZero(_))
    ));
}

#[test]
fn user_functions_bind_their_arguments() {
    let mut d = Design::new();
    let (sq, rec, unresolved) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        // sq(x) = x * x
        let x = b.param_ref("x");
        let body = b.binary(BinOp::Mul, x, x);
        b.func(top, "sq", &["x"], body, Span::NONE);
        // g(y) = g(y): infinite recursion
        let y = b.param_ref("y");
        let inner = b.user_call("g", &[y]);
        b.func(top, "g", &["y"], inner, Span::NONE);
        let sq_def =
            b.design()
                .lookup(top, spicerack_ir::DefKind::Func, b.design().find_name("sq"));
        let g_def = b
            .design()
            .lookup(top, spicerack_ir::DefKind::Func, b.design().find_name("g"));
        b.resolve(inner, g_def);
        let three = b.num(3.0);
        let sq = b.user_call("sq", &[three]);
        b.resolve(sq, sq_def);
        let one = b.num(1.0);
        let rec = b.user_call("g", &[one]);
        b.resolve(rec, g_def);
        let unresolved = b.user_call("h", &[one]);
        (sq, rec, unresolved)
    };
    assert_eq!(eval(&d, sq, &BOUND), Ok(9.0)); // 3 * 3
    assert!(matches!(eval(&d, rec, &BOUND), Err(Domain::CallDepth(_))));
    assert_eq!(
        eval(&d, unresolved, &BOUND),
        Err(Domain::Unbound(unresolved))
    );
}

#[test]
fn bindings_answer_the_reads() {
    let mut d = Design::new();
    let reads = {
        let mut b = d.builder();
        let p = b.param_ref("p");
        let v1 = b.v("out", None);
        let v2 = b.v("out", Some("ref"));
        let i = b.i("v1");
        let one = b.num(1.0);
        let draw = b.draw(
            Dist::AbsUnif,
            DrawWhen::Sampling,
            DrawPer::Instance,
            &[one, one],
        );
        let leaves = [b.time(), b.temp(), b.freq(), b.tstep(), b.tstop(), b.tnom()];
        (p, v1, v2, i, draw, leaves)
    };
    let (p, v1, v2, i, draw, leaves) = reads;
    let n = |s: &str| d.find_name(s).0 as f64;
    assert_eq!(eval(&d, p, &BOUND), Ok(10.0 + n("p")));
    assert_eq!(eval(&d, v1, &BOUND), Ok(n("out")));
    assert_eq!(eval(&d, v2, &BOUND), Ok(n("out") - 0.5));
    assert_eq!(eval(&d, i, &BOUND), Ok(-n("v1")));
    // AbsUnif = 2, Sampling = 1, Instance = 2: 1 + 200 + 10 + 2
    assert_eq!(eval(&d, draw, &BOUND), Ok(213.0));
    let want = [1e-9, 27.0, 1e6, 1e-12, 1e-6, 25.0];
    for (e, w) in leaves.iter().zip(want) {
        assert_eq!(eval(&d, *e, &BOUND), Ok(w));
    }
    let unbound = Fixed { bound: false };
    for e in [p, v1, i, draw, leaves[0]] {
        assert_eq!(eval(&d, e, &unbound), Err(Domain::Unbound(e)));
    }
}

#[test]
fn strings_and_lists_are_not_numbers() {
    let mut d = Design::new();
    let (s, sum, list) = {
        let mut b = d.builder();
        let s = b.string("4.8");
        let one = b.num(1.0);
        let sum = b.binary(BinOp::Add, s, one);
        let list = b.list(&[one, one]);
        (s, sum, list)
    };
    assert_eq!(eval(&d, s, &BOUND), Err(Domain::NotANumber(s)));
    assert_eq!(eval(&d, sum, &BOUND), Err(Domain::NotANumber(s)));
    assert_eq!(eval(&d, list, &BOUND), Err(Domain::NotANumber(list)));
}

#[test]
fn a_deep_expression_does_not_overflow_the_stack() {
    let mut d = Design::new();
    let top = {
        let mut b = d.builder();
        let mut e = b.num(0.0);
        let one = b.num(1.0);
        for _ in 0..100_000 {
            e = b.binary(BinOp::Add, one, e); // 1 + (1 + (… + 0))
        }
        e
    };
    assert_eq!(eval(&d, top, &BOUND), Ok(100_000.0));
}

#[test]
fn the_a11_members() {
    exact(call(Func::StepHalf, &[-1.0]), 0.0);
    exact(call(Func::StepHalf, &[0.0]), 0.5); // the half step at zero
    exact(call(Func::StepHalf, &[2.0]), 1.0);
    exact(call(Func::PowSigned, &[2.0, 3.0]), 8.0);
    exact(call(Func::PowSigned, &[-2.0, 3.0]), -8.0); // −pow(2, 3)
    near(call(Func::PowSigned, &[-2.0, 0.5]), -SQRT_2); // −pow(2, 0.5): defined, unlike Pow
    near(call(Func::PowSigned, &[-8.0, 1.0 / 3.0]), -2.0);
    out_of_domain(call(Func::PowSigned, &[0.0, -1.0]));
    exact(call(Func::Sqr, &[-3.0]), 9.0);
    exact(call(Func::Sqr, &[1.5]), 2.25);
}
