//! The door for element names used as references (A32): `Editor::map_element_uses` calls the
//! pass on every by-name element reference an item holds, each path segment one use with the
//! scope its name is read in, and stores the renamed names back; `Design::element_uses` is its
//! read-only twin. The kernel's own part is `Expr::I`.
//!
//! Oracles: the scopes worked by hand from the subcircuits and instances; the IR text after a
//! rename.
//!
//! Contents: none (tests only).

use spicerack_ir::{Design, Diags, ElementUse, dump, parse};

// ---- private ----

fn read(text: &str) -> Design {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    d
}

fn text(d: &Design) -> String {
    let mut out = String::new();
    dump(d, d.entry(), &mut out);
    out
}

#[test]
fn a_current_read_is_an_element_use_and_is_renamed() {
    let src =
        "file \"t.sp\" deck\n  element resistor r1 (a b) value=1k\n  param p = i(r1) + i(r9)\n";
    let mut d = read(src);
    let top = d.file(d.entry()).top;
    let (r1, r9) = (d.find_name("r1"), d.find_name("r9"));
    let uses: Vec<ElementUse> = d.element_uses().into_iter().map(|(_, u)| u).collect();
    assert_eq!(
        uses,
        vec![
            ElementUse {
                scope: top,
                name: r1
            },
            ElementUse {
                scope: top,
                name: r9
            }
        ],
        "both reads, in tree order, read in the param's scope"
    );
    // rename r1 to rr1 in the row and in every read
    let rr1 = d.builder().name("rr1");
    let element = d.items(top)[0];
    let mut row: spicerack_ir::Element = d.row(element.index);
    row.name = rr1;
    d.editor().set(element.index, row);
    let param = d.items(top)[1];
    let mut seen = Vec::new();
    d.editor().map_element_uses(top, param, &mut |u| {
        seen.push(u);
        (u.name == r1).then_some(rr1)
    });
    assert_eq!(seen.len(), 2);
    assert!(text(&d).contains("i(rr1) + i(r9)"), "{}", text(&d));
    // an identity call changes nothing
    let before = d.clone();
    d.editor().map_element_uses(top, param, &mut |_| None);
    assert!(d.eq_ignoring_spans(&before));
}
