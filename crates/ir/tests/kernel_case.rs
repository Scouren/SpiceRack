//! `Extra::CaseSensitiveNames` per namespace: two names of one namespace that differ only in
//! ASCII case, a namespace being what a simulator's own lookup separates (the definitions of
//! one kind in a scope, the elements of a scope, the nets of a scope, the keys of one parameter
//! list), reported at the item that brings the second spelling.
//!
//! Oracles: spec examples worked by hand, each clash beside its neighbour in another namespace
//! or another scope (a case-folding target merges nothing there).
//!
//! Contents: none (tests only).

use spicerack_ir::{Design, Diags, Extra, Feature, FeatureSet, check_legal, features_used, parse};

// ---- private ----

fn read(body: &str) -> Design {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(
        &format!("file \"t.sp\" deck\n{body}"),
        &mut d.builder(),
        &mut diags,
    );
    assert!(diags.is_empty(), "{body}: {:?}", diags.rows());
    d
}

fn clash(body: &str) -> bool {
    features_used(&read(body)).contains(Feature::Extra(Extra::CaseSensitiveNames))
}

#[test]
fn one_clash_per_namespace() {
    assert!(
        clash("  param p = 1\n  param P = 2\n"),
        "two definitions of one kind"
    );
    assert!(
        clash("  element resistor r1 (a b) value=1k\n  element resistor R1 (c d) value=1k\n"),
        "two elements of a scope"
    );
    assert!(
        clash("  element resistor r1 (a b) value=1k\n  element resistor r2 (A b) value=1k\n"),
        "two nets of a scope"
    );
    assert!(
        clash("  subckt s () w=1 W=2\n"),
        "two keys of one parameter list"
    );
}

#[test]
fn names_of_two_namespaces_do_not_clash() {
    // an element `IS` beside a parameter `is`; one name in two scopes; a net beside an element
    assert!(!clash(
        "  element resistor IS (a b) value=1k\n  param is = 1\n"
    ));
    assert!(!clash("  param p = 1\n  subckt s ()\n    param P = 2\n"));
    assert!(!clash("  element resistor r1 (R1 b) value=1k\n"));
    // a string the pool holds but no live item uses
    let mut d = read("  param p = 1\n");
    d.builder().name("P");
    assert!(!features_used(&d).contains(Feature::Extra(Extra::CaseSensitiveNames)));
}

#[test]
fn the_clash_is_reported_at_the_second_item() {
    let d = read("  param p = 1\n  param P = 2\n");
    let top = d.file(d.entry()).top;
    let second = d.items(top)[1];
    let legal =
        FeatureSet::ALL.minus(FeatureSet::EMPTY.with(Feature::Extra(Extra::CaseSensitiveNames)));
    let mut diags = Diags::new();
    check_legal(&d, &legal, &mut diags);
    assert_eq!(diags.rows().len(), 1);
    assert_eq!(diags.rows()[0].item, second);
}
