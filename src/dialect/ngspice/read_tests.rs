//! Tests of the ngspice reader and writer spines and the structure area, through
//! `dialect::read` / `dialect::write` (the spine runs `ir::check` after the read, so an `X`
//! row fails these too).
//!
//! Oracles: the facts each test cites (ngspice manual clause, INV row, ngspice-45 source), stated
//! as the items and bindings expected; the writer's text written by hand; the round trip
//! (read → write → read gives the same design, and the text is a fixpoint: law L6 on these
//! decks).
//!
//! Contents: tests only.

use super::codes;
use crate::dialect::{self, Policy, Sources};
use crate::ir::{self, ItemKind, OpaqueClass};

// ---- private ----

/// `src` read by the ngspice row, leniently.
fn read(src: &Sources) -> (ir::Design, ir::Diags) {
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let mut d = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut d, &mut diags);
    let bugs: Vec<_> = diags
        .rows()
        .iter()
        .filter(|r| r.code.letter == b'X')
        .collect();
    assert!(bugs.is_empty(), "ir::check: {bugs:?}");
    (d, diags)
}

/// One deck read.
fn deck(text: &str) -> (ir::Design, ir::Diags) {
    read(&Sources::one("deck.sp", text))
}

/// The entry file written by the ngspice row.
fn write(d: &ir::Design) -> String {
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let mut diags = ir::Diags::new();
    let text = dialect::write(d, d.entry(), row, &mut diags);
    assert!(
        !diags.rows().iter().any(|r| dialect::refuses(r.code)),
        "{diags:?}"
    );
    text
}

/// The kinds of a scope's items.
fn kinds(d: &ir::Design, scope: ir::ScopeId) -> Vec<ItemKind> {
    d.items(scope).iter().map(|i| i.kind).collect()
}

/// The codes of `diags`.
fn codes_of(diags: &ir::Diags) -> Vec<ir::Code> {
    diags.rows().iter().map(|r| r.code).collect()
}

/// The `Param` item `k` of `scope`.
fn param(d: &ir::Design, scope: ir::ScopeId, k: usize) -> ir::item::Param {
    let item = d.items(scope)[k];
    assert_eq!(item.kind, ItemKind::Param);
    d.row(item.index)
}

/// The opaque item `k` of `scope`: its class and text.
fn opaque(d: &ir::Design, scope: ir::ScopeId, k: usize) -> (OpaqueClass, String) {
    let item = d.items(scope)[k];
    assert_eq!(item.kind, ItemKind::Opaque, "item {k}");
    let o: ir::item::Opaque = d.row(item.index);
    (o.class, d.text(o.text).to_string())
}

/// The design a written deck reads back to equals the first read, and the text is a fixpoint.
fn round_trip(text: &str) {
    let (first, _) = deck(text);
    let once = write(&first);
    let (second, _) = deck(&once);
    assert!(
        first.eq_ignoring_spans(&second),
        "read(write(read(t))) != read(t):\n{once}"
    );
    assert_eq!(write(&second), once, "the written text is a fixpoint");
}

#[test]
fn the_title_comments_and_end_frame_a_deck() {
    // INV 14.12 (line 1 of a deck is its title), 11.07 (ngspice drops `.end` and reads on),
    // manual §1.5 (`*` comment lines).
    let text = "my deck\n* first\n.param a=1\n  * second\n.end\n.param b=2\n";
    let (d, diags) = deck(text);
    let top = d.file(d.entry()).top;
    use ItemKind::{Comment, Param, Title};
    assert_eq!(kinds(&d, top), [Title, Comment, Param, Comment, Param]);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        write(&d),
        "my deck\n* first\n.param a=1\n* second\n.param b=2\n.end\n"
    );
    round_trip(text);
}

