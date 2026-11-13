//! `Policy::Strict` (the spine's `read`): an `Unread` statement refused (`S0101`), an unresolved
//! parameter read refused at its item (`S0104`, R63, A38) unless it stands in a library section
//! the program never selects (R63a, on the VACASK and ngspice readers), a formal default's held
//! read passed, a run item whose target nothing defines refused at its item (`S0105`, R70,
//! A43); `Lenient` keeps them all.
//!
//! Oracles: hand-written IR text; the refusal's code, item and name stated by hand; for R63a the
//! simulators' runs of the same decks (VACASK settle `b_strict_sel_*`, ngspice-45 run here).
//!
//! Contents: tests only.

use spicerack::dialect::{
    self, Policy, STRICT_UNBOUND, STRICT_UNBOUND_TARGET, STRICT_UNREAD, Sources,
};
use spicerack::ir;

use super::ir_row;

/// The R63a decks: (dialect, directory, the decks' extension, the library's file name as
/// the deck spells it, the library's fixture file).
const SELECT_DECKS: [(&str, &str, &str, &str, &str); 2] = [
    (
        "vacask",
        "tests/fixtures/dialect/vacask/settle/",
        ".sim",
        "sel.lib",
        "{stem}--sel.lib",
    ),
    (
        "ngspice",
        "tests/fixtures/spine/strict_sel/",
        ".sp",
        "sel.lib.sp",
        "sel.lib.sp",
    ),
];

// ---- private ----

#[test]
fn strict_refuses_an_unread_statement_and_lenient_keeps_it() {
    let text = "file \"o.sp\" deck\n  opaque ngspice unread \".foo bar\"\n";
    let src = Sources::one("o.ir", text);
    let strict = dialect::transpile(&src, ir_row(), ir_row(), Policy::Strict);
    let codes: Vec<ir::Code> = strict
        .expect_err("strict")
        .rows()
        .iter()
        .map(|d| d.code)
        .collect();
    assert!(codes.contains(&STRICT_UNREAD), "{codes:?}");
    let lenient = dialect::transpile(&src, ir_row(), ir_row(), Policy::Lenient);
    assert_eq!(lenient.expect("lenient keeps it").text, text);
}

#[test]
fn strict_refuses_an_unresolved_parameter_read_at_its_item_and_lenient_keeps_it() {
    // R63, A38: in a complete deck (strict) a read no definition binds is one the source
    // simulator rejects ("Variable or constant 'u' not defined.", VACASK which3).
    let text = "file \"u.sp\" deck\n  param a = 1\n  param x = a + u\n";
    let src = Sources::one("u.ir", text);
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, ir_row(), Policy::Strict, &mut design, &mut diags);
    let x = design
        .reachable()
        .into_iter()
        .map(|(i, _)| i)
        .filter(|i| i.kind == ir::ItemKind::Param)
        .nth(1)
        .expect("the second param");
    let rows: Vec<(ir::Code, ir::ItemRef, &str)> = diags
        .rows()
        .iter()
        .map(|d| (d.code, d.item, design.str(ir::NameId(d.arg))))
        .collect();
    assert_eq!(rows, [(STRICT_UNBOUND, x, "u")]);
    assert!(dialect::refuses(STRICT_UNBOUND));
    let lenient = dialect::transpile(&src, ir_row(), ir_row(), Policy::Lenient);
    assert_eq!(lenient.expect("lenient keeps it").text, text);
}

#[test]
fn strict_passes_a_formal_defaults_held_read() {
    // A29, A37: a head default's unbound read is looked up from each instance's scope: held,
    // not unresolved.
    let text = "file \"h.sp\" deck\n  subckt s (p) w=(u * 2)\n";
    let src = Sources::one("h.ir", text);
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, ir_row(), Policy::Strict, &mut design, &mut diags);
    assert!(
        ir::unbound_reads(&design).iter().all(|r| r.held),
        "the fixture holds a held read only"
    );
    assert!(
        !ir::unbound_reads(&design).is_empty(),
        "the fixture holds a read"
    );
    assert!(diags.is_empty(), "{:?}", diags.rows());
}

#[test]
fn strict_refuses_a_run_item_whose_target_nothing_defines() {
    // R70, A43 (the pair's F7): an alter of an element no statement defines ran on at the old
    // value on ngspice ("no such device"); a strict read refuses each such run item.
    let head = "file \"t.sp\" deck\n  param r = 1k\n  element resistor r1 (a 0) value=1k\n";
    for (run, name) in [
        ("alter (element_value r9) value=2k", "r9"),
        ("alter (param q) value=2k", "q"),
        ("analysis dc (source_dc v9) (list values=[0, 1])", "v9"),
        (
            "loop sweep keep_last (device_param r9 r) (list values=[1k, 2k])",
            "r9",
        ),
    ] {
        let text = format!("{head}  {run}\n");
        let (design, diags) = strict(&text);
        let rows: Vec<(ir::Code, ir::ItemKind, &str)> = diags
            .rows()
            .iter()
            .map(|d| (d.code, d.item.kind, design.str(ir::NameId(d.arg))))
            .collect();
        let kind = rows.first().map(|r| r.1);
        assert_eq!(rows.len(), 1, "{run}: {rows:?}");
        assert_eq!(rows[0].0, STRICT_UNBOUND_TARGET, "{run}");
        assert_eq!(rows[0].2, name, "{run}");
        assert!(
            kind != Some(ir::ItemKind::Element),
            "{run}: at the run item"
        );
        let lenient = dialect::transpile(
            &Sources::one("t.ir", &text),
            ir_row(),
            ir_row(),
            Policy::Lenient,
        );
        assert!(lenient.is_ok(), "{run}: lenient keeps it");
    }
    assert!(dialect::refuses(STRICT_UNBOUND_TARGET));
}

