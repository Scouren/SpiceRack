//! The writer's spine: one file of a legal design → Spectre text, the reader's image read back.
//! The title on line 1 of an entry deck, then the file's items in order, each body below its
//! item and indented two spaces per level. One exhaustive `match` over `ir::ItemKind`: each arm
//! spells its item here or in its area (`write_elements.rs`, `write_run.rs`). An `Opaque` is
//! copied when its dialect is `spectre` (the spine's `legalize` refused every other).
//!
//! The structure items: `//` comments, `include "path"` (R78: relative to the file),
//! `global a b`, `parameters k=v`, `subckt name (ports)` with its formals as the body's first
//! `parameters` line, `ends name`; `if (c) {` … `} else if (c) {` … `} else {` … `}`;
//! `real f(real a) {` / `return e;` / `}`; `statistics { process { vary … } mismatch { … } }`.
//!
//! Contents: `Writer` (`Writer::line`, `Writer::value`, `Writer::opt_name`), `write`
//! (crate-private)

use std::collections::HashSet;

use super::expr_write::write_expr;
use super::{write_elements, write_run};
use crate::ir;

/// The text being written and where it is.
pub(super) struct Writer<'d, 'o> {
    /// The legal design.
    pub(super) design: &'d ir::Design,
    /// The text so far.
    pub(super) out: &'o mut String,
    /// The current depth: two spaces of indent each.
    depth: usize,
    /// The file being written: its paths are spelled relative to it (R78).
    file: ir::FileId,
    /// Diagnostics (a path spelled as read).
    diags: &'o mut ir::Diags,
    /// The statement names this call made up (an `options` statement needs one).
    made: HashSet<String>,
}

impl Writer<'_, '_> {
    /// Starts a line at the current depth.
    pub(super) fn line(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }

    /// Appends ` key=value`, a compound value in parentheses (an unparenthesized comparison in
    /// a parameter value does not read back).
    pub(super) fn value(&mut self, key: &str, e: ir::ExprId) {
        self.out.push(' ');
        self.out.push_str(key);
        self.out.push('=');
        let compound = matches!(
            self.design.expr(e),
            ir::Expr::Unary(..) | ir::Expr::Binary(..) | ir::Expr::Ternary(..)
        );
        if compound {
            self.out.push('(');
        }
        write_expr(self.design, e, self.out);
        if compound {
            self.out.push(')');
        }
    }

    /// A statement name no name of the design and no earlier one of this call has.
    pub(super) fn opt_name(&mut self, stem: &str) -> String {
        // Bounded: each step tries a new number; the design's names are finite.
        for k in 1u32.. {
            let name = format!("{stem}{k}");
            if self.design.find_name(&name).is_none() && self.made.insert(name.clone()) {
                return name;
            }
        }
        unreachable!("a free name exists")
    }
}

/// Appends file `file` of a legal design.
pub(super) fn write(
    design: &ir::Design,
    file: ir::FileId,
    out: &mut String,
    diags: &mut ir::Diags,
) {
    let top = design.file(file).top;
    let deck = design.file(file).role == ir::Role::Deck;
    let titled = design
        .items(top)
        .first()
        .is_some_and(|i| i.kind == ir::ItemKind::Title);
    assert!(!deck || titled, "FEATURES lacks DeckWithoutTitle");
    let mut w = Writer {
        design,
        out,
        depth: 0,
        file,
        diags,
        made: HashSet::new(),
    };
    scope(&mut w, top);
}

// ---- private ----

/// Every item of `scope`, in order.
fn scope(w: &mut Writer<'_, '_>, scope: ir::ScopeId) {
    for &at in w.design.items(scope) {
        item(w, at);
    }
}

/// One item.
fn item(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    use ir::ItemKind as K;
    let d = w.design;
    match at.kind {
        K::Title => {
            let row: ir::item::Title = d.row(at.index);
            w.out.push_str(d.text(row.text));
            w.out.push('\n');
        }
        K::Comment => {
            let row: ir::item::Comment = d.row(at.index);
            w.line();
            w.out.push_str("//");
            w.out.push_str(d.text(row.text));
            w.out.push('\n');
        }
        K::Opaque => {
            let row: ir::item::Opaque = d.row(at.index);
            assert_eq!(
                d.str(row.dialect),
                "spectre",
                "legalize drops foreign opaques"
            );
            w.out.push_str(d.text(row.text));
            w.out.push('\n');
        }
        K::Include => {
            let row: ir::item::Include = d.row(at.index);
            assert!(row.section.is_none(), "FEATURES lacks IncludeSection");
            let path = match crate::dialect::written_path(d, w.file, d.str(row.path)) {
                Ok(text) => text,
                Err(read) => {
                    w.diags.push_at(crate::dialect::PATH_AS_READ, d, at, 0);
                    read
                }
            };
            w.line();
            w.out.push_str(&format!("include \"{path}\"\n"));
        }
        K::Global => {
            let row: ir::item::Global = d.row(at.index);
            w.line();
            w.out.push_str("global 0");
            for &n in d.names(row.nets) {
                w.out.push(' ');
                w.out.push_str(d.str(n));
            }
            w.out.push('\n');
        }
        K::Param => {
            let row: ir::item::Param = d.row(at.index);
            w.line();
            w.out.push_str("parameters");
            w.value(d.str(row.name), row.value);
            w.out.push('\n');
        }
        K::Subckt => subckt(w, d.row(at.index)),
        K::If => if_chain(w, d.row(at.index)),
        K::Func => func(w, d.row(at.index)),
        K::Statistics => statistics(w, d.row(at.index)),
        K::Model => write_elements::model(w, d.row(at.index)),
        K::Element => write_elements::element(w, &d.row(at.index)),
        K::Analysis => write_run::analysis(w, d.row(at.index)),
        K::Loop => {
            let row: ir::Loop = d.row(at.index);
            write_run::loop_head(w, row);
            w.depth += 1;
            scope(w, row.body);
            w.depth -= 1;
            w.line();
            w.out.push_str("}\n");
        }
        K::Option => write_run::option(w, d.row(at.index)),
        K::Env => write_run::env(w, d.row(at.index)),
        K::Save => write_run::save(w, d.row(at.index)),
        K::Load
        | K::Section
        | K::DeviceLibrary
        | K::Alter
        | K::NodeSet
        | K::Print
        | K::Measure
        | K::Elaborate
        | K::Reset => unreachable!("FEATURES lacks {:?}", at.kind),
    }
}

