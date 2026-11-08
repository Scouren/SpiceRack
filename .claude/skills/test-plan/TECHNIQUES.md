# Techniques

Each entry: what it catches, what it misses, its oracle, cost, tooling. Tool status moves fast, so verify the Zig/Rust version in use before relying on a flag.

## Asserts and invariants
- **Catches:** corruption at the moment it happens, not three modules later. Multiplies every other technique: a fuzzer only sees crashes, and asserts turn wrong state into crashes.
- **Misses:** anything the invariant doesn't state.
- **Practice:** assert pre/postconditions and invariants densely (TigerStyle: ≥ 2 asserts per function; assert both the positive and negative space). Assert that loops terminate within bounds.
- **Zig:** `std.debug.assert`; Debug/ReleaseSafe trap on overflow, OOB, null, `unreachable`. **Rust:** `assert!`, `debug_assert!`; `overflow-checks = true` in test profiles.

## Example (unit / integration) tests
- **Catches:** behaviors you thought of; regressions of known bugs.
- **Misses:** everything you didn't think of.
- **Oracle:** hand-worked value from spec, paper or datasheet. Never recompute it the way the code does.
- **Practice:** test public seams, not internals. Every bug report → failing test first → fix.

## Coverage (line → branch → MC/DC → object-code branch)
- **Catches:** nothing directly. It's a *map of what never ran*.
- **Misses:** wrong outputs on paths that did run. 100% coverage with weak asserts finds nothing.
- **Levels:** line < branch < MC/DC (each condition independently flips the decision; DO-178C Level A, SQLite's standard). Object-code/machine-code branch coverage (Kelley's rung 3) includes compiler-generated branches such as bounds checks.
- **Practice:** use it to find untested code, then test, assert-unreachable, or delete. Gate on "does not drop" (ratchet), not on a target that invites assert-free filler tests.
- **Zig (0.17):** no built-in line/branch coverage and no machine-code branch or MC/DC tool. Line coverage: wrap the test binary in `kcov` via `zig test --test-cmd kcov --test-cmd <outdir> --test-cmd-bin`. The `-ffuzz` build has sancov edge counters, which only feed the fuzzer UI. Cheap substitute: **coverage marks** (TigerBeetle `src/testing/marks.zig`; Ferrous Systems "coverage marks"). Code logs "hit mark X", and one specific test asserts it hit X.
- **Rust:** `-C instrument-coverage` / `cargo llvm-cov` (stable, region coverage). Branch coverage is nightly-only (`-Zcoverage-options=branch`). MC/DC support was **removed** from rustc in Aug 2025 (rust#144999).
- **SQLite trick:** `ALWAYS()`/`NEVER()` macros mark defensive branches so coverage builds don't count them; `testcase()` macros force boundary values and bitmask bits to be hit.

## Fuzzing (coverage-guided)
- **Catches:** crashes, hangs, asserts, sanitizer reports, OOM on inputs no human writes. Best at parsers, decoders, frontends.
- **Misses:** silent wrong output, unless the harness checks a property or differential oracle.
- **Practice:** one target per untrusted-input boundary. Make it structure-aware once byte-level fuzzing plateaus (`arbitrary` crate, grammar generator). Seed with real fixtures. Check the corpus in, replay it at merge. Add dictionaries of keywords/tokens. Each crash → minimized → regression test.
- **Zig (0.17, same API since 0.16):** `std.testing.fuzz(ctx, testOne, .{ .corpus = &.{...} })`, where `testOne(ctx, s: *std.testing.Smith) !void` pulls structured values (`s.value(T)`, `s.valueRangeAtMost`, `s.slice`, `s.bytes`, `s.eos`). Run with `zig build test --fuzz` (web UI) or `--fuzz=1M` (bounded, CI); `-j` for multiprocess. Plain `zig build test` only replays the corpus plus one empty input, so the corpus is the regression suite; embed crash files with `@embedFile`. Leaks and error logs per input count as findings. **Limitation:** the fuzzer has no comparison feedback (no `trace_cmp` hooks; 0.17 notes: "no changes were made to the fuzzer itself", improvement deferred to a future cycle), so it struggles with magic values: write *generators* with Smith that build valid structure, rather than relying on byte mutation. For byte-level parsers, AFL++ via `kristoff-it/zig-afl-kit` is an option (check version compatibility).
- **Rust:** `cargo fuzz` (libFuzzer) + `arbitrary` for structured input; `cargo afl`; `bolero` unifies fuzz + property. Replay corpus: `cargo fuzz run <t> corpus/<t> -- -runs=0`.
- **Exemplar:** wasmtime `fuzz/`: differential against other engines, IR interpreter vs compiled code, incremental vs from-scratch compile; generators and oracles in a fuzzer-agnostic crate.

## Property-based testing
- **Catches:** edge cases across a generated input space; shrinks failures to a minimal case.
- **Misses:** whatever the property doesn't state; deep states random generation rarely reaches.
- **Oracle:** an invariant (round-trip, idempotence, commutativity, ordering, conservation, model agreement).
- **Stateful/model-based:** generate a sequence of operations; run on real system + simple model; compare after each step (QuickCheck state machines, `proptest-state-machine`).
- **Zig:** no std PBT library, but a fuzz test whose body checks properties *is* PBT with coverage guidance, using `Smith` as the generator. For deterministic runs in normal tests, a seeded `std.Random.DefaultPrng` loop that prints the seed on failure. Swarm-test APIs via comptime reflection (matklad, "Swarm Testing Data Structures"). **Rust:** `proptest` (+ state-machine module), `quickcheck`.

## Differential testing
- **Catches:** silent wrong output, the class fuzzing alone misses.
- **Misses:** bugs shared by both implementations (common spec misreading).
- **References:** another tool (ngspice, KLayout, GCC/Clang), a naive version in the same repo, scalar vs SIMD, CPU vs GPU, Debug vs ReleaseFast, old version vs new.
- **Practice:** a fuzzer generates the input, both sides run, compare with a justified tolerance. Csmith-style generators for compilers. Write the naive oracle *before* the fast version.

## Metamorphic testing
- **Catches:** wrong numerical/scientific output when no exact oracle exists.
- **Oracle:** a relation between runs: scale input → output scales; permute node order → same answer; split interval → same integral; add a zero-valued element → no change; EMI for compilers (mutate dead code → same behavior).

## Exhaustive testing
- **Catches:** everything in a small domain. Beats random when the domain fits: all 2^16 inputs, all lengths 0..2×width, all alignments, all small graphs up to N nodes.
- **Practice:** matklad "Generate All The Things": write the generator so the same code does exhaustive-for-small and random-for-large (TigerBeetle `src/testing/exhaustigen.zig`, `exhaustigen-rs`). All 2^32 f32 values is feasible for elementary functions (Dawson, "There are only four billion floats").

## Deterministic simulation testing (DST)
- **Catches:** concurrency, ordering, crash-recovery, network/disk fault bugs; replays from a seed.
- **Requirement:** all nondeterminism (time, RNG, IO, scheduling, threads) goes through an injectable interface. Design for this from day one; retrofitting is expensive.
- **Practice:** a seeded simulator runs the real code with fake time/IO, injects faults (swarm testing: randomly enable subsets of fault types per seed), and checks invariants against a model. Exemplars: FoundationDB, TigerBeetle VOPR. **Rust:** `madsim`, `turmoil`, `shuttle`, `loom` (exhaustive interleavings for small concurrent primitives).

## Fault injection
- **Zig (0.17):** `std.testing.allocator` (a `std.heap.SafeAllocator` since 0.17, replacing `DebugAllocator`: thread-safe, reports all leaks on deinit, panics on double free and on resize/free races), `std.testing.FailingAllocator`, `std.testing.checkAllAllocationFailures(backing, testFn, args)` (fails allocation #i for every i; reports `error.SwallowedOutOfMemoryError` if code hid the failure). `std.testing.io` for test IO. Wrap IO behind an interface to inject errors.
- **Template:** SQLite testing.html §3: OOM on Nth malloc, IO error after N ops followed by an integrity check, crash/power-loss simulation, compound faults.
- **Rust:** `failpoints`/`fail` crate; custom allocator; injectable IO traits.

## Sanitizers and dynamic checkers
- ASan, UBSan, TSan, MSan on C/FFI code and on Rust (`-Zsanitizer`, nightly). **Miri** for Rust `unsafe` (UB, aliasing, uninit). CUDA `compute-sanitizer` (memcheck, racecheck, initcheck, synccheck). Zig: Debug/ReleaseSafe safety checks; `-fsanitize-thread` (TSan); `-fsanitize-c` (UBSan for C code).

## Mutation testing
- **Catches:** weak tests: mutants that survive mean no test would notice that bug.
- **Rust:** `cargo mutants`. **C/C++:** `mull`. **Zig:** no tool exists; a script that flips one comparison/operator at a time and checks `zig build test` fails. SQLite mutates branch instructions in its assembly (testing.html §7.6). Run nightly or pre-release; it's slow.

## Lightweight formal methods
- Bounded model checking: **Kani** (Rust) proves properties for all inputs up to a bound, good for small pure functions and `unsafe` blocks.
- Translation validation: Alive2 (LLVM peephole correctness). Pattern for compilers: check each transformation's output equivalent to input on the given program.
- SMT-check rewrite rules (Cranelift ISLE / Crocus-style).

## Golden / snapshot
- **Catches:** unintended output change. **Misses:** a wrong output that got blessed. Review every snapshot diff. A golden file is only as good as its provenance: record where each expected value came from (spec, reference tool, hand derivation) and re-derive suspicious ones. **Rust:** `insta`. **Zig:** compare against files under `tests/` with an update flag in `build.zig` (the Zig linker tests do this since 0.17: objdump snapshots + running the artifact, regenerated with `-Dlink-snapshot-update`).

## Gating and flakiness
- Not Rocket Science Rule (Graydon Hoare): main always passes; the CI tests the merged result before it lands (merge queue / bors).
- Test sizes (Google): small = one process, no IO/sleep; medium = localhost; large = anything. Gate merges on small + medium.
- Flaky test: fix or quarantine the same day; track quarantine count; zero quarantined tests on core at release.
