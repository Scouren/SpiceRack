"""Differential-testing and dependency-validation harness for stage DAGs.

Standard library only. This finds bugs; it proves nothing.

Use: declare the parallel formulation as named stages with explicit dependencies, supply the CPU
reference, and run the suite. The simulator hands each stage a view containing only its declared
dependencies, so an undeclared read raises instead of silently passing. Independent stages are
then executed in many randomized legal orders to catch stages that share a buffer and quietly
assume an order.

See verification.md for the workflow and example_compaction.py for a complete run.
"""

from __future__ import annotations

import argparse
import json
import math
import random
import subprocess
import sys
import traceback
from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any, Callable, Iterable, Sequence

# --------------------------------------------------------------------------- errors


class DagError(Exception):
    """Malformed stage DAG: cycle, duplicate name, or dangling dependency."""


class UndeclaredDependency(KeyError):
    """A stage read a value it did not declare as a dependency."""


# --------------------------------------------------------------------------- stages


@dataclass
class Stage:
    """One stage of the parallel formulation.

    run(ctx) -> value stored under this stage's name.
    ctx["input"] is the problem input; ctx[dep] is each declared dependency's output.
    """

    dependencies: list[str] = field(default_factory=list)
    run: Callable[[Mapping[str, Any]], Any] | None = None
    note: str = ""


class StageView(Mapping):
    """Read-only ctx that refuses anything the stage did not declare."""

    def __init__(self, state: dict[str, Any], allowed: Iterable[str], stage_name: str):
        self._state = state
        self._allowed = set(allowed) | {"input"}
        self._stage = stage_name

    def __getitem__(self, key: str) -> Any:
        if key not in self._allowed:
            raise UndeclaredDependency(
                f"stage {self._stage!r} read {key!r}, which is not in its dependencies "
                f"{sorted(self._allowed - {'input'})}"
            )
        return self._state[key]

    def __iter__(self):
        return iter(k for k in self._allowed if k in self._state)

    def __len__(self) -> int:
        return sum(1 for _ in self)


def normalize(stages: Any) -> dict[str, Stage]:
    """Accept a dict or a sequence of (name, Stage) pairs; reject duplicate names."""
    if isinstance(stages, Mapping):
        return dict(stages)
    seen: dict[str, Stage] = {}
    for name, stage in stages:
        if name in seen:
            raise DagError(f"duplicate stage name {name!r}")
        seen[name] = stage
    return seen


# --------------------------------------------------------------------------- topology


def validate_dag(stages: Any) -> dict[str, Stage]:
    """Check for dangling dependencies, self-edges, and cycles. Returns the normalized dict."""
    st = normalize(stages)
    for name, stage in st.items():
        for dep in stage.dependencies:
            if dep not in st:
                raise DagError(f"stage {name!r} depends on unknown stage {dep!r}")
            if dep == name:
                raise DagError(f"stage {name!r} depends on itself")
    remaining = {n: set(s.dependencies) for n, s in st.items()}
    order: list[str] = []
    while remaining:
        ready = sorted(n for n, deps in remaining.items() if not deps)
        if not ready:
            raise DagError(f"cycle among stages: {_find_cycle(remaining)}")
        for n in ready:
            del remaining[n]
            order.append(n)
        for deps in remaining.values():
            deps.difference_update(ready)
    return st


def _find_cycle(remaining: dict[str, set[str]]) -> str:
    """Report one concrete cycle from the stuck set, for a readable error."""
    start = sorted(remaining)[0]
    path = [start]
    seen = {start}
    node = start
    while True:
        nxt = next((d for d in sorted(remaining[node]) if d in remaining), None)
        if nxt is None:
            return " -> ".join(path) + " (unresolvable: " + ", ".join(sorted(remaining)) + ")"
        if nxt in seen:
            return " -> ".join(path + [nxt])
        path.append(nxt)
        seen.add(nxt)
        node = nxt


def topological_order(stages: Any) -> list[str]:
    """One deterministic legal order."""
    st = validate_dag(stages)
    remaining = {n: set(s.dependencies) for n, s in st.items()}
    order: list[str] = []
    while remaining:
        ready = sorted(n for n, deps in remaining.items() if not deps)
        n = ready[0]
        del remaining[n]
        order.append(n)
        for deps in remaining.values():
            deps.discard(n)
    return order


def random_topological_order(stages: Any, rng: random.Random) -> list[str]:
    """A uniformly-chosen-next-ready legal order."""
    st = validate_dag(stages)
    remaining = {n: set(s.dependencies) for n, s in st.items()}
    order: list[str] = []
    while remaining:
        ready = sorted(n for n, deps in remaining.items() if not deps)
        n = rng.choice(ready)
        del remaining[n]
        order.append(n)
        for deps in remaining.values():
            deps.discard(n)
    return order


