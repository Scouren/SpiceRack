//! Tests of ngspice's two expression evaluators, read (`expr_read.rs`) and written
//! (`expr_write.rs`).
//!
//! Oracles: (1) the value ngspice-45 printed for the same text, run in `nix develop .#testing`
//! (`.param p={…}` into a 1 Ω load for numparam; `B1 b 0 V=…` with `V(c) = 1` for the B-source
//! parser; the scratch script is `ev2.sh` of the S3.0 report), compared with `ir::eval` of the
//! tree read; (2) hand-written text for the writer, read back to the same tree (law L5 on
//! expressions); (3) the codes ngspice's own errors map to.
//!
//! Contents: tests only.

use super::codes;
use super::expr_lex;
use super::expr_read::{Fail, Tree, parse_in};
use super::expr_write;
use super::name::Folder;
use super::tables::Eval;
use crate::ir;

/// `V(any) = 1`; draws give their nominal; nothing else is bound.
struct Unit;

impl ir::Bindings for Unit {
    fn param(&self, _: ir::Ref) -> Option<f64> {
        None
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        Some(1.0)
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        Some(1.0)
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
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, args: &[f64]) -> Option<f64> {
        Some(args[0])
    }
}

// ---- private ----

/// `text` read by the evaluator its own spelling picks (no definitions around it).
fn parse(text: &str) -> Result<Tree, Fail> {
    let l = expr_lex::leaves(text);
    let eval = if l.circuit || l.temp {
        Eval::Behavioural
    } else {
        Eval::Numparam
    };
    parse_in(text, eval)
}

/// A design holding `text` read as one expression, and its root.
fn read(text: &str) -> Result<(ir::Design, ir::ExprId), Fail> {
    let tree = parse(text)?;
    let mut d = ir::Design::new();
    let e = {
        let mut b = d.builder();
        let (_, top) = b.file("t", ir::Role::Library);
        tree.emit(&mut b, &mut Folder::default(), top, &mut Vec::new())
    };
    Ok((d, e))
}

/// The value of `text` under [`Unit`].
fn value(text: &str) -> f64 {
    let (d, e) = read(text).unwrap_or_else(|f| panic!("{text}: {f:?}"));
    ir::eval(&d, e, &Unit).unwrap_or_else(|x| panic!("{text}: {x:?}"))
}

/// `text` written back.
fn written(text: &str) -> String {
    let (d, e) = read(text).unwrap_or_else(|f| panic!("{text}: {f:?}"));
    let mut out = String::new();
    expr_write::expr(&d, e, &mut out);
    out
}

#[test]
fn numparam_and_b_source_values_are_ngspices() {
    // (text, ngspice-45's printed value); `v(c)` forces the B-source parser (V(c) = 1).
    let cases = [
        ("2**3**2", 64.0),
        ("(-2)**3", 8.0),
        ("-2**2", -4.0),
        ("2*-2**2", 8.0),
        ("v(c)*2*-2**2", -8.0),
        ("2**0.5**2", 2.0),
        ("pwr(0-3,2)", 9.0),
        ("pow(0-2,3)", -8.0),
        ("v(c)*pow(0-2,3)", 8.0),
        ("0==1<2", 1.0),
        ("(v(c)-1)==1<2", 0.0),
        ("1?0?4:8:16", 4.0),
        ("v(c)?0?4:8:16", 8.0),
        ("1?0:1?3:4", 0.0),
        ("1?1?1?2:3:4:5", 2.0),
        ("1?0?2:3+1:4", 2.0),
        ("0?1?2:3:4?5:6", 5.0),
        ("0?2:3?4:5", 4.0),
        ("7%3", 1.0),
        ("-7%3", -1.0),
        ("(-7)\\2", 3.0),
        ("-7\\2", -3.0),
        ("7\\2.5", 2.0),
        ("nint(2.5)", 2.0),
        ("nint(-2.5)", -2.0),
        ("nint(0.5)", 0.0),
        ("int(-1.5)", -1.0),
        ("!0*5", 5.0),
        ("!1<2", 1.0),
        ("-!0", -1.0),
        ("-v(c)**2", -1.0),
        ("(-v(c))**2", 1.0),
        ("-2*v(c)", -2.0),
        ("2mil", 2e-3),
        ("1e3k", 1e6),
        ("0.050000*20", 1.0),
        ("ternary_fcn(0,1,2)", 2.0),
        ("arctan(1)", std::f64::consts::FRAC_PI_4),
        ("ln(exp(2))", 2.0),
        ("log(v(c)*100)", 100f64.ln()),
        ("v(c)*uramp(-2)", 0.0),
        ("min(3,v(c))", 1.0),
        ("3-2-1", 0.0),
        ("8/4/2", 1.0),
        ("1||0&&0", 1.0),
        ("v(c)+1>=2", 1.0),
        ("u2(0.5*v(c))", 0.5),
        ("u(0*v(c))", 0.5),
        ("v(c)*pwr(0-3,2)", -9.0),
        ("sqr(3)", 9.0),
    ];
    for (text, want) in cases {
        let got = value(text);
        assert!(
            (got - want).abs() <= 1e-12 * want.abs().max(1.0),
            "{text}: {got} vs {want}"
        );
    }
    let pi = value("pi*v(c)");
    assert_eq!(pi, std::f64::consts::PI, "B-source pi is M_PI");
    assert!((value("2mil*v(c)") - 5.08e-5).abs() < 1e-20, "B-source mil");
}

