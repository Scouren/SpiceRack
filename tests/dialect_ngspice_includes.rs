//! The ngspice reader's binding through includes (ruling R71): a scope sees the top-level
//! definitions of each file it includes, and the named section of a `.lib file section`, in
//! the order of the deck after expansion; an include in a `.subckt` body is local to it; a
//! `.param` defined twice keeps the last, a `.subckt` the first; an include cycle is refused.
//!
//! Oracle: ngspice-45 runs, the settle decks under `tests/fixtures/dialect/ngspice/settle/`
//! (`include_subckt_*`, `inc_*`), each with its printed value or message on its second line;
//! the test reads each deck with the files it names and asserts what each reference binds to.
//!
//! Contents: tests only.

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

/// The settle folder.
const SETTLE: &str = "tests/fixtures/dialect/ngspice/settle/";

// ---- private ----

#[test]
fn an_x_card_binds_to_a_subckt_of_a_file_included_before_or_after_it() {
    // include_subckt_ok.sp (the include after the card) and inc_after.sp (before): i(v1) =
    // -5e-4, R1 = 2k of the included sub; include_subckt_bad.sp: "Too many parameters", P1108
    for deck in ["include_subckt_ok.sp", "inc_after.sp"] {
        let (d, _) = read(&[deck, "include_subckt.inc"]);
        assert_eq!(
            master_file(&d, "x1"),
            Some("include_subckt.inc".into()),
            "{deck}"
        );
    }
    let (_, diags) = read(&["include_subckt_bad.sp", "include_subckt.inc"]);
    assert!(codes(&diags).contains(&"P1108".to_string()), "{diags:?}");
}

#[test]
fn an_include_in_a_subckt_body_is_local_to_it() {
    // inc_local_in.sp: i(v1) = -7.5e-4 (X1's 2k and R2 = pl = 4k inside wrap);
    // inc_local_out.sp: "unknown subckt: x2 n1 sub"; inc_local_param.sp: "Undefined parameter [pl]"
    let (d, _) = read(&["inc_local_in.sp", "inc_local.inc"]);
    assert_eq!(master_file(&d, "x1"), Some("inc_local.inc".into()));
    assert_eq!(value_file(&d, "r2"), Some("inc_local.inc".into()));
    let (d, _) = read(&["inc_local_out.sp", "inc_local.inc"]);
    assert_eq!(master_file(&d, "x2"), None, "sub is wrap's, not the top's");
    let (d, _) = read(&["inc_local_param.sp", "inc_local.inc"]);
    assert_eq!(value_file(&d, "r3"), None, "pl is wrap's");
}

#[test]
fn a_lib_call_binds_its_named_section_only() {
    // inc_section_named.sp: i(v1) = -5e-4; inc_section_other.sp: "unknown subckt: x1 n1 other"
    let (d, _) = read(&["inc_section_named.sp", "inc_sections.lib"]);
    assert_eq!(master_file(&d, "x1"), Some("inc_sections.lib".into()));
    let (d, _) = read(&["inc_section_other.sp", "inc_sections.lib"]);
    assert_eq!(master_file(&d, "x1"), None);
}

#[test]
fn a_name_defined_in_the_deck_and_the_include_keeps_the_first_subckt_and_the_last_param() {
    // inc_twice_before.sp: 1.25 mA (the deck's sub 1k, the include's p 4k); inc_twice_after.sp:
    // 1.5 mA (the include's sub 2k, the deck's p 1k); "redefinition of .subckt sub, ignored"
    let (d, _) = read(&["inc_twice_before.sp", "inc_twice.inc"]);
    assert_eq!(master_file(&d, "x1"), Some("inc_twice_before.sp".into()));
    assert_eq!(value_file(&d, "r2"), Some("inc_twice.inc".into()));
    let (d, _) = read(&["inc_twice_after.sp", "inc_twice.inc"]);
    assert_eq!(master_file(&d, "x1"), Some("inc_twice.inc".into()));
    assert_eq!(value_file(&d, "r2"), Some("inc_twice_after.sp".into()));
}

#[test]
fn a_model_a_param_and_a_nested_include_bind_across_files() {
    // inc_model_param.sp: i(v1) = -6.0203468e-3 (D1's dm and R1 = rv = 2k, read before the
    // include); inc_nested.sp: i(v1) = -5e-4 (sub two includes down)
    let (d, _) = read(&["inc_model_param.sp", "inc_model_param.inc"]);
    assert_eq!(master_file(&d, "d1"), Some("inc_model_param.inc".into()));
    assert_eq!(value_file(&d, "r1"), Some("inc_model_param.inc".into()));
    let (d, _) = read(&["inc_nested.sp", "inc_nested_a.inc", "inc_nested_b.inc"]);
    assert_eq!(master_file(&d, "x1"), Some("inc_nested_b.inc".into()));
}

#[test]
fn an_include_cycle_is_refused() {
    // inc_cycle.sp: ngspice-45 crashes (exit 139); the two includes of the cycle are P1015
    let (_, diags) = read(&["inc_cycle.sp", "inc_cycle_a.inc", "inc_cycle_b.inc"]);
    let cycles = codes(&diags).iter().filter(|c| *c == "P1015").count();
    assert_eq!(cycles, 2, "{diags:?}");
}

/// The files read (the first the deck) from the settle folder, leniently.
fn read(files: &[&str]) -> (ir::Design, ir::Diags) {
    let mut src = Sources::new();
    for f in files {
        let text = std::fs::read_to_string(format!("{SETTLE}{f}")).expect("a settle file");
        src.add(f, &text);
    }
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let mut d = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, row, Policy::Lenient, &mut d, &mut diags);
    (d, diags)
}

/// The codes of `diags`.
fn codes(diags: &ir::Diags) -> Vec<String> {
    diags.rows().iter().map(|r| r.code.to_string()).collect()
}

/// The element named `name` (folded), anywhere in `d`.
fn element(d: &ir::Design, name: &str) -> ir::Element {
    let item = d
        .reachable()
        .into_iter()
        .map(|(i, _)| i)
        .find(|i| {
            i.kind == ir::ItemKind::Element
                && d.str(d.row::<ir::Element>(i.index).name)
                    .eq_ignore_ascii_case(name)
        })
        .unwrap_or_else(|| panic!("an element {name}"));
    d.row(item.index)
}

/// The file of a definition.
fn def_file(d: &ir::Design, def: ir::DefId) -> Option<String> {
    (!def.is_none()).then(|| {
        d.str(d.file(d.scope(d.def(def).scope).file).path)
            .to_string()
    })
}

/// The file whose definition the master of element `name` is bound to, or `None`.
fn master_file(d: &ir::Design, name: &str) -> Option<String> {
    def_file(d, element(d, name).model.def)
}

/// The file whose `.param` the value `{p}` of element `name` reads, or `None` (unresolved).
fn value_file(d: &ir::Design, name: &str) -> Option<String> {
    let e = element(d, name);
    match d.expr(d.passive(&e).value) {
        ir::Expr::Param(r) => def_file(d, r.def),
        other => panic!("{name}: a parameter read, not {other:?}"),
    }
}