#[test]
fn the_last_param_wins_and_a_read_before_it_binds_to_it() {
    // INV 3.01 (ngspice keeps the last definition; a parameter between the two reads the
    // last: run XY G4); ngspice-45 inp_sort_params (a .param may read one defined later).
    let (d, diags) = deck("t\n.param a=1\n.param b={a*2}\n.param a=3 c=4\n");
    let top = d.file(d.entry()).top;
    assert_eq!(
        opaque(&d, top, 1),
        (OpaqueClass::Inert, ".param a=1".to_string())
    );
    assert!(codes_of(&diags).contains(&codes::DUPLICATE_PARAM));
    let b = param(&d, top, 2);
    let a = param(&d, top, 3);
    assert_eq!((d.str(b.name), d.str(a.name)), ("b", "a"));
    let ir::Expr::Binary(ir::BinOp::Mul, read_a, _) = d.expr(b.value) else {
        panic!("b = a*2")
    };
    let ir::Expr::Param(r) = d.expr(read_a) else {
        panic!("a read")
    };
    assert_eq!(d.def(r.def).item, d.items(top)[3], "b reads the winning a");
    round_trip("t\n.param a=1\n.param b={a*2}\n.param a=3 c=4\n");
}

#[test]
fn nested_subcircuits_scope_their_parameters() {
    // Manual §2.4.3 (`.subckt` nests; `params:`), INV-constructs B2-B4; INV 3.06 (nearest
    // declaration wins); INV 11.01 (`gnd` is node 0).
    let text = "t\n.param w=9\n.subckt outer a GND params: w=1\n.param k={w+1}\n.subckt inner x\n.param j={w*k}\n.ends\n.ends outer\n";
    let (d, _) = deck(text);
    let top = d.file(d.entry()).top;
    let outer: ir::item::Subckt = d.row(d.items(top)[1].index);
    let ports: Vec<&str> = d.names(outer.ports).iter().map(|&n| d.str(n)).collect();
    assert_eq!(ports, ["a", "0"]);
    assert_eq!(kinds(&d, outer.body), [ItemKind::Param, ItemKind::Subckt]);
    let inner: ir::item::Subckt = d.row(d.items(outer.body)[1].index);
    let j = param(&d, inner.body, 0);
    let ir::Expr::Binary(_, w, k) = d.expr(j.value) else {
        panic!("w*k")
    };
    let def_of = |e| match d.expr(e) {
        ir::Expr::Param(r) => d.def(r.def),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        def_of(w).scope,
        outer.body,
        "w is outer's head parameter, not the top one"
    );
    assert_eq!(
        def_of(k).item,
        d.items(outer.body)[0],
        "k is outer's local .param"
    );
    round_trip(text);
}

#[test]
fn an_if_chain_is_one_item_with_a_branch_per_clause() {
    // Manual §2.15 (`.if`/`.elseif`/`.else`/`.endif`), INV-constructs B28 (a single `=` is
    // equality in a condition: numparam `fetchoperator` reads `=` as `==`).
    let text =
        "t\n.param s=2\n.if (s = 1)\n* one\n.elseif (s == 2)\n.include \"x.sp\"\n.else\n.endif\n";
    let (d, diags) = deck(text);
    let top = d.file(d.entry()).top;
    let i: ir::item::If = d.row(d.items(top)[1].index);
    let br = d.branches(i.branches);
    assert_eq!(br.len(), 3);
    assert!(matches!(
        d.expr(br[0].cond),
        ir::Expr::Binary(ir::BinOp::Eq, ..)
    ));
    assert!(br[2].cond.is_none());
    assert_eq!(kinds(&d, br[0].body), [ItemKind::Comment]);
    assert_eq!(opaque(&d, br[1].body, 0).0, OpaqueClass::InvalidInSource);
    assert!(codes_of(&diags).contains(&codes::BRANCH_DEF));
    assert!(d.items(br[2].body).is_empty());
    assert_eq!(
        write(&d),
        "t\n.param s=2\n.if (s == 1)\n* one\n.elseif (s == 2)\n.include \"x.sp\"\n.else\n.endif\n.end\n"
    );
}

