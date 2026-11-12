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
//! - `pub fn legalize(design: &mut ir::Design, target: &ir::FeatureSet, diags: &mut ir::Diags)`
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
/// that removes one of them and whose `requires` `target` has, run (the first such pass per
/// feature per round) and then `ir::check`ed, until nothing is missing or a round changes
/// nothing. Then `ir::check_legal` reports what is left: one `U0001` row per (item, feature) at
/// the item. A check violation after a pass is that pass's bug (a debug assertion).
///
/// # Panics
/// When the rounds do not settle within one more than the number of passes (passes that undo
/// each other).
pub fn legalize(design: &mut ir::Design, target: &ir::FeatureSet, diags: &mut ir::Diags) {
    let bound = all().count() + 1;
    let mut missing = ir::features_used(design).minus(*target);
    let mut rounds = 0;
    while !missing.is_empty() {
        assert!(rounds < bound, "legalize: the passes do not settle");
        rounds += 1;
        let mut open = missing;
        for pass in all() {
            if !open.contains(pass.removes) || !pass.requires.minus(*target).is_empty() {
                continue;
            }
            open = open.minus(ir::FeatureSet::EMPTY.with(pass.removes));
            (pass.run)(design, diags);
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

/// Every registered pass, in the order [`legalize`] tries them.
pub fn all() -> impl Iterator<Item = &'static Pass> {
    GROUPS.iter().flat_map(|g| g.passes.iter())
}

/// The catalogue row of a pass code (`U0100..=U0499`), or `None` for any other code.
pub fn info(code: ir::Code) -> Option<ir::Info> {
    let rows = GROUPS.iter().flat_map(|g| g.codes.iter());
    rows.copied().find(|(c, _)| *c == code).map(|(_, i)| i)
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

    #[test]
    fn pass_names_are_unique() {
        let names: Vec<&str> = all().map(|p| p.name).collect();
        for (i, n) in names.iter().enumerate() {
            assert!(!names[i + 1..].contains(n), "{n} registered twice");
        }
    }
}
