//! The storage core: names and texts, pools, files, scopes, item tables, definitions and lookup,
//! the editor, and the two design equalities.
//!
//! Oracles: the inverse (what goes in through `Builder` comes back by handle through `Design`);
//! a model (a plain `Vec` of what was pushed, compared with the design's ids and order);
//! hand-built designs whose expected rows are written out in the test.
//!
//! Contents: none (tests only).

use spicerack_ir::item::{Branch, Func, If, Include, Param, Section, Subckt, Title};
use spicerack_ir::scope::{File, Scope};
use spicerack_ir::{
    BinOp, DefId, DefKind, Design, Expr, ExprId, FileId, ItemKind, ItemRef, LoadKind, NameId,
    OpaqueClass, Ref, Role, ScopeId, ScopeKind, Span,
};

// ---- private ----

fn span(start: u32) -> Span {
    Span {
        file: FileId(0),
        start,
        len: 4,
    }
}

#[test]
fn ground_is_name_zero() {
    let d = Design::new();
    assert_eq!(d.find_name("0"), NameId::GROUND);
    assert_eq!(d.str(NameId::GROUND), "0");
    assert_eq!(d.find_name("a"), NameId::NONE);
    assert_eq!(d.files().count(), 0);
    assert_eq!(d.expr_count(), 0);
    assert!(d.definitions().is_empty());
}

#[test]
fn names_are_exact_bytes_in_first_seen_order() {
    let mut d = Design::new();
    // model: the distinct names in first-seen order, after ground
    let input = ["a", "B", "a", "b", "net-1", "B", "vdd!", "x.y", "µ"];
    let mut model: Vec<&str> = vec!["0"];
    let mut ids = Vec::new();
    {
        let mut b = d.builder();
        for s in input {
            ids.push(b.name(s));
            if !model.contains(&s) {
                model.push(s);
            }
        }
    }
    for (s, id) in input.iter().zip(&ids) {
        let want = model.iter().position(|m| m == s).unwrap() as u32;
        assert_eq!(id.0, want, "{s}");
        assert_eq!(d.str(*id), *s);
        assert_eq!(d.find_name(s), *id);
    }
    assert_ne!(
        d.find_name("a"),
        d.find_name("A"),
        "the IR never folds case"
    );
}

#[test]
fn many_names_round_trip() {
    let mut d = Design::new();
    let model: Vec<String> = (0..20_000)
        .map(|i| format!("n{}", i * 7919 % 20_000))
        .collect();
    let ids: Vec<NameId> = {
        let mut b = d.builder();
        model.iter().map(|s| b.name(s)).collect()
    };
    for (s, id) in model.iter().zip(&ids) {
        assert_eq!(d.str(*id), s);
    }
}

#[test]
#[should_panic(expected = "a name is non-empty")]
fn empty_name_asserts() {
    let mut d = Design::new();
    d.builder().name("");
}

#[test]
fn texts_and_pools_come_back() {
    let mut d = Design::new();
    let (t1, t2, t3, names, exprs, params, empty, e1, e2) = {
        let mut b = d.builder();
        let t1 = b.text("hello");
        let t2 = b.text("hello");
        let t3 = b.text("");
        let names = b.names(&["x", "y", "x"]);
        let e1 = b.num(1.0);
        let e2 = b.num(2.0);
        let exprs = b.exprs(&[e2, e1]);
        let params = b.params(&[("w", e1), ("l", e2)]);
        let empty = b.names(&[]);
        (t1, t2, t3, names, exprs, params, empty, e1, e2)
    };
    assert_ne!(t1, t2, "texts are appended, not interned");
    assert_eq!(d.text(t1), "hello");
    assert_eq!(d.text(t2), "hello");
    assert_eq!(d.text(t3), "");
    let (x, y) = (d.find_name("x"), d.find_name("y"));
    assert_eq!(d.names(names), &[x, y, x]);
    assert_eq!(d.exprs(exprs), &[e2, e1]);
    assert_eq!(
        d.params(params),
        &[(d.find_name("w"), e1), (d.find_name("l"), e2)]
    );
    assert!(d.names(empty).is_empty());
    assert_eq!(names.len(), 3);
}

#[test]
fn files_scopes_and_entry() {
    let mut d = Design::new();
    let (lib, lib_top, deck, deck_top) = {
        let mut b = d.builder();
        let (lib, lib_top) = b.file("models.lib", Role::Library);
        let (deck, deck_top) = b.file("top.sp", Role::Deck);
        (lib, lib_top, deck, deck_top)
    };
    assert_eq!((lib, deck), (FileId(0), FileId(1)));
    assert_eq!(d.files().collect::<Vec<_>>(), vec![lib, deck]);
    assert_eq!(
        d.file(deck),
        File {
            path: d.find_name("top.sp"),
            role: Role::Deck,
            top: deck_top
        }
    );
    assert_eq!(
        d.scope(lib_top),
        Scope {
            kind: ScopeKind::File,
            parent: ScopeId::NONE,
            owner: ItemRef::NONE,
            file: lib
        }
    );
    assert_eq!(d.entry(), deck, "the first Deck file");
    assert_eq!(d.find_file("models.lib"), lib);
    assert_eq!(d.find_file("nope.sp"), FileId::NONE);
    assert!(d.items(deck_top).is_empty());
}

