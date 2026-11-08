# Domains

Each section: bug classes that matter, and the stack in priority order. A project usually matches several; merge their stacks.

## Compiler / translator / code generator
Frontends, IR passes, codegen, DSL → code emitters, transpilers.
- **Bug classes:** frontend crash on malformed input; accepting invalid programs / rejecting valid ones; miscompilation (wrong semantics, silent); optimization changes behavior; nondeterministic output; spec-clause gaps.
- **Stack:**
  1. Asserts on IR invariants after every pass (a verifier pass run in test builds).
  2. Spec conformance tests: one test per spec clause, positive + negative; track clause → test map.
  3. Fuzz the frontend (lexer → parser → semantic analysis) with bytes, then grammar-aware generation. Oracle: no crash; every rejection has a diagnostic.
  4. Program generator (Csmith/YARPGen style) producing valid, UB-free programs → compile → run → differential against a reference compiler/interpreter, or between optimization levels.
  5. EMI / metamorphic: mutate unexecuted code, insert identity operations → output must not change.
  6. Round-trip: parse → print → parse yields the same AST.
  7. Golden tests for diagnostics and emitted code; determinism check (compile twice, byte-compare; Zig CI checks stage3/stage4 byte-identity).
  8. Swarm testing in the generator: randomly disable language features per run (42% more distinct C compiler crashes than one tuned config).
  9. Conformance tracking as a gate: per-test PASS/FAIL list checked in; CI fails on any PASS→FAIL.
  10. Translation validation where feasible (check each pass's output against its input; Alive2 model).
- **Exemplars:** Zig `test/` (behavior tests run on every backend), rustc compiletest UI tests + crater, LLVM lit/FileCheck + Alive2, Cranelift/wasmtime differential fuzzing, GCC torture suite, CompCert.

## Stateful systems
Databases, simulators with state over time, schedulers, DAG executors, caches, anything with concurrency or crash recovery.
- **Bug classes:** illegal state transitions; ordering/interleaving races; crash-recovery corruption; resource leaks; liveness (hangs); nondeterministic results under parallelism.
- **Stack:**
  1. All time, RNG, IO and scheduling behind injectable interfaces (needed for everything below).
  2. Invariants asserted after each transition.
  3. Model-based property tests: random operation sequences vs a simple sequential model.
  4. Deterministic simulation: seeded schedule + faults, swarm-enabled fault types, replay by seed; run many seeds nightly.
  5. Exhaustive interleavings for small primitives (`loom`, or bounded enumeration).
  6. Parallel == sequential: run with jobs=1 and jobs=N, compare outputs bit-for-bit (or justified tolerance).
  7. TSan / Zig thread safety checks on the concurrent path.
- **Exemplars:** FoundationDB simulation, TigerBeetle VOPR + TigerStyle, Jepsen (for distributed correctness claims).

## Numerical solvers / simulators
Linear/nonlinear solvers, ODE/DAE integrators, SPICE-like circuit simulators, optimizers, LP/SAT/constraint solvers.
- **Bug classes:** wrong answer within plausible range (silent); convergence failure; ill-conditioning sensitivity; NaN/Inf propagation; tolerance too loose to catch regressions; platform-dependent results.
- **Stack:**
  1. Asserts: finite checks at boundaries, residual/norm checks after each solve.
  2. Certificate checks: verifying an answer is cheaper than finding it. Use **scaled residual ratios**, not stored answers: LAPACK tests compute e.g. ‖LU − A‖/(n‖A‖ε) on generated matrices of controlled condition number and fail above a threshold (LAWN 41 uses 30), so the oracle scales with conditioning. Also KKT/duality gap, constraint satisfaction, SAT model check / DRAT proof check, conservation laws (KCL/KVL at every node, evaluated with an independent simple device evaluator), and analytic Jacobian vs finite-difference for every device model / generated derivative.
  3. Analytic cases: closed-form solutions (RC step, RLC resonance, Method of Manufactured Solutions for PDEs/DAEs: pick the solution, derive the source term, check convergence *order*, not just error).
  4. Differential against reference tools (ngspice, Xyce, SciPy, LAPACK) on a fixture corpus, tolerance per quantity: abs + rel, justified.
  5. Metamorphic: scale units, permute node/variable order, add zero elements, split time steps, transpose problem → predictable change.
  6. Property tests over generated well-posed problems (random SPD matrices, random small circuits with known DC solution).
  7. Fuzz the input parser/netlist frontend separately (compiler domain).
  8. Convergence-order and tolerance-tightening tests: halve the step → error drops by the expected factor.
- **Tolerance discipline:** state ULP or rel/abs per quantity; compare with `std.math.approxEqRel`/`approxEqAbs` (Zig) or `approx`/`float-cmp` (Rust); never one global epsilon.

## GPU kernels / multi-backend compute
- **Bug classes:** CPU and GPU disagree; races and missing barriers; out-of-bounds on tail blocks; uninitialized shared memory; backend-specific codegen bugs; nondeterministic reductions.
- **Stack:**
  1. CPU reference = oracle. Same kernel source on CPU is the cheapest differential.
  2. Differential CPU vs each GPU backend over generated inputs, sizes chosen to hit the edges: 0, 1, block−1, block, block+1, non-power-of-two, large.
  3. Tolerance: bitwise for integer ops; ULP-bounded for float, wider for reductions (order changes the sum). Or make reductions deterministic.
  4. `compute-sanitizer` (memcheck, racecheck, initcheck, synccheck) on CUDA; equivalent ROCm tooling for HIP.
  5. Codegen golden tests (emitted PTX/asm) for the abstraction's zero-cost claim; review diffs.
  6. Run GPU tiers on real hardware in nightly if merge CI lacks GPUs; CPU backend at merge. A backend that has never run on hardware is untested, whatever its asm checks say.
  7. Atomics/memory ordering: asm checks prove the instruction is *present*, not that the algorithm is correct; add stress tests on hardware.

## SIMD / vectorized code
- **Bug classes:** tail handling; alignment; lane-crossing mistakes; wraparound/overflow differences vs scalar; target-specific paths (SSE2/AVX2/AVX-512/NEON) untested on CI machine.
- **Stack:**
  1. Scalar oracle for every vector routine.
  2. Exhaustive lengths 0..(4 × max vector width), all start alignments/offsets, every element type.
  3. Property/fuzz with random data at random lengths vs scalar.
  4. Exhaustive values for 8/16-bit lanes.
  5. Run on each target feature set: CI matrix with `-mcpu`, or Intel SDE / QEMU to emulate AVX-512 on machines without it.
  6. Benchmark gate in nightly (perf regression is a bug for a perf library).

## Geometry / EDA / physical verification
DRC, LVS, ERC, PEX, place-and-route, layout generators.
- **Bug classes:** missed violations (false negative: worst case, signs off a bad chip); false positives; coordinate/grid/rounding errors; edge cases (touching, collinear, zero-area, self-intersecting, huge coordinates); rules silently not run.
- **Stack:**
  1. Asserts: geometric invariants (polygon orientation, non-self-intersection after boolean ops, on-grid).
  2. Seeded-violation tests: generate a clean layout, inject one known violation, assert it's found exactly once at the right location. Every rule gets positive + negative + boundary (exactly at limit) cases.
  3. Differential against KLayout/Magic/Netgen on real PDK decks and designs.
  4. Metamorphic: translate, rotate 90°, mirror, scale by grid, reorder shapes, flatten vs hierarchical → same violations (transformed).
  5. Property tests on geometry kernels (boolean ops: area(A∪B) = area(A) + area(B) − area(A∩B)).
  6. Fuzz GDS/OASIS/deck parsers.
  7. Placement/routing: certificate check (output passes DRC/LVS), constraint satisfaction asserted, determinism by seed.

## Parsers / file formats / libraries with public API
- **Stack:** fuzz every parser; round-trip; differential against a reference parser; API example tests; doc tests (Rust `///` examples, Zig doctests); semver/API diff checks (`cargo semver-checks`).
