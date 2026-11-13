//! The kernel rows' IR text helpers: what the rows' `RowFacts::dump` and `parse`
//! (`item_facts.rs`, `item_facts_scopes.rs`, `item_facts_defs.rs`) share, and a load's
//! modules as `text::dump` writes them (gathered once per dump there, per load here).
//!
//! Cites: `crates/ir/src/text.rs` (grammar); `rewrite/IR-KERNEL.md` §5, §9.
//!
//! Contents: crate: `dump_load(&Load, &Design, &[NameId], &mut String)`,
//! `modules_of(&Design, ItemRef) -> Vec<NameId>`, `names(&Design, &[NameId], &str, &mut String)`,
//! `class_word(OpaqueClass) -> &'static str`, `distinct_text(&[Cow<'_, str>]) -> bool`,
//! `defined(&Builder<'_>, ScopeId, DefKind, &str) -> bool`

use std::borrow::Cow;

use crate::build::Builder;
use crate::design::Design;
use crate::id::{ItemRef, NameId, ScopeId};
use crate::item::{DefKind, Load, LoadKind, OpaqueClass};
use crate::text_expr::{write_name, write_string};

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

/// The `Module` names a load defines, in definition order. O(definitions): for one load.
pub(crate) fn modules_of(d: &Design, at: ItemRef) -> Vec<NameId> {
    let defs = d.definitions().iter();
    defs.filter(|def| def.kind == DefKind::Module && def.item == at)
        .map(|def| def.name)
        .collect()
}

/// Writes names in a name position, separated by `sep`.
pub(crate) fn names(d: &Design, list: &[NameId], sep: &str, out: &mut String) {
    for (k, &n) in list.iter().enumerate() {
        if k > 0 {
            out.push_str(sep);
        }
        write_name(d.str(n), out);
    }
}

/// The IR-text word of an opaque class.
pub(crate) fn class_word(class: OpaqueClass) -> &'static str {
    match class {
        OpaqueClass::Unread => "unread",
        OpaqueClass::Private => "private",
        OpaqueClass::Inert => "inert",
        OpaqueClass::InvalidInSource => "invalid",
        OpaqueClass::Script => "script",
        OpaqueClass::Hint => "hint",
    }
}

/// Whether no name appears twice.
pub(crate) fn distinct_text(names: &[Cow<'_, str>]) -> bool {
    names
        .iter()
        .enumerate()
        .all(|(k, n)| !names[..k].contains(n))
}

/// Whether `scope` already has a definition of `kind` named `name` (the builder asserts on a
/// second one; the reader reports it instead).
pub(crate) fn defined(b: &Builder<'_>, scope: ScopeId, kind: DefKind, name: &str) -> bool {
    let id = b.design().find_name(name);
    !id.is_none() && b.design().def_index.contains_key(&(scope, kind, id))
}
