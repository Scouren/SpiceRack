---
name: spice-engine
description: SPICE engine internals — MNA stamping, sparse LU replay, Newton with limiting, transient step control, device derivatives, and oracle testing against ngspice or Xyce. Use when building or modifying a circuit simulator's matrix stamp, linear solver, Newton loop or convergence test, integrator or timestep control, or a device model's evaluation; or when an engine's result disagrees with ngspice.
---

A SPICE engine is a **replay** machine. Topology fixes the matrix pattern once; every Newton iteration, timestep and frequency point replays new values through tapes built from that pattern. Nothing on the per-iteration path searches, allocates or computes an index. Every answer is judged against an **oracle**: an analytic solution, ngspice, or Xyce, named per test.

```
netlist ─► unknowns + CSC pattern + scatter tapes          once per topology
        ─► ordering, full factor (pivot tape)               once per pattern; again on pivot failure
        ─► per iteration: limit + device eval (dual numbers) ─► scatter ─► refactor replay ─► solve ─► convergence test
        ─► per timestep: companion coefficients, LTE bound, breakpoints
        ─► per frequency point: LaneLu lanes replaying one pivot tape
```

## Tolerances and limits (ngspice defaults)

Probed from ngspice 44.2 (`option` in a `.control` block) and matching the ngspice 44 manual §11.1.2 and §11.1.4. Port these values, not textbook ones.

| Option | Default | Floors / bounds |
|---|---|---|
| `reltol` | 1e-3 | relative part of every acceptance test |
| `vntol` | 1e-6 V | node-voltage Newton test |
| `abstol` | 1e-12 A | branch/device-current Newton test, LTE current floor |
| `chgtol` | 1e-14 C | charge floor in the LTE test |
| `trtol` | 7 | LTE deflation factor |
| `gmin` | 1e-12 S | device junction shunt |
| `pivrel` / `pivtol` | 1e-3 / 1e-13 | threshold pivoting |
| `itl1` / `itl2` / `itl4` | 100 / 50 / 10 | DC, DC-sweep and homotopy-rung, transient-point Newton budgets |
| `gminsteps` / `srcsteps` | 1 / 1 | 1 selects the adaptive (Gillespie) ladders |
| `method` / `maxord` / `xmu` | trap / 2 / 0.5 | integrator |

## Steps

### 1. Freeze the pattern

Unknowns are the non-ground node voltages plus one branch current per voltage-defined element (V, L, E, H), the MNA block system `[Y B; C D][V; I] = [J; E]` (Nagel 1975, eq. 3.28, after Ho, Ruehli & Brennan). The pattern is the union of every instance's structural entries. Build three u32 tapes per instance at freeze time: `gath` (unknown per terminal), `rhs_idx` (residual row per terminal, ground → a trash row `n`) and `slots` (CSC position per structural entry, ground → a trash slot `nnz`). A stamp is then `vals[slots[k]] += g`, with no branch on ground. Size the tapes and choose their element types with the `data-oriented-design` skill.

Done when: no device stamp computes an index or tests for ground at eval time; the pattern and tapes are immutable after freeze, and a topology change re-runs this step; the slot-tape test in [`numerics.zig`](numerics.zig) passes. Tape layout, the trash cells and the ESPice ABI: [`LINEAR.md`](LINEAR.md).

### 2. Factor once, replay forever

Split the LU into three phases with different lifetimes: **ordering** (BTF + AMD, pattern only), **full factor** (threshold partial pivoting that keeps the diagonal while `|a_kk| ≥ τ·max|column|`, τ = 1e-3; it records the L/U patterns, the pivot sequence and the per-column topological order) and **refactor** (numeric replay of that tape: no DFS, no pivot search). A replay whose pivot collapses against its column fails, the caller reruns the full factor on the same values, and only that failure reaches the Newton caller as singular. Values identical to the last factor skip the factor entirely.

Independent matrices that share a pattern (frequency points) replay one pivot tape in SIMD lanes; read the `LaneLu` section of [`LINEAR.md`](LINEAR.md) before vectorizing any factorization, and the `zig` skill's `simd-first.md` for the lane-axis rule.

Done when: refactor allocates nothing and searches nothing; a failed replay falls back to a full factor before reporting singular; every lane kernel's `W == 1` instantiation is the scalar oracle and a test proves lane `l` equals the scalar replay of lane `l`.

### 3. Newton: limit, then test

One iteration, in ngspice's order (`niiter.c`, `niconv.c`): load every device, each limiting its controlling voltages against the previous iterate (`pnjlim` for junctions, `fetlim`/`limvds` for MOS gate and drain); a limiting event marks the iteration non-converged. Factor, solve. Then, only if nothing was limited and this is not iteration 1, run the convergence test: every node within `reltol·max(|new|,|old|) + vntol` (`abstol` on current rows), and every device's linearized current within the same form on `abstol`. Diagonal `gmin` loads into the factor. When plain Newton fails `itl1`, climb the homotopy ladder: dynamic gmin stepping, then Gillespie source stepping, each rung's solve capped at `itl2`.

