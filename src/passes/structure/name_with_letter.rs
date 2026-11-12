//! Pass `name_with_letter`: a design in, the same design out with every element whose name
//! begins with none of its SPICE letters (`ir::spice_letters`, ASCII case ignored) renamed to
//! its canonical letter, lower case, before its name (`load` a resistor → `rload`; `amp1` an
//! instance → `xamp1`), or that and `_<n>` when its scope already holds the name in any case.
//! Every by-name reference follows (`Editor::map_element_uses`: `Expr::I`, couplings, sensing
//! elements, sweep, alter and probe targets, each segment of a hierarchical path, A32). It
//! removes `CircuitExtra::NameWithoutLetter` (R23, A13: a SPICE-card dialect spells the kind
//! by the name's first letter) and requires nothing.
//!
//! The rename map is data (A32): a `RENAMED` note at each renamed element, its argument the old
//! name's id (the row holds the new one), so the run side gives results back under the
//! source's names. An element some reference may name through a segment the IR cannot resolve
//! (an instance of a master no read file defines: `ElementUse::scope` `NONE`) keeps its name with
//! an `UNRESOLVED_USE` note rather than half a path renamed.
//!
//! Cites: R23; `rewrite/AMEND-A12.md` A13, A32; `crates/ir/src/letter.rs`,
//! `crates/ir/src/element_uses.rs`.
//!
//! Contents: `pub fn name_with_letter(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const RENAMED: ir::Code`, `pub const UNRESOLVED_USE: ir::Code`

use std::collections::{HashMap, HashSet};

use crate::ir;

/// The note at a renamed element: `arg` is its old name's `NameId`.
pub const RENAMED: ir::Code = ir::Code::new(b'U', 311);
/// The note at an element left unrenamed: a reference to its name cannot be resolved.
pub const UNRESOLVED_USE: ir::Code = ir::Code::new(b'U', 312);

/// Renames each element without its letter and every reference to it.
pub fn name_with_letter(design: &mut ir::Design, diags: &mut ir::Diags) {
    let reachable = design.reachable();
    let unresolved: HashSet<String> = design
        .element_uses()
        .into_iter()
        .filter(|(_, u)| u.scope.is_none())
        .map(|(_, u)| design.str(u.name).to_ascii_lowercase())
        .collect();
    // the names each scope holds, folded, and the elements to rename in listed order
    let mut taken: HashMap<ir::ScopeId, HashSet<String>> = HashMap::new();
    let mut bare: Vec<(ir::ItemRef, ir::ScopeId, u8)> = Vec::new();
    for &(item, scope) in &reachable {
        if item.kind != ir::ItemKind::Element {
            continue;
        }
        let e = design.row::<ir::Element>(item.index);
        let name = design.str(e.name).to_ascii_lowercase();
        taken.entry(scope).or_default().insert(name.clone());
        let letters = ir::spice_letters(design, &e);
        let first = name.as_bytes().first().copied().unwrap_or(0);
        if letters.is_empty() || letters.iter().any(|l| first.eq_ignore_ascii_case(l)) {
            continue;
        }
        match unresolved.contains(&name) {
            true => diags.push_at(UNRESOLVED_USE, design, item, 0),
            false => bare.push((item, scope, letters[0])),
        }
    }
    if bare.is_empty() {
        return;
    }
    // the plan: (scope, old, new) per element, a bijection per scope
    let mut plan: HashMap<(ir::ScopeId, ir::NameId), ir::NameId> = HashMap::new();
    for &(item, scope, letter) in &bare {
        let e = design.row::<ir::Element>(item.index);
        let old = e.name;
        let names = taken.entry(scope).or_default();
        let base = format!("{}{}", letter.to_ascii_lowercase() as char, design.str(old));
        let bound = names.len() + 1;
        let new = std::iter::once(base.clone())
            .chain((1..=bound).map(|n| format!("{base}_{n}")))
            .find(|n| !names.contains(&n.to_ascii_lowercase()))
            .expect("fewer names than candidates");
        names.insert(new.to_ascii_lowercase());
        let new = design.builder().name(&new);
        plan.insert((scope, old), new);
        let mut row = e;
        row.name = new;
        design.editor().set(item.index, row);
        diags.push_at(RENAMED, design, item, old.0);
    }
    let mut editor = design.editor();
    for &(item, scope) in &reachable {
        editor.map_element_uses(scope, item, &mut |u| plan.get(&(u.scope, u.name)).copied());
    }
}
