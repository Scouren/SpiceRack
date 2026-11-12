//! The structure rows → VACASK statements: `Title` (line 1), `Comment` (`//text`: one line), `Include` (`include "path" [section=name]`), `Section`,
//! `Load` (`load "path"`), `Subckt` (`subckt name (ports)`, its head as one `parameters` line,
//! then its body, `ends`), `Param` (`parameters name=value`), `Global`, `If` (`@if c`,
//! `@elseif c`, `@else`, `@end`), and an `Opaque` of VACASK's text family copied as written.
//! Each function writes its statement's first line and leaves the body to the spine.
//!
//! Cites: VD input-include, cir-subckt, cir-nodes, cir-conditional, cir-loading.
//!
//! Contents: `NO_LIBRARY`, `CONSTANT_LOCAL`, `LOAD_KIND`, `CODES`, `title`, `comment`, `include`, `section`, `section_end`,
//! `load`, `device_library`, `admits`,
//! `subckt`, `subckt_end`, `param`, `global`, `branch`, `if_end`, `opaque` (crate-private)

use super::expr_write::write_expr;
use super::name::{self, Place};
use super::tables;
use super::write::Writer;
use crate::ir;

/// A device library of a family VACASK ships no file for. ponytail: goes when a `DeviceLibrary`
/// row states `Feature::Family` (it states none: `library.rs`); asked of the circuit owner.
pub(super) const NO_LIBRARY: ir::Code = ir::Code::new(b'W', 2002);

/// A subcircuit's local parameter whose value is constant: VACASK reads every constant
/// `parameters` entry of a subcircuit as a formal an instance may override (run c1), so a constant
/// local has no spelling. ponytail: a kernel feature for it is asked; this refusal goes then.
pub(super) const CONSTANT_LOCAL: ir::Code = ir::Code::new(b'W', 2005);

/// A load whose kind its path's extension does not state (VACASK compiles a `.va`, loads the
/// rest as OSDI: VD cir-loading).
pub(super) const LOAD_KIND: ir::Code = ir::Code::new(b'W', 2004);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        CONSTANT_LOCAL,
        ir::Info {
            title: "a constant local parameter in a subcircuit",
            cite: "VD cir-subckt (Parameters); DIALECT-VACASK Appendix A run c1",
            explain: "VACASK makes every subcircuit parameter whose value folds to a constant a formal an instance may override; a local one that must not be overridden has no VACASK spelling.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        NO_LIBRARY,
        ir::Info {
            title: "a device library of a family VACASK ships no file for",
            cite: "VD dev-spice, dev-builtin; the store's lib/vacask/mod listing (tables.rs)",
            explain: "VACASK names a device library by its file: the family's default (no path) or another file it ships for that family. A family VACASK has no file for, or a path that is no file of the family, has no VACASK spelling.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        LOAD_KIND,
        ir::Info {
            title: "a load whose kind its file's extension does not state",
            cite: "VD cir-loading (a `.va` is compiled, any other file loaded as OSDI)",
            explain: "VACASK decides from the extension whether a `load` compiles Verilog-A; a Verilog-A load of another extension, or an OSDI load of a `.va`, has no VACASK spelling.",
            kind: ir::Kind::Refusal,
        },
    ),
];

/// Line 1 of an entry deck: the title's text, or nothing.
pub(super) fn title(w: &mut Writer<'_, '_>, row: Option<ir::item::Title>) {
    if let Some(row) = row {
        w.out.push_str(w.design.text(row.text));
    }
    w.out.push('\n');
}

/// A comment line.
pub(super) fn comment(w: &mut Writer<'_, '_>, row: ir::item::Comment) {
    w.line();
    w.out.push_str("//");
    w.out.push_str(w.design.text(row.text));
    w.out.push('\n');
}