#[test]
fn each_evaluator_picks_its_own_members() {
    // ngspice-45 ptfuncs.c / xpressn.c: numparam `pow` is C's pow, B `pow` is |x|^y; B `ln`
    // and `sqrt` are the real functions with domain errors (not of |x|, run "out of range").
    let call = |text: &str| {
        let (d, e) = read(text).expect(text);
        match d.expr(e) {
            ir::Expr::Call(f, _) => f,
            other => panic!("{text}: {other:?}"),
        }
    };
    assert_eq!(call("pow(2,3)"), ir::Func::Pow);
    assert_eq!(call("pow(v(a),3)"), ir::Func::PowAbs);
    assert_eq!(call("ln(v(a))"), ir::Func::Ln);
    assert_eq!(call("sqrt(v(a))"), ir::Func::Sqrt);
    assert_eq!(call("exp(v(a))"), ir::Func::Exp);
    assert_eq!(call("pwr(2,3)"), ir::Func::PowAbs);
    let (d, e) = read("agauss(1,0.1,3)").expect("a draw");
    let draw = d.expr(e);
    let want = ir::Expr::Draw(
        ir::Dist::AbsGauss,
        ir::DrawWhen::Always,
        ir::DrawPer::Reference,
        match draw {
            ir::Expr::Draw(.., l) => l,
            _ => panic!("{draw:?}"),
        },
    );
    assert_eq!(
        draw, want,
        "INV 12.01-12.03: a fresh sample at every reference"
    );
}

#[test]
fn what_ngspice_rejects_or_the_ir_lacks_is_coded() {
    let fail = |text: &str| parse(text).expect_err(text);
    let invalid = |code| Fail {
        code,
        class: ir::OpaqueClass::InvalidInSource,
    };
    let unread = Fail {
        code: codes::EXPR_NO_MEMBER,
        class: ir::OpaqueClass::Unread,
    };
    // runs: `{2*-q}` "wrongly determined negation", `{5-+2}`, `{!-1}`, `1k5` in braces
    assert_eq!(fail("2*-q"), invalid(codes::NEGATION));
    // `min(1)` is no error: numparam's min of one argument is that argument (settle
    // max_one_arg.sp)
    for text in ["5-+2", "!-1", "1k5", "(1", "1)", "1 2", "1?2"] {
        assert_eq!(fail(text), invalid(codes::EXPR_INVALID), "{text}");
    }
    assert_eq!(fail("1?2:0?1?3:4:5"), invalid(codes::TERNARY_NEST));
    // no IR member: B `eq0`…`le0`, `pwl`; a string
    for text in ["eq0(v(a))", "pwl(v(a),0,0,1,1)", "\"s\""] {
        assert_eq!(fail(text), unread, "{text}");
    }
}

