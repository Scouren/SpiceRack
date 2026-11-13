//! `dialect::legalize_with`'s outer loop (R80, R80a–R80c): a deck's conversion (`Roots::Entry`)
//! drops an unused definition holding a refusal, with its `U0314` note, while a kit's
//! (`Roots::Every`) refuses it; and its laws over generated designs on every built-in writer:
//! every refusal it emits, by item, code and argument, is one its first round had (the loop only
//! shortens a refusal list); a second `legalize_with` of the result drops nothing and emits the
//! same refusals; the notes come before the refusals; one thread and eight give the same rows.
//!
//! Oracles: a hand-written deck with the refusal and the drop stated by hand; the first round
//! itself, as `Roots::Every` runs it (one round: it drops nothing); the second run; threads.
//!
//! Contents: tests only.

use spicerack::dialect::{self, BUILTIN, Roots, Sources};
use spicerack::ir;
use spicerack::ir::r#gen::generate;
use spicerack::passes::structure::unused::{UNUSED_DROPPED, dropped_for};

use super::NO_SUBCKT;

/// The seeds the generated laws run.
const SEEDS: std::ops::Range<u64> = 0..64;

/// A moderate generated design (the dialect laws' size).
const SIZE: ir::r#gen::Size = ir::r#gen::Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

/// A deck with one subcircuit nothing instances.
const DECK: &str = "file \"a.sp\" deck\n  title \"t\"\n  subckt s (p q)\n    element resistor r2 (p q) value=2\n  element resistor r1 (a 0) value=1\n";

// ---- private ----

#[test]
fn a_deck_drops_an_unused_subckt_its_target_lacks_and_a_kit_refuses_it() {
    let subckt = u32::from(ir::Feature::Item(ir::ItemKind::Subckt).index());
    let src = Sources::one("a.ir", DECK);
    let ir_row = dialect::by_name("ir").expect("the ir row");
    let written = dialect::transpile(&src, ir_row, &NO_SUBCKT, dialect::Policy::Lenient)
        .expect("the unused subckt is dropped, not refused");
    let notes: Vec<(ir::Code, u32)> = written
        .notes
        .rows()
        .iter()
        .map(|d| (d.code, d.arg))
        .collect();
    let packed = notes
        .iter()
        .map(|n| (n.0, dropped_for(n.1)))
        .collect::<Vec<_>>();
    assert_eq!(packed, [(UNUSED_DROPPED, (ir::diag::UNSUPPORTED, subckt))]);
    assert_eq!(
        written.text,
        "file \"a.sp\" deck\n  title \"t\"\n  element resistor r1 (a 0) value=1\n"
    );
    // a kit: every definition is a root
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &src,
        ir_row,
        dialect::Policy::Lenient,
        &mut design,
        &mut diags,
    );
    dialect::legalize(&mut design, &NO_SUBCKT, &mut diags);
    let rows: Vec<(ir::Code, u32)> = diags.rows().iter().map(|d| (d.code, d.arg)).collect();
    assert_eq!(rows, [(ir::diag::UNSUPPORTED, subckt)]);
    // used, it stays refused in a deck too
    let used = DECK.replace(
        "  element resistor r1",
        "  element instance x1 (a 0) model=s\n  element resistor r1",
    );
    let src = Sources::one("a.ir", &used);
    let refused = dialect::transpile(&src, ir_row, &NO_SUBCKT, dialect::Policy::Lenient)
        .expect_err("a used subckt is refused");
    let rows: Vec<(ir::Code, u32)> = refused.rows().iter().map(|d| (d.code, d.arg)).collect();
    assert_eq!(rows, [(ir::diag::UNSUPPORTED, subckt)]);
}

