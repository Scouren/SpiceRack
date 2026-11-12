//! Passes group `run` (the run): a design in, the design without one feature out, one
//! rule per file in this folder.
//!
//! Owns the passes over analyses, options, state items, loops and outputs
//! (`tran_explicit_cap`, `tf_to_dcxf`, `whole_run_state_first`, `hoist_run_items`,
//! `loop_keep_last_to_alter`, `rshunt_to_gshunt`, … in `rewrite/DIALECT-VACASK.md` §6). It runs
//! last and may read what every other group produced.
//!
//! Contract (`passes` module header, `rewrite/SPINE.md` §4):
//! - one rule per file in this folder (its rows share the file), a helper shared by this
//!   group's passes in this folder too;
//! - a pass imports `crate::ir`, `crate::passes` (for `Pass`) and this group only;
//! - `PASSES` lists the passes in the order `legalize` tries them within the group;
//! - `CODES` holds the notes the passes put at items they leave, codes `U0400..=U0499`, each
//!   named in its pass's `Pass::notes`;
//! - each pass has its fixtures in `tests/fixtures/passes/run/<pass>/<case>.ir` and
//!   `.out.ir`, and meets the laws of `tests/pass_laws.rs` (P1–P5); its meaning (the reference
//!   evaluator or a simulator on both forms) is its own test.
//!
//! Cites: `rewrite/SPINE.md` §4; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub mod ic_next_only`, `pub mod loop_end`, `pub mod names_dropped`, `pub mod run_items_together`, `pub mod tran_ceiling_given`,
//! `pub mod whole_run_state_first`,
//! `pub(super) const PASSES: &[Pass]`, `pub(super) const CODES: &[(ir::Code, ir::Info)]`

pub mod ic_next_only;
pub mod loop_end;
pub mod names_dropped;
pub mod run_items_together;
pub mod tran_ceiling_given;
pub mod whole_run_state_first;

use super::Pass;
use crate::ir;
use tran_ceiling_given as tc;
use whole_run_state_first as wr;

