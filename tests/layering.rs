//! The dependency rules of the rewrite (REWRITE-PLAN §2.2 rule 5, §2.3), over every `use`
//! line under `src/`: a dialect (`src/dialect/<d>.rs` or `src/dialect/<d>/`) imports no other
//! dialect but its declared parent, nor `syntax::render`, `passes` or the facade; a pass
//! (`src/passes.rs`, `src/passes/`) imports only `ir` and `passes`; a shared helper directory
//! (`src/dialect/spice/`, `src/dialect/named/`) names no dialect.
//!
//! Oracle: the rule table below, written from the plan, and a synthetic tree with one known
//! violation per rule (each rule goes red on its own case).
//!
//! Contents: tests only.

use std::path::Path;

/// Files of `src/dialect/` that are the spine, not dialects.
const SPINE: &[&str] = &["mod", "facade", "sources"];
/// Shared helper directories under `src/dialect/`: no dialect parameter, no dialect named.
const SHARED: &[&str] = &["spice", "named"];
/// A dialect and the one sibling it may import (ngspice flavours, §4.3).
const PARENTS: &[(&str, &str)] = &[("espice", "ngspice"), ("rspice", "ngspice")];
/// What no dialect imports (crate paths, by prefix).
const DIALECT_FORBIDS: &[&str] = &["syntax::render", "passes", "dialect::facade"];
/// All a pass imports (crate paths, by prefix).
const PASSES_ALLOW: &[&str] = &["ir", "passes"];

// ---- private ----

#[test]
fn no_file_under_src_has_a_forbidden_edge() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    assert!(files.len() > 100, "the walk reached the tree");
    assert!(files.iter().any(|(p, _)| p == "dialect/mod.rs"));
    let found = violations(&files);
    assert!(found.is_empty(), "forbidden edges:\n{}", found.join("\n"));
}

#[test]
fn each_rule_catches_its_own_case() {
    let tree = |p: &str, t: &str| (p.to_string(), t.to_string());
    let files = [
        tree("dialect/mod.rs", "pub mod ngspice;\n"),
        tree(
            "dialect/ngspice/read.rs",
            "use crate::dialect::xyce::lex;\nuse super::write;\n",
        ),
        tree(
            "dialect/ngspice/write.rs",
            "use crate::{ir, syntax::render::Written};\n",
        ),
        tree("dialect/xyce/mod.rs", "use super::{Read, ngspice::read};\n"),
        tree(
            "dialect/espice.rs",
            "use super::ngspice::read;\nuse crate::dialect::{facade, Sources};\n",
        ),
        tree("dialect/spice/cards.rs", "use crate::dialect::xyce;\n"),
        tree("dialect/facade.rs", "use crate::syntax::render;\n"),
        tree(
            "passes/pulse.rs",
            "use crate::ir;\nuse crate::syntax::lang::Lang;\nuse super::Pass;\n",
        ),
        tree("passes/mod.rs", "use crate::{ir, dialect};\n"),
    ];
    let found = violations(&files);
    let expected = [
        "dialect/ngspice/read.rs: dialect::xyce::lex",
        "dialect/ngspice/write.rs: syntax::render::Written",
        "dialect/xyce/mod.rs: dialect::ngspice::read",
        "dialect/espice.rs: dialect::facade",
        "dialect/spice/cards.rs: dialect::xyce",
        "passes/pulse.rs: syntax::lang::Lang",
        "passes/mod.rs: dialect",
    ];
    assert_eq!(found, expected);
}

/// Every `.rs` file under `dir`, as (path relative to `root` with `/`, text), sorted.
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("src/ is readable")
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let path = e.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            let rel = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, std::fs::read_to_string(&path).expect("UTF-8 source")));
        }
    }
}

/// `path: import` for each forbidden edge, in file then line order.
fn violations(files: &[(String, String)]) -> Vec<String> {
    let dialects: Vec<String> = files
        .iter()
        .filter_map(|(p, _)| owner(p))
        .filter(|d| !SHARED.contains(&d.as_str()))
        .collect();
    let mut out = Vec::new();
    for (path, text) in files {
        let module = module_path(path);
        for import in imports(text, &module) {
            if forbidden(path, &import, &dialects) {
                out.push(format!("{path}: {import}"));
            }
        }
    }
    out
}

