# Numerics: Newton, limiting, convergence, homotopy, integration, derivatives

Read this when writing or changing the Newton loop, a limiting function, the convergence test, the operating-point ladder, the integrator, timestep control, or device derivative code. Every formula is transcribed from the cited primary source; ngspice line numbers are from the source tree at `git.code.sf.net/p/ngspice/ngspice` HEAD 940bb50 (2026-10-16). The runnable versions are in [`numerics.zig`](numerics.zig) (`zig test numerics.zig`, Zig 0.17).

## The iteration contract

`maths/ni/niiter.c` runs, per iteration: `CKTload` (every device evaluates at the current iterate, limits its controlling voltages and stamps), factor, solve, then

```c
if ((ckt->CKTnoncon == 0) && (iterno != 1))   /* niiter.c:310 */
    ckt->CKTnoncon = NIconvTest(ckt);
else
    ckt->CKTnoncon = 1;
```

So a limiting event (a device increments `CKTnoncon` during load) and iteration 1 both force another iteration before the convergence test runs at all.

## Limiting

Limiting is SPICE's globalization: instead of a line search on ‖F‖, each device clamps the change of its own controlling voltages between iterations, then stamps the linearization at the limited voltage, so F stays consistent. The limited voltage replaces the solver's in that device's state.

### pnjlim (junctions)

`spicelib/devices/devsup.c:50`, `DEVpnjlim(vnew, vold, vt, vcrit, &icheck)`:

```
if vnew > vcrit and |vnew - vold| > 2·vt:
    if vold > 0:
        arg = (vnew - vold) / vt
        vnew = arg > 0 ? vold + vt·(2 + ln(arg - 2))
                       : vold - vt·(2 + ln(2 - arg))
    else:
        vnew = vt · ln(vnew / vt)
    icheck = 1
elif vnew < 0:
    floor = vold > 0 ? -vold - 1 : 2·vold - 1
    if vnew < floor: vnew = floor; icheck = 1
    else icheck = 0
else icheck = 0
```

`arg − 2` is positive whenever the first branch runs, since `|arg| > 2`. The critical voltage is Nagel's point of minimum radius of curvature, `vcrit = vt · ln(vt / (√2 · Is))` (Nagel 1975 eq. 5.23, p. 140; ngspice `dio/diotemp.c:206` computes the same with `CONSTroot2`). Nagel's §5.7 comparison on 18 bipolar circuits found Colon's critical-voltage method, the ancestor of this branch, the most reliable of the three limiters tried (Table 5.3: the only one to converge on all five TTL circuits). The `icheck` flag is what vetoes convergence: the diode load turns it into `CKTnoncon++` (`dio/dioload.c`).

`pnjlim` and `critVoltage` with hand-computed cases (forward and reverse branches, the strict `> 2·vt` boundary on binary-exact values, both reverse floors) are the first test in `numerics.zig`.

### fetlim (MOS gate drive)

`devsup.c:93`, `DEVfetlim(vnew, vold, vto)`, with `vtsthi = |2(vold − vto)| + 2`, `vtstlo = |vold − vto| + 1`, `vtox = vto + 3.5`, `delv = vnew − vold`:

| Region of `vold` | Step | Result |
|---|---|---|
| deep on, `vold ≥ vtox` | falling, `vnew ≥ vtox` | `vold − vtstlo` if `−delv > vtstlo` |
| deep on | falling, `vnew < vtox` | `max(vnew, vto + 2)` |
| deep on | rising | `vold + vtsthi` if `delv ≥ vtsthi` |
| middle, `vto ≤ vold < vtox` | falling | `max(vnew, vto − 0.5)` |
| middle | rising | `min(vnew, vto + 4)` |
| off, `vold < vto` | falling | `vold − vtsthi` if `−delv > vtsthi` |
| off | rising, `vnew ≤ vto + 0.5` | `vold + vtstlo` if `delv > vtstlo` |
| off | rising, `vnew > vto + 0.5` | `vto + 0.5` |

