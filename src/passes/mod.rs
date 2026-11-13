//! Legalization: a design and a target's feature set in, the same design rewritten to use only
//! what the target has out, or `U0001` refusals (`ir::check_legal`'s, one at each item) for
//! what no pass removes. Passes know features, never dialects.
//!
//! The passes live in four groups, each one owner's: `structure` (subcircuits, models, names,
//! libraries, decks), `expr` (expressions), `waves` (sources and elements), `run` (analyses,
//! options, loops, outputs). A group is `src/passes/<group>.rs` while it holds no pass, then
//! `src/passes/<group>/mod.rs` with one file per pass beside it; it exports
//! `pub(super) const PASSES: &[Pass]` (its own order) and `pub(super) const CODES` (its rows,
//! in its range of [`GROUPS`]). A pass imports `crate::ir`, `crate::passes` (for [`Pass`]) and
//! its own group's folder only; a helper shared by passes of one group lives in that group's
//! folder (`tests/layering.rs` checks it).
//!
//! Order is meaning: [`legalize`] tries the groups in [`GROUPS`] order and each group's passes
//! in its own order. A pass may produce input for the passes after it, never rely on one after
//! it: `structure` first (every other pass walks the scopes, names and definitions it settles),
//! then `expr` (wave and run passes read fields expression passes have substituted, such as the
//! transient leaves), then `waves`, then `run` (it may read anything the others produced).
//!
//! A pass that leaves an item carrying its feature (it cannot convert it) puts one coded note of
//! its own at that item (`Pass::notes`), saying why; `ir::check_legal` then refuses the item.
//!
//! Cites: `REWRITE-PLAN.md` §4.4; `rewrite/SPINE.md` §4; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents:
//! - `pub mod expr`, `pub mod run`, `pub mod structure`, `pub mod waves`
//! - `pub struct Pass`, `pub struct Group`, `pub static GROUPS: &[Group]`
//! - `pub fn legalize(design: &mut ir::Design, target: &ir::FeatureSet, diags: &mut ir::Diags)`,
//!   `pub fn legal(passes: &[&Pass], target: &ir::FeatureSet) -> ir::FeatureSet`
//! - `pub fn all() -> impl Iterator<Item = &'static Pass>`, `pub fn info(code: ir::Code) -> Option<ir::Info>`

pub mod expr;
pub mod run;
pub mod structure;
pub mod waves;

use crate::ir;

/// One IR → IR transform that removes a feature.
#[derive(Clone, Copy, Debug)]
pub struct Pass {
    /// Its name: its file's name, its fixtures' folder (`tests/fixtures/passes/<group>/<name>/`).
    pub name: &'static str,
    /// The feature it removes.
    pub removes: ir::Feature,
    /// What the target must have for the pass's output to be legal: every feature its output
    /// may use that its input did not (law P3).
    pub requires: ir::FeatureSet,
    /// The codes of the notes it puts at an item it leaves carrying `removes` (law P2); each a
    /// row of its group's codes.
    pub notes: &'static [ir::Code],
    /// The transform. It leaves a design `ir::check` accepts.
    pub run: fn(&mut ir::Design, &mut ir::Diags),
}

/// One owner's passes and its diagnostic rows.
#[derive(Clone, Copy, Debug)]
pub struct Group {
    /// The group's name: its file or folder under `src/passes/`.
    pub name: &'static str,
    /// Its passes, in the order `legalize` tries them.
    pub passes: &'static [Pass],
    /// Its catalogue rows, each code in `range`.
    pub codes: &'static [(ir::Code, ir::Info)],
    /// Its code range: `U` + these numbers, inclusive.
    pub range: (u16, u16),
}

/// The groups, in the order `legalize` tries them (the order is meaning: module header).
pub static GROUPS: &[Group] = &[
    Group {
        name: "structure",
        passes: structure::PASSES,
        codes: structure::CODES,
        range: (300, 399),
    },
    Group {
        name: "expr",
        passes: expr::PASSES,
        codes: expr::CODES,
        range: (100, 199),
    },
    Group {
        name: "waves",
        passes: waves::PASSES,
        codes: waves::CODES,
        range: (200, 299),
    },
    Group {
        name: "run",
        passes: run::PASSES,
        codes: run::CODES,
        range: (400, 499),
    },
];