#[test]
fn the_loop_only_removes_refusals_and_a_second_run_changes_nothing() {
    let mut dropped_on = Vec::new();
    for row in BUILTIN.iter().filter(|r| r.write.is_some()) {
        let mut drops = 0;
        for seed in SEEDS {
            let design = generate(seed, &ir::FeatureSet::ALL, SIZE);
            let (round1, first) = legalized(&design, row, Roots::Every);
            let (once, rows) = legalized(&design, row, Roots::Entry);
            drops += usize::from(rows.iter().any(|d| d.code == UNUSED_DROPPED));
            let emitted = refusals(&rows);
            let new = not_had(&emitted, &refusals(&first), &round1);
            assert!(
                new.is_empty(),
                "{} seed {seed}: refusals the first round did not have: {new:?}",
                row.name
            );
            let last_note = rows.iter().rposition(|d| !dialect::refuses(d.code));
            let first_refusal = rows.iter().position(|d| dialect::refuses(d.code));
            if let (Some(n), Some(r)) = (last_note, first_refusal) {
                assert!(n < r, "{} seed {seed}: the notes come first", row.name);
            }
            let (_, again) = legalized(&once, row, Roots::Entry);
            assert!(
                !again.iter().any(|d| d.code == UNUSED_DROPPED),
                "{} seed {seed}: a second run drops nothing",
                row.name
            );
            assert_eq!(
                refusals(&again),
                emitted,
                "{} seed {seed}: a second run, the same refusals",
                row.name
            );
        }
        dropped_on.push((row.name, drops));
    }
    let named = |n: &str| dropped_on.iter().find(|d| d.0 == n).map_or(0, |d| d.1);
    assert!(
        named("ngspice") > 0 && named("vacask") > 0,
        "the loop drops on some seed of each simulator's row: {dropped_on:?}"
    );
}

/// The law's own oracle: a refusal at an item the first round left counts by (item, code,
/// argument); one at an item a later rule made counts against an unmatched first-round refusal
/// of its kind, code and argument, once; anything else is new.
#[test]
fn not_had_sees_a_new_refusal_and_a_rewritten_items_old_one() {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(DECK, &mut design.builder(), &mut diags);
    let el = |i| ir::ItemRef::new(ir::ItemKind::Element, i);
    let (u, w) = (ir::diag::UNSUPPORTED, ir::Code::new(b'W', 9001));
    // r2 (index 0) is listed; index 7 is a row a later round's rule made
    assert!(not_had(&[(el(0), u, 3)], &[(el(0), u, 3)], &design).is_empty());
    assert_eq!(
        not_had(&[(el(0), w, 3)], &[(el(0), u, 3)], &design),
        [(el(0), w, 3)]
    );
    assert!(not_had(&[(el(7), u, 3)], &[(el(0), u, 3)], &design).is_empty());
    let twice = [(el(7), u, 3), (el(8), u, 3)];
    assert_eq!(not_had(&twice, &[(el(0), u, 3)], &design), [(el(8), u, 3)]);
}

#[test]
fn one_thread_and_eight_emit_the_same_rows() {
    for row in BUILTIN.iter().filter(|r| r.write.is_some()) {
        for seed in SEEDS.take(8) {
            let design = generate(seed, &ir::FeatureSet::ALL, SIZE);
            let (_, once) = legalized(&design, row, Roots::Entry);
            let threads: Vec<Vec<ir::Diag>> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..8)
                    .map(|_| s.spawn(|| legalized(&design, row, Roots::Entry).1))
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("a thread"))
                    .collect()
            });
            assert!(
                threads.iter().all(|t| *t == once),
                "{} seed {seed}: the threads differ",
                row.name
            );
        }
    }
}

/// `design` legalized for `row` under `roots` (a clone), with the rows it emitted.
fn legalized(
    design: &ir::Design,
    row: &dialect::DialectRow,
    roots: Roots,
) -> (ir::Design, Vec<ir::Diag>) {
    let mut out = design.clone();
    let mut diags = ir::Diags::new();
    dialect::legalize_with(&mut out, row, roots, &mut diags);
    (out, diags.rows().to_vec())
}