It returns only the voltage, no flag; the MOS load decides what counts as limited. Tested in `numerics.zig`.

### limvds (MOS drain-source)

`devsup.c:21`: for `vold ≥ 3.5`, rising is capped at `min(vnew, 3·vold + 2)` and falling below 3.5 is floored at 2; for `vold < 3.5`, rising is capped at 4 and falling floored at −0.5.

## Convergence test

**Nodes** (`maths/ni/niconv.c:40-79`): for every unknown, with `tol = reltol·max(|old|, |new|) + atol`, `atol = vntol` on a voltage row (`SP_VOLTAGE`) and `abstol` otherwise, fail if `|new − old| > tol`; a NaN fails at once. ngspice 44 manual §1.4.1 eq. 1.7 states the voltage half. `rowConverged` in `numerics.zig` transcribes it, strict `>` included.

**Devices** (`niconv.c:83`, `CKTconvTest`): `NEWCONV` is defined (`include/ngspice/macros.h:19`), so every device's convergence routine runs too. The diode's (`dio/dioconv.c:79`) predicts the current from the last linearization, `î = i + g·Δv`, and fails if `|î − i| > reltol·max(|î|, |i|) + abstol` (manual §1.4.2 eq. 1.9).

**What it does not check.** Neither test reads the KCL residual. Kundert & Clifford (1993, slide 20) note SPICE "never actually verifies that KCL is satisfied" and can falsely converge; Spectre adds `|f_n(v)| < reltol·max_j |f_nj(v)| + abstol`. A residual gate is a legitimate engine choice and a divergence from ngspice: record it as one. ESPice adds a row-scaled residual gate on DC solves only and refuses any iterate with `|x| > 1e9` (`dev/analysis/tolerance-system.md`).

**Damping.** If the engine damps a step, scale the whole Δx vector; clamping components independently changes the Newton direction (ESPice `dev/solvers/newton-raphson-convergence.md`). ESPice found a monotone-‖F‖ backtracking safeguard double-limits junction settling, turning 3-iteration settles into 50 (same doc; unverified here).

## Operating-point ladder

ngspice `CKTop` (`spicelib/analysis/cktop.c:45-100`): plain Newton at `itl1` → gmin stepping (`gminsteps == 1`: `dynamic_gmin`, then `new_gmin` if that fails and `dyngmin` is unset; `> 1`: the fixed `spice3_gmin`) → source stepping (`srcsteps == 1`: `gillespie_src`; `> 1`: `spice3_src`) → `OPtran` when the `optran` command was given. Every rung's solve is capped at `itl2` (`CKTdcTrcvMaxIter`). `diagGmin` is added inside the factor to every structural diagonal (`LoadGmin`), distinct from the device `gmin`.

**Dynamic gmin** (`cktop.c:162-260`): `factor = gminFactor` (default 10, `cktntask.c:125`); start at `diagGmin = 1e-2 / factor`; target `max(gmin, gshunt)`.
- Success: save `(x, state)`. Fewer than `itl2/4` iterations → `factor *= √factor`, capped at `gminFactor`. More than `3·itl2/4` → `factor = max(√factor, 1.00005)`. Step `diagGmin /= factor`, landing exactly on the target when the next step would pass it. Stop on success at the target.
- Failure: if `factor < 1.00005` give up; else `factor = factor^(1/4)`, restore `(x, state)`, retry at `oldGmin / factor`.
- Exit with `diagGmin = gshunt` and a final solve.

**Gillespie source stepping** (`cktop.c:481-672`): zero `x` and state, solve at source factor 0; if that fails, bootstrap with a gmin ladder from `gmin·10¹⁰` down one decade per solve. Then step the source factor from 0 by `raise = 0.001`. Success: save, `raise *= 1.5` if fast (≤ `itl2/4`), `*= 0.5` if slow (> `3·itl2/4`). Failure: stop if the factor gained less than 1e-8 since the last success; else `raise /= 10` (capped at 0.01) and restore.

## Integration