#[test]
fn includes_resolve_against_the_sources_and_a_missing_one_is_noted() {
    // INV-constructs A8, A11 (`.include`, `.lib file section`), SPINE §2 (paths relative to the
    // includer); a library file has no title (A2, 14.12).
    let mut src = Sources::new();
    src.add(
        "dir/deck.sp",
        "t\n.include 'lib/m.sp'\n.lib \"lib/c.lib\" TT\n.inc gone.sp\n",
    );
    src.add("dir/lib/m.sp", ".param m=1\n");
    src.add("dir/lib/c.lib", ".lib tt\n.param c=2\n.endl tt\n");
    let (d, diags) = read(&src);
    let top = d.file(d.entry()).top;
    let inc = |k: usize| -> (String, String) {
        let i: ir::item::Include = d.row(d.items(top)[k].index);
        let sec = if i.section.is_none() {
            String::new()
        } else {
            d.str(i.section).into()
        };
        (d.str(i.path).into(), sec)
    };
    assert_eq!(inc(1), ("dir/lib/m.sp".into(), String::new()));
    assert_eq!(inc(2), ("dir/lib/c.lib".into(), "TT".into()));
    assert_eq!(inc(3), ("gone.sp".into(), String::new()));
    assert_eq!(codes_of(&diags), [codes::MISSING_FILE]);
    let lib = d.find_file("dir/lib/c.lib");
    assert_eq!(d.file(lib).role, ir::Role::Library);
    let ltop = d.file(lib).top;
    assert_eq!(kinds(&d, ltop), [ItemKind::Section]);
    let sec: ir::item::Section = d.row(d.items(ltop)[0].index);
    assert_eq!(
        d.str(sec.name),
        "TT",
        "one spelling per name: the first seen"
    );
    let row = dialect::by_name("ngspice").expect("row");
    let mut nd = ir::Diags::new();
    assert_eq!(
        dialect::write(&d, lib, row, &mut nd),
        ".lib TT\n.param c=2\n.endl\n"
    );
    let refs = row
        .read
        .expect("reader")
        .references("t\n.include a.sp\n.lib b.lib s\n.lib sect\n");
    assert_eq!(
        refs,
        [
            dialect::Reference::File("a.sp".into()),
            dialect::Reference::File("b.lib".into())
        ]
    );
}

#[test]
fn global_drops_ground_and_funcs_bind_their_formals() {
    // Manual §2.6 (`.global`), INV 11.01 (`gnd` is 0); manual §2.12 (`.func`).
    let text = "t\n.global vdd GND Vdd vss\n.func f(x, y) {x*y+z}\n.param z=1 p={f(2,3)}\n";
    let (d, _) = deck(text);
    let top = d.file(d.entry()).top;
    let g: ir::item::Global = d.row(d.items(top)[1].index);
    let nets: Vec<&str> = d.names(g.nets).iter().map(|&n| d.str(n)).collect();
    assert_eq!(nets, ["vdd", "vss"]);
    let f: ir::item::Func = d.row(d.items(top)[2].index);
    let ir::Expr::Binary(ir::BinOp::Add, xy, z) = d.expr(f.body) else {
        panic!("x*y+z")
    };
    let ir::Expr::Binary(_, x, _) = d.expr(xy) else {
        panic!("x*y")
    };
    let def = |e| match d.expr(e) {
        ir::Expr::Param(r) => d.def(r.def),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        def(x).kind,
        ir::DefKind::Func,
        "a formal binds to its function"
    );
    assert_eq!(
        def(z).item,
        d.items(top)[3],
        "a free name binds to the .param"
    );
    let p = param(&d, top, 4);
    let ir::Expr::UserCall(r, _) = d.expr(p.value) else {
        panic!("f(2,3)")
    };
    assert_eq!(d.def(r.def).item, d.items(top)[2]);
    assert_eq!(
        write(&d),
        "t\n.global vdd vss\n.func f(x, y) {x * y + z}\n.param z=1\n.param p={f(2, 3)}\n.end\n"
    );
    round_trip(text);
}

#[test]
fn what_ngspice_rejects_or_no_area_reads_is_opaque_and_coded() {
    // INV 2.06 (a .param reading the circuit), manual §2.4 (ports, a head default), parse.rs
    // P0004 (a stray closer or `.elseif`) and P0001 (a statement led by a number: Verbatim);
    // `.title` (no rule yet). Only statements the spine and the structure area own: the areas'
    // cards (elements, models, analyses, outputs, `.control`) are their own tests'.
    let text = "t\n.param p={v(a)}\n.subckt s a a\nX1 a\n.ends\n.ends\n.elseif (1)\n.subckt q b w=\n.ends\n5 a b\n.title x\n.global gnd\n";
    let (d, diags) = deck(text);
    let top = d.file(d.entry()).top;
    assert_eq!(d.items(top)[0].kind, ItemKind::Title);
    let classes: Vec<OpaqueClass> = (1..d.items(top).len())
        .map(|k| opaque(&d, top, k).0)
        .collect();
    use OpaqueClass::{Inert, InvalidInSource as Bad, Unread};
    assert_eq!(classes, [Bad, Bad, Bad, Bad, Bad, Unread, Unread, Inert]);
    let c = codes_of(&diags);
    let want = [
        codes::PARAM_READS_CIRCUIT,
        codes::MALFORMED,
        codes::MALFORMED,
        codes::MALFORMED,
        codes::MALFORMED,
        codes::UNREAD,
        codes::UNREAD,
        codes::GLOBAL_GROUND,
    ];
    assert_eq!(c, want);
    assert_eq!(
        opaque(&d, top, 2).1,
        ".subckt s a a\nX1 a\n.ends",
        "a block is one statement, its body in it"
    );
    round_trip(text);
}

