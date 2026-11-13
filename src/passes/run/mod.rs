//! Passes group `run` (the run): a design in, the design without one feature out, one
//! rule per file in this folder.
//!
//! Owns the passes over analyses, options, state items, loops and outputs
//! (`tran_explicit_cap`, `tf_to_dcxf`, `whole_run_state_first`'s rows, `hoist_run_items`,
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
//! Contents: `pub mod alter_at_reset`, `pub mod dc_axes_to_loops`, `pub mod device_ic`, `pub mod device_ic_in_op`, `pub mod drop_print`, `pub mod ic_next_only`, `pub mod inert_options`, `pub mod loop_end`, `pub mod names_dropped`, `pub mod names_given`, `pub mod noise_ref_unit`, `pub mod param_arguments`, `pub mod run_items_together`, `pub mod save_set_to_all`, `pub mod sweep_forms`, `pub mod tran_ceiling_given`, `pub mod tran_first_step_to_print`, `pub mod tran_print_step`,
//! `pub mod whole_run_state_first`,
//! `pub(super) const PASSES: &[Pass]`, `pub(super) const CODES: &[(ir::Code, ir::Info)]` (its rows in
//! `codes.rs`)

pub mod alter_at_reset;
mod codes;
pub mod dc_axes_to_loops;
pub mod device_ic;
pub mod device_ic_in_op;
pub mod drop_print;
pub mod ic_next_only;
pub mod inert_options;
pub mod loop_end;
pub mod names_dropped;
pub mod names_given;
pub mod noise_ref_unit;
pub mod param_arguments;
pub mod run_items_together;
pub mod save_set_to_all;
pub mod sweep_forms;
pub mod tran_ceiling_given;
pub mod tran_first_step_to_print;
pub mod tran_print_step;
pub mod whole_run_state_first;

use super::Pass;
use crate::ir;
use crate::ir::RunExtra;
use tran_ceiling_given as tc;
use tran_first_step_to_print as fs;
use whole_run_state_first as wr;

pub(super) use codes::CODES;