/// The group's passes, in the order `legalize` tries them.
pub(super) const PASSES: &[Pass] = &[
    Pass {
        name: "whole_run_state_first",
        removes: ir::Feature::Run(ir::RunExtra::WholeRunMidRun),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::RunInterleaved)),
        notes: &[wr::STATE_ELSEWHERE],
        run: wr::whole_run_state_first,
    },
    Pass {
        name: "whole_run_to_from_here",
        removes: ir::Feature::Run(ir::RunExtra::StateWholeRun),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::StateFromHere)),
        notes: &[wr::MID_RUN, wr::RESET_RESTORES, wr::FROM_HERE_SAME],
        run: wr::whole_run_to_from_here,
    },
    Pass {
        name: "whole_run_save_to_from_here",
        removes: ir::Feature::Run(ir::RunExtra::SaveWholeRun),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::SaveFromHere)),
        notes: &[wr::MID_RUN, wr::RESET_RESTORES],
        run: wr::whole_run_save_to_from_here,
    },
    Pass {
        name: "tran_cap_ngspice_to_given",
        removes: ir::Feature::Run(ir::RunExtra::TranCapNgspice),
        requires: ceiling(&[ir::BinOp::Sub, ir::BinOp::Div], true),
        notes: &[tc::NO_PRINT_STEP],
        run: tc::tran_cap_ngspice_to_given,
    },
    Pass {
        name: "tran_cap_span50_to_given",
        removes: ir::Feature::Run(ir::RunExtra::TranCapSpan50),
        requires: ceiling(&[ir::BinOp::Sub, ir::BinOp::Div], false),
        notes: &[],
        run: tc::tran_cap_span50_to_given,
    },
    Pass {
        name: "tran_cap_span10_to_given",
        removes: ir::Feature::Run(ir::RunExtra::TranCapSpan10),
        requires: ceiling(&[ir::BinOp::Sub, ir::BinOp::Div], false),
        notes: &[],
        run: tc::tran_cap_span10_to_given,
    },
    Pass {
        name: "tran_cap_none_to_given",
        removes: ir::Feature::Run(ir::RunExtra::TranCapNone),
        requires: ceiling(&[ir::BinOp::Sub], false),
        notes: &[],
        run: tc::tran_cap_none_to_given,
    },
    Pass {
        name: "ic_beyond_next_to_next_only",
        removes: ir::Feature::Run(ir::RunExtra::IcBeyondNext),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Run(ir::RunExtra::IcNextOnly))
            .with(ir::Feature::Run(ir::RunExtra::StateFromHere))
            .with(ir::Feature::Run(ir::RunExtra::NodeSetMidRun)),
        notes: &[ic_next_only::IC_KEPT],
        run: ic_next_only::ic_beyond_next_to_next_only,
    },
    Pass {
        name: "ic_next_only_to_beyond",
        removes: ir::Feature::Run(ir::RunExtra::IcNextOnly),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::IcBeyondNext)),
        notes: &[ic_next_only::IC_KEPT],
        run: ic_next_only::ic_next_only_to_beyond,
    },
    Pass {
        name: "analysis_name_dropped",
        removes: ir::Feature::Run(ir::RunExtra::AnalysisNamed),
        requires: ir::FeatureSet::EMPTY,
        notes: &[],
        run: names_dropped::analysis_name_dropped,
    },
    Pass {
        name: "dc_axis_name_dropped",
        removes: ir::Feature::Run(ir::RunExtra::DcAxisNamed),
        requires: ir::FeatureSet::EMPTY,
        notes: &[],
        run: names_dropped::dc_axis_name_dropped,
    },
    Pass {
        name: "loop_name_dropped",
        removes: ir::Feature::Run(ir::RunExtra::LoopNamed),
        requires: ir::FeatureSet::EMPTY,
        notes: &[names_dropped::LOOP_NAME_READ],
        run: names_dropped::loop_name_dropped,
    },
    Pass {
        name: "loop_restore_to_keep_last",
        removes: ir::Feature::Run(ir::RunExtra::LoopRestore),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Run(ir::RunExtra::LoopKeepLast))
            .with(ir::Feature::Item(ir::ItemKind::Alter))
            .with(ir::Feature::Run(ir::RunExtra::AlterParam)),
        notes: &[loop_end::LOOP_END_KEPT],
        run: loop_end::loop_restore_to_keep_last,
    },
    Pass {
        name: "loop_keep_last_to_restore",
        removes: ir::Feature::Run(ir::RunExtra::LoopKeepLast),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Run(ir::RunExtra::LoopRestore))
            .with(ir::Feature::Item(ir::ItemKind::Alter))
            .with(ir::Feature::Run(ir::RunExtra::AlterParam)),
        notes: &[loop_end::LOOP_END_KEPT],
        run: loop_end::loop_keep_last_to_restore,
    },
    // after `whole_run_state_first`: a move may interleave
    Pass {
        name: "run_items_together",
        removes: ir::Feature::Run(ir::RunExtra::RunInterleaved),
        requires: ir::FeatureSet::EMPTY,
        notes: &[run_items_together::HOLDS_RUN_ITEMS],
        run: run_items_together::run_items_together,
    },
];