Done when: a limited device vetoes acceptance that iteration; iteration 1 is never accepted; one acceptance function serves every solver path (direct, Krylov, homotopy rungs); `pnjlim`, `fetlim` and the row test in [`numerics.zig`](numerics.zig) pass their hand-computed cases. Formulas, the ladder constants, and where the engine may diverge from ngspice: [`NUMERICS.md`](NUMERICS.md).

### 4. Transient: integrate charge, bound the LTE

Devices return charge `q(x)` and its Jacobian `C = ∂q/∂x`; the integrator differentiates `q`, never `C(v)·dv/dt`, so charge is conserved by construction. The Newton matrix is `G + α·C` with α from the method (BE `1/h`, trap `2/h` at `xmu = 0.5`). Start at BE, and drop to BE at every landed breakpoint; promote to order 2 when the order-2 bound exceeds `1.05·h`. After each converged step, re-read `q` at the solution and run ngspice's `CKTterr` per charge state: accept when the bound exceeds `0.9·h`, take the next step as `min(2h, bound)`; a Newton failure cuts `h` by 8 and drops to order 1. Steps clamp to land exactly on source breakpoints.

Pick trap for accuracy and Gear-2 where trap's point-to-point ringing shows; `xmu` slightly below 0.5 damps trap.

Done when: the charge-conservation test in [`numerics.zig`](numerics.zig) passes against the engine's own companion; the LTE runs per charge state, not per summed node; accepted timepoints match ngspice's grid on a linear RC deck. Formulas and rules with their `dctran.c` lines: [`NUMERICS.md`](NUMERICS.md).

### 5. Device derivatives: dual numbers

A device evaluates once over forward-mode dual numbers: the value half is the residual, the derivative lanes are its Jacobian row, one lane per terminal unknown the expression reads. The value half stays `f64` whatever width the derivative half uses, so the residual never changes with it. No hand-written derivative survives next to the model.

Done when: every device's dual Jacobian matches a central finite difference in a test (the `test-plan` skill's analytic-vs-FD certificate); a constant partial is stamped once, outside the per-iteration path. The minimal `Dual(N)` with its test is in [`numerics.zig`](numerics.zig).

### 6. Make it deterministic

Same deck, same bytes, every run. Floating-point sums reassociate only in one fixed order: fixed lane accumulators and one fixed combine order, so every caller rounds the same way. Parallel stamping sums per-worker slabs in a fixed worker order, which is deterministic per worker count but regroups the serial sum (ESPice measured at most 1 ulp against serial; unverified here). Bit-identity across worker counts needs what GPU stamping already does: per-contribution staging cells reduced in tape order, never atomics. Where the goal is bit-for-bit agreement with ngspice (LTE, integrator coefficients, any sum ngspice computes in index order), write a plain index-order loop and say why in a comment; vector reassociation there is a divergence. Assert the invariants that keep this true with the `assertions` skill.

Done when: two runs at the same worker count produce byte-identical output, and any worker-count dependence is either removed or measured and written down; every reassociating reduction goes through one helper; every ngspice-parity sum is a plain loop carrying a comment.

### 7. Test against an oracle

Every fixture deck carries its oracle beside it: an analytic derivation, or the exact ngspice/Xyce command and version that produced the reference, plus per-column `rtol`/`atol`. A deck whose feature is missing says so in its header, `* KNOWN GAP: <cause>`, and runs as an expected failure; when it starts passing the run fails until the marker is removed.

Done when: every deck has a named oracle and tolerances; every expected failure carries a `KNOWN GAP` line naming its cause; no deck on the pass list fails. Oracle production, the fixture schema, transient-grid comparison and Xyce: [`TESTING.md`](TESTING.md); test layering beyond the corpus: the `test-plan` skill.

## What is not a lane

Transient timesteps (sequential in t), Newton iterations, harmonic-balance harmonics (coupled through the nonlinearity) and PSS shooting. Lanes are frequency points, sweep points and Monte Carlo samples; derivatives are dual lanes; instances are threads.

## Sources

Nagel, *SPICE2: A Computer Program to Simulate Semiconductor Circuits*, UCB ERL-M520, 1975 (scanned PDF, pages read: §3.7, §5.6–5.8, §6.7–6.10). ngspice 44 manual §1.4, §11.1; ngspice 44.2 binary, probed; ngspice source at `git.code.sf.net/p/ngspice/ngspice` HEAD 940bb50 (`niiter.c`, `niconv.c`, `devsup.c`, `cktterr.c`, `ckttrunc.c`, `dctran.c`, `nicomcof.c`, `cktop.c`, `dioconv.c`, `macros.h`). Kundert & Clifford, *Achieving Accurate Results With a Circuit Simulator*, 1993 (kenkundert.com). Hoekstra et al., *Xyce Mathematical Formulation*, SAND2004-2283. ESPice `AGENTS.md`, `dev/solvers/`, `dev/analysis/transient-integration.md`, `src/solver/lane_lu.zig`. Ho, Ruehli & Brennan 1975 (IEEE TCAS) and Kundert's *Designer's Guide to SPICE and Spectre* were not read (paywalled); their claims here come through Nagel §3.7 and the 1993 slides.
