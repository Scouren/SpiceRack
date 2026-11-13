//! R80's step `passes::structure::unused::drop_unused` on hand-written IR text. The oracle is
//! metamorphic, apart from the step's own walk: a deck with an unused definition of each kind
//! holding a feature the target lacks converts to the same text as the deck without it, with
//! one note per (definition, feature); the same definition made used is kept, and its refusal
//! comes back. Each way a definition is used (a section an include names, an unresolved name, a
//! bin's base, an opaque statement's text, another used definition) is one case whose expected
//! text is written by hand.
//!
//! Contents: tests only.

use spicerack::ir;
use spicerack::passes;
use spicerack::passes::structure::unused::{
    Roots, UNUSED_DROPPED, drop_unused, dropped_for, lacking,
};

/// The deck every case adds to: nothing in it lacks a feature.
const BASE: &str = "file \"a.sp\" deck\n  title \"t\"\n  element resistor r1 (a 0) value=1\n";

const DIODE: ir::Feature = ir::Feature::Family(ir::ModelFamily::Diode);

const ABS: ir::Feature = ir::Feature::Func(ir::Func::Abs);

// ---- private ----

/// The target lacks the diode family and `abs`.
fn target() -> ir::FeatureSet {
    let absent = ir::FeatureSet::EMPTY
        .with(ir::Feature::Family(ir::ModelFamily::Diode))
        .with(ir::Feature::Func(ir::Func::Abs));
    ir::FeatureSet::ALL.minus(absent)
}

/// `text` parsed, `drop_unused` run on it: the design's text after, the notes' features, the
/// count it returned, and the refusals `check_legal` still reports.
fn run(text: &str) -> (String, Vec<ir::Feature>, u32, usize) {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "the fixture parses: {:?}", diags.rows());
    let refused = lacking(&design, &target());
    let n = drop_unused(&mut design, &refused, Roots::Entry, &mut diags);
    let mut notes = Vec::new();
    for d in diags.rows() {
        assert_eq!(d.code, UNUSED_DROPPED, "only the step's notes");
        let (code, arg) = dropped_for(d.arg);
        assert_eq!(code, ir::diag::UNSUPPORTED, "a feature's refusal");
        notes.push(ir::Feature::from_index(arg as u16));
    }
    let mut after = ir::Diags::new();
    ir::check(&design, &mut after);
    assert!(
        after.is_empty(),
        "the design stays checked: {:?}",
        after.rows()
    );
    ir::check_legal(&design, &target(), &mut after);
    let mut out = String::new();
    for f in design.files() {
        ir::dump(&design, f, &mut out);
    }
    (out, notes, n, after.len())
}

/// One call of the step on `design` under its laws: P1 (`checked` runs `ir::check`), the
/// generated property (`checked`), the count and the notes (every outermost definition that
/// went, and only those, has a note at it), `features_used(after) ⊆ features_used(before)`,
/// and P4 (a second call drops nothing and leaves the text). Returns the count.
fn laws(design: &mut ir::Design, target: &ir::FeatureSet, seed: u64) -> u32 {
    let before = design.clone();
    let mut notes = ir::Diags::new();
    let refused = lacking(design, target);
    let n = drop_unused(design, &refused, Roots::Entry, &mut notes);
    let gone = checked(&before, design, target, seed);
    assert_eq!(
        n as usize,
        gone.len(),
        "seed {seed}: the count is the outermost drops"
    );
    let mut noted: Vec<ir::ItemRef> = notes.rows().iter().map(|d| d.item).collect();
    noted.dedup();
    let (mut a, mut b) = (noted.clone(), gone.clone());
    a.sort_by_key(|i| (i.kind as u8, i.index));
    a.dedup();
    b.sort_by_key(|i| (i.kind as u8, i.index));
    assert_eq!(a, b, "seed {seed}: a note at every drop, and only there");
    let grew = ir::features_used(design).minus(ir::features_used(&before));
    assert!(grew.is_empty(), "seed {seed}: the step adds no feature");
    let text = |d: &ir::Design| {
        let mut t = String::new();
        d.files().for_each(|f| ir::dump(d, f, &mut t));
        t
    };
    let mut again = design.clone();
    let mut more = ir::Diags::new();
    let refused = lacking(&again, target);
    let n2 = drop_unused(&mut again, &refused, Roots::Entry, &mut more);
    assert_eq!(n2, 0, "seed {seed}: P4");
    assert_eq!(text(&again), text(design), "seed {seed}: P4");
    n
}