#[test]
#[should_panic(expected = "a file path is unique")]
fn duplicate_file_asserts() {
    let mut d = Design::new();
    let mut b = d.builder();
    b.file("a.sp", Role::Deck);
    b.file("a.sp", Role::Library);
}

#[test]
fn items_keep_order_rows_and_spans() {
    let mut d = Design::new();
    let (top, items) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let t = b.title(top, "rc divider", span(0));
        let c = b.comment(top, "a note", span(10));
        let one = b.num(1.0);
        let p = b.param(top, "rval", one, span(20));
        let i = b.include(top, "models.lib", Some("tt"), span(30));
        let g = b.global(top, &["vdd", "vss"], span(40));
        let l = b.load(top, LoadKind::Osdi, "psp.osdi", span(50));
        let o = b.opaque(top, "ngspice", OpaqueClass::Private, ".stateye x", span(60));
        (top, vec![t, c, p, i, g, l, o])
    };
    assert_eq!(d.items(top), items.as_slice());
    let kinds: Vec<ItemKind> = items.iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        [
            ItemKind::Title,
            ItemKind::Comment,
            ItemKind::Param,
            ItemKind::Include,
            ItemKind::Global,
            ItemKind::Load,
            ItemKind::Opaque
        ]
    );
    for (k, item) in items.iter().enumerate() {
        assert_eq!(d.span_of(*item), span(10 * k as u32));
    }
    let title: Title = d.row(items[0].index);
    assert_eq!(d.text(title.text), "rc divider");
    let inc: Include = d.row(items[3].index);
    assert_eq!(
        (inc.path, inc.section),
        (d.find_name("models.lib"), d.find_name("tt"))
    );
    let p: Param = d.row(items[2].index);
    assert_eq!(p.name, d.find_name("rval"));
    let def = d.lookup(top, DefKind::Param, p.name);
    assert_eq!(d.def(def).item, items[2], "a param push defines it");
}

#[test]
fn subckt_section_if_and_their_scopes() {
    let mut d = Design::new();
    let (top, sub, body, sec, sec_body, iff, branches, w) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let w = b.num(1e-6);
        let (sub, body) = b.subckt(top, "inv", &["a", "y"], &[("w", w)], Span::NONE);
        let (sec, sec_body) = b.section(top, "tt", Some("lib1"), Span::NONE);
        let cond = b.param_ref("w");
        let (iff, branches) = b.if_(body, &[cond, ExprId::NONE], Span::NONE);
        (top, sub, body, sec, sec_body, iff, branches, w)
    };
    assert_eq!(
        d.scope(body),
        Scope {
            kind: ScopeKind::Subckt,
            parent: top,
            owner: sub,
            file: FileId(0)
        }
    );
    let s: Subckt = d.row(sub.index);
    assert_eq!(s.body, body);
    assert_eq!(d.names(s.ports), &[d.find_name("a"), d.find_name("y")]);
    assert_eq!(d.params(s.params), &[(d.find_name("w"), w)]);
    let sc: Section = d.row(sec.index);
    assert_eq!((sc.library, sc.body), (d.find_name("lib1"), sec_body));
    assert_eq!(d.scope(sec_body).kind, ScopeKind::Section);
    let i: If = d.row(iff.index);
    assert_eq!(i.branches, branches);
    let bs: Vec<Branch> = d.branches(branches).to_vec();
    assert_eq!(bs.len(), 2);
    assert_eq!(bs[1].cond, ExprId::NONE);
    for br in &bs {
        assert_eq!(
            d.scope(br.body),
            Scope {
                kind: ScopeKind::Branch,
                parent: body,
                owner: iff,
                file: FileId(0)
            }
        );
    }
    assert_eq!(d.items(body), &[iff]);
    assert_eq!(d.items(top), &[sub, sec]);
    // definitions: the subckt in top, its head default in its body, the section in top
    let (inv, wn, tt) = (d.find_name("inv"), d.find_name("w"), d.find_name("tt"));
    assert_eq!(d.def(d.lookup(top, DefKind::Subckt, inv)).item, sub);
    assert_eq!(
        d.def(d.lookup(body, DefKind::Subckt, inv)).item,
        sub,
        "outward lookup"
    );
    assert_eq!(d.def(d.lookup(bs[0].body, DefKind::Param, wn)).item, sub);
    assert_eq!(
        d.lookup(top, DefKind::Param, wn),
        DefId::NONE,
        "a head default is the body's"
    );
    assert_eq!(d.def(d.lookup(top, DefKind::Section, tt)).item, sec);
    assert_eq!(
        d.lookup(top, DefKind::Model, inv),
        DefId::NONE,
        "kinds are separate"
    );
}