/// Rounds of: the features `design` uses and `target` lacks; every pass, in [`GROUPS`] order,
/// that removes one of them and whose `requires` are legal for the target (A32: [`legal`]), run
/// (the first such pass per feature per round) and then `ir::check`ed, until nothing is
/// missing or a round changes nothing. Then `ir::check_legal` reports what is left: one
/// `U0001` row per (item, feature) at the item. A check violation after a pass is that pass's
/// bug (a debug assertion). A pass's note is reported once per (code, item, argument, span)
/// however many rounds ran it (R69 d): a rule that leaves its item runs again each round while
/// the feature is there.
///
/// The bound: a round that changes something runs at least one pass, and a pass runs only on a
/// feature legal for the target, whose chain of passes ends in the target's own features after
/// at most one link per pass (the least fixed point admits no cycle: a pass is legal only
/// through passes already legal); so the missing set settles within one round per link, at
/// most one more than the number of passes.
///
/// # Panics
/// When the rounds do not settle within that bound (passes that undo each other).
pub fn legalize(design: &mut ir::Design, target: &ir::FeatureSet, diags: &mut ir::Diags) {
    let passes: Vec<&Pass> = all().collect();
    legalize_with(&passes, design, target, diags);
}

/// The features legal for `target` under `passes` (A32): the least fixed point of "the target
/// has it, or a pass removes it whose `requires` are all legal". Two passes that remove each
/// other's output (`pow_to_powabs`, and `powabs_to_powre` then `powre_to_pow`) make neither legal for a target with
/// neither feature. Bounded: each round adds a feature or stops, at most one round per pass.
pub fn legal(passes: &[&Pass], target: &ir::FeatureSet) -> ir::FeatureSet {
    let mut legal = *target;
    for _ in 0..=passes.len() {
        let before = legal;
        for p in passes {
            if !legal.contains(p.removes) && p.requires.minus(legal).is_empty() {
                legal = legal.with(p.removes);
            }
        }
        if legal == before {
            return legal;
        }
    }
    panic!("legal: no fixed point within one round per pass");
}

/// Every registered pass, in the order [`legalize`] tries them.
pub fn all() -> impl Iterator<Item = &'static Pass> {
    GROUPS.iter().flat_map(|g| g.passes.iter())
}

/// The catalogue row of a pass code (`U0100..=U0499`), or `None` for any other code.
pub fn info(code: ir::Code) -> Option<ir::Info> {
    let rows = GROUPS.iter().flat_map(|g| g.codes.iter());
    rows.copied().find(|(c, _)| *c == code).map(|(_, i)| i)
}

// ---- private ----