/// The dialect (or shared directory) `path` belongs to, if it is under `dialect/` and not the
/// spine.
fn owner(path: &str) -> Option<String> {
    let rest = path.strip_prefix("dialect/")?;
    let first = rest.split('/').next()?.trim_end_matches(".rs");
    (!SPINE.contains(&first)).then(|| first.to_string())
}

/// Whether `import` (a crate path) is a forbidden edge from `path`.
fn forbidden(path: &str, import: &str, dialects: &[String]) -> bool {
    let has = |prefix: &str| import == prefix || import.starts_with(&format!("{prefix}::"));
    if path == "passes.rs" || path.starts_with("passes/") {
        return !PASSES_ALLOW.iter().any(|p| has(p));
    }
    let Some(me) = owner(path) else { return false };
    let parent = PARENTS.iter().find(|(d, _)| *d == me).map(|(_, p)| *p);
    let other = dialects
        .iter()
        .any(|d| *d != me && Some(d.as_str()) != parent && has(&format!("dialect::{d}")));
    other || DIALECT_FORBIDS.iter().any(|p| has(p))
}

/// The crate path of the module `path` holds (`dialect/ngspice/mod.rs` → `dialect::ngspice`).
fn module_path(path: &str) -> Vec<String> {
    let stem = path.trim_end_matches(".rs");
    let mut parts: Vec<String> = stem.split('/').map(str::to_string).collect();
    if matches!(parts.last().map(String::as_str), Some("mod" | "lib")) {
        parts.pop();
    }
    parts
}

/// Every crate path the `use` items of `text` import, `crate::`, `super::` and `self::`
/// resolved against `module`; braces expanded. External crates and `std` are left out.
fn imports(text: &str, module: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    // Bounded: each turn consumes at least one `use`.
    while let Some(at) = rest.find("use ") {
        let start = &rest[at..];
        let line_start = rest[..at].rfind('\n').map_or(0, |i| i + 1);
        let lead = rest[line_start..at]
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub ");
        let end = start.find(';').unwrap_or(start.len());
        if lead.trim().is_empty() {
            for path in expand(start[4..end].trim()) {
                out.extend(absolute(&path, module));
            }
        }
        rest = &start[end.min(start.len() - 1) + 1..];
    }
    out
}

/// `a::{b, c::{d}}` → `a::b`, `a::c::d`.
fn expand(tree: &str) -> Vec<String> {
    let mut tree = tree.split_whitespace().collect::<Vec<_>>().join(" ");
    for (spaced, tight) in [
        (" ::", "::"),
        (":: ", "::"),
        ("{ ", "{"),
        (" }", "}"),
        (", ", ","),
        (" ,", ","),
    ] {
        tree = tree.replace(spaced, tight);
    }
    let Some(open) = tree.find('{') else {
        return vec![tree];
    };
    let prefix = &tree[..open];
    let inner = &tree[open + 1..tree.rfind('}').unwrap_or(tree.len())];
    let mut parts = Vec::new();
    let (mut depth, mut from) = (0, 0);
    for (i, c) in inner.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&inner[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&inner[from..]);
    let mut out = Vec::new();
    for p in parts.into_iter().filter(|p| !p.is_empty()) {
        for leaf in expand(p) {
            let leaf = if leaf == "self" {
                prefix.trim_end_matches("::").to_string()
            } else {
                format!("{prefix}{leaf}")
            };
            out.push(leaf);
        }
    }
    out
}

/// `path` as a crate path from `module`, its `as` alias dropped; `None` for another crate.
fn absolute(path: &str, module: &[String]) -> Option<String> {
    let path = path.split(" as ").next().unwrap_or(path);
    let mut segs: Vec<&str> = path.split("::").collect();
    let mut base: Vec<String> = match segs.first().copied() {
        Some("crate") => Vec::new(),
        Some("super") | Some("self") => module.to_vec(),
        _ => return None,
    };
    if segs[0] == "crate" {
        segs.remove(0);
    }
    while let Some(&s) = segs.first() {
        match s {
            "super" => {
                base.pop();
            }
            "self" => {}
            _ => break,
        }
        segs.remove(0);
    }
    base.extend(segs.iter().map(|s| s.to_string()));
    Some(base.join("::"))
}
