---
name: test-plan
description: Build a release-grade test plan that catches bugs before developers or users do. Use when starting a project, preparing a release, asking how to test a compiler/simulator/solver/stateful system/GPU or SIMD kernel/library, choosing between fuzzing, coverage, property, differential or simulation testing, or setting up CI gates.
---

# Test Plan

Goal: **total bug elimination** at the source. Hipp's ladder (SQLite, Software Should Work 2026; restated by Kelley at Systems Distributed '26) ranks where bugs get found:

1. whatever developers happen to hit
2. whatever users happen to report
3. full machine-code branch coverage
4. fuzzing with properties checked
5. LLM review

Rungs 1–2 are luck. This skill moves discovery to 3–4. But SQLite's own history shows each "complete" rung being overtaken: after 100% MC/DC, fuzzers found bugs; after fuzzing, metamorphic query fuzzing found more; then LLMs found a stack overflow on a million-row input. Each time, the fix was a **new oracle or a new input distribution**, not more coverage. A path that runs and returns the wrong number passes both 3 and 4. So build the plan around oracles first, inputs second. Treat every bug from rungs 1, 2 or 5 as a sign the oracle set is incomplete. Each one gets a new oracle or generator feature, not just an example test. Any non-trivial change sends you back to the top of the ladder.

**Oracle** = the independent thing that says "this output is wrong". Every test layer names its oracle, or it isn't a test.

## Steps

### 1. Read the project

Read README, build file test steps, `tests/`, CI config, roadmap. Write down what the system takes in, what it emits, its state, its concurrency, and its targets (CPU/GPU/SIMD widths/OS).

**Done when:** you can state in one sentence each: the input language, the output, and what "correct" means for it.

### 2. Classify into domains

Match the project against every domain in [DOMAINS.md](DOMAINS.md). Most systems hit several, e.g. a simulator is compiler (netlist frontend) + numerical solver + GPU. Read every matching section.

**Done when:** every component of the project is mapped to at least one domain.

### 3. List the bug classes, assign an oracle to each

From the domain sections, list the bug classes that matter. For each, pick the cheapest oracle that sees it, using [TECHNIQUES.md](TECHNIQUES.md). Bug classes that crash are covered by asserts + sanitizers + fuzzing. Bug classes that are *silently wrong* need a real oracle:

| Oracle | Use when |
|---|---|
| Asserts / invariants in prod code | Always. Turns silent corruption into a crash a fuzzer can see |
| Reference implementation (differential) | Another tool, a naive version, the scalar path, the CPU path exists |
| Round-trip / inverse | Parse↔print, encode↔decode, serialize↔load |
| Certificate check | Solver output can be verified cheaper than found (scaled residual, KCL, feasibility, proof, Jacobian vs finite difference) |
| Metamorphic relation | No exact answer exists; known transformations must change output predictably |
| Model (state machine) | Stateful system; a simple in-memory model defines legal behavior |
| Golden / snapshot | Output is stable and human-reviewed once |
| Spec example | Hand-worked value from a standard, paper or datasheet |

**Done when:** every bug class has an oracle, and no oracle recomputes the answer the way the code under test does.

### 4. Build the stack, in order

Stop adding layers when the exit criteria (step 6) hold. Standard order, cheapest first:

1. **Asserts** in prod code: pre/postconditions, invariants. Safety checks on in test builds (Zig Debug/ReleaseSafe; Rust debug + `overflow-checks`).
2. **Example tests** at public seams: spec examples, every past bug as a regression test (`tdd` skill for the loop).
3. **Property / differential tests** with generated inputs and shrinking.
4. **Coverage-guided fuzz targets** at every untrusted-input boundary. Use the property and differential checks as fuzz oracles.
5. **Exhaustive tests** for every small domain (all u8/u16 values, all lengths 0..2×vector width, all enum combinations).
6. **Deterministic simulation** for stateful/concurrent systems: seeded RNG drives inputs, scheduling and injected faults; a failure replays from its seed.
7. **Fault injection**: every allocation fails once (`std.testing.checkAllAllocationFailures`), IO errors, timeouts.
8. **Coverage measurement** for finding gaps. Every uncovered branch is either tested, proven unreachable (assert), or deleted. Where no coverage tool exists (Zig branch coverage), use **coverage marks**: tests that assert specific code was hit.
9. **Non-functional checks**: benchmarks with regression thresholds, adversarial-size inputs (deep recursion, huge N), determinism (run twice, byte-compare; parallel vs sequential), every shipped target/ISA.
10. **Mutation testing** to check that the tests above can actually fail.

**Done when:** each bug class from step 3 is covered by a named layer, with a file path or harness name.

### 5. Gate it

Assign every layer to a tier. A slow gate gets bypassed, so keep fast tiers fast. A flaky test makes its gate meaningless, so fix or quarantine it the same day.

| Tier | Budget | Contents |
|---|---|---|
| Local / pre-commit | < 10 s | format, compile, fast unit + property (small iteration count) |
| Merge (required) | < 15 min | all example + property + differential, sanitizers, every target, fuzz corpus replay, short fuzz on changed targets, coverage must not drop |
| Nightly | hours | long fuzz, many simulation seeds, high-iteration properties, mutation testing, perf regression |
| Release | unbounded | release checklist (step 6), full target matrix, reference-tool comparison on the full fixture set |

First check that CI actually runs the tests at all. A suite that never runs automatically is not a gate. Rules: main always passes (test the merge result: merge queue). Every fuzz/simulation failure becomes a checked-in regression input. The fuzz corpus lives in the repo and is replayed at merge.

**Done when:** every layer has a tier, and the merge tier fits its budget.

### 6. Write exit criteria

Write measurable release criteria. Examples:

- branch coverage ≥ N% on core, with every exclusion justified in a comment
- each fuzz target: ≥ H CPU-hours with no new crash and coverage plateaued
- differential: 100% of fixtures within stated tolerance (ULP or rel/abs), with tolerance justified per quantity
- simulation: ≥ S seeds clean on the release commit
- mutation score ≥ M% on core modules
- zero open crash bugs; zero quarantined tests on core
- every backend/target the release claims has run the suite on real hardware or an emulator (a backend that never ran is untested, whatever its asm checks say)
- a written list of known gaps

**Done when:** every criterion is a number or a yes/no a script can check.

### 7. Write the plan

Write `TESTING.md` (or the repo's existing convention) with: domains, bug class → oracle → layer table, harness sketches, the gate table, and exit criteria. Wire the gates into the build (`zig build test`, `cargo test`, CI).

**Done when:** the plan exists in the repo and every gate in it runs from one command.

## References

- [TECHNIQUES.md](TECHNIQUES.md): each technique's catches/misses/oracle/cost, plus Zig and Rust tooling.
- [DOMAINS.md](DOMAINS.md): bug classes and recommended stacks per domain (compiler, stateful, numerical, GPU, SIMD, geometry/EDA, parser/library).
- [SOURCES.md](SOURCES.md): reading list and exemplar test suites to copy from.