/// The group's passes, in the order `legalize` tries them.
pub(super) const PASSES: &[Pass] = &[
    // one row per item kind over one body each (`whole_run_state_first.rs`)
    first(
        "whole_run_option_first",
        RunExtra::OptionWholeRunMidRun,
        wr::whole_run_option_first,
    ),
    first(
        "whole_run_env_first",
        RunExtra::EnvWholeRunMidRun,
        wr::whole_run_env_first,
    ),
    first(
        "whole_run_node_set_first",
        RunExtra::NodeSetWholeRunMidRun,
        wr::whole_run_node_set_first,
    ),
    first(
        "whole_run_save_first",
        RunExtra::SaveWholeRunMidRun,
        wr::whole_run_save_first,
    ),
    to_from_here(
        "whole_run_option_to_from_here",
        (RunExtra::OptionWholeRun, RunExtra::OptionFromHere),
        &[
            wr::MID_RUN,
            wr::RESET_RESTORES,
            wr::FROM_HERE_SAME,
            wr::IN_SUBCKT,
        ],
        wr::whole_run_option_to_from_here,
    ),
    to_from_here(
        "whole_run_env_to_from_here",
        (RunExtra::EnvWholeRun, RunExtra::EnvFromHere),
        &[
            wr::MID_RUN,
            wr::RESET_RESTORES,
            wr::FROM_HERE_SAME,
            wr::IN_SUBCKT,
        ],
        wr::whole_run_env_to_from_here,
    ),
    // a node set is no class a reset restores
    to_from_here(
        "whole_run_node_set_to_from_here",
        (RunExtra::NodeSetWholeRun, RunExtra::NodeSetFromHere),
        &[wr::MID_RUN, wr::FROM_HERE_SAME, wr::IN_SUBCKT],
        wr::whole_run_node_set_to_from_here,
    ),
    // two saves never set one thing: they add up
    to_from_here(
        "whole_run_save_to_from_here",
        (RunExtra::SaveWholeRun, RunExtra::SaveFromHere),
        &[wr::MID_RUN, wr::RESET_RESTORES, wr::IN_SUBCKT],
        wr::whole_run_save_to_from_here,
    ),
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
    // after the ceiling rows: ngspice's rule reads the print step
    Pass {
        name: "tran_print_step_to_simulator",
        removes: ir::Feature::Run(ir::RunExtra::TranPrintStep),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranSimulatorStep)),
        notes: &[tran_print_step::PRINT_STEP_READ],
        run: tran_print_step::tran_print_step_to_simulator,
    },
    // after the ceiling rows: a ceiling rule reads the print step (R73)
    Pass {
        name: "tran_simulator_step_to_print",
        removes: ir::Feature::Run(ir::RunExtra::TranSimulatorStep),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranPrintStep)),
        notes: &[fs::FIRST_STEP_KEPT, fs::NO_STEP_VALUE],
        run: fs::tran_first_step_to_print,
    },
    Pass {
        name: "tran_first_step_to_print",
        removes: ir::Feature::Run(ir::RunExtra::TranInitialStep),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranPrintStep)),
        notes: &[fs::FIRST_STEP_KEPT, fs::NO_STEP_VALUE],
        run: fs::tran_first_step_to_print,
    },
    Pass {
        name: "noise_ref_source_ac_to_unit",
        removes: ir::Feature::Run(ir::RunExtra::NoiseRefSourceAc),
        requires: ir::FeatureSet::EMPTY,
        notes: &[noise_ref_unit::NOISE_REF_KEPT],
        run: noise_ref_unit::noise_ref_source_ac_to_unit,
    },
    Pass {
        name: "device_ic_to_tran_ic",
        removes: ir::Feature::Circuit(ir::CircuitExtra::IcUicOnly),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Item(ir::ItemKind::NodeSet))
            .with(ir::Feature::Run(RunExtra::IcNextOnly))
            .with(ir::Feature::Run(RunExtra::IcDifferential))
            .with(ir::Feature::Run(RunExtra::NodeSetFromHere))
            .with(ir::Feature::Run(RunExtra::NodeSetMidRun)),
        notes: &[device_ic::DEVICE_IC_KEPT],
        run: device_ic::device_ic_to_tran_ic,
    },
    Pass {
        name: "device_ic_uic_to_in_op",
        removes: ir::Feature::Circuit(ir::CircuitExtra::IcUicOnly),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Circuit(ir::CircuitExtra::IcInOp)),
        notes: &[device_ic_in_op::IC_OP_READ],
        run: device_ic_in_op::device_ic_uic_to_in_op,
    },
    Pass {
        name: "ic_beyond_next_to_next_only",
        removes: ir::Feature::Run(ir::RunExtra::IcBeyondNext),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Run(ir::RunExtra::IcNextOnly))
            .with(ir::Feature::Run(ir::RunExtra::NodeSetFromHere))
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
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::AnalysisUnnamed)),
        notes: &[],
        run: names_dropped::analysis_name_dropped,
    },
    Pass {
        name: "dc_axis_name_dropped",
        removes: ir::Feature::Run(ir::RunExtra::DcAxisNamed),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::DcAxisUnnamed)),
        notes: &[],
        run: names_dropped::dc_axis_name_dropped,
    },
    Pass {
        name: "loop_name_dropped",
        removes: ir::Feature::Run(ir::RunExtra::LoopNamed),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::LoopUnnamed)),
        notes: &[names_dropped::LOOP_NAME_READ],
        run: names_dropped::loop_name_dropped,
    },
    Pass {
        name: "analysis_name_given",
        removes: ir::Feature::Run(ir::RunExtra::AnalysisUnnamed),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::AnalysisNamed)),
        notes: &[],
        run: names_given::analysis_name_given,
    },
    Pass {
        name: "dc_axis_name_given",
        removes: ir::Feature::Run(ir::RunExtra::DcAxisUnnamed),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::DcAxisNamed)),
        notes: &[],
        run: names_given::dc_axis_name_given,
    },
    Pass {
        name: "loop_name_given",
        removes: ir::Feature::Run(ir::RunExtra::LoopUnnamed),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::LoopNamed)),
        notes: &[names_given::LOOP_NAME_UNREAD],
        run: names_given::loop_name_given,
    },
    Pass {
        name: "loop_restore_to_keep_last",
        removes: ir::Feature::Run(ir::RunExtra::LoopRestore),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Run(ir::RunExtra::LoopKeepLast))
            .with(ir::Feature::Item(ir::ItemKind::Alter))
            .with(ir::Feature::Run(ir::RunExtra::AlterParam))
            .with(ir::Feature::Run(ir::RunExtra::AlterParamAtOnce)),
        notes: &[loop_end::LOOP_END_KEPT],
        run: loop_end::loop_restore_to_keep_last,
    },
    Pass {
        name: "loop_keep_last_to_restore",
        removes: ir::Feature::Run(ir::RunExtra::LoopKeepLast),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Run(ir::RunExtra::LoopRestore))
            .with(ir::Feature::Item(ir::ItemKind::Alter))
            .with(ir::Feature::Run(ir::RunExtra::AlterParam))
            .with(ir::Feature::Run(ir::RunExtra::AlterParamAtOnce)),
        notes: &[loop_end::LOOP_END_KEPT],
        run: loop_end::loop_keep_last_to_restore,
    },
    Pass {
        name: "save_set_to_all",
        removes: ir::Feature::Run(ir::RunExtra::SaveAnalysisSet),
        requires: ir::FeatureSet::EMPTY,
        notes: &[save_set_to_all::SAVE_SET_KEPT],
        run: save_set_to_all::save_set_to_all,
    },
    sweep(
        "sweep_dc_lin_points_to_step",
        (RunExtra::SweepDcLinPoints, RunExtra::SweepDcLinStep),
        &[sweep_forms::SWEEP_KEPT],
        sweep_forms::sweep_dc_lin_points_to_step,
    ),
    sweep(
        "sweep_dc_list_to_step",
        (RunExtra::SweepDcList, RunExtra::SweepDcLinStep),
        &[sweep_forms::SWEEP_KEPT],
        sweep_forms::sweep_dc_list_to_step,
    ),
    sweep(
        "sweep_freq_list_to_lin_points",
        (RunExtra::SweepFreqList, RunExtra::SweepFreqLinPoints),
        &[sweep_forms::FREQ_LIST_KEPT],
        sweep_forms::sweep_freq_list_to_lin_points,
    ),
    // after the sweep rows: a loop's values are listed from the forms they leave
    Pass {
        name: "dc_axis_device_param_to_loop",
        removes: ir::Feature::Run(RunExtra::DcAxisDeviceParam),
        requires: DC_LOOPS,
        notes: &[dc_axes_to_loops::DC_AXIS_KEPT],
        run: dc_axes_to_loops::dc_axis_device_param_to_loop,
    },
    Pass {
        name: "dc_axis_model_param_to_loop",
        removes: ir::Feature::Run(RunExtra::DcAxisModelParam),
        requires: DC_LOOPS,
        notes: &[dc_axes_to_loops::DC_AXIS_KEPT],
        run: dc_axes_to_loops::dc_axis_model_param_to_loop,
    },
    Pass {
        name: "dc_restart_to_loops",
        removes: ir::Feature::Run(RunExtra::DcRestart),
        requires: DC_LOOPS,
        notes: &[dc_axes_to_loops::DC_AXIS_KEPT],
        run: dc_axes_to_loops::dc_restart_to_loops,
    },
    Pass {
        name: "alter_param_at_once_to_at_reset",
        removes: ir::Feature::Run(RunExtra::AlterParamAtOnce),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Run(RunExtra::AlterParamAtReset))
            .with(ir::Feature::Item(ir::ItemKind::Reset))
            .with(ir::Feature::Run(RunExtra::ResetValues))
            .with(ir::Feature::Run(RunExtra::ResetOptions))
            .with(ir::Feature::Run(RunExtra::ResetTemp))
            .with(ir::Feature::Run(RunExtra::ResetTnom))
            .with(ir::Feature::Run(RunExtra::ResetSaves)),
        notes: &[alter_at_reset::ALTER_KEPT],
        run: alter_at_reset::alter_param_at_once_to_at_reset,
    },
    Pass {
        name: "analysis_param_args_to_values",
        removes: ir::Feature::Run(RunExtra::AnalysisParamArgument),
        requires: ir::FeatureSet::EMPTY,
        notes: &[param_arguments::PARAM_ARG_KEPT],
        run: param_arguments::analysis_param_args_to_values,
    },
    // one row per option proven inert (`inert_options.rs`)
    inert(inert_options::INERT[0]),
    inert(inert_options::INERT[1]),
    inert(inert_options::INERT[2]),
    inert(inert_options::INERT[3]),
    inert(inert_options::INERT[4]),
    inert(inert_options::INERT[5]),
    inert(inert_options::INERT[6]),
    inert(inert_options::INERT[7]),
    inert(inert_options::INERT[8]),
    inert(inert_options::INERT[9]),
    inert(inert_options::INERT[10]),
    inert(inert_options::INERT[11]),
    inert(inert_options::INERT[12]),
    inert(inert_options::INERT[13]),
    inert(inert_options::INERT[14]),
    inert(inert_options::INERT[15]),
    Pass {
        name: "drop_print",
        removes: ir::Feature::Item(ir::ItemKind::Print),
        requires: ir::FeatureSet::EMPTY,
        notes: &[drop_print::PRINT_KEPT],
        run: drop_print::drop_print,
    },
    // after the `whole_run_*_first` rows: a move may interleave
    Pass {
        name: "run_items_together",
        removes: ir::Feature::Run(ir::RunExtra::RunInterleaved),
        requires: ir::FeatureSet::EMPTY,
        notes: &[run_items_together::HOLDS_RUN_ITEMS],
        run: run_items_together::run_items_together,
    },
];

