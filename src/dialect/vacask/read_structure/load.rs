//! `load "path" [map=…]` → a `DeviceLibrary` (a file VACASK ships for a family, no map) or a
//! `Load` with its modules defined (an identity `map=` or none). Part of `read_structure`.
//!
//! Cites: VD cir-loading; DIALECT-VACASK §3.1; R17, R39, R45.
//!
//! Contents: `load` (crate-private to the dialect)

use super::super::read::{self, Lower};
use super::super::{codes, tables};
use super::{REDEFINED, TOP_ONLY, assign, defined_here, top_level};
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// `load "path" [map=…]`.
pub(in crate::dialect::vacask) fn load(l: &mut Lower<'_, '_, '_>, n: Node) {
    if !top_level(l) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(TOP_ONLY));
    }
    let fields = l.cst.ast.fields(n).to_vec();
    let Some(&path) = fields.first() else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(codes::PRIVATE));
    };
    if l.cst.ast.tag(Node(path)) != Tag::Str {
        return read::opaque(l, n, ir::OpaqueClass::Private, Some(codes::PRIVATE));
    }
    // `map=["m", "m", …]` loads only the modules named, under their own names: the load's
    // modules. Any other `map=` (a rename, a wildcard) has no IR spelling.
    let modules = match fields.get(1..) {
        Some([]) | None => Some(Vec::new()),
        Some([only]) => identity_map(l, *only),
        Some(_) => None,
    };
    let Some(modules) = modules else {
        return read::opaque(l, n, ir::OpaqueClass::Private, Some(codes::PRIVATE));
    };
    let path = read::field_name(l.cst, path);
    let span = l.span(n);
    if let (Some(family), true) = (tables::library(&path), modules.is_empty()) {
        let default = tables::default_file(family).is_some_and(|d| d == path);
        let path = if default {
            ir::NameId::NONE
        } else {
            l.b.name(&path)
        };
        let row = ir::DeviceLibrary { path, family };
        l.b.device_library(l.scope, row, span);
        return;
    }
    let kind = match path.ends_with(".va") {
        true => ir::LoadKind::VerilogA,
        false => ir::LoadKind::Osdi,
    };
    let item = l.b.load(l.scope, kind, &path, span);
    for module in modules {
        let id = l.b.name(&module);
        if defined_here(l, ir::DefKind::Module, &module) {
            l.diags.push(REDEFINED, span, 0);
            continue;
        }
        l.b.define(ir::DefKind::Module, id, l.scope, item);
    }
}

// ---- private ----

/// The modules of a `map=[…]` field whose pairs each map a module to itself, in order; `None`
/// for any other field.
fn identity_map(l: &Lower<'_, '_, '_>, field: u32) -> Option<Vec<String>> {
    let (key, value) = assign(l, field)?;
    let ast = &l.cst.ast;
    let v = Node(value);
    if key != "map" || ast.tag(v) != Tag::Vector {
        return None;
    }
    let at = ast.rhs(v);
    let items: Vec<u32> = (0..ast.extra(at + 1))
        .map(|k| ast.extra(at + 2 + k))
        .collect();
    if items.is_empty() || !items.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::new();
    for pair in items.chunks(2) {
        if pair.iter().any(|&x| ast.tag(Node(x)) != Tag::Str) {
            return None;
        }
        let (from, to) = (
            read::field_name(l.cst, pair[0]),
            read::field_name(l.cst, pair[1]),
        );
        if from != to || from.contains('*') {
            return None;
        }
        out.push(from);
    }
    Some(out)
}
