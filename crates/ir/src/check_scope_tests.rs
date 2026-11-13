//! `RefScope`'s shapes (F34): a reference bound to a definition in a subcircuit's body from a
//! sibling (a body parameter, a nested subcircuit as a master) or a function's formal read
//! outside it is reported; a subcircuit's own head, a nested body, and a file included inside
//! the body are not. A child of `check_tests.rs` (`#[path]`), for the 500-line cap.
//!
//! Oracle: hand-built designs, each the smallest of its shape, against the expected rows.
//!
//! Contents: none (tests only).

use super::{Base, base, violations};
use crate::check::Violation;
use crate::id::ExprId;
use crate::item::{DefKind, Role};
use crate::span::Span;

// ---- private ----

#[test]
fn a_reference_stands_inside_its_definitions_subcircuit() {
    use crate::element::ElemKind;
    use crate::id::Ref;
    type Shape = fn(&mut Base);
    let cases: [(&str, Shape, Vec<Violation>); 7] = [
        (
            "a sibling's body parameter",
            |b| {
                let top = b.d.file(crate::id::FileId(0)).top;
                let j = b.d.lookup(b.sub_body, DefKind::Param, b.d.find_name("j"));
                let mut bl = b.d.builder();
                let (_, body) = bl.subckt(top, "t", &["a"], &[], Span::NONE);
                let read = bl.param_ref_to(j);
                bl.param(body, "y", read, Span::NONE);
            },
            vec![Violation::RefScope],
        ),
        (
            "a function's formal read outside it",
            |b| {
                let top = b.d.file(crate::id::FileId(0)).top;
                let f = b.d.lookup(top, DefKind::Func, b.d.find_name("f"));
                let read = b.d.builder().param_ref("x");
                b.d.exprs.b[read.index()] = f.0;
                b.d.builder().param(top, "z", read, Span::NONE);
            },
            vec![Violation::RefScope],
        ),
        (
            "an instance of a subcircuit defined in a sibling's body",
            |b| {
                let top = b.d.file(crate::id::FileId(0)).top;
                let sub_body = b.sub_body;
                let mut bl = b.d.builder();
                bl.subckt(sub_body, "inner", &["p"], &[], Span::NONE);
                let (_, body) = bl.subckt(top, "t", &["a"], &[], Span::NONE);
                let inner =
                    bl.design()
                        .lookup(sub_body, DefKind::Subckt, bl.design().find_name("inner"));
                let mut h = bl.head("x1", &["a"], Some("inner"), &[], ExprId::NONE);
                h.model = Ref {
                    name: bl.design().find_name("inner"),
                    def: inner,
                };
                bl.instance(body, ElemKind::Instance, h, Span::NONE);
            },
            vec![Violation::RefScope],
        ),
        (
            "a default reading its own subcircuit's formal",
            |b| {
                let top = b.d.file(crate::id::FileId(0)).top;
                let mut bl = b.d.builder();
                let one = bl.num(1.0);
                let read = bl.param_ref("w");
                let (_, body) = bl.subckt(top, "u", &["a"], &[("w", one), ("l", read)], Span::NONE);
                let w = bl
                    .design()
                    .lookup(body, DefKind::Param, bl.design().find_name("w"));
                bl.resolve(read, w);
            },
            vec![],
        ),
        (
            "a nested subcircuit reading its enclosing formal",
            |b| {
                let w = b.d.lookup(b.sub_body, DefKind::Param, b.d.find_name("w"));
                let sub_body = b.sub_body;
                let mut bl = b.d.builder();
                let (_, inner) = bl.subckt(sub_body, "inner", &["p"], &[], Span::NONE);
                let read = bl.param_ref_to(w);
                bl.param(inner, "y", read, Span::NONE);
            },
            vec![],
        ),
        (
            "a file included in the body reading the body's formal",
            |b| {
                let w = b.d.lookup(b.sub_body, DefKind::Param, b.d.find_name("w"));
                let sub_body = b.sub_body;
                let mut bl = b.d.builder();
                bl.include(sub_body, "body.inc", None, Span::NONE);
                let (_, inc_top) = bl.file("body.inc", Role::Library);
                let read = bl.param_ref_to(w);
                bl.param(inc_top, "y", read, Span::NONE);
            },
            vec![],
        ),
        (
            "a file included elsewhere reading the body's formal",
            |b| {
                let w = b.d.lookup(b.sub_body, DefKind::Param, b.d.find_name("w"));
                let top = b.d.file(crate::id::FileId(0)).top;
                let mut bl = b.d.builder();
                bl.include(top, "body.inc", None, Span::NONE);
                let (_, inc_top) = bl.file("body.inc", Role::Library);
                let read = bl.param_ref_to(w);
                bl.param(inc_top, "y", read, Span::NONE);
            },
            vec![Violation::RefScope],
        ),
    ];
    for (what, shape, want) in cases {
        let mut b = base();
        shape(&mut b);
        assert_eq!(violations(&b.d), want, "{what}");
    }
}