/// The group's catalogue rows (`U0400..=U0499`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        wr::STATE_ELSEWHERE,
        ir::Info {
            title: "a whole-run state item after a run item that cannot move before it",
            cite: "rewrite/AMEND-A12.md A14; R24",
            explain: "The target's state is positional, so a state item that holds for the whole run must stand before the first analysis. This one, or that analysis, is listed in an included file or a body, and moving it there would change what the include or the body holds, so it is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        wr::MID_RUN,
        ir::Info {
            title: "a whole-run state item after a run item",
            cite: "rewrite/AMEND-A12.md A14; R24",
            explain: "The target holds state from where it stands. This whole-run item stands after an analysis, loop or alter, where holding from here would miss the run items before it, so it is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        wr::RESET_RESTORES,
        ir::Info {
            title: "a whole-run state item whose class a reset restores",
            cite: "rewrite/AMEND-A12.md A27, A28",
            explain: "The target holds state from where it stands, and a reset in the deck restores this class: it would drop the item once positional, where the whole-run item outlives it, so the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        wr::FROM_HERE_SAME,
        ir::Info {
            title: "a whole-run state item a positional one sets the same thing as",
            cite: "rewrite/AMEND-A12.md A14",
            explain: "The deck sets this quantity both for the whole run and from a point on; which of the two holds after that point is not stated, so the item keeps its mode and is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        tc::NO_PRINT_STEP,
        ir::Info {
            title: "a transient under ngspice's step ceiling with no print step",
            cite: "rewrite/AMEND-A12.md A24; R47",
            explain: "ngspice's default ceiling is the smaller of the print step and a fiftieth of the span; this transient leaves the print step to the simulator, so the ceiling has no value to state and the analysis is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        ic_next_only::IC_KEPT,
        ir::Info {
            title: "an initial condition or node set whose reach the target cannot state",
            cite: "rewrite/AMEND-A12.md A22",
            explain: "The target states a condition either for the next analysis only or for every analysis after it, not this one's way. The rule copies or re-labels a deck's conditions together, and here it cannot: a condition or an analysis stands in a body or loop, a value holds a random draw, one node is set both for the whole run and from a point on, or a next-only condition precedes an analysis that is not the last. The condition is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        names_dropped::NAME_DROPPED,
        ir::Info {
            title: "a statement name the target cannot spell, dropped",
            cite: "rewrite/AMEND-A12.md A16, A22, A32",
            explain: "The target has no name on this analysis, DC axis or loop, so the name was dropped; it labels results only. The note's argument is the old name: results are given back under it.",
            kind: ir::Kind::Note,
        },
    ),
    (
        names_dropped::LOOP_NAME_READ,
        ir::Info {
            title: "a named loop whose body may read its name",
            cite: "rewrite/AMEND-A12.md A27",
            explain: "The target has no loop name. This loop's body holds a statement kept as its dialect's text, which may read the name as the loop variable, so the name cannot be dropped and the loop is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        loop_end::LOOP_END_KEPT,
        ir::Info {
            title: "a loop whose targets' value after it the target cannot state",
            cite: "rewrite/AMEND-A12.md A22, A27",
            explain: "The target ends this loop the other way (restoring its targets, or keeping the last value), so an alter after the loop must set what the source holds there. It cannot here: a target is not a parameter, the value is not a stated expression (a range sweep, a Monte Carlo loop) or holds a random draw, the loop stands in a body, or something before it already changed the target. The loop is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        run_items_together::HOLDS_RUN_ITEMS,
        ir::Info {
            title: "a circuit item between run items that holds run items",
            cite: "rewrite/AMEND-A12.md A22",
            explain: "The target lists a file's run items together. A circuit item between them would move before them, but it holds run items in its body or includes a file that does, and the move would reorder those, so it is refused.",
            kind: ir::Kind::Note,
        },
    ),
];

// ---- private ----

/// What a ceiling row's expression may add: the given ceiling, its operators and their tags.
const fn ceiling(ops: &[ir::BinOp], min: bool) -> ir::FeatureSet {
    let mut set = ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranMaxStep));
    if !ops.is_empty() {
        set = set.with(ir::Feature::Expr(ir::ExprTag::Binary));
    }
    let mut k = 0;
    while k < ops.len() {
        set = set.with(ir::Feature::Binary(ops[k]));
        k += 1;
    }
    if min {
        set = set
            .with(ir::Feature::Func(ir::Func::Min))
            .with(ir::Feature::Expr(ir::ExprTag::Call));
    }
    set
}
