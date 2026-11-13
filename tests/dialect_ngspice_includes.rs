//! The ngspice reader's binding through includes (ruling R71): a scope sees the top-level
//! definitions of each file it includes, and the named section of a `.lib file section`, in
//! the order of the deck after expansion; an include in a `.subckt` body is local to it; a
//! `.param` defined twice keeps the last, a `.subckt` the first; an include cycle is refused.
//!
//! Oracle: ngspice-45 runs, the settle decks under `tests/fixtures/dialect/ngspice/settle/`
//! (`include_subckt_*`, `inc_*`, `inst_cycle_inc`), each with its printed value or message on its second line;
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

#[test]
fn an_instance_cycle_through_an_included_file_is_refused() {
    // inst_cycle_inc.sp: "unknown subckt: x1.xb.xa.xb…", exit 1 (a calls b of the included
    // file, b calls a); the X card closing it is P1110, so `check` sees no cycle (X0024)
    let (d, diags) = read(&["inst_cycle_inc.sp", "inst_cycle_inc.inc"]);
    assert!(codes(&diags).contains(&"P1110".to_string()), "{diags:?}");
    let mut found = ir::Diags::new();
    ir::check(&d, &mut found);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_section_calling_another_section_of_its_own_file_is_no_cycle_and_reads_its_caller() {
    // inc_lib_self.sp: i(v1) = -5e-4 (mim's R1 = 1k*scale, scale from the calling corner
    // section): the gf180 kit's idiom (sm141064.ngspice); no P1015, X1 and scale bound
    let (d, diags) = read(&["inc_lib_self.sp", "inc_lib_self.lib"]);
    assert!(!codes(&diags).contains(&"P1015".to_string()), "{diags:?}");
    assert_eq!(master_file(&d, "x1"), Some("inc_lib_self.lib".into()));
    let reads: Vec<ir::DefId> = (0..d.expr_count_all())
        .filter_map(|k| match d.expr(ir::ExprId(k as u32)) {
            ir::Expr::Param(r) if d.str(r.name) == "scale" => Some(r.def),
            _ => None,
        })
        .collect();
    assert!(
        !reads.is_empty() && reads.iter().all(|r| !r.is_none()),
        "scale bound: {reads:?}"
    );
}

#[test]
fn section_chains_of_one_file_are_no_cycle() {
    // inc_lib_chain.sp: a calls b calls c, d calls c too: i(v1) = -5e-4, "redefinition of
    // .subckt leaf, ignored"; no P1015, X1 bound
    let (d, diags) = read(&["inc_lib_chain.sp", "inc_lib_chain.lib"]);
    assert!(!codes(&diags).contains(&"P1015".to_string()), "{diags:?}");
    assert_eq!(master_file(&d, "x1"), Some("inc_lib_chain.lib".into()));
}

#[test]
fn a_formal_binds_in_its_own_subckt_also_in_a_lib_section() {
    // formal_scope_lib.sp: i(v1) = -0.5 (a's w = 1), i(v2) = -1/12 (b's w = 4 at x2), v(k) = 9
    // (the top w): F34, the reader bound every subckt's formal w to the last subckt's
    let (d, _) = read(&["formal_scope_lib.sp", "formal_scope.lib"]);
    assert_eq!(read_owner(&d, "x"), Some("a".into()));
    assert_eq!(read_owner(&d, "y"), Some("b".into()));
    assert_eq!(read_owner(&d, "t"), None, "t reads the top w");
}

#[test]
fn a_func_defined_in_the_deck_and_the_include_keeps_the_last() {
    // dup_func_inc_before.sp: v(n) = 5 (the included f, after the deck's); dup_func_inc_after.sp:
    // v(n) = 2 (the deck's f, after the include): a .func keeps the last, as a .param
    let (d, _) = read(&["dup_func_inc_before.sp", "dup_func.inc"]);
    assert_eq!(call_file(&d), Some("dup_func.inc".into()));
    let (d, _) = read(&["dup_func_inc_after.sp", "dup_func.inc"]);
    assert_eq!(call_file(&d), Some("dup_func_inc_after.sp".into()));
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

/// The subcircuit whose formal the `.param name={w}` reads (its value one parameter read), or
/// `None` when the bound definition stands outside every subcircuit.
fn read_owner(d: &ir::Design, name: &str) -> Option<String> {
    let item = d
        .reachable()
        .into_iter()
        .map(|(i, _)| i)
        .find(|i| {
            i.kind == ir::ItemKind::Param
                && d.str(d.row::<ir::item::Param>(i.index).name)
                    .eq_ignore_ascii_case(name)
        })
        .unwrap_or_else(|| panic!("a .param {name}"));
    let p: ir::item::Param = d.row(item.index);
    let ir::Expr::Param(r) = d.expr(p.value) else {
        panic!("{name}: a parameter read");
    };
    assert!(!r.def.is_none(), "{name}: bound");
    let owner = d.scope(d.def(r.def).scope).owner;
    (owner.kind == ir::ItemKind::Subckt).then(|| {
        d.str(d.row::<ir::item::Subckt>(owner.index).name)
            .to_string()
    })
}

/// The file of the `.func` the design's one user call binds to, or `None` (unresolved).
fn call_file(d: &ir::Design) -> Option<String> {
    let calls: Vec<ir::Ref> = (0..d.expr_count())
        .filter_map(
            |k| match d.expr(ir::ExprId(u32::try_from(k).expect("fits"))) {
                ir::Expr::UserCall(r, _) => Some(r),
                _ => None,
            },
        )
        .collect();
    assert_eq!(calls.len(), 1, "one user call: {calls:?}");
    def_file(d, calls[0].def)
}
