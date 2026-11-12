//! The structure items → ngspice cards: `Title` (line 1, the spine's), `Comment` → `*text`,
//! `Include` → `.include "path"` or `.lib "path" section`, `Global` → `.global`, `Opaque` →
//! its text copied (this family's only: the spine refuses another's), `Subckt` → `.subckt …
//! params: …` and `.ends`, `Section` → `.lib name` and `.endl`, `If` → `.if (c)` … `.endif`,
//! `Param` → `.param name=value`, `Func` → `.func name(args) {body}`. Each is an area function
//! of `write.rs`'s dispatch (`fn(&mut Out, ir::ItemRef)`); `FEATURES` is what they spell.
//!
//! Cites: ngspice manual §2.3, §2.4, §2.6–§2.8, §2.12, §2.15; `rewrite/DIALECT-NGSPICE.md` §4;
//! `rewrite/INV-constructs.md` A3, A8, A11, A13, A21, B1–B6, B13, C1, C6.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) fn title`, `comment`,
//! `include`, `global`, `opaque`, `subckt`, `section`, `if_`, `param`, `func` (each
//! `(w: &mut Out<'_>, item: ir::ItemRef)`); `pub(super) fn admits(&ir::Design, ir::ItemRef,
//! ir::ScopeId, &mut ir::Diags)`

use super::codes;
use super::expr_write;
use super::tables;
use super::write::Out;
use crate::ir::{self, Extra, Feature, FeatureSet, ItemKind};

/// The structure items and placements ngspice spells (A13: `CircuitExtra::NameWithoutLetter`
/// stays out of the union when the circuit owner adds it): the kernel items but `Load` (an OSDI
/// load is a `.control` command, S3.4), nested subcircuits and subcircuit-local functions
/// (manual §2.4.3, B3, B6), and library calls (A11). Not: a `.param` or load in an `.if` branch
/// (manual §2.15), a section inside a library wrapper (Spectre's), names that differ only in
/// case (ngspice folds them).
pub(super) const FEATURES: FeatureSet = tables::feature_set(&[
    Feature::Item(ItemKind::Title),
    Feature::Item(ItemKind::Comment),
    Feature::Item(ItemKind::Include),
    Feature::Item(ItemKind::Global),
    Feature::Item(ItemKind::Opaque),
    Feature::Item(ItemKind::Subckt),
    Feature::Item(ItemKind::Section),
    Feature::Item(ItemKind::If),
    Feature::Item(ItemKind::Param),
    Feature::Item(ItemKind::Func),
    Feature::Extra(Extra::NestedSubckt),
    Feature::Extra(Extra::LocalFunc),
    Feature::Extra(Extra::IncludeSection),
    // ngspice resolves a name through every enclosing subcircuit (numparam's nested levels), and
    // a head default may read a parameter (A12 K2)
    Feature::Extra(Extra::EnclosingRef),
    Feature::Extra(Extra::FormalReadsParam),
]);

/// A `Title` past the first item: the IR puts it first, and the spine writes line 1, so a
/// title here is a check violation that never reaches a writer.
pub(super) fn title(_w: &mut Out<'_>, _item: ir::ItemRef) {
    unreachable!("ngspice write: a title is the first item of a deck (ir::check)")
}

/// `*text`, one line per line of the comment.
pub(super) fn comment(w: &mut Out<'_>, item: ir::ItemRef) {
    let c: ir::item::Comment = w.design.row(item.index);
    let text = w.design.text(c.text).to_string();
    for line in text.split('\n') {
        w.out.push('*');
        w.line(line);
    }
}

/// `.include "path"`, or `.lib "path" section` for one section.
pub(super) fn include(w: &mut Out<'_>, item: ir::ItemRef) {
    let inc: ir::item::Include = w.design.row(item.index);
    let path = w.design.str(inc.path).to_string();
    if inc.section.is_none() {
        w.line(&format!(".include \"{path}\""));
    } else {
        let section = w.design.str(inc.section).to_string();
        w.line(&format!(".lib \"{path}\" {section}"));
    }
}

/// `.global n …`.
pub(super) fn global(w: &mut Out<'_>, item: ir::ItemRef) {
    let g: ir::item::Global = w.design.row(item.index);
    let mut text = String::from(".global");
    for &n in w.design.names(g.nets) {
        text.push(' ');
        text.push_str(w.design.str(n));
    }
    w.line(&text);
}

/// The statement's ngspice text as read. Asserts it is this family's (the spine refuses any
/// other before the writer runs).
pub(super) fn opaque(w: &mut Out<'_>, item: ir::ItemRef) {
    let o: ir::item::Opaque = w.design.row(item.index);
    assert_eq!(
        w.design.str(o.dialect),
        "ngspice",
        "ngspice write: another family's opaque (dialect::write refuses it)"
    );
    let text = w.design.text(o.text).to_string();
    w.line(&text);
}

/// `.subckt name ports params: k=v …`, the body, `.ends`.
pub(super) fn subckt(w: &mut Out<'_>, item: ir::ItemRef) {
    let s: ir::item::Subckt = w.design.row(item.index);
    let mut text = format!(".subckt {}", w.design.str(s.name));
    for &p in w.design.names(s.ports) {
        text.push(' ');
        text.push_str(w.design.str(p));
    }
    let params = w.design.params(s.params);
    if !params.is_empty() {
        text.push_str(" params:");
    }
    for &(name, value) in params {
        text.push(' ');
        text.push_str(w.design.str(name));
        assert!(
            !value.is_none(),
            "ngspice: a subcircuit parameter has a default"
        );
        text.push('=');
        expr_write::value(w.design, value, &mut text);
    }
    w.line(&text);
    w.open(s.body, ".ends\n".to_string());
}