#[test]
fn a_strict_read_refuses_unread_statements() {
    // SPINE §3, RULINGS R10: Policy::Strict turns each Unread into a refusal.
    let row = dialect::by_name("ngspice").expect("row");
    let src = Sources::one("d.sp", "t\n5 a b\n");
    assert!(dialect::transpile(&src, row, row, Policy::Strict).is_err());
    let ok = dialect::transpile(&src, row, row, Policy::Lenient).expect("lenient");
    assert_eq!(ok.text, "t\n5 a b\n.end\n");
}

#[test]
fn ten_thousand_nested_subcircuits_read_and_write_in_a_debug_build() {
    // DIALECT-NGSPICE S3.0: walks whose depth follows the input use explicit stacks.
    let n = 10_000;
    let mut text = String::from("t\n");
    for k in 0..n {
        text.push_str(&format!(".subckt s{k} a\n"));
    }
    for _ in 0..n {
        text.push_str(".ends\n");
    }
    let (d, _) = deck(&text);
    let out = write(&d);
    assert_eq!(out.matches(".subckt").count(), n);
    assert_eq!(out.matches(".ends").count(), n);
}

#[test]
fn any_bytes_read_returns_and_checks_clean() {
    // L1, L2 on noise: never a panic, never an X row (`read` asserts none).
    const ALPHABET: &[u8] = b".param .subckt .ends .if .else .endif .lib .endl .func .include ( ) { } = + - * ? : , \n x1 gnd 1k a b ' \"";
    let mut state = 7u64;
    for _ in 0..64 {
        let mut text = String::from("t\n");
        for _ in 0..300 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            text.push(ALPHABET[(state >> 33) as usize % ALPHABET.len()] as char);
        }
        let _ = deck(&text);
    }
}

#[test]
fn what_ngspice_cannot_spell_and_no_feature_states_is_refused_by_admits() {
    // R21 (`Write::admits`): never a literal (R28: respelled exactly), a head parameter with no default (manual
    // §2.4.3), a multi-line comment, an `.if` reading the temperature (numparam decides it once),
    // a string value (numparam has none).
    let mut d = ir::Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("d.sp", ir::Role::Deck);
        let centi = b.literal(ir::Literal {
            mantissa: 3,
            frac: 0,
            exp10: -2,
            form: ir::NumForm::Scale,
        });
        b.param(top, "a", centi, ir::Span::NONE);
        let mil = b.literal(ir::Literal {
            mantissa: 2,
            frac: 0,
            exp10: 0,
            form: ir::NumForm::Mil,
        });
        b.param(top, "b", mil, ir::Span::NONE);
        b.subckt(top, "s", &["p"], &[("w", ir::ExprId::NONE)], ir::Span::NONE);
        b.comment(top, "two\nlines", ir::Span::NONE);
        let t = b.temp();
        b.if_(top, &[t], ir::Span::NONE);
        let text = b.string("4.8.3");
        b.param(top, "v", text, ir::Span::NONE);
    }
    let row = dialect::by_name("ngspice").expect("row");
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut d, row, &mut diags);
    // the admits rows; `U0001` for `Expr(Str)` also comes until the model area's FEATURES add it
    let mut got: Vec<ir::Code> = codes_of(&diags)
        .into_iter()
        .filter(|c| c.letter == b'W')
        .collect();
    got.sort();
    let mut want = vec![
        codes::NO_DEFAULT,
        codes::LINE_BREAK,
        codes::READS_RUN,
        codes::TEXT_VALUE,
    ];
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn a_row_reference_queued_by_an_area_binds_to_a_definition_pushed_later() {
    // R3 (the reader binds by ngspice's rule: the nearest definition outward), A11
    // (`Builder::bind_ref`): an instance before its `.subckt`, as an X card standing first.
    let mut d = ir::Design::new();
    let (item, sub) = {
        let mut b = d.builder();
        let (_, top) = b.file("d.sp", ir::Role::Deck);
        let head = b.head("x1", &["a"], Some("s"), &[], ir::ExprId::NONE);
        let item = b.instance(top, ir::ElemKind::Instance, head, ir::Span::NONE);
        let (sub, _) = b.subckt(top, "s", &["p"], &[], ir::Span::NONE);
        (item, sub)
    };
    let mut b = d.builder();
    let name = b.design().find_name("s");
    let top = b.design().file(b.design().entry()).top;
    let rows = [super::read::RowRef {
        item,
        k: 0,
        scope: top,
        kind: ir::DefKind::Subckt,
        name,
    }];
    super::read::bind_rows(&mut b, &rows);
    let e: ir::Element = d.row(item.index);
    assert_eq!(d.def(e.model.def).item, sub);
}

