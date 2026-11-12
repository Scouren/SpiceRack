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
//! `(w: &mut Out<'_>, item: ir::ItemRef)`); `pub(super) fn admits(&ir::Design, &mut ir::Diags)`

use super::codes;
use super::expr_write;
use super::tables;
use super::write::Out;
use crate::ir::{self, Extra, Feature, FeatureSet, ItemKind, ScopeKind};

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
    Feature::Extra(Extra::IncludeSection),
    // ngspice resolves a name through every enclosing subcircuit (numparam's nested levels), and
    // a head default may read a parameter (A12 K2)
    Feature::Extra(Extra::EnclosingRef),
    Feature::Extra(Extra::FormalNotConstant),
    // a head default's read of no sibling formal is looked up from each instance's scope (A29,
    // runs which–which3): unbound; a read bound outside the body (`FormalOuterRead`) ngspice
    // cannot state (R62, A37)
    Feature::Extra(Extra::FormalUnboundRead),
    // a `.param` in a `.subckt` body is that body's local, constant or not (A25, R48)
    Feature::Extra(Extra::SubcktConstantParam),
])
// where ngspice reads each structure item (runs under the S3.0 report's `runs/` decks): a
// subcircuit holds everything but a `.lib` section (`.func`, `.include`, `.global`, `.param`,
// nested `.subckt` and `.if` all run inside one); a section holds definitions, includes and
// conditionals (its `.func`, `.include`, `.subckt`, `.if` run) but no section (P0009); an `.if`
// branch holds comments, conditionals and kept text only (manual §2.15: instances and models).
.union(FeatureSet::placed(
    ScopeKind::Subckt,
    &[
        ItemKind::Comment,
        ItemKind::Include,
        ItemKind::Global,
        ItemKind::Opaque,
        ItemKind::Subckt,
        ItemKind::If,
        ItemKind::Param,
        ItemKind::Func,
    ],
))
.union(FeatureSet::placed(
    ScopeKind::Section,
    &[
        ItemKind::Comment,
        ItemKind::Include,
        ItemKind::Opaque,
        ItemKind::Subckt,
        ItemKind::If,
        ItemKind::Param,
        ItemKind::Func,
    ],
))
.union(FeatureSet::placed(
    ScopeKind::Branch,
    &[ItemKind::Comment, ItemKind::Opaque, ItemKind::If],
));

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
        w.line(&format!("*{line}"));
    }
}

/// `.include "path"`, or `.lib "path" section` for one section.
pub(super) fn include(w: &mut Out<'_>, item: ir::ItemRef) {
    let inc: ir::item::Include = w.design.row(item.index);
    let path = w.path(item, w.design.str(inc.path));
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
    // a `.control` pair is a script region's commands (R49): adjacent pairs join; a `Script`
    // pair is one whole block (one statement read back), written as it stands
    let commands = o.class != ir::OpaqueClass::Script;
    let pair = commands
        .then_some(())
        .and(text.strip_prefix(".control\n"))
        .and_then(|t| {
            let t = t.strip_suffix('\n').unwrap_or(t);
            t.strip_suffix(".endc")
                .map(|b| b.strip_suffix('\n').unwrap_or(b))
        });
    match pair {
        Some(body) => body.lines().for_each(|cmd| w.script(cmd)),
        None => w.line(&text),
    }
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
    w.line(head.trim_end_matches('\n'));
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

/// Appends a refusal for any item but a model whose expression holds a node the evaluator its
/// class picks cannot spell (`W1007`: a draw in a value that reads `temper` or the circuit,
/// which the B-source parser writes and which has no `unif`). Placements, formals without a
/// default, dynamic `.if` conditions, line breaks and string or list values (`StringValue`,
/// `ListValue`, X0020, X0021: A25 item 4) are features or invariants (A15), not refusals.
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    // ponytail: the roots through the pass door on a copy (the kernel has no read-only twin of
    // `Editor::map_exprs`); a read-only walk when the kernel has one
    let mut copy = design.clone();
    let mut ed = copy.editor();
    for (item, _) in design.reachable() {
        if item.kind == ItemKind::Model {
            continue;
        }
        let mut unspelled = false;
        ed.map_exprs(item, &mut |e, _| {
            // a string or list root is `StringValue`/`ListValue` (or X0021): a feature states it
            if e.is_none() || matches!(design.expr(*e), ir::Expr::Str(_) | ir::Expr::List(_)) {
                return;
            }
            let eval = expr_write::eval_of(design, *e);
            unspelled |= !expr_write::spells_in(design, *e, eval);
        });
        if unspelled {
            diags.push_at(codes::UNSPELLED_VALUE, design, item, 0);
        }
    }
}