/// `.lib name`, the body, `.endl`.
pub(super) fn section(w: &mut Out<'_>, item: ir::ItemRef) {
    let s: ir::item::Section = w.design.row(item.index);
    let text = format!(".lib {}", w.design.str(s.name));
    w.line(&text);
    w.open(s.body, ".endl\n".to_string());
}

/// `.if (c)`, then each branch's body after its clause line, then `.endif`.
pub(super) fn if_(w: &mut Out<'_>, item: ir::ItemRef) {
    let i: ir::item::If = w.design.row(item.index);
    let branches = w.design.branches(i.branches).to_vec();
    assert!(!branches.is_empty(), "an if has a branch");
    let clause = |w: &Out<'_>, k: usize| {
        let b = branches[k];
        if b.cond.is_none() {
            return ".else\n".to_string();
        }
        let mut s = String::from(if k == 0 { ".if (" } else { ".elseif (" });
        expr_write::expr(w.design, b.cond, &mut s);
        s.push_str(")\n");
        s
    };
    let head = clause(w, 0);
    w.out.push_str(&head);
    for k in (0..branches.len()).rev() {
        let tail = if k + 1 < branches.len() {
            clause(w, k + 1)
        } else {
            ".endif\n".to_string()
        };
        w.open(branches[k].body, tail);
    }
}

/// `.param name=value`.
pub(super) fn param(w: &mut Out<'_>, item: ir::ItemRef) {
    let p: ir::item::Param = w.design.row(item.index);
    let mut text = format!(".param {}=", w.design.str(p.name));
    expr_write::value(w.design, p.value, &mut text);
    w.line(&text);
}

/// `.func name(a, b) {body}`.
pub(super) fn func(w: &mut Out<'_>, item: ir::ItemRef) {
    let f: ir::item::Func = w.design.row(item.index);
    let args: Vec<&str> = w
        .design
        .names(f.args)
        .iter()
        .map(|&a| w.design.str(a))
        .collect();
    let mut text = format!(".func {}({}) {{", w.design.str(f.name), args.join(", "));
    expr_write::expr(w.design, f.body, &mut text);
    text.push('}');
    w.line(&text);
}

/// Appends a refusal for a structure item ngspice cannot spell for a reason no feature states
/// (R21): a head parameter with no default (`W1001`), a multi-line title or comment (`W1002`), a
/// section in a section or a definition or include in an `.if` branch (`W1004`), an `.if`
/// condition that reads the run or a value that reads the circuit (`W1005`), a string or list
/// value (`W1006`).
pub(super) fn admits(d: &ir::Design, item: ir::ItemRef, scope: ir::ScopeId, diags: &mut ir::Diags) {
    let span = d.span_of(item);
    let mut refuse = |code| diags.push(code, span, 0);
    let in_kind = |k: ir::ScopeKind| d.scope(scope).kind == k;
    let broken = |t: &str| t.contains(['\n', '\r']);
    let mut values: Vec<(ir::ExprId, ir::ExprClass)> = Vec::new();
    match item.kind {
        ItemKind::Title => {
            let t: ir::item::Title = d.row(item.index);
            let text = d.text(t.text);
            if broken(text) {
                refuse(codes::LINE_BREAK);
            }
        }
        ItemKind::Comment => {
            let c: ir::item::Comment = d.row(item.index);
            if broken(d.text(c.text)) {
                refuse(codes::LINE_BREAK);
            }
        }
        ItemKind::Subckt => {
            let s: ir::item::Subckt = d.row(item.index);
            for &(_, v) in d.params(s.params) {
                if v.is_none() {
                    refuse(codes::NO_DEFAULT);
                } else {
                    values.push((v, ir::ExprClass::Circuit));
                }
            }
        }
        ItemKind::If => {
            let i: ir::item::If = d.row(item.index);
            for b in d.branches(i.branches).iter().filter(|b| !b.cond.is_none()) {
                values.push((b.cond, ir::ExprClass::Temp));
            }
        }
        ItemKind::Param => {
            let p: ir::item::Param = d.row(item.index);
            values.push((p.value, ir::ExprClass::Circuit));
        }
        ItemKind::Func => {
            let f: ir::item::Func = d.row(item.index);
            values.push((f.body, ir::ExprClass::Circuit));
        }
        _ => {}
    }
    let placed = match item.kind {
        ItemKind::Section => in_kind(ir::ScopeKind::Section) || in_kind(ir::ScopeKind::Branch),
        ItemKind::Subckt | ItemKind::Func | ItemKind::Include => in_kind(ir::ScopeKind::Branch),
        _ => false,
    };
    if placed {
        refuse(codes::PLACEMENT);
    }
    for (e, limit) in values {
        let class = ir::expr_class(d, e);
        if item.kind != ItemKind::Func && class >= limit {
            refuse(codes::READS_RUN);
        }
        if expr_write::holds_text(d, e) {
            refuse(codes::TEXT_VALUE);
        }
    }
}
