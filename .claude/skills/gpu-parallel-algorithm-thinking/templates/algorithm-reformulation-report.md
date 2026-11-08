# Reformulation Report: <algorithm>

**Verdict:** <artificial | weakenable | nested-parallel | thin | needs-new-math |
span-win-work-loss | movement-bound>

## 1. Semantic contract
Inputs and domains · outputs · side effects · mutated state · required ordering · determinism ·
error behaviour · observable intermediates.
**Equivalence level:** bitwise / numeric ±tol / exact-arithmetic / application-semantic /
statistical.
Floating-point reassociation: permitted? where?

## 2. Sequential algorithm summary
What it computes, in one paragraph, without reference to how it loops.

## 3. Dependency analysis
Per loop or phase: what carries state, what is pure, what generates work, what generates indices.

## 4. Original dependency DAG
Nodes, edges, edge classes (RAW / anti / output / control / memory-reuse / counter / structure /
mathematical). Longest path.

## 5. Artificial vs essential
| edge | class | artificial? | transformation that removes it |
|---|---|---|---|

## 6. Mathematical properties
Operators and the properties they were tested for. Lifted-state scan: applicable? state size?
composition cost?

## 7. Candidate reformulations
A (minimal) / B (aggressive) / C (higher-work, regular), each as a composition of named
primitives, with the library that already implements each one.

## 8. Work–span comparison
Insert `candidate-comparison-table.md`.

## 9. Correctness obligations
What must hold for each transformation to be valid; how each is tested.

## 10. Selected algorithm
Why this one wins, at what input sizes, on what hardware.

## 11. GPU execution mapping
Per stage: what one thread / warp / block owns · reuse · synchronization scope · load-balancing
unit · launch structure.

## 12. Data-oriented representation
Output of `/data-oriented-design`: layouts, index types, buffers, temporaries, alignment,
coalescing argument.

## 13. Synchronization and communication
Chosen scope per boundary and why a smaller scope does not suffice.

## 14. Verification plan
Stage DAG, comparator mode, invariants, metamorphic rules, generators, boundary sizes.

## 15. Benchmark plan
Baselines · what is timed · input distributions and sizes · transfers in/out · numerical modes ·
repetitions and statistic · expected break-even size.

## 16. Risks and rejected alternatives
| candidate | rejected because | would win when |
|---|---|---|