#[test]
fn a_read_in_a_measure_binds_to_a_measure_before_a_parameter() {
    // ngspice-45 measure.c do_measure: every other measure runs first and enters its result with
    // nupa_add_param, so inside `.meas ... param=` a measure hides a `.param` of its name; outside
    // one the `.param` is read.
    let mut d = ir::Design::new();
    let mut b = d.builder();
    let (_, top) = b.file("d.sp", ir::Role::Deck);
    let one = b.num(1.0);
    let p = b.param(top, "x", one, ir::Span::NONE);
    let c = b.comment(top, "stands in for a measure item", ir::Span::NONE);
    let x = b.name("x");
    let m = b.define(ir::DefKind::Measure, x, top, c);
    let inside = b.param_ref("x");
    let outside = b.param_ref("x");
    let pending = [(inside, top), (outside, top)];
    super::read::bind_reads(&mut b, &pending, &[(0, 1)]);
    let def = |e| match b.design().expr(e) {
        ir::Expr::Param(r) => r.def,
        other => panic!("{other:?}"),
    };
    assert_eq!(def(inside), m);
    assert_eq!(b.design().def(def(outside)).item, p);
}

#[test]
fn a_subckts_formals_are_known_from_any_enclosing_block() {
    // inpcom.c found_mult_param: an X line's m= is the multiplier unless the subcircuit declares
    // m; the declare step answers for a .subckt defined anywhere in the enclosing blocks.
    let lex = crate::syntax::Lang::NGSPICE.lexicon().expect("lexicon");
    let text = "t\n.subckt outer a\n.subckt inner b params: M=1 w=2\n.ends\n.ends\n.subckt top c m=3\n.ends\n";
    let toks = crate::syntax::tokenize(text, lex);
    let ast = crate::syntax::parse(&toks, lex);
    let d = super::declare::declare(&ast, &toks);
    let root = ast.root();
    assert_eq!(d.formals(root, "TOP"), Some(&["m".to_string()][..]));
    assert_eq!(d.formals(root, "inner"), None, "inner is outer's");
    let outer = crate::syntax::ast::Node(ast.fields(root)[1]);
    let want = ["m".to_string(), "w".to_string()];
    assert_eq!(d.formals(outer, "inner"), Some(&want[..]));
}

#[test]
fn a_blank_line_one_is_an_empty_title() {
    // INV 14.12: line 1 of a deck is its title whatever it holds, so every ngspice deck has one.
    let (d, _) = deck("\n.param a=1\n");
    let top = d.file(d.entry()).top;
    let t: ir::item::Title = d.row(d.items(top)[0].index);
    assert_eq!(d.items(top)[0].kind, ItemKind::Title);
    assert_eq!(d.text(t.text), "");
    assert_eq!(write(&d), "\n.param a=1\n.end\n");
}

#[test]
fn width_and_probe_cards_reach_the_output_area() {
    // ngspice's lexicon has no `.probe` keyword, so both arrive as plain cards; the dispatch
    // sends them to `read_outputs::card` (its stub: `Unread` with `P1001`), not the run area.
    let (d, diags) = deck("t\n.width out=80\n.probe v(a)\n");
    let top = d.file(d.entry()).top;
    assert_eq!(opaque(&d, top, 1).0, OpaqueClass::Unread);
    assert_eq!(opaque(&d, top, 2).0, OpaqueClass::Unread);
    assert_eq!(diags.len(), 2);
}
