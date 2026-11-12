//! The kernel rows' IR text: each of the 11 rows of `item.rs` written as the fields after its
//! keyword (`dump_*`), and read back from them (`parse_*`). The kernel's `text::dump` and
//! `text::parse` write the keyword, the indentation and the bodies; an `If`'s `elif`/`else`
//! lines are theirs too, so an `If` row's own text is only its first condition.
//!
//! A reader pushes in the order a builder caller would (expressions as read, then the item with
//! its names), so `parse(dump(d))` rebuilds `d`'s ids; it refuses, with one `TEXT_SYNTAX` row,
//! what the builder would assert on (a title outside a deck's top, ground among globals, a
//! repeated port, parameter or argument, a second definition of one name) and pushes nothing.
//!
//! Cites: `crates/ir/src/text.rs` (grammar); `rewrite/IR-KERNEL.md` §5, §9.
//!
//! Contents: crate: `modules_of(&Design, ItemRef) -> Vec<NameId>`, `dump_title`, `dump_comment`, `dump_include`, `dump_global`, `dump_load`,
//! `dump_opaque`, `dump_subckt`, `dump_section`, `dump_if`, `dump_param`, `dump_func` (each
//! `(row, &Design, ItemRef, &Ctx, &mut String)`, but `dump_load(row, &Design, &[NameId], &mut
//! String)`); `parse_title`, `parse_comment`,
//! `parse_include`, `parse_global`, `parse_load`, `parse_opaque`, `parse_subckt`,
//! `parse_section`, `parse_if`, `parse_param`, `parse_func` (each `(&mut Words<'_>, ScopeId,
//! &mut Builder<'_>, &mut Diags) -> ItemRef`)

use std::borrow::Cow;

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::id::{ExprId, ItemRef, NameId, ScopeId};
use crate::item::{
    Comment, DefKind, Func, Global, If, Include, Load, LoadKind, Opaque, OpaqueClass, Param, Role,
    ScopeKind, Section, Subckt, Title,
};
use crate::text::Words;
use crate::text_expr::{Ctx, write_expr, write_name, write_string, write_value};

pub(crate) fn dump_title(row: &Title, d: &Design, _: ItemRef, _: &Ctx, out: &mut String) {
    write_string(d.text(row.text), out);
}

pub(crate) fn dump_comment(row: &Comment, d: &Design, _: ItemRef, _: &Ctx, out: &mut String) {
    write_string(d.text(row.text), out);
}

pub(crate) fn dump_include(row: &Include, d: &Design, _: ItemRef, _: &Ctx, out: &mut String) {
    write_string(d.str(row.path), out);
    if !row.section.is_none() {
        out.push_str(" section=");
        write_name(d.str(row.section), out);
    }
}

pub(crate) fn dump_global(row: &Global, d: &Design, _: ItemRef, _: &Ctx, out: &mut String) {
    names(d, d.names(row.nets), " ", out);
}

/// `veriloga|osdi "path" [modules=(…)]`: `modules` are the `Module` definitions whose item is
/// this load, in definition order (`text::dump` gathers them once per dump; `modules_of` finds
/// them for one load).
pub(crate) fn dump_load(row: &Load, d: &Design, modules: &[NameId], out: &mut String) {
    out.push_str(match row.kind {
        LoadKind::VerilogA => "veriloga ",
        LoadKind::Osdi => "osdi ",
    });
    write_string(d.str(row.path), out);
    if !modules.is_empty() {
        out.push_str(" modules=(");
        names(d, modules, " ", out);
        out.push(')');
    }
}

pub(crate) fn dump_opaque(row: &Opaque, d: &Design, _: ItemRef, _: &Ctx, out: &mut String) {
    write_name(d.str(row.dialect), out);
    out.push(' ');
    out.push_str(class_word(row.class));
    out.push(' ');
    write_string(d.text(row.text), out);
}

pub(crate) fn dump_subckt(row: &Subckt, d: &Design, _: ItemRef, ctx: &Ctx, out: &mut String) {
    write_name(d.str(row.name), out);
    out.push_str(" (");
    names(d, d.names(row.ports), " ", out);
    out.push(')');
    // A10: the head defaults are read from the body, where the sibling parameters are defined
    let body = Ctx {
        scope: row.body,
        ..*ctx
    };
    for &(name, value) in d.params(row.params) {
        out.push(' ');
        write_name(d.str(name), out);
        if !value.is_none() {
            out.push('=');
            write_value(d, value, &body, out);
        }
    }
}

pub(crate) fn dump_section(row: &Section, d: &Design, _: ItemRef, _: &Ctx, out: &mut String) {
    write_name(d.str(row.name), out);
    if !row.library.is_none() {
        out.push_str(" library=");
        write_name(d.str(row.library), out);
    }
}

