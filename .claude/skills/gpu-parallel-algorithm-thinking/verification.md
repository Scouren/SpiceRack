# Verification

Reached from step 7. `scripts/verify_dag_equivalence.py`, stdlib only. It finds counterexamples;
it does not prove equivalence. Say so in the report, every time.

Run the worked example first — it is the template:

```
python scripts/example_compaction.py            # ok
python scripts/example_compaction.py --bug      # inclusive-vs-exclusive scan, caught
python scripts/example_compaction.py --bug-undeclared   # hidden dependency, caught
```

## What it checks

**DAG validity** — duplicate names, dependencies on nonexistent stages, self-edges, cycles.
Cycle errors print a concrete path.

**Undeclared reads** — each stage runs against a `StageView` holding only `input` plus its
declared dependencies. Reading anything else raises `UndeclaredDependency`. This is the
mechanism that catches "it works because scan happens to run before total".

**Randomized legal orders** — `orders_per_input` schedules per input, each a valid topological
order with the next ready stage chosen at random. Catches stages that share a buffer and assume
an order the DAG never declared.

**Differential** — CPU reference vs the stage simulator, and optionally vs a real GPU binary via
`--gpu-cmd`: the adapter writes `{"input": ...}` to the process's stdin and reads
`{"output": ...}` from its stdout.

**Invariants** — `Invariant(name, check(input, output))`. Reach for: output count conservation,
sortedness, permutation preservation, sum preservation, reachability, bounds, monotonicity, no
duplicate ownership, no unwritten output slot.

**Metamorphic rules** — `Metamorphic(name, transform(input, rng), relation(out, out'))`. Reach
for: permute independent inputs, duplicate the input, scale all values, split and recombine
partitions, prepend an identity element, reorder commutative inputs. These catch bugs that
survive when the CPU reference has the *same* misconception.

## Equivalence modes

`mode="exact"` · `mode="unordered"` · `mode=approx(abs_tol=, rel_tol=)` · any
`callable(expected, actual) -> bool`. The mode must match the equivalence level frozen in step 1;
picking `approx` to make a red suite go green is a contract change and needs saying out loud.

## Input coverage

`standard_generators(make, max_size)` covers sizes on warp/block/tile/power-of-two boundaries and
their ±1 neighbours, plus a random size. Add named `Generator`s for the shape-specific cases:
empty, singleton, all-pass, all-fail, extremal values, duplicate-heavy, sorted, reverse-sorted,
skewed/heavy-tailed, and — for graph work — star, path, clique, disconnected, and self-loop
structures.

## On failure

Each report prints kind, generator, seed, input, expected, actual, stage order, comparator mode,
the failed invariant, and a command that reproduces it. Fix the formulation, not the tolerance.
