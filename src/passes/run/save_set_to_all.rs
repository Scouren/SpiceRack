//! Pass `save_set_to_all`: a design in, the same design out with every save scoped to a set of
//! analysis kinds (`SaveAnalysisSet`) widened to every kind, when the design holds no analysis
//! of a kind outside that set. It removes `Run(SaveAnalysisSet)` and requires nothing (a save
//! for every kind carries no scope feature).
//!
//! Exact: a save's scope decides only which vectors the analyses it governs write; an analysis
//! of a kind in the set is governed before and after, and no other analysis exists. Settled
//! by runs (same deck, `save v(out)` then op, ac, tran): VACASK's `save v(out)` writes `out`
//! alone in op1.raw and tr1.raw and every unknown in ac1.raw (`in`, `mid`, `out`,
//! `v1:flow(br)`); ngspice-45's `.save v(out)` writes `v(out)` alone in all three plots. So
//! with an analysis outside the set the widened save would drop vectors that analysis writes in
//! the source: the save keeps its scope with a `SAVE_SET_KEPT` note.
//!
//! No follow-up rule scopes the save instead (a from-here save before each analysis of the set,
//! the default restored before each other), for two facts settled by runs
//! (`tests/passes_run/decks/save_default.{sim,sp}`, `save_script.sp`; one deck of v1, r1, l1 and a diode with
//! `rs`): VACASK's default ac set holds each OSDI device's internal unknowns (`d1:a_int`,
//! `d1:implicit_equation_0`; `sp_mos1` adds `implicit_equation_0..4`), which ngspice's default
//! never writes and no IR field predicts; and an ngspice script `save all` before the ac shows
//! up in the op that ran before it (a `v(all)` vector), and a later `delete all` with
//! `save v(out)` does not narrow the next op back.
//!
//! Cites: `rewrite/AMEND-A12.md` A22, A27; R29, R43; `crates/ir/src/output.rs` (`Save`).
//!
//! Contents: `pub fn save_set_to_all(&mut ir::Design, &mut ir::Diags)`,
//! `pub const SAVE_SET_KEPT: ir::Code`

use super::names_dropped::of_kind;
use crate::ir;

/// The note at a save whose set leaves out the kind of an analysis in the design.
pub const SAVE_SET_KEPT: ir::Code = ir::Code::new(b'U', 412);

/// Widens every set-scoped save to every analysis kind, or notes it.
pub fn save_set_to_all(design: &mut ir::Design, diags: &mut ir::Diags) {
    let ran: Vec<ir::AnalysisKind> = of_kind(design, ir::ItemKind::Analysis)
        .into_iter()
        .map(|a| design.row::<ir::Analysis>(a.index).kind)
        .collect();
    for item in of_kind(design, ir::ItemKind::Save) {
        let mut save = design.row::<ir::Save>(item.index);
        let n = save.analyses.len();
        if n < 2 || save.analyses == ir::AnalysisKinds::ALL {
            continue; // one kind or every kind: no set
        }
        if ran.iter().any(|&k| !save.analyses.contains(k)) {
            diags.push_at(SAVE_SET_KEPT, design, item, 0);
            continue;
        }
        save.analyses = ir::AnalysisKinds::ALL;
        design.editor().set(item.index, save);
        debug_assert!(design.row::<ir::Save>(item.index).analyses == ir::AnalysisKinds::ALL);
    }
}