#[test]
fn the_writer_spells_the_fewest_parentheses_that_read_back() {
    let cases = [
        ("2**3**2", "2 ** 3 ** 2"),
        ("2**(3**2)", "2 ** (3 ** 2)"),
        ("-2**2", "-2 ** 2"),
        ("(-2)**2", "(-2) ** 2"),
        ("a-(b-c)", "a - (b - c)"),
        ("a*(b+c)", "a * (b + c)"),
        ("-a+b", "-a + b"),
        ("b+(-a)", "b + (-a)"),
        ("(-a)*b", "(-a) * b"),
        ("3*-2", "3 * -2"),
        ("-(2)", "-(2)"),
        ("-2", "-2"),
        ("1?(0?4:8):16", "1 ? (0 ? 4 : 8) : 16"),
        ("1?2:0?3:4", "1 ? 2 : 0 ? 3 : 4"),
        ("0==1<2", "0 == 1 < 2"),
        ("0==(1<2)", "0 == (1 < 2)"),
        ("v(a)==1<2", "v(a) == 1 < 2"),
        ("(v(a)==1)<2", "(v(a) == 1) < 2"),
        ("7\\2", "int(abs(7 / 2))"),
        ("7%3", "7 % 3"),
        ("pwr(2,3)", "2 ** 3"),
        ("!(a+b)", "!(a + b)"),
        ("ternary_fcn(1,2,3)", "1 ? 2 : 3"),
        ("-v(a)**2", "-v(a) ** 2"),
        ("(-v(a))**2", "(-v(a)) ** 2"),
        ("-2*v(a)", "-2 * v(a)"),
        ("(-2)**v(a)", "(-2) ** v(a)"),
        ("pi*v(a)", "3.14159265358979324 * v(a)"),
        ("2mil*v(a)", "2mil * v(a)"),
        ("2mil", "2m"),
        ("i(vdd)*temper", "i(vdd) * temper"),
        ("f(a, b)+1", "f(a, b) + 1"),
        ("{1+x}*2", "(1 + x) * 2"),
        ("u2(v(a))", "min(max(v(a), 0), 1)"),
        ("i(R1)*2", "i(R1) * 2"),
        ("u(v(a))", "u(v(a))"),
        ("pwr(v(a),3)", "pwr(v(a), 3)"),
        ("sqr(2)", "sqr(2)"),
    ];
    for (text, want) in cases {
        assert_eq!(written(text), want, "{text}");
        let (a, _) = read(text).expect(text);
        let (b, _) = read(want).expect(want);
        assert!(
            a.eq_ignoring_spans(&b),
            "{want} reads back as {text}'s tree"
        );
    }
}

#[test]
fn a_ten_thousand_deep_nest_reads_and_writes_without_recursion() {
    // DIALECT-NGSPICE S3.0 rule: explicit stacks; run in a debug build.
    let n = 10_000;
    let deep = format!("{}1{}", "(".repeat(n), ")".repeat(n));
    assert_eq!(written(&deep), "1");
    let right = format!("{}a{}", "a-(".repeat(n), ")".repeat(n));
    let text = written(&right);
    assert_eq!(
        text.matches('(').count(),
        n - 1,
        "the innermost `(a)` is an atom"
    );
    let (a, _) = read(&right).expect("deep");
    let (b, _) = read(&text).expect("deep written");
    assert!(a.eq_ignoring_spans(&b));
}

#[test]
fn a_field_ngspice_hands_the_b_source_parser_is_written_and_checked_in_its_grammar() {
    // inpcom.c inp_bsource_compat: a B source's expression is the B-source parser's even when
    // constant. `{2**3}` and `pow(-2,3)` there: PowAbs spells (`**`), C's `Pow` does not (B `pow`
    // is |x|^y: ptfuncs.c PTpower).
    let (d, e) = read("2**3+1").expect("constant");
    let mut out = String::new();
    expr_write::expr_in(&d, e, Eval::Behavioural, &mut out);
    assert_eq!(out, "2 ** 3 + 1");
    assert!(expr_write::spells_in(&d, e, Eval::Behavioural));
    let (d, e) = read("pow(0-2,3)").expect("numparam pow");
    assert!(!expr_write::spells_in(&d, e, Eval::Behavioural));
    assert!(expr_write::spells_in(&d, e, Eval::Numparam));
    let (d, e) = read("v(a)*2").expect("a probe");
    assert!(!expr_write::spells_in(&d, e, Eval::Numparam));
    let (d, e) = read("2mil").expect("numparam milli");
    assert!(expr_write::spells_in(&d, e, Eval::Behavioural));
}