/// What a `dc_axes_to_loops` row's output may add: list sweep loops that keep their last value
/// over the kinds of target a DC axis has, their names, an operating point in their bodies.
const DC_LOOPS: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Item(ir::ItemKind::Loop))
    .with(ir::Feature::Placed(
        ir::ScopeKind::Loop,
        ir::ItemKind::Analysis,
    ))
    .with(ir::Feature::Placed(ir::ScopeKind::Loop, ir::ItemKind::Loop))
    .with(ir::Feature::Analysis(ir::AnalysisKind::Op))
    .with(ir::Feature::Expr(ir::ExprTag::Num))
    .with(ir::Feature::Run(RunExtra::LoopKeepLast))
    .with(ir::Feature::Run(RunExtra::SweepLoopList))
    .with(ir::Feature::Run(RunExtra::LoopOverOp))
    .with(ir::Feature::Run(RunExtra::LoopNamed))
    .with(ir::Feature::Run(RunExtra::LoopUnnamed))
    .with(ir::Feature::Run(RunExtra::LoopParam))
    .with(ir::Feature::Run(RunExtra::LoopSourceDc))
    .with(ir::Feature::Run(RunExtra::LoopElementValue))
    .with(ir::Feature::Run(RunExtra::LoopDeviceParam))
    .with(ir::Feature::Run(RunExtra::LoopModelParam));