/// The checker of the generated property; returns the outermost definitions gone.
fn checked(
    before: &ir::Design,
    after: &ir::Design,
    target: &ir::FeatureSet,
    seed: u64,
) -> Vec<ir::ItemRef> {
    use std::collections::{HashMap, HashSet};
    let mut e = ir::Diags::new();
    ir::check(after, &mut e);
    assert!(e.is_empty(), "seed {seed}: checked after: {:?}", e.rows());
    let was: HashMap<ir::ItemRef, ir::ScopeId> = before.reachable().into_iter().collect();
    let is: HashSet<ir::ItemRef> = after.reachable().into_iter().map(|p| p.0).collect();
    let def = |k: ir::ItemKind| {
        use ir::ItemKind as K;
        matches!(
            k,
            K::Subckt | K::Model | K::Param | K::Func | K::DeviceLibrary
        )
    };
    let owner = |i: &ir::ItemRef| before.scope(was[i]).owner;
    let gone: Vec<ir::ItemRef> = was.keys().filter(|i| !is.contains(i)).copied().collect();
    let outer: Vec<ir::ItemRef> = gone
        .iter()
        .filter(|i| !gone.contains(&owner(i)))
        .copied()
        .collect();
    for i in &outer {
        assert!(def(i.kind), "seed {seed}: {i:?} went and is no definition");
    }
    let mut names = HashSet::new();
    for &i in &is {
        for (r, _) in refs_and_reads(after, i) {
            if r.is_resolved() {
                let d = after.definitions()[r.def.index()].item;
                assert!(
                    is.contains(&d),
                    "seed {seed}: {i:?} binds {d:?}, which went"
                );
            } else if !r.name.is_none() {
                names.insert(after.str(r.name).to_ascii_lowercase());
            }
        }
    }
    let mut held = ir::Diags::new();
    ir::check_legal(before, target, &mut held);
    let held: Vec<ir::ItemRef> = held.rows().iter().map(|d| d.item).collect();
    for d in &outer {
        let at = |i: &ir::ItemRef| {
            let mut s = was.get(i).copied().unwrap_or(ir::ScopeId::NONE);
            // bounded by the scope tree's depth
            while !s.is_none() && before.scope(s).owner != *d {
                s = before.scope(s).parent;
            }
            i == d || !s.is_none()
        };
        let named = refs_and_reads(before, *d).iter().any(|(r, _)| {
            r.is_resolved() && outer.contains(&before.definitions()[r.def.index()].item)
        });
        let inside_named = was.keys().filter(|i| at(i)).any(|&i| {
            refs_and_reads(before, i).iter().any(|(r, _)| {
                r.is_resolved() && outer.contains(&before.definitions()[r.def.index()].item)
            })
        });
        assert!(
            held.iter().any(at) || named || inside_named,
            "seed {seed}: {d:?} went without a lacking feature"
        );
        let o = owner(d);
        if o == ir::ItemRef::NONE || o.kind == ir::ItemKind::Section {
            for x in before.definitions().iter().filter(|x| x.item == *d) {
                let name = before.str(x.name).to_ascii_lowercase();
                assert!(
                    !names.contains(&name),
                    "seed {seed}: {name} went and is spelled"
                );
            }
        }
    }
    outer
}

/// Every reference `item`'s row holds and every parameter read and user call in its
/// expressions.
fn refs_and_reads(design: &ir::Design, item: ir::ItemRef) -> Vec<(ir::Ref, ir::DefKind)> {
    let mut out = Vec::new();
    design.item_refs(item, &mut out);
    let mut handles = Vec::new();
    design.item_handles(item, &mut handles);
    let mut todo: Vec<ir::ExprId> = Vec::new();
    for h in handles {
        match h {
            ir::Handle::Expr(x) | ir::Handle::ExprDyn(x) => todo.push(x),
            ir::Handle::Exprs(l) => todo.extend_from_slice(design.exprs(l)),
            ir::Handle::Params(l) => todo.extend(design.params(l).iter().map(|p| p.1)),
            ir::Handle::Branches(l) => todo.extend(design.branches(l).iter().map(|b| b.cond)),
            _ => {}
        }
    }
    // bounded: expression rows form a tree below their parents
    while let Some(x) = todo.pop() {
        if x.is_none() {
            continue;
        }
        match design.expr(x) {
            ir::Expr::Param(r) => out.push((r, ir::DefKind::Param)),
            ir::Expr::UserCall(r, l) => {
                out.push((r, ir::DefKind::Func));
                todo.extend_from_slice(design.exprs(l));
            }
            ir::Expr::Call(_, l) | ir::Expr::Draw(_, _, _, l) | ir::Expr::List(l) => {
                todo.extend_from_slice(design.exprs(l))
            }
            ir::Expr::Unary(_, a) => todo.push(a),
            ir::Expr::Binary(_, a, b) => todo.extend([a, b]),
            ir::Expr::Ternary(c, a, b) => todo.extend([c, a, b]),
            _ => {}
        }
    }
    out
}