/// `subckt name (ports)`, its formals, its body, `ends name`.
fn subckt(w: &mut Writer<'_, '_>, row: ir::item::Subckt) {
    let d = w.design;
    w.line();
    w.out.push_str("subckt ");
    w.out.push_str(d.str(row.name));
    let ports = d.names(row.ports);
    if !ports.is_empty() {
        let names: Vec<&str> = ports.iter().map(|&p| d.str(p)).collect();
        w.out.push_str(&format!(" ({})", names.join(" ")));
    }
    w.out.push('\n');
    w.depth += 1;
    let params = d.params(row.params);
    if !params.is_empty() {
        w.line();
        w.out.push_str("parameters");
        for &(k, v) in params {
            assert!(!v.is_none(), "FEATURES lacks FormalWithoutDefault");
            w.value(d.str(k), v);
        }
        w.out.push('\n');
    }
    scope(w, row.body);
    w.depth -= 1;
    w.line();
    w.out.push_str("ends ");
    w.out.push_str(d.str(row.name));
    w.out.push('\n');
}

/// `if (c) {` … `} else if (c) {` … `} else {` … `}`.
fn if_chain(w: &mut Writer<'_, '_>, row: ir::item::If) {
    let d = w.design;
    for (k, b) in d.branches(row.branches).iter().enumerate() {
        w.line();
        match (k, b.cond.is_none()) {
            (0, _) => w.out.push_str("if ("),
            (_, false) => w.out.push_str("} else if ("),
            (_, true) => w.out.push_str("} else {\n"),
        }
        if !b.cond.is_none() {
            write_expr(d, b.cond, w.out);
            w.out.push_str(") {\n");
        }
        w.depth += 1;
        scope(w, b.body);
        w.depth -= 1;
    }
    w.line();
    w.out.push_str("}\n");
}

/// `real name(real a, …) {`, `return e;`, `}`.
fn func(w: &mut Writer<'_, '_>, row: ir::item::Func) {
    let d = w.design;
    let args: Vec<String> = d
        .names(row.args)
        .iter()
        .map(|&a| format!("real {}", d.str(a)))
        .collect();
    w.line();
    w.out.push_str(&format!(
        "real {}({}) {{\n",
        d.str(row.name),
        args.join(", ")
    ));
    w.line();
    w.out.push_str("  return ");
    write_expr(d, row.body, w.out);
    w.out.push_str(";\n");
    w.line();
    w.out.push_str("}\n");
}

/// `statistics { process { vary … } mismatch { vary … } }`.
fn statistics(w: &mut Writer<'_, '_>, row: ir::Statistics) {
    let d = w.design;
    assert!(
        row.process_correlations.is_empty() && row.mismatch_correlations.is_empty(),
        "FEATURES lacks StatsCorrelation"
    );
    assert!(row.truncate.is_none(), "FEATURES lacks StatsTruncate");
    w.line();
    w.out.push_str("statistics {\n");
    for (word, list) in [("process", row.process), ("mismatch", row.mismatch)] {
        if list.is_empty() {
            continue;
        }
        w.line();
        w.out.push_str(&format!("  {word} {{\n"));
        for v in d.varies(list) {
            assert!(v.mean.is_none(), "FEATURES lacks VaryMean");
            let (dist, key) = match v.dist {
                ir::VaryDist::Gauss => ("gauss", "std"),
                ir::VaryDist::Unif => ("unif", "N"),
                ir::VaryDist::LogNormal => ("lnorm", "std"),
            };
            w.line();
            w.out
                .push_str(&format!("    vary {} dist={dist}", d.str(v.param.name)));
            w.value(key, v.spread);
            if v.percent {
                w.out.push_str(" percent=yes");
            }
            w.out.push('\n');
        }
        w.line();
        w.out.push_str("  }\n");
    }
    w.line();
    w.out.push_str("}\n");
}