#[test]
fn inner_definition_shadows_outer() {
    let mut d = Design::new();
    let (top, body, outer, inner) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let one = b.num(1.0);
        let outer = b.param(top, "k", one, Span::NONE);
        let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
        let two = b.num(2.0);
        let inner = b.param(body, "k", two, Span::NONE);
        (top, body, outer, inner)
    };
    let k = d.find_name("k");
    assert_eq!(d.def(d.lookup(body, DefKind::Param, k)).item, inner);
    assert_eq!(d.def(d.lookup(top, DefKind::Param, k)).item, outer);
}

#[test]
fn func_binds_its_formal_arguments() {
    let mut d = Design::new();
    let (top, x, y, item) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let x = b.param_ref("x");
        let y = b.param_ref("y");
        let body = b.binary(BinOp::Mul, x, y);
        let item = b.func(top, "f", &["x"], body, Span::NONE);
        (top, x, y, item)
    };
    let f = d.lookup(top, DefKind::Func, d.find_name("f"));
    assert_eq!(d.def(f).item, item);
    assert_eq!(
        d.expr(x),
        Expr::Param(Ref {
            name: d.find_name("x"),
            def: f
        })
    );
    assert_eq!(d.expr(y), Expr::Param(Ref::unresolved(d.find_name("y"))));
    let row: Func = d.row(item.index);
    assert_eq!(d.names(row.args), &[d.find_name("x")]);
}

#[test]
fn define_and_resolve() {
    let mut d = Design::new();
    let (top, load, module, p, e, call) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let load = b.load(top, LoadKind::VerilogA, "psp.va", Span::NONE);
        let n = b.name("psp103va");
        let module = b.define(DefKind::Module, n, top, load);
        let one = b.num(1.0);
        let p = b.param(top, "g", one, Span::NONE);
        let e = b.param_ref("g");
        let call = b.user_call("h", &[e]);
        (top, load, module, p, e, call)
    };
    assert_eq!(d.def(module).kind, DefKind::Module);
    assert_eq!(d.def(module).item, load);
    assert_eq!(
        d.lookup(top, DefKind::Module, d.find_name("psp103va")),
        module
    );
    let g = d.lookup(top, DefKind::Param, d.find_name("g"));
    assert_eq!(d.def(g).item, p);
    d.builder().resolve(e, g);
    assert_eq!(
        d.expr(e),
        Expr::Param(Ref {
            name: d.find_name("g"),
            def: g
        })
    );
    let Expr::UserCall(r, _) = d.expr(call) else {
        panic!("a user call")
    };
    assert!(!r.is_resolved());
}

#[test]
#[should_panic(expected = "one definition per scope, kind and name")]
fn duplicate_definition_asserts() {
    let mut d = Design::new();
    let mut b = d.builder();
    let (_, top) = b.file("top.sp", Role::Deck);
    let one = b.num(1.0);
    b.param(top, "k", one, Span::NONE);
    b.param(top, "k", one, Span::NONE);
}

#[test]
#[should_panic(expected = "resolve binds a definition of the same name")]
fn resolve_to_the_wrong_name_asserts() {
    let mut d = Design::new();
    let mut b = d.builder();
    let (_, top) = b.file("top.sp", Role::Deck);
    let one = b.num(1.0);
    b.param(top, "k", one, Span::NONE);
    let def = b
        .design()
        .lookup(top, DefKind::Param, b.design().find_name("k"));
    let e = b.param_ref("j");
    b.resolve(e, def);
}

#[test]
fn editor_rewrites_rows_and_lists() {
    let mut d = Design::new();
    let (top, a, c) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let a = b.comment(top, "first", span(0));
        let c = b.comment(top, "second", span(10));
        (top, a, c)
    };
    {
        let mut ed = d.editor();
        ed.items_mut(top).reverse();
        let t = ed.builder().text("rewritten");
        ed.set(a.index, spicerack_ir::item::Comment { text: t });
        assert_eq!(ed.design().items(top), &[c, a]);
    }
    let row: spicerack_ir::item::Comment = d.row(a.index);
    assert_eq!(d.text(row.text), "rewritten");
    assert_eq!(d.span_of(a), span(0), "set keeps the span");
}

