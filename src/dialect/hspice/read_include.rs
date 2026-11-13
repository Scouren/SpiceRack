//! The include statements → `Include` items: `.include path` (the whole file) and `.lib path
//! section` (one section of it); an include cycle is `InvalidInSource` (ngspice-45 crashes).
//! Split from `read_structure.rs`.
//!
//! Cites: ngspice manual §2.7 (`.include`), §2.8 (`.lib`); ngspice-45 runs (settle
//! `inc_cycle.sp`, exit 139; `inc_lib_self.sp`).
//!
//! Contents: `pub(super) fn include(lw: &mut Lower<'_, '_>, st: Stmt)`

use std::collections::HashSet;

use super::codes;
use super::cst;
use super::read::{Lower, Stmt};
use super::read_structure::{def_name, in_branch, malformed};
use crate::dialect::Reference;
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// `.include path` (whole file) or `.lib path section` (one section of it).
pub(super) fn include(lw: &mut Lower<'_, '_>, st: Stmt) {
    if in_branch(lw, st) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::BRANCH_DEF);
    }
    let fields = lw.ast.fields(st.node);
    let want = if lw.ast.tag(st.node) == Tag::LibCall {
        2
    } else {
        1
    };
    if fields.len() != want {
        return malformed(lw, st);
    }
    let path = cst::unquote(&lw.text(Node(fields[0])));
    if path.is_empty() {
        return malformed(lw, st);
    }
    let section = (want == 2).then(|| lw.names.fold(&lw.text(Node(fields[1]))));
    let path = lw.include_path(st, &path);
    if includes_back(
        lw,
        st,
        (
            path.clone(),
            section.as_deref().map(str::to_ascii_lowercase),
        ),
    ) {
        // ngspice-45 crashes on an include cycle (settle inc_cycle.sp: exit 139)
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::INCLUDE_CYCLE);
    }
    let span = lw.span(st.node);
    lw.b.include(st.scope, &path, section.as_deref(), span);
}

// ---- private ----

/// Whether the include of `target` (a file, and the section a `.lib` call names) reads back,
/// itself or through the includes and calls it holds, the (file, section) the statement `st`
/// stands in: an include cycle. Keyed by (file, section), so a section calling another section
/// of its own file is none (settle inc_lib_self.sp, the gf180 kit's idiom). A worklist with a
/// visited set over the sources' texts.
fn includes_back(lw: &Lower<'_, '_>, st: Stmt, target: (String, Option<String>)) -> bool {
    let here = (
        lw.path.to_string(),
        (lw.ast.tag(st.container) == Tag::Section)
            .then(|| def_name(lw.ast, lw.toks, st.container))
            .flatten()
            .map(|n| n.to_ascii_lowercase()),
    );
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();
    let mut todo = vec![target];
    // Bounded: each (file, section) is taken once, and only files of the sources are followed.
    while let Some(node) = todo.pop() {
        if node == here {
            return true;
        }
        if !seen.insert(node.clone()) {
            continue;
        }
        let Some((_, text)) = lw.src.files().iter().find(|(p, _)| *p == node.0) else {
            continue;
        };
        for (path, section) in calls(text, node.1.as_deref()) {
            let resolved = Reference::File(path).resolve(&node.0);
            todo.push((resolved, section.map(|s| s.to_ascii_lowercase())));
        }
    }
    false
}

/// The `.include` paths and `.lib path section` calls of `text` that stand in `section` (`None`:
/// outside every `.lib name` … `.endl` section), as written.
fn calls(text: &str, section: Option<&str>) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut inside: Option<String> = None;
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(first) = words.first().map(|w| w.to_ascii_lowercase()) else {
            continue;
        };
        let unq = |w: &str| w.trim_matches(['"', '\'']).to_string();
        match (first.as_str(), words.len()) {
            (".lib", 2) => inside = Some(words[1].to_ascii_lowercase()),
            (".endl", _) => inside = None,
            (".lib", n) if n >= 3 && inside.as_deref() == section => {
                out.push((unq(words[1]), Some(words[2].to_string())));
            }
            (".include" | ".inc", n) if n >= 2 && inside.as_deref() == section => {
                out.push((unq(words[1]), None));
            }
            _ => {}
        }
    }
    out
}
