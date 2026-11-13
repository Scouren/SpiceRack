//! The VACASK reader's binding through includes (R3: by VACASK's rule; item 2 of `vacask2`):
//! the definitions an include brings in (the included file's top, or the section `section=`
//! names, and what those include in turn) stand in the scope that holds the include, before or
//! after the include line, and are visible to the scopes inside it; parameters, models and
//! subcircuits alike.
//!
//! Oracle: VACASK 36be560 on the settle decks `b_incsec*.sim` (what it printed, asserted here
//! first), then the reader on the same files: what each read and master binds to.
//!
//! Contents: tests, and for `recursion` `vacask`.

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

use std::path::Path;
use std::process::Command;

/// The settle folder.
const SETTLE: &str = "tests/fixtures/dialect/vacask/settle/";

/// What VACASK printed for settle deck `<stem>.sim`, run in a fresh directory with each
/// `<stem>--<name>` file beside it as `<name>`.
pub(super) fn vacask(stem: &str) -> String {
    let dir = tempfile::tempdir().expect("a temporary directory");
    for e in std::fs::read_dir(SETTLE).expect("the settle directory") {
        let p = e.expect("an entry").path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let local = match name.strip_prefix(&format!("{stem}--")) {
            Some(rest) => rest.to_string(),
            None if name == format!("{stem}.sim") => name,
            None => continue,
        };
        std::fs::copy(&p, dir.path().join(local)).expect("a settle file");
    }
    let done = Command::new("vacask")
        .arg(Path::new(&format!("{stem}.sim")))
        .current_dir(dir.path())
        .output()
        .expect("vacask not runnable: run in `nix develop .#testing`");
    let mut out = String::from_utf8_lossy(&done.stdout).into_owned();
    out.push_str(&String::from_utf8_lossy(&done.stderr));
    out
}

// ---- private ----

#[test]
fn vacask_binds_through_an_included_section_before_and_after_the_include() {
    // b_incsec: in section mc, early=pd*10 stands before `include … section=dfl`, late=pt*(1+pd)
    // after it, other.inc (included in mc) reads pd; rm and cell are dfl's
    let got = resistances(&vacask("b_incsec"));
    let want = [
        ("ra", 2.2),
        ("rb", 1.0),
        ("rc", 10.0),
        ("r9", 1000.0),
        ("x1:r1", 3000.0),
    ];
    let want: Vec<(String, f64)> = want.iter().map(|(n, v)| (n.to_string(), *v)).collect();
    assert_eq!(got, want);
    // the same name at the deck's top and in the included section: neither wins
    // b_incsec_sub: a subcircuit inside the included section reads the section's q and model
    let got = resistances(&vacask("b_incsec_sub"));
    assert_eq!(got, vec![("x1:r1".to_string(), 5000.0)]);
    for deck in ["b_incsec_top_before", "b_incsec_top_after"] {
        assert!(vacask(deck).contains("Parameter redefinition."), "{deck}");
    }
}

#[test]
fn the_reader_binds_a_read_and_a_master_through_an_included_section() {
    let d = read();
    // section mc of lib.lib reads dfl's pd and pt
    assert_eq!(
        param_def(&d, "early", "pd"),
        Some(("lib.lib".into(), "dfl".into()))
    );
    assert_eq!(
        param_def(&d, "late", "pt"),
        Some(("lib.lib".into(), "dfl".into()))
    );
    // other.inc, included in mc, reads dfl's pd
    assert_eq!(
        param_def(&d, "ov", "pd"),
        Some(("lib.lib".into(), "dfl".into()))
    );
    // mc's r9 and x1 bind dfl's model and subcircuit
    assert_eq!(master_def(&d, "r9"), Some(("lib.lib".into(), "dfl".into())));
    assert_eq!(master_def(&d, "x1"), Some(("lib.lib".into(), "dfl".into())));
}

#[test]
fn the_reader_binds_a_deck_read_through_its_section_include() {
    let d = read();
    // the deck's ra reads late (mc's), rc reads ov (other.inc's, two includes down)
    assert_eq!(value_def(&d, "ra"), Some(("lib.lib".into(), "mc".into())));
    assert_eq!(value_def(&d, "rb"), Some(("lib.lib".into(), "mc".into())));
    assert_eq!(
        value_def(&d, "rc"),
        Some(("other.inc".into(), String::new()))
    );
}