/// `(cond)` of the first branch; the kernel writes the `elif`/`else` lines.
pub(crate) fn dump_if(row: &If, d: &Design, _: ItemRef, ctx: &Ctx, out: &mut String) {
    let first = d.branches(row.branches)[0];
    out.push('(');
    write_expr(d, first.cond, ctx, out);
    out.push(')');
}

pub(crate) fn dump_param(row: &Param, d: &Design, _: ItemRef, ctx: &Ctx, out: &mut String) {
    write_name(d.str(row.name), out);
    out.push_str(" = ");
    write_expr(d, row.value, ctx, out);
}

/// `name(a, b) = body`, the body written with this function's formal arguments in scope.
pub(crate) fn dump_func(row: &Func, d: &Design, at: ItemRef, ctx: &Ctx, out: &mut String) {
    write_name(d.str(row.name), out);
    out.push('(');
    names(d, d.names(row.args), ", ", out);
    out.push_str(") = ");
    // the function defines itself in the scope that lists it: one index lookup
    let formals = match d.def_index.get(&(ctx.scope, DefKind::Func, row.name)) {
        Some(&def) if d.def(def).item == at => def,
        Some(_) | None => crate::id::DefId::NONE,
    };
    write_expr(d, row.body, &Ctx { formals, ..*ctx }, out);
}

pub(crate) fn parse_title(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let s = b.design().scope(scope);
    let deck = s.kind == ScopeKind::File && b.design().file(s.file).role == Role::Deck;
    let Some(text) = w.string(diags) else {
        return ItemRef::NONE;
    };
    if !deck {
        w.error(diags);
        return ItemRef::NONE;
    }
    b.title(scope, &text, w.line_span())
}

pub(crate) fn parse_comment(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let Some(text) = w.string(diags) else {
        return ItemRef::NONE;
    };
    b.comment(scope, &text, w.line_span())
}

pub(crate) fn parse_include(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let Some(path) = w.string(diags) else {
        return ItemRef::NONE;
    };
    let section = if w.key() == Some("section") {
        let Some(s) = w.name_text(diags) else {
            return ItemRef::NONE;
        };
        Some(s)
    } else {
        None
    };
    b.include(scope, &path, section.as_deref(), w.line_span())
}

pub(crate) fn parse_global(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let mut nets: Vec<Cow<'_, str>> = Vec::new();
    while !w.at_end() {
        let Some(n) = w.name_text(diags) else {
            return ItemRef::NONE;
        };
        nets.push(n);
    }
    if nets.is_empty() || nets.iter().any(|n| n == "0") || !distinct(&nets) {
        w.error(diags);
        return ItemRef::NONE;
    }
    let refs: Vec<&str> = nets.iter().map(|n| n.as_ref()).collect();
    b.global(scope, &refs, w.line_span())
}

pub(crate) fn parse_load(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let kind = match w.next() {
        Some("veriloga") => LoadKind::VerilogA,
        Some("osdi") => LoadKind::Osdi,
        _ => {
            w.error(diags);
            return ItemRef::NONE;
        }
    };
    let Some(path) = w.string(diags) else {
        return ItemRef::NONE;
    };
    let mut modules = Vec::new();
    if w.key() == Some("modules") {
        let Some(m) = w.names_text(diags) else {
            return ItemRef::NONE;
        };
        modules = m;
    }
    let taken = |b: &Builder<'_>, n: &str| defined(b, scope, DefKind::Module, n);
    if !distinct(&modules) || modules.iter().any(|m| taken(b, m)) {
        w.error(diags);
        return ItemRef::NONE;
    }
    let item = b.load(scope, kind, &path, w.line_span());
    for m in &modules {
        let name = b.name(m);
        b.define(DefKind::Module, name, scope, item);
    }
    item
}

pub(crate) fn parse_opaque(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let Some(dialect) = w.name_text(diags) else {
        return ItemRef::NONE;
    };
    let class = match w.next() {
        Some("unread") => OpaqueClass::Unread,
        Some("private") => OpaqueClass::Private,
        Some("inert") => OpaqueClass::Inert,
        Some("invalid") => OpaqueClass::InvalidInSource,
        Some("script") => OpaqueClass::Script,
        _ => {
            w.error(diags);
            return ItemRef::NONE;
        }
    };
    let Some(text) = w.string(diags) else {
        return ItemRef::NONE;
    };
    b.opaque(scope, &dialect, class, &text, w.line_span())
}

