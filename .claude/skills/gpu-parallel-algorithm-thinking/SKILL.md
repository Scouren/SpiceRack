---
name: gpu-parallel-algorithm-thinking
description: Treats CPU code as one schedule of an algorithm, not the algorithm — recovers the dependency DAG and searches for shorter-span GPU formulations. Use when rewriting a sequential CPU algorithm for a GPU (CUDA, HIP/ROCm, compute shaders), when a recurrence or loop looks inherently sequential and parallelism must be found, when choosing between competing parallel formulations, or when deciding whether GPU execution is worth it at all. Also use when another skill needs dependency-DAG recovery or work/span analysis.
---

# GPU Parallel Algorithm Thinking

**The CPU code is one schedule, not the algorithm.** Recover the dependency structure and the
mathematical meaning, then search for an equivalent formulation with shorter **span** and a
GPU-native data flow.

Two jobs. Never merge them:

| | owns |
|---|---|
| this skill | what computation and what dependency structure should exist |
| `/data-oriented-design` | how the chosen data is represented for the hardware |

One thread per CPU-loop iteration is a mapping decision. Making it before step 4 is the
characteristic failure of this work.

## Verdicts

Every run ends on one of these, named:

- **artificial** — the dependency is an implementation artifact; transform it.
- **weakenable** — real but groupable, reorderable, speculatable, or expressible as a primitive.
- **nested-parallel** — genuinely sequential, but surrounding dimensions parallelize.
- **thin** — insufficient useful parallelism at the stated workload; stay on CPU or go hybrid.
- **needs-new-math** — parallelism requires a different formulation, not a different loop.
- **span-win-work-loss** — shorter span, too much extra work.
- **movement-bound** — parallel, but data movement or synchronization sinks it on this GPU.

State the verdict early and revise it explicitly. A run that reaches step 14 without ever
naming one skipped the analysis.

---

## Steps

### 1. Freeze the contract

Keep the CPU reference runnable for the whole project. Write down: input domains, outputs,
side effects, mutated state, required ordering, required determinism, error behaviour, whether
intermediates are externally observable.

Then pick the equivalence level, explicitly, one of: **bitwise** / **numeric within tol** /
**exact-arithmetic** / **application-semantic** / **statistical**.

Floating-point `+` and `*` are not associative. Reordering them is a contract change — allowed
only once the chosen level permits it, and recorded when taken.

_Done when:_ equivalence level is named and every mutated-state item is listed.

### 2. Recover the DAG

Node = operation, iteration, or stage. Edge A→B = B needs a value or effect from A.

Classify each edge — true RAW / anti (WAR) / output (WAW) / control / **memory-reuse** /
**counter-or-allocator** / **data-structure-induced** / **mathematically-required**.

For every edge ask: *ordered because the problem requires it, or because the CPU runs one
instruction at a time?*

_Done when:_ every edge carries a class, and the longest path is identified.

### 3. Cut the artificial edges

The last four classes above usually vanish under: renaming storage · private output space per
task · immutable intermediate buffers · splitting one loop into stages · replacing a global
counter with **count–scan–scatter** · replacing ordered accumulation with tree reduction ·
pointers → indices · recursion → level sets or worklists.

_Done when:_ a second DAG exists with only essential edges, and each removed edge names the
transformation that removed it.

### 4. Search transformations

Read `transformations.md` and work its checklist against the essential DAG. Search every
parallel dimension — inputs, outputs, batch, space, vertices/edges, tree levels, DP diagonals,
candidates, within-reduction, within-object, pipeline overlap. Stopping at outer-loop
parallelism is not a search.

_Done when:_ each surviving sequential span has been tested against the algebra checklist and
the lifted-state-scan question, pass or fail recorded.

### 5. Build three candidates

Before building candidates, read the worked example matching the problem's shape:
value-carrying recurrence or running aggregate → `examples/prefix_scan.md` · filtered or
appended output → `examples/stream_compaction.md` · 2D DP recurrence →
`examples/wavefront_dp.md` · graph, worklist, or frontier traversal →
`examples/irregular_frontier.md`. The last two each include a transformation that is
theoretically parallel and practically worse.

