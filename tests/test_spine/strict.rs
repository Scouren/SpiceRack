//! `Policy::Strict` (the spine's `read`): an `Unread` statement refused (`S0101`), an unresolved
//! parameter read refused at its item (`S0104`, R63, A38), a formal default's held read passed;
//! `Lenient` keeps both.
//!
//! Oracles: hand-written IR text; the refusal's code, item and name stated by hand.
//!
//! Contents: tests only.

use spicerack::dialect::{self, Policy, STRICT_UNBOUND, STRICT_UNREAD, Sources};
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
