//! Expression rows: every tag encodes through `Builder` and decodes through `Design::expr` to
//! the same `Expr`; children always have lower ids than their parent; arities are the stated
//! fact table and a call outside it asserts.
//!
//! Oracles: the inverse (build, decode, compare with the `Expr` written in the test); the
//! acyclicity invariant (child id < parent id) over every row built here.
//!
//! Contents: none (tests only).

use spicerack_ir::{
    BinOp, Design, Dist, DrawPer, DrawWhen, Expr, ExprId, ExprTag, Func, Literal, NameId, NumForm,
    Ref, UnOp,
};

// ---- private ----

/// Every child id of `e`, read through the public decoder.
fn children(d: &Design, e: ExprId) -> Vec<ExprId> {
    match d.expr(e) {
        Expr::Call(_, l) | Expr::UserCall(_, l) | Expr::Draw(_, _, _, l) | Expr::List(l) => {
            d.exprs(l).to_vec()
        }
        Expr::Unary(_, a) => vec![a],
        Expr::Binary(_, a, b) => vec![a, b],
        Expr::Ternary(c, a, b) => vec![c, a, b],
        Expr::Num(_)
        | Expr::Str(_)
        | Expr::Param(_)
        | Expr::V(..)
        | Expr::I(_)
        | Expr::Time
        | Expr::Temp
        | Expr::Freq
        | Expr::TStep
        | Expr::TStop
        | Expr::Tnom => vec![],
    }
}

#[test]
fn every_tag_round_trips() {
    let mut d = Design::new();
    let lit = Literal {
        mantissa: 25,
        frac: 1,
        exp10: -3,
        form: NumForm::Scale,
    };
    let mut built: Vec<(ExprId, ExprTag)> = Vec::new();
    let (num, s, p, a, b2, call, uc, un, bin, ter, v1, v2, i, list) = {
        let mut b = d.builder();
        let num = b.literal(lit);
        let s = b.string("4.8");
        let p = b.param_ref("w");
        let a = b.num(2.0);
        let b2 = b.num(3.0);
        let call = b.call(Func::Pow, &[a, b2]);
        let uc = b.user_call("f", &[a]);
        let un = b.unary(UnOp::Neg, a);
        let bin = b.binary(BinOp::IntDiv, a, b2);
        let ter = b.ternary(p, a, b2);
        let v1 = b.v("out", None);
        let v2 = b.v("out", Some("ref"));
        let i = b.i("v1");
        let list = b.list(&[a, b2, num]);
        (num, s, p, a, b2, call, uc, un, bin, ter, v1, v2, i, list)
    };
    let n = |x: &str| d.find_name(x);
    let Expr::Num(id) = d.expr(num) else {
        panic!("Num")
    };
    assert_eq!(d.num(id), lit);
    assert_eq!(d.expr(s), Expr::Str(n("4.8")));
    assert_eq!(d.expr(p), Expr::Param(Ref::unresolved(n("w"))));
    let Expr::Call(Func::Pow, args) = d.expr(call) else {
        panic!("Call")
    };
    assert_eq!(d.exprs(args), &[a, b2]);
    let Expr::UserCall(r, args) = d.expr(uc) else {
        panic!("UserCall")
    };
    assert_eq!((r, d.exprs(args)), (Ref::unresolved(n("f")), &[a][..]));
    assert_eq!(d.expr(un), Expr::Unary(UnOp::Neg, a));
    assert_eq!(d.expr(bin), Expr::Binary(BinOp::IntDiv, a, b2));
    assert_eq!(d.expr(ter), Expr::Ternary(p, a, b2));
    assert_eq!(d.expr(v1), Expr::V(n("out"), NameId::NONE));
    assert_eq!(d.expr(v2), Expr::V(n("out"), n("ref")));
    assert_eq!(d.expr(i), Expr::I(n("v1")));
    let Expr::List(items) = d.expr(list) else {
        panic!("List")
    };
    assert_eq!(d.exprs(items), &[a, b2, num]);
    for (e, tag) in [
        (num, ExprTag::Num),
        (s, ExprTag::Str),
        (p, ExprTag::Param),
        (call, ExprTag::Call),
        (uc, ExprTag::UserCall),
        (un, ExprTag::Unary),
        (bin, ExprTag::Binary),
        (ter, ExprTag::Ternary),
        (v1, ExprTag::V),
        (i, ExprTag::I),
        (list, ExprTag::List),
    ] {
        built.push((e, tag));
    }
    let leaves = {
        let mut b = d.builder();
        [
            (b.time(), Expr::Time),
            (b.temp(), Expr::Temp),
            (b.freq(), Expr::Freq),
            (b.tstep(), Expr::TStep),
            (b.tstop(), Expr::TStop),
            (b.tnom(), Expr::Tnom),
        ]
    };
    for (e, want) in leaves {
        assert_eq!(d.expr(e), want);
        built.push((e, want.tag()));
    }
    for (e, tag) in &built {
        assert_eq!(d.expr(*e).tag(), *tag, "{e:?}");
    }
    for k in 0..d.expr_count() {
        let e = ExprId(k as u32);
        for c in children(&d, e) {
            assert!(c < e, "child {c:?} below parent {e:?}");
        }
    }
}

