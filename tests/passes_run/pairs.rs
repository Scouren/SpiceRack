//! Which run passes have their simulator proof as a pair deck (`rewrite/SPINE.md` §8a), and
//! the strict list of those still to come.
//!
//! Contents: `pub const PENDING_PAIRS: &[&str]`,
//! `pub fn every_run_pass_has_its_pair_deck_or_a_pending_marker_and_not_both()`

use std::path::Path;

use spicerack::passes;

/// The run passes whose simulator proof (a pair deck naming it in `proves =`) is to come:
/// compared by name with the decks, both ways.
pub const PENDING_PAIRS: &[&str] = &[
    // proven on both simulators by `passes_run/dc_loops.rs`, not a pair deck: the pair harness
    // compares one DC plot with one, and the rule writes one op plot a pass
    "dc_axis_device_param_to_loop",
    "dc_axis_model_param_to_loop",
    // proven on ngspice by `passes_run/options.rs` (the raw file unchanged); no VACASK deck holds
    // these options
    "drop_option_noacct",
    "drop_option_acct",
    "drop_option_list",
    "drop_option_nomod",
    "drop_option_nopage",
    "drop_option_node",
    "drop_option_opts",
    "drop_option_warn",
    "drop_option_maxwarns",
    "drop_option_norefvalue",
    "drop_option_noinit",
    "drop_option_nfreqs",
    "drop_option_fourgridsize",
    "drop_option_klu",
    "drop_option_sparse",
    "drop_option_trtol",
    // proven on ngspice by `passes_run/options.rs` (prints): no VACASK deck holds a print
    "drop_print",
    "whole_run_option_first",
    "whole_run_env_first",
    "whole_run_node_set_first",
    "whole_run_save_first",
    "whole_run_option_to_from_here",
    "whole_run_env_to_from_here",
    "whole_run_node_set_to_from_here",
    "whole_run_save_to_from_here",
    "run_items_together",
    "tran_cap_ngspice_to_given",
    "tran_cap_span50_to_given",
    "tran_cap_span10_to_given",
    "tran_cap_none_to_given",
    "ic_beyond_next_to_next_only",
    "ic_next_only_to_beyond",
    "analysis_name_dropped",
    "dc_axis_name_dropped",
    "loop_name_dropped",
    "analysis_name_given",
    "dc_axis_name_given",
    "loop_name_given",
    "tran_print_step_to_simulator",
    "tran_simulator_step_to_print",
    "tran_first_step_to_print",
    "device_ic_to_tran_ic",
    "loop_restore_to_keep_last",
    "loop_keep_last_to_restore",
];

/// Every run pass has a pair deck naming it in `proves =` or a [`PENDING_PAIRS`] row, not
/// both, and every row names a registered pass.
pub fn every_run_pass_has_its_pair_deck_or_a_pending_marker_and_not_both() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    let mut proven = Vec::new();
    for cell in std::fs::read_dir(root).into_iter().flatten().flatten() {
        for deck in std::fs::read_dir(cell.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if deck.path().extension().is_some_and(|e| e == "pair") {
                let text = std::fs::read_to_string(deck.path()).unwrap_or_default();
                for line in text.lines() {
                    if let Some((k, v)) = line.split('#').next().unwrap_or("").split_once('=')
                        && k.trim() == "proves"
                    {
                        proven.push(v.trim().to_string());
                    }
                }
            }
        }
    }
    let group = passes::GROUPS
        .iter()
        .find(|g| g.name == "run")
        .expect("the group");
    let mut wrong = Vec::new();
    for pass in group.passes {
        if PENDING_PAIRS.contains(&pass.name) == proven.iter().any(|p| p == pass.name) {
            wrong.push(pass.name);
        }
    }
    wrong.extend(
        PENDING_PAIRS
            .iter()
            .filter(|p| !group.passes.iter().any(|q| q.name == **p)),
    );
    assert!(
        wrong.is_empty(),
        "pair deck against PENDING_PAIRS: {wrong:?}"
    );
}