Never commit to the first parallel idea.

- **A** minimal transformation
- **B** aggressive reformulation
- **C** higher-work, more regular

Optional: sort-based · frontier-based · dense-replaces-sparse · speculative ·
library-composed · persistent-kernel · CPU/GPU hybrid.

Express each as a composition of named primitives (map, scan, segmented scan, compaction,
scatter, gather, histogram, radix sort, reduce-by-key, stencil, transpose, frontier expansion,
work queue, pointer jumping, tree contraction, ...). For each primitive: which dependency it
replaces, which property makes that valid, its work and span, its temporary storage.

Reach for CUB, Thrust, rocPRIM, cuBLAS/rocBLAS, cuSPARSE/rocSPARSE, or CUTLASS before writing
a custom scan, sort, reduction, or compaction.

_Done when:_ three candidates exist as primitive compositions, none of them raw kernel code.

### 6. Compare and choose

Fill `templates/candidate-comparison-table.md`. W, S, W/S, global traffic, temporaries, kernel
launches, global syncs, atomic contention, divergence, load imbalance, memory regularity,
determinism, numerics, complexity, useful input range.

Shorter span is not a win by itself — it is bought with work, traffic, storage, or contention,
and the price goes in the table.

_Done when:_ the recommendation says why the winner wins, and rejected candidates are kept
with their reason (a reject can win at a different size or on different hardware).

### 7. Verify before optimizing

Use `scripts/verify_dag_equivalence.py`; see `verification.md`. Declare the stage DAG, run the
randomized-legal-order simulator, differential-test against the CPU reference, add invariants
and metamorphic rules, hit the boundary sizes.

This is randomized testing. Never call its result a proof.

_Done when:_ the DAG is acyclic, order-independent across randomized schedules, and matching on
random, empty, singleton, extremal, duplicate-heavy, sorted, reverse, skewed, and
warp/block/tile/power-of-two-boundary inputs.

### 8. Map to the machine

Read `gpu-mapping.md`. Per stage, decide what one thread / warp / block / persistent block /
grid phase / GPU owns, what data is reused, where synchronization and communication are
required, and what the unit of load balancing is.

_Done when:_ every stage has an ownership level and a named synchronization scope, chosen at the
smallest scope that suffices.

### 9. Invoke `/data-oriented-design`

Now, with the layout questions from `gpu-mapping.md` answered.

_Done when:_ SoA, flat buffers, index-based relationships, batched operations, and explicit
ownership are in place, or a preserved CPU layout is justified in writing.

### 10. Baseline, then one change at a time

Implement the simplest correct GPU version. Keep three implementations where practical: CPU
reference, naive GPU, reformulated GPU. Profile. Name the dominant bottleneck from the list in
`gpu-mapping.md`. Change one major factor. Re-run step 7. Re-profile. Repeat.

Benchmarks measure speed; they do not test correctness.

_Done when:_ the report names the limiting factor with the profiler counter or
bandwidth/FLOP arithmetic that shows it, the best remaining change is predicted (with that
arithmetic) to gain less than the measured run-to-run variance, and every reported speedup
carries baseline, hardware, input size and distribution, transfers-included yes/no, numerical
mode, repetition count, and statistic.

---

## Report

On a real algorithm, produce `templates/algorithm-reformulation-report.md` before writing
optimized kernels. Sections may shrink for easy cases; the reformulation/mapping split stays.

## Guardrails

- Identify a genuinely sequential critical path as such, and say plainly when more parallelism
  needs new mathematics rather than a new loop nest.
- Occupancy is one input to a bandwidth or latency argument, never the target.
- Prefer the hypothesis you can measure over the advice you can repeat.
- Hybrid CPU/GPU decompositions and "this belongs on the CPU" are legitimate outcomes.
- Assume associativity only after naming the operator *and* its numerical domain.

## References

`references/resources.md` — annotated sources, each naming the step it feeds.
