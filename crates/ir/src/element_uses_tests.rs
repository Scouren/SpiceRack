//! Path resolution for element uses (A32): segment 0 read in the item's scope, each next one in
//! the body of the master of the instance before it; unresolved after a non-instance.
//!
//! Oracles: the scopes worked by hand.
//!
//! Contents: none (tests only).

use crate::design::Design;
use crate::diag::Diags;
use crate::element_uses::path_scopes;
use crate::id::ScopeId;
use crate::item::{DefKind, Subckt};

// ---- private ----

#[test]
fn a_path_walks_into_each_instances_master() {
    let text = concat!(
        "file \"t.sp\" deck\n",
        "  subckt t ()\n",
        "    element resistor r1 (a b) value=1k\n",
        "  subckt s ()\n",
        "    element instance x2 () model=t\n",
        "  element instance x1 () model=s\n",
    );
    let mut d = Design::new();
    let mut diags = Diags::new();
    crate::text::parse(text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let top = d.file(d.entry()).top;
    let body = |n: &str| {
        let def = d.lookup(top, DefKind::Subckt, d.find_name(n));
        d.row::<Subckt>(d.def(def).item.index).body
    };
    let path = [d.find_name("x1"), d.find_name("x2"), d.find_name("r1")];
    assert_eq!(path_scopes(&d, top, &path), vec![top, body("s"), body("t")]);
    let bad = [d.find_name("r1"), d.find_name("x2")];
    assert_eq!(
        path_scopes(&d, top, &bad),
        vec![top, ScopeId::NONE],
        "r1 is no instance here"
    );
}