def simulate(stages: Any, problem_input: Any, order: Sequence[str]) -> dict[str, Any]:
    """Execute stages in the given order, each seeing only its declared dependencies."""
    st = normalize(stages)
    state: dict[str, Any] = {"input": problem_input}
    for name in order:
        stage = st[name]
        if stage.run is None:
            raise DagError(f"stage {name!r} has no run function")
        state[name] = stage.run(StageView(state, stage.dependencies, name))
    return state


# --------------------------------------------------------------------------- comparison


def compare_exact(a: Any, b: Any) -> bool:
    return a == b


def compare_unordered(a: Any, b: Any) -> bool:
    """Collection equality ignoring order (and ignoring multiplicity only if you sort-dedupe)."""
    try:
        return sorted(a) == sorted(b)
    except TypeError:
        return sorted(map(repr, a)) == sorted(map(repr, b))


def approx(abs_tol: float = 0.0, rel_tol: float = 1e-6) -> Callable[[Any, Any], bool]:
    """Elementwise float comparator over scalars and nested sequences."""

    def cmp(a: Any, b: Any) -> bool:
        if isinstance(a, (int, float)) and isinstance(b, (int, float)):
            return math.isclose(a, b, abs_tol=abs_tol, rel_tol=rel_tol)
        if isinstance(a, (list, tuple)) and isinstance(b, (list, tuple)):
            return len(a) == len(b) and all(cmp(x, y) for x, y in zip(a, b))
        return a == b

    return cmp


COMPARATORS = {"exact": compare_exact, "unordered": compare_unordered, "approx": approx()}


def resolve_comparator(mode: Any) -> tuple[str, Callable[[Any, Any], bool]]:
    if callable(mode):
        return getattr(mode, "__name__", "custom"), mode
    if mode in COMPARATORS:
        return mode, COMPARATORS[mode]
    raise ValueError(f"unknown comparator mode {mode!r}")


# --------------------------------------------------------------------------- inputs


@dataclass
class Generator:
    """Named input generator. rng-driven ones are reproducible from the seed."""

    name: str
    make: Callable[[random.Random], Any]


def boundary_sizes(max_size: int = 4096) -> list[int]:
    """Sizes that sit on warp, block, tile, and power-of-two edges, plus their neighbours."""
    seeds = [0, 1, 2, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 1023, 1024, 1025]
    out = {s for s in seeds if s <= max_size}
    out.update({max_size - 1, max_size})
    return sorted(out)


def standard_generators(
    make: Callable[[random.Random, int], Any], max_size: int = 1024
) -> list[Generator]:
    """Wrap a size-parameterized builder into the adversarial family from the skill.

    `make(rng, n)` returns an input of length n using rng for values.
    """
    gens = [
        Generator(f"size_{n}", lambda r, n=n: make(r, n)) for n in boundary_sizes(max_size)
    ]
    gens.append(Generator("random", lambda r: make(r, r.randint(0, max_size))))
    return gens


# --------------------------------------------------------------------------- spec


@dataclass
class Invariant:
    name: str
    check: Callable[[Any, Any], bool]  # (problem_input, output) -> bool


@dataclass
class Metamorphic:
    """Known input→output relationship, e.g. permuting independent inputs."""

    name: str
    transform: Callable[[Any, random.Random], Any]  # input -> transformed input
    relation: Callable[[Any, Any], bool]  # (output_of_original, output_of_transformed) -> bool


@dataclass
class Spec:
    cpu_reference: Callable[[Any], Any]
    stages: Any
    output_stage: str
    generators: list[Generator]
    mode: Any = "exact"
    invariants: list[Invariant] = field(default_factory=list)
    metamorphic: list[Metamorphic] = field(default_factory=list)
    gpu_command: list[str] | None = None  # subprocess adapter; JSON in, JSON out
    orders_per_input: int = 8


@dataclass
class Failure:
    kind: str
    generator: str
    seed: int
    problem_input: Any
    expected: Any = None
    actual: Any = None
    order: Sequence[str] | None = None
    mode: str = ""
    detail: str = ""

    def report(self, argv0: str) -> str:
        def clip(x: Any, n: int = 400) -> str:
            s = repr(x)
            return s if len(s) <= n else s[:n] + f"... ({len(s)} chars)"

        lines = [
            f"FAIL [{self.kind}] generator={self.generator} seed={self.seed}",
            f"  input    : {clip(self.problem_input)}",
            f"  expected : {clip(self.expected)}",
            f"  actual   : {clip(self.actual)}",
            f"  order    : {list(self.order) if self.order else '-'}",
            f"  mode     : {self.mode}",
        ]
        if self.detail:
            lines.append(f"  detail   : {self.detail}")
        lines.append(
            f"  reproduce: python {argv0} --seed {self.seed} --only {self.generator}"
        )
        return "\n".join(lines)


# --------------------------------------------------------------------------- gpu adapter