#[test]
fn every_operator_and_function_round_trips() {
    let mut d = Design::new();
    let mut cases = Vec::new();
    {
        let mut b = d.builder();
        let x = b.num(0.5);
        for op in UnOp::ALL {
            cases.push((b.unary(op, x), Expr::Unary(op, x)));
        }
        for op in BinOp::ALL {
            cases.push((b.binary(op, x, x), Expr::Binary(op, x, x)));
        }
        for f in Func::ALL {
            let (min, _) = f.arity();
            let args = vec![x; min as usize];
            let e = b.call(f, &args);
            cases.push((e, Expr::Num(spicerack_ir::NumId::NONE))); // checked below by shape
        }
    }
    let mut funcs = Func::ALL.iter();
    for (e, want) in cases {
        match want {
            Expr::Num(_) => {
                let f = *funcs.next().unwrap();
                let Expr::Call(got, args) = d.expr(e) else {
                    panic!("Call")
                };
                assert_eq!(got, f);
                assert_eq!(args.len(), f.arity().0 as usize);
            }
            other => assert_eq!(d.expr(e), other),
        }
    }
}

#[test]
fn every_draw_combination_round_trips() {
    let mut d = Design::new();
    let mut cases = Vec::new();
    {
        let mut b = d.builder();
        let x = b.num(1.0);
        for dist in Dist::ALL {
            for when in DrawWhen::ALL {
                for per in DrawPer::ALL {
                    let n = dist.arity().0 as usize;
                    let e = b.draw(dist, when, per, &vec![x; n]);
                    cases.push((e, dist, when, per, n));
                }
            }
        }
    }
    for (e, dist, when, per, n) in cases {
        let Expr::Draw(gd, gw, gp, args) = d.expr(e) else {
            panic!("Draw")
        };
        assert_eq!((gd, gw, gp, args.len()), (dist, when, per, n));
    }
}

#[test]
fn arities_are_the_fact_table() {
    for f in Func::ALL {
        let (min, max) = f.arity();
        assert!(min >= 1 && min <= max, "{f:?}");
    }
    assert_eq!(Func::Sin.arity(), (1, 1));
    assert_eq!(Func::Pow.arity(), (2, 2));
    assert_eq!(Func::Atan2.arity(), (2, 2));
    assert_eq!(Func::Clamp.arity(), (3, 3));
    assert_eq!(Func::Pwl.arity(), (3, u8::MAX));
    assert_eq!(Func::StepHalf.arity(), (1, 1));
    assert_eq!(Func::PowSigned.arity(), (2, 2));
    assert_eq!(Func::Sqr.arity(), (1, 1));
    assert_eq!(Dist::AbsGauss.arity(), (3, 3));
    assert_eq!(Dist::Limit.arity(), (2, 2));
    assert!(Func::Ddt.reads_circuit() && Func::Idt.reads_circuit());
    assert!(!Func::Exp.reads_circuit());
}

#[test]
#[should_panic(expected = "argument count within the function's arity")]
fn call_outside_arity_asserts() {
    let mut d = Design::new();
    let mut b = d.builder();
    let x = b.num(1.0);
    b.call(Func::Sin, &[x, x]);
}

#[test]
#[should_panic(expected = "a built number is finite")]
fn non_finite_num_asserts() {
    let mut d = Design::new();
    d.builder().num(f64::INFINITY);
}

#[test]
#[should_panic(expected = "a literal meets its invariants")]
fn invalid_literal_asserts() {
    let mut d = Design::new();
    d.builder().literal(Literal {
        mantissa: 0,
        frac: 2,
        exp10: 0,
        form: NumForm::Plain,
    });
}

#[test]
fn idt_takes_an_initial_value() {
    // A28 (VACASK run bp3: `idt(x, ic)` starts from ic, the operating point reads ic): one
    // optional second operand; absent is 0, so a literal 0 there is a second spelling of one
    // meaning and `check` refuses it (LiteralShape, the non-canonical zero)
    let read = |e: &str| {
        let mut d = spicerack_ir::Design::new();
        let mut diags = spicerack_ir::Diags::new();
        let text =
            format!("file \"t.sp\" deck\n  element behav b1 (a 0) expr=({e}) output=current\n");
        spicerack_ir::parse(&text, &mut d.builder(), &mut diags);
        let mut checked = spicerack_ir::Diags::new();
        spicerack_ir::check(&d, &mut checked);
        (
            diags.len(),
            checked.rows().iter().map(|r| r.code).collect::<Vec<_>>(),
        )
    };
    assert_eq!(read("idt(v(a))"), (0, vec![]), "from 0");
    assert_eq!(read("idt(v(a), 3)"), (0, vec![]), "from 3");
    let zero = spicerack_ir::diag::violation_code(spicerack_ir::Violation::LiteralShape);
    assert_eq!(
        read("idt(v(a), 0)"),
        (0, vec![zero]),
        "absent is 0: one spelling"
    );
    assert_eq!(read("idt(v(a), 1, 2)").0, 1, "at most two operands");
    assert_eq!(spicerack_ir::Func::Idt.arity(), (1, 2));
}