/// An `include` line.
pub(super) fn include(w: &mut Writer<'_, '_>, row: ir::item::Include) {
    w.line();
    w.out.push_str("include ");
    let path = w.design.str(row.path);
    // The reader resolved the path against the including file; VACASK resolves it again.
    let prefix = format!("{}/", w.dir);
    let path = match w.dir.is_empty() {
        true => path,
        false => path.strip_prefix(&prefix).unwrap_or(path),
    };
    string(path, w.out);
    if !row.section.is_none() {
        w.out.push_str(" section=");
        name::spell(w.design.str(row.section), Place::Other, w.out);
    }
    w.out.push('\n');
}

/// A `section` line (its body follows).
pub(super) fn section(w: &mut Writer<'_, '_>, row: ir::item::Section) {
    w.line();
    w.out.push_str("section ");
    name::spell(w.design.str(row.name), Place::Other, w.out);
    w.out.push('\n');
}

/// `endsection`.
pub(super) fn section_end(w: &mut Writer<'_, '_>) {
    w.line();
    w.out.push_str("endsection\n");
}

/// A `load` line, its modules as an identity `map=` (none: VACASK loads every module).
pub(super) fn load(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let row: ir::item::Load = d.row(at.index);
    w.line();
    w.out.push_str("load ");
    string(d.str(row.path), w.out);
    // The load's modules: `map=` loads exactly those, each under its own name (VD cir-loading).
    let modules: Vec<ir::NameId> = d
        .definitions()
        .iter()
        .filter(|def| def.kind == ir::DefKind::Module && def.item == at)
        .map(|def| def.name)
        .collect();
    if !modules.is_empty() {
        w.out.push_str(" map=[");
        for (k, &m) in modules.iter().enumerate() {
            if k > 0 {
                w.out.push_str(", ");
            }
            string(d.str(m), w.out);
            w.out.push_str(", ");
            string(d.str(m), w.out);
        }
        w.out.push(']');
    }
    w.out.push('\n');
}

/// A `load` of a device library: its path, or the family's default file.
pub(super) fn device_library(w: &mut Writer<'_, '_>, row: ir::DeviceLibrary) {
    let path = match row.path.is_none() {
        true => tables::default_file(row.family).expect("admits refused a family with no file"),
        false => w.design.str(row.path).to_string(),
    };
    w.line();
    w.out.push_str("load ");
    string(&path, w.out);
    w.out.push('\n');
}

/// Appends a refusal for what no feature states and VACASK cannot spell: a load whose kind its
/// extension does not state; a device library VACASK cannot name; a constant local parameter
/// of a subcircuit.
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    for (at, scope) in design.reachable() {
        let refused = match at.kind {
            ir::ItemKind::Load => {
                let row: ir::item::Load = design.row(at.index);
                let va = design.str(row.path).ends_with(".va");
                (va != (row.kind == ir::LoadKind::VerilogA)).then_some(LOAD_KIND)
            }
            ir::ItemKind::DeviceLibrary => {
                let row: ir::DeviceLibrary = design.row(at.index);
                let default = tables::default_file(row.family);
                // VACASK names a library by its file: the family's default, or a file it
                // ships for that family (a variant), which reads back as this row.
                let spelled = match row.path.is_none() {
                    true => default.is_some(),
                    false => {
                        let path = design.str(row.path);
                        tables::library(path) == Some(row.family)
                            && default.as_deref() != Some(path)
                    }
                };
                (!spelled).then_some(NO_LIBRARY)
            }
            ir::ItemKind::Param if in_subckt(design, scope) => {
                let row: ir::item::Param = design.row(at.index);
                constant(design, row.value).then_some(CONSTANT_LOCAL)
            }
            _ => None,
        };
        if let Some(code) = refused {
            diags.push(code, design.span_of(at), 0);
        }
    }
}

/// The `subckt` line and its head's `parameters` line (its body follows).
pub(super) fn subckt(w: &mut Writer<'_, '_>, row: ir::item::Subckt) {
    let d = w.design;
    w.line();
    w.out.push_str("subckt ");
    name::spell(d.str(row.name), Place::Other, w.out);
    w.out.push_str(" (");
    for (k, &port) in d.names(row.ports).iter().enumerate() {
        if k > 0 {
            w.out.push(' ');
        }
        name::spell(d.str(port), Place::Node, w.out);
    }
    w.out.push_str(")\n");
    let head = d.params(row.params);
    if head.is_empty() {
        return;
    }
    w.depth += 1;
    w.line();
    w.depth -= 1;
    w.out.push_str("parameters");
    for &(key, value) in head {
        w.out.push(' ');
        name::spell(d.str(key), Place::Other, w.out);
        w.out.push('=');
        write_expr(d, value, w.out);
    }
    w.out.push('\n');
}