def call_gpu(command: list[str], problem_input: Any) -> Any:
    """Send {"input": ...} on stdin, expect {"output": ...} on stdout."""
    proc = subprocess.run(
        command,
        input=json.dumps({"input": problem_input}),
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(proc.stdout)["output"]


# --------------------------------------------------------------------------- suite


def run_suite(
    spec: Spec,
    seeds: Sequence[int] = (0, 1, 2, 3, 4),
    only: str | None = None,
    verbose: bool = False,
) -> list[Failure]:
    """Validate the DAG, then differential-test every generator against every seed."""
    validate_dag(spec.stages)
    mode_name, cmp = resolve_comparator(spec.mode)
    failures: list[Failure] = []
    checked = 0

    for seed in seeds:
        for gen in spec.generators:
            if only and only != gen.name:
                continue
            rng = random.Random((seed, gen.name).__hash__() & 0xFFFFFFFF)
            problem_input = gen.make(rng)
            expected = spec.cpu_reference(problem_input)

            orders = [topological_order(spec.stages)]
            order_rng = random.Random(seed * 7919 + 13)
            for _ in range(max(0, spec.orders_per_input - 1)):
                orders.append(random_topological_order(spec.stages, order_rng))

            broken = False
            for order in orders:
                checked += 1
                try:
                    actual = simulate(spec.stages, problem_input, order)[spec.output_stage]
                except UndeclaredDependency as exc:
                    failures.append(
                        Failure("undeclared-dependency", gen.name, seed, problem_input,
                                order=order, mode=mode_name, detail=str(exc))
                    )
                    broken = True
                    break
                except Exception:  # noqa: BLE001 — surface any stage crash with context
                    failures.append(
                        Failure("stage-exception", gen.name, seed, problem_input,
                                order=order, mode=mode_name, detail=traceback.format_exc(limit=3))
                    )
                    broken = True
                    break

                if not cmp(expected, actual):
                    failures.append(
                        Failure("differential", gen.name, seed, problem_input, expected,
                                actual, order, mode_name)
                    )
                    broken = True
                    break

                for inv in spec.invariants:
                    if not inv.check(problem_input, actual):
                        failures.append(
                            Failure("invariant", gen.name, seed, problem_input, expected,
                                    actual, order, mode_name, detail=inv.name)
                        )

            if spec.gpu_command and not broken:
                try:
                    gpu_out = call_gpu(spec.gpu_command, problem_input)
                except Exception:  # noqa: BLE001
                    failures.append(
                        Failure("gpu-adapter", gen.name, seed, problem_input,
                                mode=mode_name, detail=traceback.format_exc(limit=3))
                    )
                else:
                    if not cmp(expected, gpu_out):
                        failures.append(
                            Failure("differential-gpu", gen.name, seed, problem_input,
                                    expected, gpu_out, None, mode_name)
                        )

            for mm in (() if broken else spec.metamorphic):
                mm_rng = random.Random(seed * 104729 + len(mm.name))
                other = mm.transform(problem_input, mm_rng)
                try:
                    base = simulate(spec.stages, problem_input, orders[0])[spec.output_stage]
                    trans = simulate(spec.stages, other, orders[0])[spec.output_stage]
                except Exception:  # noqa: BLE001
                    failures.append(
                        Failure("metamorphic-exception", gen.name, seed, other, order=orders[0],
                                mode=mode_name,
                                detail=f"{mm.name}: {traceback.format_exc(limit=3)}")
                    )
                    continue
                if not mm.relation(base, trans):
                    failures.append(
                        Failure("metamorphic", gen.name, seed, problem_input, base, trans,
                                orders[0], mode_name, detail=f"{mm.name} on {other!r}")
                    )

    if verbose:
        print(f"{checked} schedule executions across {len(seeds)} seeds", file=sys.stderr)
    return failures


def main(spec: Spec, argv: Sequence[str] | None = None) -> int:
    """CLI wrapper: --seed, --seeds, --only, --gpu-cmd, -v. Returns a shell exit code."""
    p = argparse.ArgumentParser(description="stage-DAG differential test")
    p.add_argument("--seed", type=int, action="append", help="repeatable; pins exact seeds")
    p.add_argument("--seeds", type=int, default=5, dest="n_seeds",
                   help="number of seeds when --seed is unused")
    p.add_argument("--only", help="run a single named generator")
    p.add_argument("--gpu-cmd", nargs=argparse.REMAINDER, help="subprocess GPU adapter command")
    p.add_argument("-v", "--verbose", action="store_true")
    args = p.parse_args(argv)

    if args.gpu_cmd:
        spec.gpu_command = args.gpu_cmd
    seeds = args.seed if args.seed else list(range(args.n_seeds))

    try:
        failures = run_suite(spec, seeds=seeds, only=args.only, verbose=args.verbose)
    except DagError as exc:
        print(f"DAG INVALID: {exc}")
        return 2

    if not failures:
        print(f"ok — no counterexample found over seeds {seeds} "
              f"(randomized testing, not a proof)")
        return 0
    for f in failures[:10]:
        print(f.report(sys.argv[0]))
        print()
    print(f"{len(failures)} failure(s); showing at most 10")
    return 1


if __name__ == "__main__":
    print(__doc__)
    print("This module is a library. Run example_compaction.py for a working suite.")