#[test]
fn equality_ignores_spans_and_meaning_ignores_spelling() {
    let build = |start: u32, k: spicerack_ir::Literal| {
        let mut d = Design::new();
        {
            let mut b = d.builder();
            let (_, top) = b.file("top.sp", Role::Deck);
            let v = b.literal(k);
            b.param(
                top,
                "r",
                v,
                Span {
                    file: FileId(0),
                    start,
                    len: 1,
                },
            );
        }
        d
    };
    use spicerack_ir::{Literal, NumForm};
    let k = Literal {
        mantissa: 1,
        frac: 0,
        exp10: 3,
        form: NumForm::Scale,
    };
    let e3 = Literal {
        mantissa: 1,
        frac: 0,
        exp10: 3,
        form: NumForm::Exponent,
    };
    let two = Literal {
        mantissa: 2,
        frac: 0,
        exp10: 3,
        form: NumForm::Scale,
    };
    assert!(
        build(0, k).eq_ignoring_spans(&build(99, k)),
        "spans are ignored"
    );
    assert!(
        !build(0, k).eq_ignoring_spans(&build(0, e3)),
        "1k and 1e3 are two literals"
    );
    assert!(build(0, k).eq_meaning(&build(5, e3)), "but one number");
    assert!(!build(0, k).eq_meaning(&build(0, two)));
    assert!(Design::new().eq_ignoring_spans(&Design::new()));
}

#[test]
fn reachable_is_files_in_order_bodies_after_their_item() {
    let mut d = Design::new();
    let want = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let c = b.comment(top, "first", Span::NONE);
        let (outer, outer_body) = b.subckt(top, "outer", &[], &[], Span::NONE);
        let (inner, inner_body) = b.subckt(outer_body, "inner", &[], &[], Span::NONE);
        let deep = b.comment(inner_body, "deep", Span::NONE);
        let one = b.num(1.0);
        let (iff, branches) = b.if_(outer_body, &[one, ExprId::NONE], Span::NONE);
        let (then_body, else_body) = {
            let br = b.design().branches(branches);
            (br[0].body, br[1].body)
        };
        let t = b.comment(then_body, "then", Span::NONE);
        let e = b.comment(else_body, "else", Span::NONE);
        let after = b.comment(top, "after", Span::NONE);
        let (_, lib) = b.file("lib.sp", Role::Library);
        let l = b.comment(lib, "lib", Span::NONE);
        vec![
            (c, top),
            (outer, top),
            (inner, outer_body),
            (deep, inner_body),
            (iff, outer_body),
            (t, then_body),
            (e, else_body),
            (after, top),
            (l, lib),
        ]
    };
    assert_eq!(d.reachable(), want);
}

#[test]
fn an_empty_list_is_the_one_empty_list() {
    let mut d = Design::new();
    let (lists, sub) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        b.global(top, &["vdd"], Span::NONE); // the name pool is no longer empty
        let one = b.num(1.0);
        b.exprs(&[one]);
        let lists = (b.names(&[]), b.name_ids(&[]), b.exprs(&[]), b.params(&[]));
        let (sub, _) = b.subckt(top, "s", &[], &[], Span::NONE);
        (lists, sub)
    };
    use spicerack_ir::{ExprList, NameList, ParamList};
    assert_eq!(
        lists,
        (
            NameList::EMPTY,
            NameList::EMPTY,
            ExprList::EMPTY,
            ParamList::EMPTY
        )
    );
    let row: Subckt = d.row(sub.index);
    assert_eq!((row.ports, row.params), (NameList::EMPTY, ParamList::EMPTY));
}

#[test]
fn bind_ref_binds_a_row_reference_pushed_before_its_definition() {
    use spicerack_ir::{ElemKind, Element, NO_ROW, NameList, ParamList};
    // an instance pushed before its subcircuit, bound afterwards by the reader's own rule,
    // equals the same pushes with the reference resolved from the start
    let build = |bind_later: bool| {
        let mut d = Design::new();
        {
            let mut b = d.builder();
            let (_, top) = b.file("top.sp", Role::Deck);
            let master = b.name("inv");
            let name = b.name("x1");
            // the subckt pushed next gets definition 0
            let def = if bind_later { DefId::NONE } else { DefId(0) };
            let row = Element {
                kind: ElemKind::Instance,
                name,
                terminals: NameList::EMPTY,
                model: Ref { name: master, def },
                params: ParamList::EMPTY,
                mult: ExprId::NONE,
                detail: NO_ROW,
            };
            let x1 = b.push(top, row, Span::NONE);
            b.subckt(top, "inv", &[], &[], Span::NONE);
            if bind_later {
                let sub = b.design().lookup(top, DefKind::Subckt, master);
                b.bind_ref(x1, 0, sub);
            }
        }
        d
    };
    assert!(build(true).eq_ignoring_spans(&build(false)));
}
