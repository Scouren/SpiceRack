//! The door for element names used as references (A32): `Editor::map_element_uses` calls the
//! pass on every by-name element reference an item holds, each path segment one use with the
//! scope its name is read in, and stores the renamed names back; `Design::element_uses` is its
//! read-only twin. The kernel's own part is `Expr::I`.
//!
//! Oracles: the scopes worked by hand from the subcircuits and instances; the IR text after a
//! rename.
//!
//! Contents: none (tests only).

use spicerack_ir::{Design, Diags, ElementUse, dump, parse_unchecked};

// ---- private ----

fn read(text: &str) -> Design {
    let mut d = Design::new();
    let mut diags = Diags::new();
    // a parameter reading `i(…)` is a `ProbeContext` violation that `parse` would refuse; the
    // element-use walk is about the reads, so the text is built as written
    parse_unchecked(text, &mut d.builder(), &mut diags);
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

#[cfg(feature = "gen")]
#[test]
fn every_listed_use_is_spelled_in_its_item() {
    // A32 soundness, the lead's property 1 (oracle: the IR text): renaming one listed use
    // through the door, and nothing else, changes the design's dump; a use the walk listed but
    // the item does not spell would leave the text as it was
    use spicerack_ir::FeatureSet;
    use spicerack_ir::r#gen::{Size, generate};
    let size = Size {
        items: 30,
        scope_depth: 3,
        expr_depth: 4,
        files: 1,
    };
    let mut checked = 0;
    for seed in 0..500u64 {
        let d = generate(seed, &FeatureSet::ALL, size);
        let before = text(&d);
        let mut per_item = std::collections::HashMap::new();
        for (item, _) in d.element_uses() {
            *per_item.entry(item).or_insert(0usize) += 1;
        }
        for (item, scope) in d.reachable() {
            for k in 0..per_item.get(&item).copied().unwrap_or(0) {
                let mut e = d.clone();
                let marker = e.builder().name("zz_marker");
                let mut seen = 0;
                e.editor().map_element_uses(scope, item, &mut |_| {
                    seen += 1;
                    (seen == k + 1).then_some(marker)
                });
                assert_ne!(
                    text(&e),
                    before,
                    "seed {seed}: use {k} of {item:?} is not spelled"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 100, "only {checked} uses checked");
}

#[cfg(feature = "gen")]
#[test]
fn renaming_every_element_leaves_no_old_name_in_the_text() {
    // A32 completeness, the lead's property 2 (oracle: the IR text as a whole, which knows
    // nothing of the rows): rename every element of every generated design and every use the
    // door lists that resolves to one; a reference some row forgot keeps its old name and is
    // found as a whole token, whichever row holds it
    use spicerack_ir::r#gen::{Size, generate};
    use spicerack_ir::{Element, FeatureSet, ItemKind, NameId, ScopeId};
    use std::collections::{HashMap, HashSet};
    let size = Size {
        items: 30,
        scope_depth: 3,
        expr_depth: 4,
        files: 1,
    };
    let (mut renamed, mut uses) = (0, 0);
    // the floor is on uses, not seeds: the generator's stream shifts with every change
    for seed in 0..20_000u64 {
        if seed >= 1000 && uses >= 20 {
            break;
        }
        let d = generate(seed, &FeatureSet::ALL, size);
        let mut e = d.clone();
        let items = d.reachable();
        let elements: Vec<(spicerack_ir::ItemRef, ScopeId, NameId)> = items
            .iter()
            .filter(|(i, _)| i.kind == ItemKind::Element)
            .map(|&(i, s)| (i, s, d.row::<Element>(i.index).name))
            .collect();
        let mut fresh: HashMap<NameId, NameId> = HashMap::new();
        for &(_, _, n) in &elements {
            let marker = e.builder().name(&format!("zz{}", d.str(n)));
            fresh.insert(n, marker);
        }
        let named: HashSet<(ScopeId, NameId)> = elements.iter().map(|&(_, s, n)| (s, n)).collect();
        for &(item, scope) in &items {
            e.editor().map_element_uses(scope, item, &mut |u| {
                let hit = named.contains(&(u.scope, u.name));
                uses += usize::from(hit);
                hit.then(|| fresh[&u.name])
            });
        }
        for &(item, _, n) in &elements {
            let row = Element {
                name: fresh[&n],
                ..e.row::<Element>(item.index)
            };
            e.editor().set(item.index, row);
        }
        renamed += elements.len();
        let after = text(&e);
        let old: HashSet<&str> = elements.iter().map(|&(_, _, n)| d.str(n)).collect();
        let left: Vec<&str> = after
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|t| old.contains(t))
            .collect();
        assert!(left.is_empty(), "seed {seed}: {left:?} left\n{after}");
    }
    assert!(
        renamed >= 1000 && uses >= 20,
        "{renamed} elements, {uses} uses renamed"
    );
}
