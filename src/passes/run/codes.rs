//! The run group's catalogue rows: one `ir::Info` per note code its passes put at an item they
//! leave (`U0400..=U0499`; this owner's blocks: `U0400`–`U0416`, `U0420`–`U0439`; `passes`:
//! `U0417`–`U0419`, `U0440`–`U0449`), each row's code a constant of its rule's file. Moved out
//! of `mod.rs` beside the registry rows, unchanged and in the same order.
//!
//! Contents: `pub(crate) const CODES: &[(ir::Code, ir::Info)]`

use super::{
    alter_at_reset, dc_axes_to_loops, drop_print, fs, ic_next_only, inert_options, loop_end,
    names_dropped, names_given, param_arguments, run_items_together, save_set_to_all, sweep_forms,
    tc, tran_print_step, wr,
};
use crate::ir;

/// The group's catalogue rows (`U0400..=U0499`).
pub(crate) const CODES: &[(ir::Code, ir::Info)] = &[
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
        tran_print_step::PRINT_STEP_DROPPED,
        ir::Info {
            title: "a transient's print step the target has no field for, dropped",
            cite: "rewrite/RULINGS.md R67; rewrite/AMEND-A12.md A41",
            explain: "The target has no output interval: it records the steps it takes. Once nothing reads the interval it only resamples the output, so it was dropped; the note's argument is its expression, and the results are given back on that grid.",
            kind: ir::Kind::Note,
        },
    ),
    (
        tran_print_step::PRINT_STEP_READ,
        ir::Info {
            title: "a transient's print step something still reads",
            cite: "rewrite/RULINGS.md R67; rewrite/AMEND-A12.md A24, A41",
            explain: "The target has no output interval, but this one is read: by ngspice's default step ceiling (min of the interval and a fiftieth of the span), by a $tstep in an expression, or by further segments of their own intervals. Dropping it would change the run, so the item is refused; the rules that write the ceiling and substitute $tstep run first.",
            kind: ir::Kind::Note,
        },
    ),
    (
        fs::FIRST_STEP_MOVED,
        ir::Info {
            title: "a transient's first step moved into its print step",
            cite: "rewrite/RULINGS.md R73",
            explain: "The target has no first-step field and needs a print step; this transient gave a first step and no print step, so the first step became the print step and the first step is the target's own rule, which takes one no larger (R73). The note's argument is the value.",
            kind: ir::Kind::Note,
        },
    ),
    (
        fs::FIRST_STEP_KEPT,
        ir::Info {
            title: "a transient's first step that cannot move",
            cite: "rewrite/RULINGS.md R73",
            explain: "The target has no first-step field. This one cannot become the print step: the transient already has one, its step ceiling is still a simulator's rule that would read it, or a $tstep in an expression would read it. The ceiling rules and the $tstep substitution run first; otherwise the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        fs::NO_STEP_VALUE,
        ir::Info {
            title: "a transient with neither a print step nor a first step",
            cite: "rewrite/RULINGS.md R73",
            explain: "The target needs a print step, and nothing in the deck determines one: no print step and no first step to move into it (R13: no invented value). The item is refused.",
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
        names_given::NAME_GIVEN,
        ir::Info {
            title: "a statement the target must name, given a name",
            cite: "rewrite/AMEND-A12.md A35; R59",
            explain: "The target names every analysis, DC axis and loop, and this one had no name, so it was given one: its kind's word and a count, unused by any other statement of its kind. It labels results only. The note's argument is the new name: results are given back under the source's addressing.",
            kind: ir::Kind::Note,
        },
    ),
    (
        names_given::LOOP_NAME_UNREAD,
        ir::Info {
            title: "an unnamed loop whose body may read a name",
            cite: "rewrite/AMEND-A12.md A35",
            explain: "The target names every loop. This loop's body holds a statement kept as its dialect's text, which may read a variable a new loop name would shadow, so no name is given and the loop is refused.",
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
        save_set_to_all::SAVE_SET_KEPT,
        ir::Info {
            title: "a save scoped to a set of analysis kinds, beside an analysis of another kind",
            cite: "rewrite/AMEND-A12.md A22; R43",
            explain: "The target's save governs every analysis. This one governs some kinds only, and the design runs an analysis of another kind, which writes its own default set in the source; widened, the save would drop vectors from that analysis's results, so it is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        sweep_forms::SWEEP_KEPT,
        ir::Info {
            title: "a DC axis no linear step states exactly",
            cite: "rewrite/IR-RUN.md §1; ngspice-45 dctrcurv.c",
            explain: "The target's DC axis is linear by step. This axis's values are no such grid: a value is not a literal, the list is not arithmetic or repeats a value, the step does not terminate within a literal's 18 digits, or ngspice's end test (2.2e-13, absolute) would run another number of points. The analysis is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        sweep_forms::FREQ_LIST_KEPT,
        ir::Info {
            title: "a frequency list other than one value of an AC analysis",
            cite: "rewrite/IR-RUN.md §1; ngspice-45 acan.c",
            explain: "The target sweeps frequency by decade, octave or a linear count. One listed frequency of an AC analysis is a linear sweep of one point; two or more are not exact (ngspice's linear sweep of two points runs one, and its end test scales with reltol), and another analysis's list is not rewritten. The analysis is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        dc_axes_to_loops::DC_AXIS_KEPT,
        ir::Info {
            title: "a DC axis over a device or model parameter the target cannot sweep",
            cite: "rewrite/IR-RUN.md §1; INV-semantics 8.09",
            explain: "The target's DC analysis sweeps a source, an element's value or the temperature, so this axis would become a loop of operating points. That is exact only for an analysis that restarts each point (a loop's points start afresh, a continued sweep from the previous point), that stands at its run's top with no analysis after it (the loop keeps its last value), whose peeled axes a loop can sweep, over values stated as decimal literals (no log grid). One of these fails, so the analysis is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        dc_axes_to_loops::AS_LOOPS,
        ir::Info {
            title: "a DC analysis written as loops of operating points",
            cite: "rewrite/IR-RUN.md §1; rewrite/AMEND-A12.md A32",
            explain: "The source's one DC plot over every axis is written as one plot per pass of these loops, outermost slowest: an operating point each, or a DC over the axes left inside. The peeled axes' swept vectors are not written; their values at each pass are the loops' lists, in order. The note's argument is the analysis's name: its results are given back under it.",
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
    (
        inert_options::OPTION_DROPPED,
        ir::Info {
            title: "an option that changes no result, dropped",
            cite: "R67, R83; tests/passes_run/decks/options_probe.sp",
            explain: "The target has no such option, and this one changes nothing the run writes: with and without it, ngspice-45 writes the same raw file (it changes the log only), or for a solver selection the same within that solver's rounding (1e-7 relative), so it was dropped. The note's argument is the key.",
            kind: ir::Kind::Note,
        },
    ),
    (
        inert_options::OPTION_KEPT,
        ir::Info {
            title: "an inert option the rule cannot drop here",
            cite: "R67",
            explain: "The option changes no result, but it stands where dropping it would change something else: in a loop's body or a branch (the body's shape is a feature of its own), or as a loop's or alter's target. It is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        drop_print::PRINT_DROPPED,
        ir::Info {
            title: "a print card dropped",
            cite: "R49, R83",
            explain: "The target has no print card here, and a print writes the run's log, never its saved results: ngspice writes the same raw file with and without it. It was dropped.",
            kind: ir::Kind::Note,
        },
    ),
    (
        drop_print::PRINT_KEPT,
        ir::Info {
            title: "a print card in a loop's body or a branch",
            cite: "R83",
            explain: "The print changes no result, but dropping it from a loop's body or a branch changes the body's shape, a feature of its own, so it is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        alter_at_reset::ALTER_KEPT,
        ir::Info {
            title: "a parameter alter whose reset would undo other script state",
            cite: "R70; rewrite/AMEND-A12.md A43",
            explain: "The target's parameter alter takes effect at the next reset, so a reset would follow it; a reset also undoes value alters, options, temperatures and saves in force from earlier in the run (or every earlier pass in a loop's body), and one of those is in force here, so the alter is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        param_arguments::PARAM_ARG_KEPT,
        ir::Info {
            title: "an analysis or loop argument that reads a parameter its value cannot replace",
            cite: "R68; rewrite/AMEND-A12.md A42",
            explain: "The target takes no circuit parameter in an analysis's or loop's argument, so a read there is written as the parameter's value. Here it cannot be: the parameter is not a top-level one with a constant value, an alter, a loop or a statistics block may change it, or the read stands in a function's body. The item is refused.",
            kind: ir::Kind::Note,
        },
    ),
];
