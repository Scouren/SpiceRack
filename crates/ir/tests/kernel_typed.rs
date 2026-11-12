//! Typed arithmetic (A12 K1, R16): the `Int` literal form, the tag the reference evaluator puts
//! on every value, `DivTyped` and `ToInt`, the `IntLiteral` feature, and their IR-text spelling.
//!
//! Oracles: the tag table of AMEND-A12 K1 at hand-worked points; VACASK's printed values for runs
//! i1, t3 and f2 (DIALECT-VACASK Appendix A) as spec examples, where the run decides `Sgn`
//! (Int-valued: `sgn(-3)/2` printed 0); the inverse for the text spelling.
//!
//! Contents: none (tests only).

use spicerack_ir::{
    BinOp, Bindings, Builder, Design, Diags, Dist, DrawPer, DrawWhen, ExprId, Extra, Feature, Func,
    Literal, NameId, NumForm, Ref, Role, Span, UnOp, Value, dump, eval_typed, features_used, parse,
};

/// Parameters by name: `a` = Int 1, `b` = Int 3, `c` = Real 1.0, `xi` = Int 3, `xr` = Real 3.0.
struct Passed<'d>(&'d Design);

impl Bindings for Passed<'_> {
    fn param(&self, p: Ref) -> Option<f64> {
        self.param_value(p).map(|v| v.number)
    }
    fn param_value(&self, p: Ref) -> Option<Value> {
        match self.0.str(p.name) {
            "a" => Some(Value::int(1.0)),
            "b" => Some(Value::int(3.0)),
            "c" => Some(Value::real(1.0)),
            "xi" => Some(Value::int(3.0)),
            "xr" => Some(Value::real(3.0)),
            _ => None,
        }
    }
    fn voltage(&self, _: NameId, _: NameId) -> Option<f64> {
        None
    }
    fn current(&self, _: NameId) -> Option<f64> {
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
    fn draw(&self, _: Dist, _: DrawWhen, _: DrawPer, _: &[f64]) -> Option<f64> {
        None
    }
}

// ---- private ----

fn int(n: i64) -> Literal {
    Literal {
        mantissa: n,
        frac: 0,
        exp10: 0,
        form: NumForm::Int,
    }
}

fn real(mantissa: i64, frac: u8) -> Literal {
    Literal {
        mantissa,
        frac,
        exp10: 0,
        form: NumForm::Plain,
    }
}