// ---- private ----

/// A `whole_run_*_first` row: it removes the kind's `*WholeRunMidRun` and requires
/// `RunInterleaved` (a moved item may land after a circuit item).
const fn first(
    name: &'static str,
    removes: RunExtra,
    run: fn(&mut ir::Design, &mut ir::Diags),
) -> Pass {
    Pass {
        name,
        removes: ir::Feature::Run(removes),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(RunExtra::RunInterleaved)),
        notes: &[wr::STATE_ELSEWHERE],
        run,
    }
}

/// A `whole_run_*_to_from_here` row: it removes the kind's `*WholeRun` and requires its
/// `*FromHere` (`modes`: that pair).
const fn to_from_here(
    name: &'static str,
    modes: (RunExtra, RunExtra),
    notes: &'static [ir::Code],
    run: fn(&mut ir::Design, &mut ir::Diags),
) -> Pass {
    Pass {
        name,
        removes: ir::Feature::Run(modes.0),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(modes.1)),
        notes,
        run,
    }
}

/// An `inert_options` row: it removes its key's `Option` feature and requires nothing.
const fn inert(row: inert_options::Row) -> Pass {
    Pass {
        name: row.0,
        removes: ir::Feature::Option(row.1),
        requires: ir::FeatureSet::EMPTY,
        notes: &[inert_options::OPTION_KEPT],
        run: row.2,
    }
}

/// A `sweep_forms` row: it removes the form `forms.0` and requires the form `forms.1`.
const fn sweep(
    name: &'static str,
    forms: (RunExtra, RunExtra),
    notes: &'static [ir::Code],
    run: fn(&mut ir::Design, &mut ir::Diags),
) -> Pass {
    Pass {
        name,
        removes: ir::Feature::Run(forms.0),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Run(forms.1)),
        notes,
        run,
    }
}

/// What a ceiling row's expression may add: the given ceiling, its operators and their tags.
const fn ceiling(ops: &[ir::BinOp], min: bool) -> ir::FeatureSet {
    let mut set = ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranMaxStep));
    if !ops.is_empty() {
        set = set.with(ir::Feature::Expr(ir::ExprTag::Binary));
    }
    // both forms: a field reading `$temp` makes the root dynamic (R14)
    let mut k = 0;
    while k < ops.len() {
        set = set
            .with(ir::Feature::Binary(ops[k]))
            .with(ir::Feature::BinaryDyn(ops[k]));
        k += 1;
    }
    if min {
        set = set
            .with(ir::Feature::Func(ir::Func::Min))
            .with(ir::Feature::FuncDyn(ir::Func::Min))
            .with(ir::Feature::Expr(ir::ExprTag::Call));
    }
    set
}
