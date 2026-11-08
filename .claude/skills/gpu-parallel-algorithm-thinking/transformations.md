# Transformation Search

Reached from step 4. Run against the essential DAG, not the source code.

## Work and span

```
W = total operations         S = longest dependency chain (span)
Parallelism = W / S          compare W_seq/S_seq against W_par/S_par
```

Also budget, per candidate: extra memory, global sync count, expected concurrency at the real
input size. `S` alone decides nothing — a `log n`-span formulation doing `n log n` work loses to
an `n`-work one whenever the machine has fewer lanes than the input has elements.

## Algebra checklist

For each operator on a sequential chain, test: associativity · commutativity · distributivity ·
identity · inverse · idempotence · monotonicity · absorption · decomposability · separability ·
locality · linearity · homomorphism into another representation · monoid/semigroup/group/ring/
semiring reading.

Associativity buys tree reduction and scan. Identity buys padding to a power of two. Inverse
buys sliding windows and "subtract the prefix". Idempotence buys tolerating duplicate work —
which buys cheap frontiers. Monotonicity buys speculation and early exit. A homomorphism buys
the whole lifted-state move below.

Use these internally; the user-facing report needs the transformation, not the vocabulary.

## Transformation questions

**Fold → tree.** Left-fold to balanced reduction. Recurrence to prefix scan. Global calculation
to local partials plus a merge.

**Counters and ordering.** Sequential counter → **count–scan–scatter**. Ordered update set →
sort–reduce–scatter. Repeated search → sort, bucket, histogram, or an index built once.
Privatize updates per thread/warp/block, merge hierarchically.

**Recurrences.** Unroll. Represent as function composition. Ask whether the composition itself
is associative (see below). Ask whether the state can be lifted into a larger associative one.

**Reshaping.** Express as matrix, tensor, graph, or semiring operations. Transpose so the
independent axis becomes the inner axis. Divide-and-conquer instead of incremental
construction. Pointer jumping, path doubling, list ranking, tree contraction to collapse
dependency depth. Evaluate a DP by diagonals, tiles, wavefronts, or blocked fronts. Convert
irregular work into a frontier. Batch many small instances into one launch.

**Trades.** Speculate several outcomes and select. Recompute redundantly instead of
communicating. Preprocess to make the downstream regular. Relax exactness to a justified
approximation. Change the data structure so the algorithm becomes parallel. Shrink a sequential
phase until it stops dominating.

**Last question, asked first in practice:** is there already a primitive for this?

## Lifted-state scan

A recurrence looks sequential because state chains:

```
x[i] = f(x[i-1], input[i])
```

Reframe each step as a state transformer `T_i(state) -> state`. If composition is associative —
`(T_c ∘ T_b) ∘ T_a == T_c ∘ (T_b ∘ T_a)` — scan the *transformers* instead of the values, then
apply the prefix composition to the initial state.

Composes associatively in practice: affine maps `(a, b) → ax + b` · small fixed-size matrices ·
state-transition tables of a finite automaton · piecewise-linear or min/max-plus segments ·
segment summaries carrying (leftmost, rightmost, aggregate, count).

This is how carry propagation, IIR filters, `scanl` over saturating arithmetic, run-length
merges, and regex/lexer scans go parallel.

Failure modes to check before committing: lifted state size grows with input (composition cost
no longer O(1)), composition is more expensive than the original step by a factor exceeding the
lane count, or the lift only closes under an operator that is non-associative in floating point.

## Primitive catalogue

Each entry: what sequential dependency it retires.

| primitive | retires |
|---|---|
| map | nothing — pure independence, the baseline |
| reduce | ordered accumulation |
| scan / segmented scan | value-carrying recurrence |
| filter / partition / stream compaction | append-to-output-list |
| scatter / gather | pointer-chasing writes / reads |
| histogram | shared counter per bucket |
| sort / radix sort | "find the matching item" search chain |
| reduce-by-key, group-by-key | grouped accumulation with a moving key |
| stencil | neighbourhood read with in-place update |
| transpose / tile | wrong-axis access order |
| frontier expansion + work queue | recursive or worklist traversal |
| bulk-synchronous iteration | while-not-converged loop |
| pointer jumping / tree contraction | linked or hierarchical depth |
| divide-and-conquer | incremental construction |
| batched search | one query at a time |
| producer–consumer pipeline | stage-at-a-time execution |

Library first: CUB and Thrust (CCCL), rocPRIM/hipCUB, cuBLAS/rocBLAS, cuSPARSE/rocSPARSE,
CUTLASS. A hand-written scan or radix sort needs a measured reason to exist.