/// Builds one expression and evaluates it with its type.
fn ev(build: impl FnOnce(&mut Builder<'_>) -> ExprId) -> Value {
    let mut d = Design::new();
    let e = build(&mut d.builder());
    eval_typed(&d, e, &Passed(&d)).expect("a value")
}

fn i(n: f64) -> Value {
    Value::int(n)
}

fn r(n: f64) -> Value {
    Value::real(n)
}

#[test]
fn the_int_literal_form() {
    assert!(int(7).is_valid() && int(-7).is_valid());
    assert!(
        int(0).is_valid(),
        "an Int zero is the second permitted zero"
    );
    assert!(!int(0).eq_value(Literal::ZERO), "and is not the real zero");
    assert!(
        !Literal { frac: 1, ..int(7) }.is_valid(),
        "an Int has no fraction digits"
    );
    assert!(
        !Literal { exp10: 3, ..int(7) }.is_valid(),
        "and no exponent"
    );
    assert_eq!(int(7).value(), 7.0);
    assert!(
        !int(7).eq_value(real(7, 0)),
        "Int 7 and real 7 mean different things"
    );
    assert!(int(7).eq_value(int(7)));
}

#[test]
fn div_typed_at_the_spec_points() {
    let div = |a: Literal, b: Literal| {
        ev(|bl| {
            let (a, b) = (bl.literal(a), bl.literal(b));
            bl.binary(BinOp::DivTyped, a, b)
        })
    };
    assert_eq!(div(int(7), int(2)), i(3.0));
    assert_eq!(div(int(-7), int(2)), i(-3.0), "toward zero");
    assert_eq!(div(int(7), real(20, 1)), r(3.5), "7 / 2.0");
    assert_eq!(div(int(1), int(3)), i(0.0));
    let third = div(real(10, 1), int(3));
    assert!(
        !third.int && (third.number - 1.0 / 3.0).abs() < 1e-15,
        "1.0 / 3"
    );
    // Div stays real division whatever the tags
    let plain = ev(|bl| {
        let (a, b) = (bl.literal(int(7)), bl.literal(int(2)));
        bl.binary(BinOp::Div, a, b)
    });
    assert_eq!(plain, r(3.5));
}

#[test]
fn every_row_of_the_tag_table() {
    // operands: Int 3 and Real 3.0
    let three = || int(3);
    let three_r = || real(30, 1);
    let un = |op: UnOp, l: Literal| {
        ev(|bl| {
            let x = bl.literal(l);
            bl.unary(op, x)
        })
    };
    let bin = |op: BinOp, a: Literal, b: Literal| {
        ev(|bl| {
            let (a, b) = (bl.literal(a), bl.literal(b));
            bl.binary(op, a, b)
        })
    };
    let call = |f: Func, args: &[Literal]| {
        ev(|bl| {
            let xs: Vec<ExprId> = args.iter().map(|l| bl.literal(*l)).collect();
            bl.call(f, &xs)
        })
    };
    assert_eq!(un(UnOp::Neg, three()), i(-3.0), "Neg: its operand's");
    assert_eq!(un(UnOp::Neg, three_r()), r(-3.0));
    for op in [BinOp::Add, BinOp::Sub, BinOp::Mul] {
        assert!(bin(op, three(), three()).int, "{op:?} of Ints is Int");
        assert!(
            !bin(op, three(), three_r()).int,
            "{op:?} with a Real is Real"
        );
    }
    assert!(call(Func::Min, &[three(), int(5)]).int && call(Func::Max, &[three(), int(5)]).int);
    assert!(!call(Func::Min, &[three_r(), int(5)]).int);
    for op in [
        BinOp::Lt,
        BinOp::Eq,
        BinOp::And,
        BinOp::Or,
        BinOp::BitAnd,
        BinOp::Shl,
    ] {
        assert!(bin(op, three_r(), int(1)).int, "{op:?} is Int");
    }
    assert!(un(UnOp::Not, three_r()).int && un(UnOp::BitNot, three()).int);
    assert_eq!(call(Func::ToInt, &[real(37, 1)]), i(3.0), "ToInt: 3.7 → 3");
    assert_eq!(call(Func::ToInt, &[real(-37, 1)]), i(-3.0));
    assert_eq!(
        call(Func::Trunc, &[real(37, 1)]),
        r(3.0),
        "Trunc stays Real"
    );
    assert!(!call(Func::Abs, &[three()]).int, "every other Func is Real");
    assert!(!call(Func::Floor, &[three()]).int);
    assert_eq!(
        call(Func::Sgn, &[int(-3)]),
        i(-1.0),
        "Sgn: Int of an Int (run f2)"
    );
    assert_eq!(call(Func::Sgn, &[real(-30, 1)]), r(-1.0));
    let ternary = |c: Literal, a: Literal, b: Literal| {
        ev(|bl| {
            let (c, a, b) = (bl.literal(c), bl.literal(a), bl.literal(b));
            bl.ternary(c, a, b)
        })
    };
    assert_eq!(
        ternary(int(1), int(3), real(40, 1)),
        i(3.0),
        "the chosen branch's"
    );
    assert_eq!(ternary(int(0), int(3), real(40, 1)), r(4.0));
    assert!(!ev(|bl| bl.temp()).int, "a leaf is Real");
    assert_eq!(
        ev(|bl| bl.param_ref("xi")),
        i(3.0),
        "a parameter read: its value's"
    );
}

#[test]
fn an_int_beyond_two_to_the_53_is_a_domain_error() {
    let mut d = Design::new();
    let e = {
        let mut bl = d.builder();
        let (a, b) = (bl.literal(int(1 << 30)), bl.literal(int(1 << 30)));
        bl.binary(BinOp::Mul, a, b)
    };
    assert!(
        matches!(
            eval_typed(&d, e, &Passed(&d)),
            Err(spicerack_ir::Domain::OutOfDomain(_))
        ),
        "2^30 · 2^30 = 2^60 as Int is beyond 2^53"
    );
    let within = ev(|bl| {
        let (a, b) = (bl.literal(int(1 << 26)), bl.literal(int(1 << 26)));
        bl.binary(BinOp::Mul, a, b)
    });
    assert_eq!(within, i((1u64 << 52) as f64), "2^52 is within");
}

#[test]
fn vacask_runs_i1_t3_f2() {
    // each `param` reads as VACASK printed it (DIALECT-VACASK Appendix A); every binary level is
    // left-associative, so `(1/3)*30` is written `1i /: 3i * 30i`
    let text = "file \"v.sim\" deck
  param i1a = 1i /: 3i * 30i + 10i
  param i1b = a /: b * 30i + 10i
  param i1c = c /: 3i * 30i + 10i
  param i1d = -7i /: 2i + 10i
  param i1e = 1i /: 3.0 * 30i + 10i
  param i1f = pow(2i, -1i) * 30i + 10i
  param i1g = 1k /: 3i + 10i
  param i1h = 1e3 /: 3i + 10i
  param t3i = xi /: 2i + 10i
  param t3r = xr /: 2i + 10i
  param f2a = to_int(3.0) /: 2i
  param f2b = floor(3.0) /: 2i
  param f2c = abs(3i) /: 2i
  param f2d = min(3i, 5i) /: 2i
  param f2e = min(3.0, 5i) /: 2i
  param f2f = sgn(-3i) /: 2i
  param f2g = (1i ? 3i : 4.0) /: 2i
  param f2h = -3i /: 2i
  param f2i = pow(3i, 1i) /: 2i
  param f2j = 3i * 1i /: 2i
  param f2k = (3i > 2i) /: 2i
";
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let want = [
        ("i1a", 10.0),
        ("i1b", 10.0),
        ("i1c", 20.0),
        ("i1d", 7.0),
        ("i1e", 20.0),
        ("i1f", 25.0),
        ("i1g", 343.333_333_333_333_3),
        ("i1h", 343.333_333_333_333_3),
        ("t3i", 11.0),
        ("t3r", 11.5),
        ("f2a", 1.0),
        ("f2b", 1.5),
        ("f2c", 1.5),
        ("f2d", 1.0),
        ("f2e", 1.5),
        ("f2f", 0.0),
        ("f2g", 1.0),
        ("f2h", -1.0),
        ("f2i", 1.5),
        ("f2j", 1.0),
        ("f2k", 0.0),
    ];
    let top = d.file(d.entry()).top;
    for (name, value) in want {
        let def = d.lookup(top, spicerack_ir::DefKind::Param, d.find_name(name));
        let row: spicerack_ir::item::Param = d.row(d.def(def).item.index);
        let got = eval_typed(&d, row.value, &Passed(&d))
            .expect("a value")
            .number;
        assert!(
            (got - value).abs() < 1e-9,
            "{name}: VACASK printed {value}, got {got}"
        );
    }
    // the text spells Int literals, `/:` and `to_int` back as written
    let mut out = String::new();
    dump(&d, d.entry(), &mut out);
    assert_eq!(out, text);
}

#[test]
fn an_int_literal_is_a_feature() {
    let used = |l: Literal| {
        let mut d = Design::new();
        {
            let mut bl = d.builder();
            let (_, top) = bl.file("t.sp", Role::Deck);
            let v = bl.literal(l);
            bl.param(top, "p", v, Span::NONE);
        }
        features_used(&d)
    };
    assert!(used(int(3)).contains(Feature::Extra(Extra::IntLiteral)));
    assert!(
        !used(real(3, 0)).contains(Feature::Extra(Extra::IntLiteral)),
        "its neighbour"
    );
}
