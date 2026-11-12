//! The IR text (`text.rs`): hand-written text of every kernel item kind parses to the rows built
//! by hand, and `dump` of those rows is that canonical text; literals and expressions spell
//! losslessly; references are spelled bare, `name?` or `name#k` by the A4 rule; a bad line is
//! one diagnostic and the rest is read; the same calls dump to the same bytes; deep nesting
//! goes through build, dump, parse, check and eval.
//!
//! Oracles: spec examples (the canonical text is written out in the test, and the hand-built
//! design is built by the builder in the order a caller writes it: expressions bottom-up and
//! left to right, then the item with its names); the inverse (`parse(dump(d))` equals `d`);
//! byte compare for determinism.
//!
//! Contents: none (tests only).

use spicerack_ir::{
    BinOp, Bindings, DefKind, Design, Diags, Dist, DrawPer, DrawWhen, ExprId, Literal, LoadKind,
    NameId, NumForm, OpaqueClass, Ref, Role, Span, check, diag::TEXT_SYNTAX, dump, eval, link,
    parse,
};

/// Bindings that bind nothing (the deep expression is constant).
struct Constant;

/// Every kernel item kind, canonical.
const DECK: &str = r#"file "top.sp" deck
  title "rc divider"
  comment "a note"
  param rval = 1k
  param half = rval / 2
  include "models.lib" section=tt
  global vdd vss
  load osdi "psp.osdi" modules=(psp103va)
  opaque ngspice private ".stateye x"
  subckt div (in out gnd) r=rval w
    param j = r * 2
    if (r > 1)
      param k = 1
    elif (r < 0)
      param k = 2
    else
      param k = 3
  section tt library=lib1
    param corner = 1
  func f(x, y) = x * y + 1.5e-3
file "models.lib" library
  comment "a \"quoted\" note"
"#;