The circuit DAE is `F(x, t) = i(x) + dq(x)/dt + u(t) = 0`. Keep `q` as the integrated state. Xyce's formulation tracks `q` over time "in order to enforce charge conservation" (Hoekstra et al. 2004 §5.2), and its condensed form keeps `q` as per-device state rather than as matrix unknowns, cutting a 70,000-MOSFET circuit from over 350,000 unknowns to about 25,000 (§5.3); Kundert & Clifford (slide 36) show capacitance-based models do not conserve charge because the charge comes from the linearized model. Xyce and Spice3f5 often stamp `αC` with `C` lagged even for a nonlinear capacitor (Xyce §5.3, text after eq. 90); stamping the analytic `C = ∂q/∂x` at the current iterate gives Newton the exact Jacobian (derived). The last-but-one test in `numerics.zig` runs a nonlinear `q(v) = c0·v + c1·v²` around a closed voltage cycle: the charge-based companion returns to zero net charge, the `C(v)·Δv` companion leaks.

**Companion coefficients** (`maths/ni/nicomcof.c`), `dq/dt ≈ ag0·q_n + (history)`:

| Method | ag0 | history |
|---|---|---|
| trap order 1 (BE) | `1/h` | `ag1 = −1/h` |
| trap order 2 | `1 / (h·(1 − xmu))` (`nicomcof.c:43`) | `ag1 = xmu / (1 − xmu)` times the previous dynamic current |
| Gear order 2–6 | solved from a Vandermonde system over the step history (`nicomcof.c:52` on) | |

At `xmu = 0.5` trap gives `2/h` and 1. For Gear-2 with `r = h/h_prev` the result is `F_dyn = α(q_n − q_{n−1}) − a₂(q_{n−1} − q_{n−2})`, `α = (1 + 2r)/((1 + r)h)`, `a₂ = r²/((1 + r)h)`, the standard variable-step BDF2 (derived; equal steps give `3/(2h)` and `1/(2h)`).

**Trap ringing.** Trap's amplification factor `(1 + z/2)/(1 − z/2)` tends to −1 as `z → −∞`, so a stiff component that the step does not resolve flips sign every step instead of decaying (derived, standard A-stability analysis). Kundert & Clifford (slides 21–22): "Point-to-point ringing is an artifact of the trapezoidal rule, switch to Gear to avoid it", and the converse, Gear and BE add artificial damping to a lossless LC. ngspice 44 manual §11.1.4: `xmu` below 0.5, "even a small reduction, e.g. to 0.495", may suppress trap ringing.

## Local truncation error