#[test]
fn strict_passes_a_run_item_whose_target_is_defined() {
    let head = "file \"t.sp\" deck\n  param r = 1k\n  element resistor r1 (a 0) value=1k\n";
    for run in [
        "alter (element_value r1) value=2k",
        "alter (param r) value=2k",
        "loop sweep keep_last (device_param r1 tc1) (list values=[1k, 2k])",
    ] {
        let (_, diags) = strict(&format!("{head}  {run}\n"));
        assert!(diags.is_empty(), "{run}: {:?}", diags.rows());
    }
}

#[test]
fn strict_passes_an_alter_through_an_instance_whose_master_is_unresolved() {
    // R70, A43: nothing proves the element missing when the path walks into a master no read
    // file defines (a kit not loaded): `ir::unbound_targets` leaves it out.
    let text = "file \"t.sp\" deck\n  element instance x1 (a 0) model=nosub\n  \
                alter (device_param r1 r path=(x1)) value=2k\n";
    let (_, diags) = strict(text);
    let codes: Vec<ir::Code> = diags.rows().iter().map(|d| d.code).collect();
    assert!(
        !codes.contains(&STRICT_UNBOUND_TARGET),
        "{:?}",
        diags.rows()
    );
}

#[test]
fn strict_refuses_an_unbound_read_only_in_a_selected_section() {
    // R63a: the library's section b reads an undefined `zz`, so does c, and d includes c. Both
    // simulators run the deck selecting a and reject b and d (VACASK settle b_strict_sel_*,
    // ngspice-45 its twins: DIALECT-VACASK Appendix A; the ngspice runs are repeated below).
    for (name, dir, ext, lib, lib_file) in SELECT_DECKS {
        let row = dialect::by_name(name).expect("a built-in row");
        for (stem, want) in [
            ("b_strict_sel_a", &[][..]),
            ("b_strict_sel_b", &["pb"][..]),
            ("b_strict_sel_d", &["pc"][..]),
        ] {
            let read = |file: &str| {
                std::fs::read_to_string(format!("{dir}{file}")).expect("a fixture file")
            };
            let mut src = Sources::new();
            src.add(&format!("{stem}{ext}"), &read(&format!("{stem}{ext}")));
            src.add(lib, &read(&lib_file.replace("{stem}", stem)));
            let mut design = ir::Design::new();
            let mut diags = ir::Diags::new();
            dialect::read(&src, row, Policy::Strict, &mut design, &mut diags);
            let got: Vec<(&str, &str)> = diags
                .rows()
                .iter()
                .filter(|d| d.code == STRICT_UNBOUND)
                .map(|d| {
                    assert_eq!(d.item.kind, ir::ItemKind::Param, "{name} {stem}");
                    let p: ir::item::Param = design.row(d.item.index);
                    (design.str(p.name), design.str(ir::NameId(d.arg)))
                })
                .collect();
            let want: Vec<(&str, &str)> = want.iter().map(|p| (*p, "zz")).collect();
            assert_eq!(got, want, "{name} {stem}: {:?}", diags.rows());
        }
    }
}

#[test]
fn ngspice_rejects_an_undefined_read_only_in_a_selected_section() {
    // The oracle of the ngspice half above: ngspice-45 runs the twins.
    let dir = SELECT_DECKS[1].1;
    for (stem, rejected) in [
        ("b_strict_sel_a", false),
        ("b_strict_sel_b", true),
        ("b_strict_sel_d", true),
    ] {
        let done = std::process::Command::new("ngspice")
            .args(["-b", &format!("{stem}.sp")])
            .current_dir(dir)
            .output()
            .expect("ngspice not runnable: run in `nix develop .#testing`");
        let out = String::from_utf8_lossy(&done.stdout).into_owned()
            + &String::from_utf8_lossy(&done.stderr);
        assert_eq!(done.status.success(), !rejected, "{stem}: {out}");
        assert_eq!(
            out.contains("Undefined parameter [zz]"),
            rejected,
            "{stem}: {out}"
        );
    }
}

/// `text` (IR text) read strictly through the `ir` row.
fn strict(text: &str) -> (ir::Design, ir::Diags) {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one("t.ir", text),
        ir_row(),
        Policy::Strict,
        &mut design,
        &mut diags,
    );
    (design, diags)
}
