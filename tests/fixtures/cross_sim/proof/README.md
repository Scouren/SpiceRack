# Proofs for `simulator-differs` and `simulator-fails` lines

A `simulator-differs:<kind> reason=<rule-id> proof=<file>` line in a cross-sim
list names one of these files (`tests/test_cross_sim/main.rs`, `listed`). Each
holds the rule, its kind (mismatches.md §4.3) and the numbers that settle it,
with the tools and versions that gave them. The harness's own kinds
(`source`, `refinement`) carry `proof=harness` instead.

A `simulator-fails reason=<rule-id> proof=<file>` line names one too: the
target's simulator fails on the written deck, runs the original in its
other mode (so the harness's control does not show the failure and the deck
is observed as `target-fails`), and the file cites the page of the target's
manual that allows the written text, with the runs
(`tests/test_cross_sim/lists.rs`, `PROVEN_FAILS`; ZERO-LEDGER P1-037).