#[test]
fn a_subckt_inside_a_section_sees_the_sections_definitions_as_the_top() {
    // b_incsec_sub: VACASK gives x1:r1 5000 (q=5 and rq of section s); a section is no scope of
    // VACASK's, so the read is no enclosing-scope read (R2003)
    let (d, diags) = read_files(&[
        ("b_incsec_sub.sim", "b_incsec_sub.sim"),
        ("b_incsec_sub--sub.lib", "sub.lib"),
    ]);
    let codes: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
    assert!(!codes.contains(&"R2003".to_string()), "{codes:?}");
    assert_eq!(master_def(&d, "r1"), Some(("sub.lib".into(), "s".into())));
    let e: ir::Element = d.row(item(&d, ir::ItemKind::Element, "r1").index);
    let mut todo = vec![d.passive(&e).value];
    let mut bound = None;
    // Bounded: each expression node is taken once (the tree is acyclic).
    while let Some(x) = todo.pop() {
        match d.expr(x) {
            ir::Expr::Param(r) => bound = place(&d, r.def),
            _ => d.children(x, &mut todo),
        }
    }
    assert_eq!(bound, Some(("sub.lib".into(), "s".into())));
}

/// b_incsec's three files as VACASK reads them (the libraries under the names it includes).
fn read() -> ir::Design {
    read_files(&[
        ("b_incsec.sim", "b_incsec.sim"),
        ("b_incsec--lib.lib", "lib.lib"),
        ("b_incsec--other.inc", "other.inc"),
    ])
    .0
}

/// The settle `files` read as VACASK reads them, each `(file, name it is included by)`.
fn read_files(files: &[(&str, &str)]) -> (ir::Design, ir::Diags) {
    let mut src = Sources::new();
    for &(file, name) in files {
        let text = std::fs::read_to_string(format!("{SETTLE}{file}")).expect("a settle file");
        src.add(name, &text);
    }
    let row = dialect::by_name("vacask").expect("the vacask row");
    let mut d = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, row, Policy::Lenient, &mut d, &mut diags);
    (d, diags)
}

/// A definition's file and section (empty outside every section).
fn place(d: &ir::Design, def: ir::DefId) -> Option<(String, String)> {
    if def.is_none() {
        return None;
    }
    let scope = d.scope(d.def(def).scope);
    let file = d.str(d.file(scope.file).path).to_string();
    let section = d
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Section)
        .map(|(i, _)| d.row::<ir::item::Section>(i.index))
        .find(|s| s.body == d.def(def).scope)
        .map_or(String::new(), |s| d.str(s.name).to_string());
    Some((file, section))
}

/// The item named `name` of `kind`, anywhere in `d`.
fn item(d: &ir::Design, kind: ir::ItemKind, name: &str) -> ir::ItemRef {
    d.reachable()
        .into_iter()
        .map(|(i, _)| i)
        .find(|i| {
            i.kind == kind
                && match kind {
                    ir::ItemKind::Element => d.str(d.row::<ir::Element>(i.index).name) == name,
                    _ => d.str(d.row::<ir::item::Param>(i.index).name) == name,
                }
        })
        .unwrap_or_else(|| panic!("an item {name}"))
}

/// The place of the definition the parameter read `read` in parameter `param`'s value binds.
fn param_def(d: &ir::Design, param: &str, read: &str) -> Option<(String, String)> {
    let p: ir::item::Param = d.row(item(d, ir::ItemKind::Param, param).index);
    let id = d.find_name(read);
    let mut todo = vec![p.value];
    // Bounded: each expression node is taken once (the tree is acyclic).
    while let Some(e) = todo.pop() {
        match d.expr(e) {
            ir::Expr::Param(r) if r.name == id => return place(d, r.def),
            _ => d.children(e, &mut todo),
        }
    }
    panic!("{param} does not read {read}")
}

/// The place of the definition element `name`'s master binds.
fn master_def(d: &ir::Design, name: &str) -> Option<(String, String)> {
    let e: ir::Element = d.row(item(d, ir::ItemKind::Element, name).index);
    place(d, e.model.def)
}

/// The place of the parameter element `name`'s value reads.
fn value_def(d: &ir::Design, name: &str) -> Option<(String, String)> {
    let e: ir::Element = d.row(item(d, ir::ItemKind::Element, name).index);
    match d.expr(d.passive(&e).value) {
        ir::Expr::Param(r) => place(d, r.def),
        other => panic!("{name}: a parameter read, not {other:?}"),
    }
}

/// The `resistance` each `print instance(…)` dump shows, by instance name, in order.
fn resistances(out: &str) -> Vec<(String, f64)> {
    let mut got = Vec::new();
    let mut name = None;
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("Osdi device instance ") {
            name = rest.split_whitespace().next().map(str::to_string);
        } else if let Some(v) = line.trim().strip_prefix("resistance = ")
            && let Some(n) = name.take()
        {
            let v = v.split_whitespace().next().expect("a value");
            got.push((n, v.parse().expect("a number")));
        }
    }
    got
}