pub(crate) fn parse_subckt(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let Some(name) = w.name_text(diags) else {
        return ItemRef::NONE;
    };
    let Some(ports) = w.names_text(diags) else {
        return ItemRef::NONE;
    };
    let mut params: Vec<(Cow<'_, str>, ExprId)> = Vec::new();
    while !w.at_end() {
        if let Some(key) = w.key() {
            let value = w.value(scope, b, diags);
            if value.is_none() {
                return ItemRef::NONE;
            }
            params.push((Cow::Borrowed(key), value));
        } else {
            let Some(key) = w.name_text(diags) else {
                return ItemRef::NONE;
            };
            params.push((key, ExprId::NONE));
        }
    }
    let keys: Vec<Cow<'_, str>> = params.iter().map(|p| p.0.clone()).collect();
    if !distinct(&ports) || !distinct(&keys) || defined(b, scope, DefKind::Subckt, &name) {
        w.error(diags);
        return ItemRef::NONE;
    }
    let ports: Vec<&str> = ports.iter().map(|p| p.as_ref()).collect();
    let params: Vec<(&str, ExprId)> = params.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
    b.subckt(scope, &name, &ports, &params, w.line_span()).0
}

pub(crate) fn parse_section(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let Some(name) = w.name_text(diags) else {
        return ItemRef::NONE;
    };
    let mut library = None;
    if w.key() == Some("library") {
        let Some(l) = w.name_text(diags) else {
            return ItemRef::NONE;
        };
        library = Some(l);
    }
    if defined(b, scope, DefKind::Section, &name) {
        w.error(diags);
        return ItemRef::NONE;
    }
    b.section(scope, &name, library.as_deref(), w.line_span()).0
}

/// One branch: `(cond)`. The kernel's `text::parse` reads an `if` with its `elif`/`else` lines
/// itself; this is the row's own one-line form.
pub(crate) fn parse_if(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let cond = w.value(scope, b, diags);
    if cond.is_none() {
        return ItemRef::NONE;
    }
    b.if_(scope, &[cond], w.line_span()).0
}

pub(crate) fn parse_param(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let Some(name) = w.name_text(diags) else {
        return ItemRef::NONE;
    };
    if !w.expect("=", diags) {
        return ItemRef::NONE;
    }
    let value = w.expr(scope, b, diags);
    if value.is_none() {
        return ItemRef::NONE;
    }
    if defined(b, scope, DefKind::Param, &name) {
        w.error(diags);
        return ItemRef::NONE;
    }
    b.param(scope, &name, value, w.line_span())
}

pub(crate) fn parse_func(
    w: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let Some(name) = w.name_text(diags) else {
        return ItemRef::NONE;
    };
    if !w.expect("(", diags) {
        return ItemRef::NONE;
    }
    let mut args: Vec<Cow<'_, str>> = Vec::new();
    // bounded by the line: each turn consumes a word
    while w.peek() != Some(")") {
        let Some(a) = w.name_text(diags) else {
            return ItemRef::NONE;
        };
        args.push(a);
        if w.peek() == Some(",") {
            w.next();
        } else if w.peek() != Some(")") {
            w.error(diags);
            return ItemRef::NONE;
        }
    }
    w.next();
    if !w.expect("=", diags) {
        return ItemRef::NONE;
    }
    let body = w.expr(scope, b, diags);
    if body.is_none() {
        return ItemRef::NONE;
    }
    if !distinct(&args) || defined(b, scope, DefKind::Func, &name) {
        w.error(diags);
        return ItemRef::NONE;
    }
    let args: Vec<&str> = args.iter().map(|a| a.as_ref()).collect();
    b.func(scope, &name, &args, body, w.line_span())
}

/// The `Module` names a load defines, in definition order. O(definitions): for one load.
pub(crate) fn modules_of(d: &Design, at: ItemRef) -> Vec<NameId> {
    let defs = d.definitions().iter();
    defs.filter(|def| def.kind == DefKind::Module && def.item == at)
        .map(|def| def.name)
        .collect()
}

// ---- private ----

/// Writes names in a name position, separated by `sep`.
fn names(d: &Design, list: &[NameId], sep: &str, out: &mut String) {
    for (k, &n) in list.iter().enumerate() {
        if k > 0 {
            out.push_str(sep);
        }
        write_name(d.str(n), out);
    }
}

fn class_word(class: OpaqueClass) -> &'static str {
    match class {
        OpaqueClass::Unread => "unread",
        OpaqueClass::Private => "private",
        OpaqueClass::Inert => "inert",
        OpaqueClass::InvalidInSource => "invalid",
        OpaqueClass::Script => "script",
    }
}

/// Whether no name appears twice.
fn distinct(names: &[Cow<'_, str>]) -> bool {
    names
        .iter()
        .enumerate()
        .all(|(k, n)| !names[..k].contains(n))
}

/// Whether `scope` already has a definition of `kind` named `name` (the builder asserts on a
/// second one; the reader reports it instead).
fn defined(b: &Builder<'_>, scope: ScopeId, kind: DefKind, name: &str) -> bool {
    let id = b.design().find_name(name);
    !id.is_none() && b.design().def_index.contains_key(&(scope, kind, id))
}
