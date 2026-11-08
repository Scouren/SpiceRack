# Proofs for `simulator-differs` lines

A `simulator-differs:<kind> reason=<rule-id> proof=<file>` line in a cross-sim
list names one of these files (`tests/test_cross_sim/main.rs`, `listed`). Each
holds the rule, its kind (mismatches.md §4.3) and the numbers that settle it,
with the tools and versions that gave them. The harness's own kinds
(`source`, `refinement`) carry `proof=harness` instead.
