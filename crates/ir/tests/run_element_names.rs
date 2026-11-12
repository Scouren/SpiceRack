//! The run rows' element names (A32): a deck's run rows in, their by-name element uses out, and
//! the IR text after a pass renames an instance. Oracles: the scopes worked by hand from the
//! subcircuits and instances; the expected text written by hand.
//!
//! Cites: `rewrite/AMEND-A12.md` A32; `crates/ir/src/run_names.rs`.
//!
//! Contents: tests only.

use spicerack_ir::{Design, Diags, ElementUse, NameId, dump, parse};

/// Two instance levels: `x1` (of `outer`) in the top, `x2` (of `inner`) in `outer`'s body,
/// `r1` and the net `n` in `inner`'s.
const DECK: &str = "file \"t.sp\" deck
  subckt inner (a)
    element resistor r1 (a n) value=1k
  subckt outer (a)
    element instance x2 (a) model=inner
  element instance x1 (in) model=outer
  save (v (x1 x2 n)) (i (x1 x2 r1))
  nodeset ic transient node=(x1 x2 n) value=1
  alter (element_value r1 path=(x1 x2)) value=2k
  analysis noise (v (out)) source=v1 (lin start=1 stop=2 step=1)
";

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
fn net_paths_use_their_instances_and_element_paths_every_segment() {
    let d = read(DECK);
    let top = d.file(d.entry()).top;
    let n = |s: &str| d.find_name(s);
    // the bodies: `inner`'s and `outer`'s, the scopes owned by the two subckts
    let items = d.items(top);
    let body = |k: usize| d.row::<spicerack_ir::item::Subckt>(items[k].index).body;
    let (inner, outer) = (body(0), body(1));
    let save = items[3];
    let uses: Vec<ElementUse> = d
        .element_uses()
        .into_iter()
        .filter(|(i, _)| *i == save)
        .map(|(_, u)| u)
        .collect();
    let u = |scope, name| ElementUse { scope, name };
    assert_eq!(
        uses,
        vec![
            // v(x1.x2.n): the two instances, not the net
            u(top, n("x1")),
            u(outer, n("x2")),
            // i(x1.x2.r1): every segment
            u(top, n("x1")),
            u(outer, n("x2")),
            u(inner, n("r1")),
        ]
    );
    let of = |k: usize| -> Vec<NameId> {
        d.element_uses()
            .into_iter()
            .filter(|(i, _)| *i == items[k])
            .map(|(_, u)| u.name)
            .collect()
    };
    assert_eq!(of(4), vec![n("x1"), n("x2")], "the node set's instances");
    assert_eq!(
        of(5),
        vec![n("x1"), n("x2"), n("r1")],
        "the alter's element path"
    );
    assert_eq!(
        of(6),
        vec![n("v1")],
        "the noise source; its voltage output is flat"
    );
}

#[test]
fn renaming_the_outer_instance_follows_every_path() {
    let mut d = read(DECK);
    let top = d.file(d.entry()).top;
    let (x1, xx1) = (d.find_name("x1"), d.builder().name("xx1"));
    let instance = d.items(top)[2];
    let mut row: spicerack_ir::Element = d.row(instance.index);
    row.name = xx1;
    d.editor().set(instance.index, row);
    let items: Vec<_> = d.items(top).to_vec();
    for item in items {
        d.editor()
            .map_element_uses(top, item, &mut |u| (u.name == x1).then_some(xx1));
    }
    let want = DECK
        .replace("instance x1 ", "instance xx1 ")
        .replace("(x1 x2", "(xx1 x2");
    assert_eq!(text(&d), want);
}
