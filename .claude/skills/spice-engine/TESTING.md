# Oracle testing against ngspice and Xyce

Read this when adding a fixture deck, producing a reference result, marking or clearing a known gap, or deciding whether a mismatch with ngspice is a bug or a divergence. ESPice's corpus (`tests/fixtures/**`, runner `tests/test_correctness.zig`) is the worked example.

## Every deck names its oracle

A deck `name.sp` sits beside `name.expected.json`. The oracle record says where the numbers came from, so a reviewer can rerun it:

```json
"oracle": {
  "kind": "ngspice-44.2",
  "derivation": "Independent ngspice-44.2 run of the preserved benchmark circuit. ...",
  "command": "ngspice -n -b -r reference.raw circuit.sp",
  "source_sha256": "5190…"
}
```

`kind` is one of: `analytic` (a closed-form derivation written in `derivation`), a reference simulator with its version (`ngspice-44.2`, `ngspice-45`, `vacask`), or a property the output must satisfy (`measure`, `input_contract`, `pole_zero_sets`, `sample_moments`, `repeatability`, `oscillation`, ...). In ESPice's corpus at the time of reading, 487 oracles are analytic and 277 come from ngspice runs. Prefer analytic where a derivation exists: it is the only oracle that cannot share the engine's modelling mistake.

`expect` holds `status` (`success` or `error`, so refusals are tested too) and per plot a `match` mode (`exact`, `samples`, `selected_rows`, `unordered`), optional `row_indices` or an `axis`, and per column `values`, `rtol`, `atol` and `magnitude` (compare |z| when the oracle only knows amplitudes). A `netlist_sha256` ties the oracle to the deck bytes, so editing the deck without regenerating the oracle fails as `StaleOracle`.

## KNOWN GAP

A deck whose feature is missing, or whose oracle is known to disagree for a recorded reason, carries a header comment:

```
* KNOWN GAP: an LTRA under .ac stamps its DC series resistance, not the frequency-dependent two-port (#111)
```

The runner reads it from the deck (`knownGap` in `test_correctness.zig`: any `*` comment line starting `KNOWN GAP:`). A failing marked deck reports XFAIL and keeps the run green; a passing marked deck reports XPASS and fails the run until someone deletes the marker. The line names the cause and the issue; there is no separate list of failing decks to drift out of date. An unsupported feature is a failure, never a skip.

Write the marker in the same commit that adds the failing deck. Clear it in the commit that makes it pass.

## Producing an ngspice oracle

`ngspice -n -b -r reference.raw deck.sp` (checked against `ngspice --help`, 44.2: `-n` skips the user's `.spiceinit`, which would otherwise change options per machine; `-b` is batch mode; `-r` writes the binary raw file). Record in the oracle:

- the version from `ngspice -v`;
- the exact command;
- the deck hash;
- options that differ from the defaults in `SKILL.md`.

The run log also names the linear solver: the 44.2 build here printed "Compiled with KLU Direct Linear Solver" for `-v` but "Using SPARSE 1.3 as Direct Linear Solver" for a run. Record which one produced the reference, since a different pivot order rounds differently (derived).

**Tolerance follows the oracle's own accuracy.** At `reltol = 1e-3` ngspice accepts any iterate whose last update is within 0.1%, so comparing against it at `rtol = 1e-6` compares its iteration noise (derived from the convergence test in `NUMERICS.md`). Either set `rtol` on the order of the oracle's `reltol`, or regenerate the oracle at tightened options and say so in `derivation`. An engine that reproduces ngspice's rules bit for bit (`SKILL.md` step 6) earns tight tolerances against an untightened oracle.

**Transient grids.** Store the reference's accepted times with its values. An engine that follows `dctran.c` rule for rule can be compared row for row (ESPice's `tran/pwl_isource_breakpoints` matches ngspice 44.2's accepted grid exactly); otherwise interpolate the engine's waveform onto the reference times and widen the tolerance by the LTE budget, and write which one the deck does.

**Divergence is a record, not a tolerance.** When the engine is deliberately different from ngspice (a residual gate, a finer LTE partition, a bug ngspice has), write the reason and the measurement where the project keeps design notes (ESPice: `dev/`), and mark the affected decks `KNOWN GAP` with that cause if they fail. Widening `rtol` until a disagreement passes hides the next regression in the same place.

## Xyce

Xyce is not installed on this machine (`which Xyce` finds nothing), so no Xyce command here was run; treat Xyce invocation details as unverified. Its regression suite is the larger oracle source: `github.com/Xyce/Xyce_Regression`, about 4,100 `.cir` netlists with gold `.prn` output, including the MCNC CircuitSim90 decks (43 circuits, up to 46,850 unknowns, several needing gmin stepping), per ESPice `dev/research/spice-benchmark-suites.md`. It is GPL-3.0: fetch it at a pinned commit rather than vendoring it. Its dialect differs (`.options timeint`, expression `.print`s), so translate, regenerate with ngspice, and keep the Xyce gold as a third opinion where ngspice and the engine disagree.

## SpiceRack as the deck builder

SpiceRack (`import spicerack`) builds a deck in Python and runs it on `ngspice`, `ltspice`, `vacask`, `spectre` or `espice`; it has no Xyce backend. One bench run on two backends is a ready differential: `tb.with_backend("ngspice")` against `tb.with_backend("espice")`. Two of its traps return plausible wrong numbers: `ac[node]` is the real part (use `ac.magnitude(node)`), and ngspice has no `.step` (loop in Python). The skill bundled in its repository (`skills/spicerack`) has the rest.

## Beyond the corpus

The fixture corpus is the differential layer. The `test-plan` skill's numerical-solver section adds the oracles that need no reference simulator: KCL residual at every node from an independent evaluator, every device Jacobian against finite differences, scaled residual ratios for the LU, random circuits with known DC solutions. Every SIMD kernel also needs its differential test against its `W == 1` instantiation (`LINEAR.md`, LaneLu).