/// The refusals of `emitted` the first round (`first`, on the design it left, `round1`) did not
/// have: one at an item that round left, by (item, code, argument); one at an item a later
/// round's rule made (a row the first round never listed), by (kind, code, argument), each of
/// the first round's at most once.
fn not_had(
    emitted: &[(ir::ItemRef, ir::Code, u32)],
    first: &[(ir::ItemRef, ir::Code, u32)],
    round1: &ir::Design,
) -> Vec<(ir::ItemRef, ir::Code, u32)> {
    let old: Vec<ir::ItemRef> = round1.reachable().into_iter().map(|p| p.0).collect();
    let mut spare: Vec<(ir::ItemKind, ir::Code, u32)> = first
        .iter()
        .filter(|r| !emitted.contains(r))
        .map(|r| (r.0.kind, r.1, r.2))
        .collect();
    let mut out = Vec::new();
    for r in emitted {
        let had = match old.contains(&r.0) {
            true => first.contains(r),
            false => match spare.iter().position(|s| *s == (r.0.kind, r.1, r.2)) {
                Some(at) => {
                    spare.swap_remove(at);
                    true
                }
                None => false,
            },
        };
        if !had {
            out.push(*r);
        }
    }
    out
}

/// The refusing rows of `rows`, by (item, code, argument).
fn refusals(rows: &[ir::Diag]) -> Vec<(ir::ItemRef, ir::Code, u32)> {
    rows.iter()
        .filter(|d| dialect::refuses(d.code))
        .map(|d| (d.item, d.code, d.arg))
        .collect()
}

/// R90a: a design whose law carriers all converted (here: none carries) ends stating the
/// target's law; one with an unconverted carrier is refused and keeps its own.
#[test]
fn a_legal_design_ends_stating_the_targets_laws_and_a_refused_one_keeps_its_own() {
    let vacask = dialect::by_name("vacask").expect("the vacask row");
    let deck = |body: &str| {
        let text = format!("laws log_zero_minus_1e99\nfile \"a.sp\" deck\n  title \"t\"\n{body}");
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(&text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        design
    };
    let mut plain = deck("  element resistor r1 (a 0) value=1\n");
    let mut diags = ir::Diags::new();
    dialect::legalize_with(&mut plain, vacask, Roots::Entry, &mut diags);
    assert!(refusals(diags.rows()).is_empty(), "{:?}", diags.rows());
    assert!(
        plain.laws().contains(ir::Law::LogZeroError),
        "{:?}",
        plain.laws()
    );
    assert!(!plain.laws().contains(ir::Law::LogZeroMinus1e99));
    let mut carried = deck("  element behav b1 (n 0) expr=(ln(v(n)))\n");
    let mut diags = ir::Diags::new();
    dialect::legalize_with(&mut carried, vacask, Roots::Entry, &mut diags);
    let member = ir::Feature::Circuit(ir::Law::LogZeroMinus1e99.member());
    let at_member = diags
        .rows()
        .iter()
        .any(|d| d.code == ir::diag::UNSUPPORTED && d.arg == u32::from(member.index()));
    assert!(at_member, "the carrier is refused: {:?}", diags.rows());
    assert!(carried.laws().contains(ir::Law::LogZeroMinus1e99));
}

/// The pairing `restate_laws` relies on: `Law::ALL` lists each point's two variants adjacent,
/// so a design stating an adjacent pair is a `LawConflict` for `ir::check`.
#[test]
fn law_all_lists_each_points_two_variants_adjacent() {
    for pair in ir::Law::ALL.chunks(2) {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse("file \"a.sp\" deck\n", &mut design.builder(), &mut diags);
        design.builder().set_laws(ir::Laws::of(pair));
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(!found.is_empty(), "{pair:?}: one point, a conflict");
    }
    for k in (1..ir::Law::ALL.len() - 1).step_by(2) {
        let across = [ir::Law::ALL[k], ir::Law::ALL[k + 1]];
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse("file \"a.sp\" deck\n", &mut design.builder(), &mut diags);
        design.builder().set_laws(ir::Laws::of(&across));
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "{across:?}: two points, no conflict");
    }
}