impl Bindings for Constant {
    fn param(&self, _: Ref) -> Option<f64> {
        None
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
        None
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

fn lit(mantissa: i64, frac: u8, exp10: i16, form: NumForm) -> Literal {
    Literal {
        mantissa,
        frac,
        exp10,
        form,
    }
}

fn plain(n: i64) -> Literal {
    if n == 0 {
        Literal::ZERO
    } else {
        lit(n, 0, 0, NumForm::Plain)
    }
}

/// `DECK`, built by hand in a caller's order, then linked (as `parse` links).
fn deck_by_hand() -> Design {
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        b.title(top, "rc divider", Span::NONE);
        b.comment(top, "a note", Span::NONE);
        let k = b.literal(lit(1, 0, 3, NumForm::Scale));
        b.param(top, "rval", k, Span::NONE);
        let r = b.param_ref("rval");
        let two = b.literal(plain(2));
        let half = b.binary(BinOp::Div, r, two);
        b.param(top, "half", half, Span::NONE);
        b.include(top, "models.lib", Some("tt"), Span::NONE);
        b.global(top, &["vdd", "vss"], Span::NONE);
        let load = b.load(top, LoadKind::Osdi, "psp.osdi", Span::NONE);
        let module = b.name("psp103va");
        b.define(DefKind::Module, module, top, load);
        b.opaque(
            top,
            "ngspice",
            OpaqueClass::Private,
            ".stateye x",
            Span::NONE,
        );
        let rv = b.param_ref("rval");
        let ports = ["in", "out", "gnd"];
        let (_, body) = b.subckt(
            top,
            "div",
            &ports,
            &[("r", rv), ("w", ExprId::NONE)],
            Span::NONE,
        );
        let r = b.param_ref("r");
        let two = b.literal(plain(2));
        let j = b.binary(BinOp::Mul, r, two);
        b.param(body, "j", j, Span::NONE);
        let r = b.param_ref("r");
        let one = b.literal(plain(1));
        let c1 = b.binary(BinOp::Gt, r, one);
        let r = b.param_ref("r");
        let zero = b.literal(plain(0));
        let c2 = b.binary(BinOp::Lt, r, zero);
        let (_, branches) = b.if_(body, &[c1, c2, ExprId::NONE], Span::NONE);
        for k in 0..3 {
            let scope = b.design().branches(branches)[k].body;
            let v = b.literal(plain(k as i64 + 1));
            b.param(scope, "k", v, Span::NONE);
        }
        let (_, sec) = b.section(top, "tt", Some("lib1"), Span::NONE);
        let one = b.literal(plain(1));
        b.param(sec, "corner", one, Span::NONE);
        let x = b.param_ref("x");
        let y = b.param_ref("y");
        let xy = b.binary(BinOp::Mul, x, y);
        let small = b.literal(lit(15, 1, -3, NumForm::Exponent));
        let fbody = b.binary(BinOp::Add, xy, small);
        b.func(top, "f", &["x", "y"], fbody, Span::NONE);
        let (_, lib) = b.file("models.lib", Role::Library);
        b.comment(lib, "a \"quoted\" note", Span::NONE);
    }
    link(&mut d, &mut Diags::new());
    d
}

fn read(text: &str) -> (Design, Diags) {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(text, &mut d.builder(), &mut diags);
    (d, diags)
}

fn write(d: &Design) -> String {
    let mut out = String::new();
    for f in d.files() {
        dump(d, f, &mut out);
    }
    out
}

/// `param p<k> = <expr>` per expression, in a deck.
fn params(exprs: &[&str]) -> String {
    let mut text = String::from("file \"e.sp\" deck\n");
    for (k, e) in exprs.iter().enumerate() {
        text.push_str(&format!("  param p{k} = {e}\n"));
    }
    text
}

#[test]
fn every_kernel_kind_parses_to_the_rows_built_by_hand() {
    let (parsed, diags) = read(DECK);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let hand = deck_by_hand();
    assert!(
        parsed.eq_ignoring_spans(&hand),
        "parse(DECK) is the hand-built design"
    );
    assert_eq!(
        write(&hand),
        DECK,
        "dump of the hand-built rows is the canonical text"
    );
    let mut checked = Diags::new();
    check(&parsed, &mut checked);
    assert!(checked.is_empty(), "{:?}", checked.rows());
}

#[test]
fn literals_spell_losslessly() {
    let spelled = [
        ("0.050000", lit(50000, 6, 0, NumForm::Plain)),
        ("100u", lit(100, 0, -6, NumForm::Scale)),
        ("1e-4", lit(1, 0, -4, NumForm::Exponent)),
        ("-2.5e-3", lit(-25, 1, -3, NumForm::Exponent)),
        ("1mil", lit(1, 0, 0, NumForm::Mil)),
        ("2.5mil", lit(25, 1, 0, NumForm::Mil)),
        ("5c", lit(5, 0, -2, NumForm::Scale)),
        ("1meg", lit(1, 0, 6, NumForm::Scale)),
        ("3peta", lit(3, 0, 15, NumForm::Scale)),
        ("4.7a", lit(47, 1, -18, NumForm::Scale)),
        ("100e18", lit(100, 0, 18, NumForm::Exponent)),
        ("0", Literal::ZERO),
        ("-7", lit(-7, 0, 0, NumForm::Plain)),
    ];
    let text = params(&spelled.map(|s| s.0));
    let (d, diags) = read(&text);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    for (k, (s, want)) in spelled.iter().enumerate() {
        let top = d.file(d.files().next().unwrap()).top;
        let def = d.lookup(top, DefKind::Param, d.find_name(&format!("p{k}")));
        let row: spicerack_ir::item::Param = d.row(d.def(def).item.index);
        let spicerack_ir::Expr::Num(n) = d.expr(row.value) else {
            panic!("{s} is a literal")
        };
        assert_eq!(d.num(n), *want, "{s}");
    }
    assert_eq!(write(&d), text);
}

#[test]
fn expressions_spell_with_the_fewest_parentheses() {
    let canonical = [
        "a - b - c",
        "a - (b - c)",
        "a * (b + c)",
        "-(1.5)",
        "-1.5",
        "--a",
        "-(a + b)",
        "!a && b || c",
        "a || b && c",
        "(a || b) && c",
        "a ? b : c ? d : e",
        "(a ? b : c) ? d : e",
        "a ? (b ? c : d) : e",
        "a & b | c ^ d",
        "a << 2 >> 1",
        "a // 2 + a / 2",
        "a == b",
        "a == b == c",
        "a < b",
        "~a",
        "pow_abs(a, 2) + @f(a, 1) - @g()",
        "round_half_even(a) * exp_lin14(a)",
        "draw:abs_gauss:always:reference(1, 0.1, 3)",
        "draw:rel_unif:sampling:call_site(1, 0.1)",
        "v(out) - v(out, ref) + i(v1)",
        "v(0) + v(`net-1`)",
        "$time * $temp + $freq - $tstep / $tstop * $tnom",
        "[1, 2, 3]",
        "\"4.8\"",
        "`odd name` + b",
        "pwl(a, 0, 0, 1, 1)",
    ];
    let text = params(&canonical);
    let (d, diags) = read(&text);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    assert_eq!(write(&d), text, "each expression dumps as written");
    // hand-written spacing and redundant parentheses read to the same rows
    let loose = params(&[
        "a-b-c",
        "a-(b-c)",
        "a*(b+c)",
        "-(1.5)",
        "-1.5",
        "- -a",
        "-(a+b)",
        "(!a&&b)||c",
        "a||(b&&c)",
        "(a||b)&&c",
        "a ? b : (c ? d : e)",
        "(a ? b : c) ? d : e",
        "a ? (b ? c : d) : e",
        "((a&b)|(c^d))",
        "(a<<2)>>1",
        "(a//2)+(a/2)",
        "(a==b)",
        "(a==b)==c",
        "((a)<(b))",
        "~(a)",
        "pow_abs( a ,2 )+@f(a,1)-@g?( )",
        "round_half_even(a)*exp_lin14(a)",
        "draw:abs_gauss:always:reference(1,0.1,3)",
        "draw:rel_unif:sampling:call_site(1,0.1)",
        "v(out)-v(out,ref)+i(v1)",
        "v(0)+v(`net-1`)",
        "$time*$temp+$freq-$tstep/$tstop*$tnom",
        "[1,2,3]",
        "\"4.8\"",
        "`odd name`+b?",
        "pwl(a,0,0,1,1)",
    ]);
    let (loose_d, diags) = read(&loose);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    assert!(
        loose_d.eq_ignoring_spans(&d),
        "spacing and parentheses do not change the rows"
    );
}

#[test]
fn references_follow_the_a4_rule() {
    // a forward read binds (link runs after the whole text); `b?` stays unresolved
    let text = "file \"r.sp\" deck\n  param a = b\n  param b = 1\n  param c = b?\n";
    let (d, diags) = read(text);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let top = d.file(d.files().next().unwrap()).top;
    let value = |name: &str| {
        let def = d.lookup(top, DefKind::Param, d.find_name(name));
        let row: spicerack_ir::item::Param = d.row(d.def(def).item.index);
        let spicerack_ir::Expr::Param(r) = d.expr(row.value) else {
            panic!("a read")
        };
        r
    };
    assert_eq!(
        value("a").def,
        d.lookup(top, DefKind::Param, d.find_name("b"))
    );
    assert!(!value("c").is_resolved());
    assert_eq!(write(&d), text);
    // a read bound against the lookup rule is spelled `name#k`, and reads back bound to k
    let mut pinned = Design::new();
    {
        let mut b = pinned.builder();
        let (_, top) = b.file("p.sp", Role::Deck);
        let one = b.literal(plain(1));
        b.param(top, "k", one, Span::NONE);
        let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
        let two = b.literal(plain(2));
        b.param(body, "k", two, Span::NONE);
        let read = b.param_ref("k");
        let outer = b
            .design()
            .lookup(top, DefKind::Param, b.design().find_name("k"));
        b.resolve(read, outer);
        b.param(body, "q", read, Span::NONE);
    }
    let text = write(&pinned);
    assert!(text.contains("param q = k#0\n"), "{text}");
    let (back, diags) = read(&text);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    assert!(back.eq_ignoring_spans(&pinned));
}

#[test]
fn a_bad_line_is_one_diagnostic_and_the_rest_is_read() {
    let text =
        "file \"b.sp\" deck\n  param = 1\n  param ok = 2\n  nonsense here\n  param z = (1 +\n";
    let (d, diags) = read(text);
    let codes: Vec<_> = diags.rows().iter().map(|r| r.code).collect();
    assert_eq!(codes, vec![TEXT_SYNTAX; 3], "{:?}", diags.rows());
    let line2 = text.find("  param = 1").unwrap() as u32;
    assert!(
        diags.rows()[0].span.start >= line2,
        "the first row points into line 2"
    );
    let top = d.file(d.files().next().unwrap()).top;
    assert!(
        !d.lookup(top, DefKind::Param, d.find_name("ok")).is_none(),
        "the good line is read"
    );
}

#[test]
fn the_same_calls_dump_the_same_bytes() {
    assert_eq!(write(&deck_by_hand()), write(&deck_by_hand()));
    let (a, _) = read(DECK);
    let (b, _) = read(DECK);
    assert_eq!(write(&a), write(&b));
}

#[test]
fn deep_nesting_goes_through_every_walk() {
    const DEPTH: usize = 10_000;
    // an expression: 1 + (1 + (… + (1 + 0)))
    let mut d = Design::new();
    let root = {
        let mut b = d.builder();
        let (_, top) = b.file("deep.sp", Role::Deck);
        // a tree, built in the order its text reads: every `1`, the `0`, then the sums inside out
        let ones: Vec<ExprId> = (0..DEPTH).map(|_| b.literal(plain(1))).collect();
        let mut e = b.literal(plain(0));
        for &one in ones.iter().rev() {
            e = b.binary(BinOp::Add, one, e);
        }
        b.param(top, "p", e, Span::NONE);
        // and a scope: subcircuits nested DEPTH deep
        let mut scope = top;
        for k in 0..DEPTH {
            scope = b.subckt(scope, &format!("s{k}"), &[], &[], Span::NONE).1;
        }
        e
    };
    let mut diags = Diags::new();
    check(&d, &mut diags);
    assert!(
        diags.is_empty(),
        "{:?}",
        &diags.rows()[..diags.len().min(3)]
    );
    assert_eq!(eval(&d, root, &Constant), Ok(DEPTH as f64));
    let text = write(&d);
    let (back, diags) = read(&text);
    assert!(
        diags.is_empty(),
        "{:?}",
        &diags.rows()[..diags.len().min(3)]
    );
    assert!(back.eq_ignoring_spans(&d), "a deep design reads back equal");
    let mut checked = Diags::new();
    check(&back, &mut checked);
    assert!(checked.is_empty());
}

#[test]
fn a_subckt_default_reading_a_sibling_needs_no_mark() {
    // A10: the head defaults are looked up from the body, so the sibling read is bare
    let text = "file \"s.sp\" deck\n  param r = 5\n  subckt s () r=1 w=(2 * r)\n";
    let (d, diags) = read(text);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let out = write(&d);
    assert!(!out.contains('#'), "{out}");
    assert_eq!(out, text);
}

#[test]
fn spans_name_the_file_block_of_their_line() {
    // offsets stay into the whole text; `file` is the block the line sits in
    let text =
        "file \"a.sp\" deck\n  param x = 1\nfile \"b.sp\" library\n  param y = 2\n  param = 3\n";
    let (d, diags) = read(text);
    assert_eq!(diags.len(), 1, "{:?}", diags.rows());
    let bad = diags.rows()[0].span;
    assert_eq!(
        bad.file,
        spicerack_ir::FileId(1),
        "the bad line is in the second block"
    );
    assert!(bad.start as usize >= text.find("  param = 3").unwrap());
    let top_b = d.file(spicerack_ir::FileId(1)).top;
    let y = d.items(top_b)[0];
    let span = d.span_of(y);
    assert_eq!(span.file, spicerack_ir::FileId(1));
    assert_eq!(
        &text[span.start as usize..(span.start + span.len) as usize],
        "param y = 2"
    );
    let top_a = d.file(spicerack_ir::FileId(0)).top;
    assert_eq!(d.span_of(d.items(top_a)[0]).file, spicerack_ir::FileId(0));
}