/// [`legalize`] over `passes` (the registry, or a test's stubs).
fn legalize_with(
    passes: &[&Pass],
    design: &mut ir::Design,
    target: &ir::FeatureSet,
    diags: &mut ir::Diags,
) {
    let legal = legal(passes, target);
    let bound = passes.len() + 1;
    let mut missing = ir::features_used(design).minus(*target);
    let mut rounds = 0;
    // the notes already reported, by (code, item, argument, span); dropped as they are pushed,
    // so the caller's earlier rows are never touched and each row is looked up once (O(1))
    let mut noted = std::collections::HashSet::new();
    while !missing.is_empty() {
        assert!(rounds < bound, "legalize: the passes do not settle");
        rounds += 1;
        let mut open = missing;
        for pass in passes {
            if !open.contains(pass.removes) || !pass.requires.minus(legal).is_empty() {
                continue;
            }
            open = open.minus(ir::FeatureSet::EMPTY.with(pass.removes));
            let mut fresh = ir::Diags::new();
            (pass.run)(design, &mut fresh);
            for d in fresh.rows() {
                let key = (d.code, d.item, d.arg, d.span.file, d.span.start);
                match (noted.insert(key), d.item == ir::ItemRef::NONE) {
                    (false, _) => {}
                    (true, true) => diags.push(d.code, d.span, d.arg),
                    (true, false) => diags.push_at(d.code, design, d.item, d.arg),
                }
            }
            let checked = diags.len();
            ir::check(design, diags);
            debug_assert_eq!(
                diags.len(),
                checked,
                "pass {} made IR that fails ir::check",
                pass.name
            );
        }
        let left = ir::features_used(design).minus(*target);
        if left == missing {
            break;
        }
        missing = left;
    }
    ir::check_legal(design, target, diags);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pass_code_is_in_its_groups_range_once_and_every_note_has_a_row() {
        let mut seen = Vec::new();
        for g in GROUPS {
            assert!(g.range.0 <= g.range.1, "{}: a range", g.name);
            for (code, row) in g.codes {
                let inside = (g.range.0..=g.range.1).contains(&code.number);
                assert!(
                    code.letter == b'U' && inside,
                    "{}: {code:?} outside its range",
                    g.name
                );
                assert!(!seen.contains(code), "{code:?} twice");
                assert!(
                    !row.title.is_empty() && !row.explain.is_empty(),
                    "{code:?} text"
                );
                assert!(
                    ir::diag::info(*code).is_none(),
                    "{code:?} is the IR crate's"
                );
                seen.push(*code);
            }
            for p in g.passes {
                for n in p.notes {
                    assert!(
                        g.codes.iter().any(|(c, _)| c == n),
                        "{}: {n:?} has no row",
                        p.name
                    );
                }
            }
        }
        let ranges: Vec<(u16, u16)> = GROUPS.iter().map(|g| g.range).collect();
        for (i, a) in ranges.iter().enumerate() {
            for b in &ranges[i + 1..] {
                assert!(a.1 < b.0 || b.1 < a.0, "ranges {a:?} and {b:?} overlap");
            }
        }
    }

    // A32's oracle: stub passes over comments, globals and params, on hand-written IR text.
    fn comment_to_global(d: &mut ir::Design, _: &mut ir::Diags) {
        replace(d, ir::ItemKind::Comment, |b, scope, k| {
            b.global(scope, &[&format!("g{k}")], ir::Span::NONE);
        });
    }

    fn global_to_param(d: &mut ir::Design, _: &mut ir::Diags) {
        replace(d, ir::ItemKind::Global, |b, scope, k| {
            let one = b.num(1.0);
            b.param(scope, &format!("p{k}"), one, ir::Span::NONE);
        });
    }

    fn global_to_comment(d: &mut ir::Design, _: &mut ir::Diags) {
        replace(d, ir::ItemKind::Global, |b, scope, _| {
            b.comment(scope, "back", ir::Span::NONE);
        });
    }

    /// Drops every item of `kind` and appends `make`'s item to its scope instead.
    fn replace(
        d: &mut ir::Design,
        kind: ir::ItemKind,
        make: impl Fn(&mut ir::Builder<'_>, ir::ScopeId, usize),
    ) {
        let found: Vec<(ir::ItemRef, ir::ScopeId)> = d
            .reachable()
            .into_iter()
            .filter(|(i, _)| i.kind == kind)
            .collect();
        let mut editor = d.editor();
        for (k, (item, scope)) in found.iter().enumerate() {
            editor.remove_item(*scope, *item);
            make(&mut editor.builder(), *scope, k);
        }
    }

    fn stub(
        name: &'static str,
        removes: ir::ItemKind,
        requires: ir::ItemKind,
        run: fn(&mut ir::Design, &mut ir::Diags),
    ) -> Pass {
        Pass {
            name,
            removes: ir::Feature::Item(removes),
            requires: ir::FeatureSet::EMPTY.with(ir::Feature::Item(requires)),
            notes: &[],
            run,
        }
    }

    fn without(kinds: &[ir::ItemKind]) -> ir::FeatureSet {
        kinds.iter().fold(ir::FeatureSet::ALL, |s, &k| {
            s.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Item(k)))
        })
    }

    fn run_on(passes: &[&Pass], target: &ir::FeatureSet) -> (String, Vec<ir::Code>) {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(
            "file \"a.sp\" deck\n  title \"t\"\n  comment \"c\"\n",
            &mut design.builder(),
            &mut diags,
        );
        assert!(diags.is_empty(), "{:?}", diags.rows());
        legalize_with(passes, &mut design, target, &mut diags);
        let mut out = String::new();
        ir::dump(&design, design.entry(), &mut out);
        (out, diags.rows().iter().map(|d| d.code).collect())
    }

    #[test]
    fn a_chain_of_two_passes_runs_both_and_ends_legal() {
        let a = stub(
            "a",
            ir::ItemKind::Comment,
            ir::ItemKind::Global,
            comment_to_global,
        );
        let b = stub(
            "b",
            ir::ItemKind::Global,
            ir::ItemKind::Param,
            global_to_param,
        );
        let target = without(&[ir::ItemKind::Comment, ir::ItemKind::Global]);
        let (out, codes) = run_on(&[&a, &b], &target);
        assert_eq!(out, "file \"a.sp\" deck\n  title \"t\"\n  param p0 = 1\n");
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn an_inverse_pair_with_neither_feature_runs_nothing_and_refuses() {
        let a = stub(
            "a",
            ir::ItemKind::Comment,
            ir::ItemKind::Global,
            comment_to_global,
        );
        let back = stub(
            "back",
            ir::ItemKind::Global,
            ir::ItemKind::Comment,
            global_to_comment,
        );
        let target = without(&[ir::ItemKind::Comment, ir::ItemKind::Global]);
        let legal = legal(&[&a, &back], &target);
        assert!(!legal.contains(ir::Feature::Item(ir::ItemKind::Comment)));
        assert!(!legal.contains(ir::Feature::Item(ir::ItemKind::Global)));
        let (out, codes) = run_on(&[&a, &back], &target);
        assert_eq!(
            out, "file \"a.sp\" deck\n  title \"t\"\n  comment \"c\"\n",
            "nothing ran"
        );
        assert_eq!(codes, [ir::diag::UNSUPPORTED]);
    }

    #[test]
    fn a_chain_whose_last_link_the_target_cannot_take_runs_nothing() {
        let a = stub(
            "a",
            ir::ItemKind::Comment,
            ir::ItemKind::Global,
            comment_to_global,
        );
        let b = stub(
            "b",
            ir::ItemKind::Global,
            ir::ItemKind::Param,
            global_to_param,
        );
        let target = without(&[
            ir::ItemKind::Comment,
            ir::ItemKind::Global,
            ir::ItemKind::Param,
        ]);
        let (out, codes) = run_on(&[&a, &b], &target);
        assert_eq!(
            out, "file \"a.sp\" deck\n  title \"t\"\n  comment \"c\"\n",
            "no half-converted design"
        );
        assert_eq!(codes, [ir::diag::UNSUPPORTED]);
    }

    /// R69 d: a rule that leaves its item with its note runs again on each round while the
    /// feature is there; `legalize` reports its note once. Two rounds are forced by a second
    /// rule that removes the globals in the first.
    #[test]
    fn a_rule_run_on_two_rounds_notes_its_item_once() {
        const KEPT: ir::Code = ir::Code::new(b'U', 999);
        fn keep(d: &mut ir::Design, diags: &mut ir::Diags) {
            let found = d
                .reachable()
                .into_iter()
                .find(|(i, _)| i.kind == ir::ItemKind::Comment);
            if let Some((item, _)) = found {
                diags.push_at(KEPT, d, item, 7);
            }
        }
        let keeper = Pass {
            name: "keeper",
            removes: ir::Feature::Item(ir::ItemKind::Comment),
            requires: ir::FeatureSet::EMPTY,
            notes: &[KEPT],
            run: keep,
        };
        let churn = stub(
            "churn",
            ir::ItemKind::Global,
            ir::ItemKind::Param,
            global_to_param,
        );
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        let text = "file \"a.sp\" deck\n  title \"t\"\n  comment \"c\"\n  global g\n";
        ir::parse(text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        let target = without(&[ir::ItemKind::Comment, ir::ItemKind::Global]);
        legalize_with(&[&keeper, &churn], &mut design, &target, &mut diags);
        let codes: Vec<ir::Code> = diags.rows().iter().map(|d| d.code).collect();
        assert_eq!(
            codes,
            [KEPT, ir::diag::UNSUPPORTED],
            "one note, one refusal"
        );
    }

    #[test]
    fn pass_names_are_unique() {
        let names: Vec<&str> = all().map(|p| p.name).collect();
        for (i, n) in names.iter().enumerate() {
            assert!(!names[i + 1..].contains(n), "{n} registered twice");
        }
    }
}
