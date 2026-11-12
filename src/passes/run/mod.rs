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
//! Contents: `pub mod run_items_together`, `pub mod tran_ceiling_given`,
//! `pub mod whole_run_state_first`,
//! `pub(super) const PASSES: &[Pass]`, `pub(super) const CODES: &[(ir::Code, ir::Info)]`

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

/// What a ceiling row's expression may add: the given ceiling and its operators.
const fn ceiling(ops: &[ir::BinOp], min: bool) -> ir::FeatureSet {
    let mut set = ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranMaxStep));
    let mut k = 0;
    while k < ops.len() {
        set = set.with(ir::Feature::Binary(ops[k]));
        k += 1;
    }
    if min {
        set = set.with(ir::Feature::Func(ir::Func::Min));
    }
    set
}