#[test]
fn an_unused_definition_of_each_kind_goes_with_one_note() {
    let cases: &[(&str, ir::Feature)] = &[
        ("  model dx diode\n", DIODE),
        (
            "  subckt s (p q)\n    model dm diode\n    element diode d1 (p q) model=dm\n",
            DIODE,
        ),
        ("  param pa = abs(-2)\n", ABS),
        ("  func fa(x) = abs(x)\n", ABS),
    ];
    for (def, feature) in cases {
        let (out, notes, n, refused) = run(&format!("{BASE}{def}"));
        assert_eq!(out, BASE, "{def}");
        assert_eq!(notes, vec![*feature], "{def}");
        assert_eq!((n, refused), (1, 0), "{def}");
    }
}

#[test]
fn a_used_definition_stays_and_is_still_refused() {
    let cases: &[&str] = &[
        "  model dx diode\n  element diode d1 (a 0) model=dx\n",
        "  subckt s (p q)\n    model dm diode\n    element diode d1 (p q) model=dm\n  element instance x1 (a 0) model=s\n",
        "  param pa = abs(-2)\n  element resistor r2 (a 0) value=pa\n",
        "  func fa(x) = abs(x)\n  element resistor r2 (a 0) value=(@fa(1))\n",
    ];
    for def in cases {
        let text = format!("{BASE}{def}");
        let (out, notes, n, refused) = run(&text);
        assert_eq!(out, text, "{def}");
        assert_eq!((notes.len(), n), (0, 0), "{def}");
        assert!(refused > 0, "{def}");
    }
}

#[test]
fn an_item_in_any_section_keeps_what_it_binds_and_a_model_nothing_binds_goes() {
    let lib = "file \"m.lib\" library\n  section tt\n    model dx diode\n    element diode d9 (a 0) model=dx\n  section ff\n    model dy diode\n";
    let deck = "file \"a.sp\" deck\n  title \"t\"\n  include \"m.lib\" section=tt\n";
    let (out, notes, n, _) = run(&format!("{deck}{lib}"));
    let kept = "file \"m.lib\" library\n  section tt\n    model dx diode\n    element diode d9 (a 0) model=dx\n  section ff\n";
    assert_eq!(out, format!("{deck}{kept}"));
    assert_eq!((notes, n), (vec![DIODE], 1));
}

#[test]
fn an_unresolved_name_uses_the_definitions_it_spells() {
    // a call binds a function only, so `@PA(1)` stays unresolved and its name uses `pa`
    let deck = format!("{BASE}  element resistor r2 (a 0) value=(@PA(1))\n  param pa = abs(-2)\n");
    let (out, notes, n, _) = run(&format!("{deck}  param pb = abs(-3)\n"));
    assert_eq!(out, deck);
    assert_eq!((notes, n), (vec![ABS], 1));
}

#[test]
fn a_model_reference_uses_every_bin_of_its_base() {
    let bins = "  model dx.1 diode bin_of=dx\n  model dx.2 diode bin_of=dx\n  model dz.1 diode bin_of=dz\n";
    let deck = format!("{BASE}  element diode d1 (a 0) model=dx?\n");
    let (out, notes, n, _) = run(&format!("{deck}{bins}"));
    let kept = "  model dx.1 diode bin_of=dx\n  model dx.2 diode bin_of=dx\n";
    assert_eq!(out, format!("{deck}{kept}"));
    assert_eq!((notes, n), (vec![DIODE], 1));
}

#[test]
fn an_opaque_statement_uses_the_names_it_spells() {
    let deck = format!(
        "{BASE}  opaque ngspice unread \".meas tran m find par('PA * 2')\"\n  param pa = abs(-2)\n  param pb = abs(-3)\n"
    );
    let (out, notes, n, _) = run(&deck);
    let kept = format!(
        "{BASE}  opaque ngspice unread \".meas tran m find par('PA * 2')\"\n  param pa = abs(-2)\n"
    );
    assert_eq!(out, kept);
    assert_eq!((notes, n), (vec![ABS], 1));
}

#[test]
fn a_definition_only_unused_ones_name_goes_with_them() {
    // `s2` holds the diode; `s1` names it and holds nothing lacking: both go, `s1` with the
    // feature of the definition it would name
    let defs = "  subckt s2 (p q)\n    model dm diode\n    element diode d1 (p q) model=dm\n  subckt s1 (p q)\n    element instance x1 (p q) model=s2\n";
    let (out, notes, n, _) = run(&format!("{BASE}{defs}"));
    assert_eq!(out, BASE);
    assert_eq!((notes, n), (vec![DIODE, DIODE], 2));
}