Nagel 1975 §6.7–6.8: the step bound with a relative tolerance is `h ≤ 2(ε_r·|ẋ| + ε_a) / |ẍ|` for BE (eq. 6.31); the `k+1`-th derivative is estimated from the stored solutions as `x^(k)(ξ) ≈ k!·DD_k` with the recursive divided difference `DD_k = (DD_{k−1}(t_{n+1}) − DD_{k−1}(t_n)) / Σ h` (eqs. 6.35–6.36). Trap's LTE is `−(h³/12)·x'''` (eq. 6.40).

ngspice `CKTterr` (`spicelib/analysis/cktterr.c`), per charge state, as transcribed in `stepBound`:

```
volttol   = abstol + reltol · max(|i_now|, |i_prev|)          # companion currents
chargetol = reltol · max(|q_0|, |q_1|, chgtol) / h_0
tol       = max(volttol, chargetol)
dd        = plain (order+1)-th divided difference of q over the last order+2 points
del       = trtol · tol / max(abstol, factor · |dd|)
del       = del^(1/order)                                      # sqrt at order 2
```

with `factor` from `trapCoeff = {.5, .08333333333}` and `gearCoeff = {.5, .2222222222, .1363636364, .096, .07299270073, .05830903790}`. Two transcription traps:

- `dd` is the plain divided difference. The `k!` of Nagel's eq. 6.35 is not applied: `numerics.zig` feeds `q = t³` on an uneven grid and gets the bound for `|dd| = 1`. Port ngspice's constants with ngspice's estimator, never one without the other.
- The coefficients are truncated decimals, not `1/12` and `2/9`. ESPice found the 4e-11 relative gap moved LTE-chosen steps enough that only 50 of 187 oracle timepoints of an RC-ladder deck hit its grid within 1e-13, and all 187 did after switching (`dev/analysis/transient-integration.md`; unverified here).

`CKTtrunc` (`ckttrunc.c:53`) takes the minimum over every device's states, then `min(2h, that)`. ngspice runs it per device charge state, not per node; summing charges per node first loses the binding state (ESPice measured per-state LTE closer to ngspice on 9 of 12 moved decks; same doc; unverified here).

**Step acceptance** (`dctran.c`):
- Newton failure: back off time, `h /= 8` (`:806`), order 1 (`:814`).
- Converged: `newdelta = CKTtrunc(h)`; accept if `newdelta > 0.9·h` (`:874`). At order 1 with `maxord > 1`, recompute at order 2 and keep order 2 only if that bound exceeds `1.05·h` (`:903`); the recomputed bound becomes the next step either way.
- Rejected: back off, `h = newdelta`, same order.
- The first timepoint skips the LTE check; the next step is capped by `tmax`.

**Breakpoints** (`dctran.c`): `delmin = 1e-11·maxStep` (`traninit.c:36`); breakpoints merge inside `minBreak`, `10·delmin` with XSPICE (`:236`) else `5e-5·maxStep`. Landing on (or within `delmin` of) a breakpoint sets order 1 and `h = min(h, 0.1·min(saveDelta, next gap))` (`:558-580`), and the first timepoint divides by 10 again. Without XSPICE, a step that would pass the breakpoint is cut to land on it, and a step within `1.9·h` of it halves the remaining gap so the last two steps are equal (`:600-620`).

## Dual numbers

Forward-mode AD: a value is `(v, ∂v/∂x_lane)`, a `@Vector(N, f64)` of partials, and every operation applies the chain rule lanewise. Seed each terminal voltage as a probe on its own lane; one evaluation of the device's current returns the residual and the full Jacobian row. `Dual(N)` in `numerics.zig` is the minimum (constant, probe, add, sub, mul, scale, exp) with a diode test checking `I`, `∂I/∂va = g` and `∂I/∂vc = −g` against the closed form.

ESPice's production family (`src/device/eval.zig` `DualFor`) adds what a real model needs:
- Sparse lanes: only unknowns the expression reads get a lane, and a binary op joins its operands' lane sets at comptime.
- Lane counts rounded up to a power of two: an unpadded width of 12 or 26 cost up to 2× because LLVM split it into shuffles and spills (`dev/devices/abi.md`; unverified here).
- A float width parameter for the derivative half only; the value half and every boundary stay `f64`, so the residual is identical at either width.

## Sources

Nagel, ERL-M520 (1975), §5.6–5.8 and §6.7–6.10 (scanned; read as page images). ngspice source HEAD 940bb50: `niiter.c`, `niconv.c`, `devsup.c`, `dioload.c`, `dioconv.c`, `diotemp.c`, `macros.h`, `cktop.c`, `cktntask.c`, `nicomcof.c`, `cktterr.c`, `ckttrunc.c`, `dctran.c`, `traninit.c`, `spsmp.c`. ngspice 44 manual §1.4, §11.1.2, §11.1.4. Kundert & Clifford, *Achieving Accurate Results With a Circuit Simulator* (1993), https://kenkundert.com/docs/eda+t93-preso.pdf. Hoekstra et al., *Xyce Parallel Electronic Simulator Design: Mathematical Formulation*, SAND2004-2283, https://xyce.sandia.gov/files/xyce/Xyce_Math_Formulation.pdf. ESPice `dev/solvers/newton-raphson-convergence.md`, `dev/solvers/homotopy-continuation.md`, `dev/analysis/transient-integration.md`, `dev/analysis/tolerance-system.md`, `dev/devices/abi.md`.
