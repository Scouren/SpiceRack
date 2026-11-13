//! `Policy::Strict` (the spine's `read`): an `Unread` statement refused (`S0101`), an unresolved
//! parameter read refused at its item (`S0104`, R63, A38), a formal default's held read passed,
//! a run item whose target nothing defines refused at its item (`S0105`, R70, A43);
//! `Lenient` keeps them all.
//!
//! Oracles: hand-written IR text; the refusal's code, item and name stated by hand.
//!
//! Contents: tests only.

use spicerack::dialect::{
    self, Policy, STRICT_UNBOUND, STRICT_UNBOUND_TARGET, STRICT_UNREAD, Sources,
};
use spicerack::ir;

use super::ir_row;

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
        "loop sweep keep_last (device_param r1 r) (list values=[1k, 2k])",
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