#[test]
fn a_definition_a_used_one_names_stays() {
    // `s2` is named by the used `s1` and the unused `s3`: it stays, and so does `s3`
    let defs = "  subckt s2 (p q)\n    model dm diode\n    element diode d1 (p q) model=dm\n  subckt s1 (p q)\n    element instance x1 (p q) model=s2\n  subckt s3 (p q)\n    element instance x1 (p q) model=s2\n  element instance x9 (a 0) model=s1\n";
    let text = format!("{BASE}{defs}");
    let (out, notes, n, refused) = run(&text);
    assert_eq!(out, text);
    assert_eq!((notes.len(), n), (0, 0));
    assert!(refused > 0);
}

#[test]
fn on_generated_designs_nothing_used_goes_and_every_drop_held_a_lacking_feature() {
    // the checker, apart from the step: after it, every reference and read a surviving item
    // binds names a surviving item; nothing but a definition (or what stands in one) went; no
    // dropped definition at a file's top or in a section is spelled by a surviving item's
    // unresolved name; and every dropped outermost definition held a refusal or named a dropped one
    let absent = (0..ir::Feature::COUNT)
        .map(ir::Feature::from_index)
        .filter(|f| matches!(f, ir::Feature::Family(_) | ir::Feature::Func(_)))
        .fold(ir::FeatureSet::EMPTY, |s, f| s.with(f));
    let target = ir::FeatureSet::ALL.minus(absent);
    let (mut ran, mut dropped_any, mut rounds_max) = (0, 0, 0);
    for seed in crate::SEEDS {
        let before = ir::r#gen::generate(seed, &ir::FeatureSet::ALL, crate::SIZE);
        // the step alone
        let mut design = before.clone();
        let n = laws(&mut design, &target, seed);
        dropped_any += usize::from(n > 0);
        // a kit: every definition is a root
        let mut kit = before.clone();
        let mut notes = ir::Diags::new();
        let refused = lacking(&kit, &target);
        assert_eq!(drop_unused(&mut kit, &refused, Roots::Every, &mut notes), 0);
        assert!(notes.is_empty(), "seed {seed}: a kit drops nothing");
        // the spine's outer loop, copied: the rules' fixed point, then the step, until a round
        // drops nothing, bounded by the definition count
        let mut design = before.clone();
        let bound = design.definitions().len();
        let mut rounds = 0;
        loop {
            let mut scratch = ir::Diags::new();
            passes::legalize(&mut design, &target, &mut scratch);
            if laws(&mut design, &target, seed) == 0 {
                break;
            }
            rounds += 1;
            assert!(
                rounds <= bound,
                "seed {seed}: each round drops a definition"
            );
        }
        rounds_max = rounds_max.max(rounds);
        ran += 1;
    }
    assert!(ran > 0);
    assert!(dropped_any > 0, "some seed drops something");
    assert!(rounds_max > 0, "some seed's loop drops something");
}

#[test]
fn a_writers_refusal_drops_an_unused_definition_and_keeps_a_used_one() {
    // a refusal no feature states (a writer's `admits` row, code and argument made up here) at
    // the resistor inside `s`: gone while nothing instances `s`, kept once something does
    let def = "  subckt s (p q)\n    element resistor r2 (p q) value=2\n";
    let refusal = ir::Code::new(b'W', 2104);
    for (used, text) in [
        (false, format!("{BASE}{def}")),
        (
            true,
            format!("{BASE}{def}  element instance x1 (a 0) model=s\n"),
        ),
    ] {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(&text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty());
        let r2 = design
            .reachable()
            .into_iter()
            .map(|p| p.0)
            .filter(|i| i.kind == ir::ItemKind::Element)
            .nth(1)
            .expect("r2");
        let n = drop_unused(&mut design, &[(r2, refusal, 5)], Roots::Entry, &mut diags);
        let mut out = String::new();
        design.files().for_each(|f| ir::dump(&design, f, &mut out));
        let notes: Vec<(ir::Code, u32)> = diags.rows().iter().map(|d| dropped_for(d.arg)).collect();
        match used {
            false => {
                assert_eq!(out, BASE);
                assert_eq!((n, notes), (1, vec![(refusal, 5)]));
            }
            true => {
                assert_eq!(out, text);
                assert_eq!((n, notes.len()), (0, 0));
            }
        }
    }
}
