"""Complete worked harness run: stream compaction as count -> exclusive scan -> scatter.

    python example_compaction.py            # passes
    python example_compaction.py --bug      # inclusive scan instead of exclusive; caught
    python example_compaction.py --bug-undeclared   # stage reads an undeclared dependency

The CPU reference appends to a list, which serializes on the output cursor. The parallel
formulation replaces that cursor with count-scan-scatter: every input computes its own output
slot from a prefix sum, so all writes are independent.
"""

from __future__ import annotations

import random
import sys

from verify_dag_equivalence import (
    Generator,
    Invariant,
    Metamorphic,
    Spec,
    Stage,
    main,
    standard_generators,
)

BUG = "--bug" in sys.argv
BUG_UNDECLARED = "--bug-undeclared" in sys.argv
sys.argv = [a for a in sys.argv if not a.startswith("--bug")]


def predicate(x: int) -> bool:
    return x % 3 != 0


def transform(x: int) -> int:
    return x * x


# ------------------------------------------------------------------ CPU reference


def cpu_reference(items: list[int]) -> list[int]:
    output = []
    for item in items:
        if predicate(item):
            output.append(transform(item))
    return output


# ------------------------------------------------------------------ stage DAG


def stage_flags(ctx) -> list[int]:
    # map: fully independent
    return [1 if predicate(x) else 0 for x in ctx["input"]]


def stage_scan(ctx) -> list[int]:
    # exclusive scan over flags: each element's output slot
    flags = ctx["flags"]
    out, running = [], 0
    for f in flags:
        if BUG:
            running += f
            out.append(running)  # inclusive — off by one, shifts every write
        else:
            out.append(running)
            running += f
    return out


def stage_total(ctx) -> int:
    return sum(ctx["flags"])


def stage_scatter(ctx) -> list[int]:
    items = ctx["input"]
    flags = ctx["flags"]
    offsets = ctx["scan"]
    total = ctx["total"] if not BUG_UNDECLARED else len(ctx["allocate"])
    out = [None] * total
    for i, item in enumerate(items):
        # the bounds test mirrors the usual `if (slot < total)` guard in a real kernel
        if flags[i] and offsets[i] < total:
            out[offsets[i]] = transform(item)
    return out


STAGES = {
    "flags": Stage(dependencies=[], run=stage_flags, note="map, W=n S=1"),
    "scan": Stage(dependencies=["flags"], run=stage_scan, note="exclusive scan, W=n S=log n"),
    "total": Stage(dependencies=["flags"], run=stage_total, note="reduce; runs alongside scan"),
    "scatter": Stage(
        # BUG_UNDECLARED omits "total" while the stage still reads it
        dependencies=["flags", "scan"] + ([] if BUG_UNDECLARED else ["total"]),
        run=stage_scatter,
        note="independent writes, W=n S=1",
    ),
}


# ------------------------------------------------------------------ checks

INVARIANTS = [
    Invariant("count matches predicate", lambda inp, out: len(out) == sum(map(predicate, inp))),
    Invariant("no unwritten slot", lambda inp, out: all(v is not None for v in out)),
    Invariant("every output is a transformed survivor",
              lambda inp, out: set(out) <= {transform(x) for x in inp if predicate(x)}),
]

METAMORPHIC = [
    Metamorphic(
        "duplicating input duplicates output",
        transform=lambda inp, rng: inp + inp,
        relation=lambda base, doubled: doubled == base + base,
    ),
    Metamorphic(
        "prepending a filtered-out element leaves output unchanged",
        transform=lambda inp, rng: [0] + inp,
        relation=lambda base, extended: extended == base,
    ),
]


def build(rng: random.Random, n: int) -> list[int]:
    return [rng.randint(-50, 50) for _ in range(n)]


GENERATORS = standard_generators(build, max_size=1024) + [
    Generator("all_pass", lambda r: [1] * r.randint(0, 300)),
    Generator("all_fail", lambda r: [3] * r.randint(0, 300)),
    Generator("duplicate_heavy", lambda r: [r.choice([1, 2, 3]) for _ in range(r.randint(0, 300))]),
    Generator("sorted", lambda r: sorted(build(r, r.randint(0, 300)))),
    Generator("reverse_sorted", lambda r: sorted(build(r, r.randint(0, 300)), reverse=True)),
    Generator("extremal", lambda r: [-(2**31), 2**31 - 1, 0, 1, -1]),
    Generator("skewed", lambda r: [r.choice([0] * 9 + [7]) for _ in range(500)]),
]

SPEC = Spec(
    cpu_reference=cpu_reference,
    stages=STAGES,
    output_stage="scatter",
    generators=GENERATORS,
    mode="exact",
    invariants=INVARIANTS,
    metamorphic=METAMORPHIC,
    orders_per_input=6,
)

if __name__ == "__main__":
    raise SystemExit(main(SPEC))