/// `ends`.
pub(super) fn subckt_end(w: &mut Writer<'_, '_>) {
    w.line();
    w.out.push_str("ends\n");
}

/// A `parameters` line of one parameter.
pub(super) fn param(w: &mut Writer<'_, '_>, row: ir::item::Param) {
    w.line();
    w.out.push_str("parameters ");
    name::spell(w.design.str(row.name), Place::Other, w.out);
    w.out.push('=');
    write_expr(w.design, row.value, w.out);
    w.out.push('\n');
}

/// A `global` line.
pub(super) fn global(w: &mut Writer<'_, '_>, row: ir::item::Global) {
    w.line();
    w.out.push_str("global");
    for &net in w.design.names(row.nets) {
        w.out.push(' ');
        name::spell(w.design.str(net), Place::Node, w.out);
    }
    w.out.push('\n');
}

/// Branch `k` of an `If`: `@if c` (k = 0), `@elseif c`, or `@else` (condition `NONE`).
pub(super) fn branch(w: &mut Writer<'_, '_>, k: usize, branch: ir::item::Branch) {
    w.line();
    match (k, branch.cond.is_none()) {
        (_, true) => w.out.push_str("@else"),
        (0, false) => w.out.push_str("@if "),
        (_, false) => w.out.push_str("@elseif "),
    }
    if !branch.cond.is_none() {
        write_expr(w.design, branch.cond, w.out);
    }
    w.out.push('\n');
}

/// `@end`.
pub(super) fn if_end(w: &mut Writer<'_, '_>) {
    w.line();
    w.out.push_str("@end\n");
}

/// An `Opaque` of VACASK's family, as written (its lines after the first as they were).
pub(super) fn opaque(w: &mut Writer<'_, '_>, row: ir::item::Opaque) {
    assert!(
        w.design.str(row.dialect) == "vacask",
        "a foreign opaque reached the VACASK writer (legalize refuses it)"
    );
    w.line();
    w.out.push_str(w.design.text(row.text));
    w.out.push('\n');
}

// ---- private ----

/// Whether `scope` is a subcircuit body or inside one.
fn in_subckt(design: &ir::Design, scope: ir::ScopeId) -> bool {
    let mut at = scope;
    // Bounded: each step moves to a parent, and the scope tree is acyclic.
    for _ in 0..1 << 16 {
        if at.is_none() {
            return false;
        }
        let s = design.scope(at);
        if s.kind == ir::ScopeKind::Subckt {
            return true;
        }
        at = s.parent;
    }
    false
}

/// Whether expression `e` folds to a constant by VACASK's rule (the reader's `is_constant`): it
/// reads no parameter and no special identifier.
fn constant(design: &ir::Design, e: ir::ExprId) -> bool {
    let mut todo = vec![e];
    // Bounded: each row of the (acyclic) expression tree is taken once per parent.
    while let Some(x) = todo.pop() {
        match design.expr(x) {
            ir::Expr::Param(_) | ir::Expr::Temp | ir::Expr::Tnom | ir::Expr::Time => return false,
            ir::Expr::Unary(_, a) => todo.push(a),
            ir::Expr::Binary(_, a, b) => todo.extend([a, b]),
            ir::Expr::Ternary(a, b, c) => todo.extend([a, b, c]),
            ir::Expr::Call(_, args) | ir::Expr::Draw(_, _, _, args) | ir::Expr::List(args) => {
                todo.extend_from_slice(design.exprs(args));
            }
            _ => {}
        }
    }
    true
}

/// `"text"`, `"` and `\\` escaped.
fn string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
