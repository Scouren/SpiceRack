---
name: spicerack
description: Build SPICE circuits, netlists and testbenches with SpiceRack (`import spicerack`). Use when writing or debugging circuit-simulation code that imports spicerack, when picking a backend (ngspice/LTspice/VACASK/Spectre), when extracting a metric such as gain, bandwidth or phase margin from a simulation result, or when reaching for the built-in analog testbench recipes.
---

SpiceRack builds a **deck** (SPICE netlist text), hands it to a simulator, and parses the result back into Python lists.

## Three objects

| Object | Holds | Build it with |
|---|---|---|
| `Circuit` | A standalone circuit. Every element type. | `ps.Circuit("name")` |
| `Subcircuit` | A reusable DUT with ports. Every element type. | `ps.Subcircuit("name", ["a","b"])` |
| `Testbench` | Stimulus wrapped around a DUT, plus queued analyses. | `ps.Testbench(dut)` |

`Circuit.simulator()` returns a `CircuitSimulator`; `Testbench` runs analyses directly.

**`Testbench` carries only stimulus: `V I R C` and the waveform source builders.** Everything else — `L K M D Q J Z E F G H T S W A X BV BI`, plus `model include lib parameter raw_spice gnd veriloga osdi verilog` — lives on `Circuit` and `Subcircuit`. Put devices in the DUT, stimulus in the bench. Reaching for `tb.model(...)` or `tb.gnd` is the most common `AttributeError` here; write the node `"0"` (or `"gnd"`, which a `Testbench` maps to `0` as a `Circuit` does).

## Build and simulate

1. **Put the device under test in a `Subcircuit`** with explicit ports. Completion: `dut.instance(...)` or `ps.Testbench(dut)` accepts it, and `tb.netlist("ngspice")` shows every device you added.
2. **Wrap it in a `Testbench` and add stimulus.** AC analysis needs `ac=` on a source: `tb.V(name="in", positive="vin", negative="0", value=0.0, ac=1.0)`. A source without `ac=` contributes nothing to an AC sweep, and the sweep returns all zeros.
3. **Queue analyses** with `add_operating_point()`, `add_dc()`, `add_ac()`, `add_transient()`, `add_noise()`. Completion: `tb.netlist(backend)` contains the matching dot-card.
4. **Save what you will read**: `tb.save("V(vout)")`. Completion: `.save` appears in the deck.
5. **Run** — `tb.ac(...)`, `tb.transient(...)`, `tb.operating_point()`. Completion: the result object indexes without raising, and `len(result["vout"])` matches the expected point count.

Read the deck with `tb.netlist(backend)` before running. It is plain text, and it is the ground truth for what the simulator will do. Whatever the target does not get as written (an output request, an option, an integrated noise) comes back as a `UserWarning` naming its code (`spicerack t:4:1: W0023 an output request not written`), on a registered dialect too (its own notes are `W0202`); the deck is still written. `text, notes = tb.netlist_with_notes(dialect)` returns the same notes as `Diagnostic` objects and warns of none. A target that cannot write the deck raises `ps.Error` (a `RuntimeError`): `e.status == "refused"`, and `e.diagnostics` lists every diagnostic with its `code`, `kind` (`"refusal"`, `"note"`), `line`, `column`, `statement` and `element`. Every other failure of a netlist, a run or a compile is `ps.Error` too: `"refused"` when no simulator could be routed, `"failed"` when a run, a compiler or a dialect's hook failed (no diagnostics unless it has a code). A refusal at no statement (routing, `W0040`) has one diagnostic whose `line`, `column`, `statement` and `element` are `None`. A bad argument stays `ValueError`.

Hand results to another tool with `raw, meas = ps.export(tb.run(), "out/bench")`: `out/bench.raw` is one ngspice raw file (every vector under the name you index a result by: `out`, `i(vin)`, `time`, whichever simulator ran), `out/bench.meas.json` the `.meas` values with their status and the steps' parameters (`spicerack.measures/1`; REFERENCE.md, "Exporting results"). The directory must exist.

## Traps

Each of these returns a plausible wrong number rather than an error.

**`ac[node]` is the real part, not the magnitude.** For a Bode plot use `ac.magnitude(node)`, `ac.magnitude_db(node)` and `ac.phase(node)` (degrees). At 100 Hz on a 159 Hz RC, `ac["vout"]` reads 0.7170 while `|H|` is 0.8467. `.magnitude()` exists only on `AcAnalysis`.

**ngspice has no `.step`.** `tb.step()` then `netlist("ngspice")` raises an error (`W0016`), and a run without a forced backend auto-routes away from ngspice (to VACASK when installed). Sweep by rebuilding the testbench in a Python loop, one run per point — `run_corners` does exactly that for corners and seeds. LTspice, VACASK and Spectre support parameter sweeps natively. On LTspice a stepped run returns a list, one result per step in step order, each with `.params` (`{"r": 2000.0}`; `{}` on an unstepped result). On VACASK a stepped result still concatenates the steps on the analysis axis (with the step value as `step_<param>`), and a stepped `OperatingPoint` raises `ValueError` rather than return the first step. LTspice runs one analysis a deck, so `tb.run()` on LTspice runs each queued analysis in a deck of its own.

**Use ASCII micro units: `u_uF`, `u_uH`, `u_uA`, `u_uV`, `u_uW`, `u_us`.** Python normalises identifiers, so a `u_µF` spelled with U+00B5 cannot be imported. The `u_Ω` family does work.

**`.nodes` and `.branches` are names, not values.** Every result type has them (and `.measures`: `{name: value}`, the value `None` when the measure failed); index the result for the numbers: `res["vout"]`. An `OperatingPoint` also takes the simulator's spelling: `op["v(vout)"]` is `op["vout"]`.

**A `UnitValue` in `**kwargs` is written as its SI number.** `M(..., W=1 @ u_MOhm)` emits `W=1e6`. Strings pass through verbatim, so expressions go in as `"{w*2}"`. `True` is a bare flag (`off=True` emits `off`); `False` and `None` emit nothing.

**Results are `list[float]`, not numpy arrays.** Slice and iterate them; `.mean()` and stride-slicing raise.

**`.measure` reaches stdout only for `ac`, `dc` and `tran`.** That is ngspice's own limit (it refuses `.measure` under `-b -r rawfile`, so `tb.netlist("ngspice")` runs the first of those three analyses from a `.control` block, `.four` becoming the `fourier` command, and the backend appends the `write`; raw `.meas`/`.four` lines count too). A `.measure` attached to a noise or pole-zero run alone is refused at run time instead of returning nothing.

## Metrics

`tb.measure("ac", "gain", "find", "vdb(vout)", "at=1k")` is a raw passthrough to the SPICE `.measure` card, so the full grammar is available — `TRIG/TARG`, `WHEN`, `FIND…AT`, `AVG`, `RMS`, `INTEG`, `DERIV`, `MAX/MIN/PP`. Results land in `result.measures`.

For extraction in Python, `spicerack.testbenches` (defined in `spicerack.testbenches.analysis`) provides `crossings`, `ac_gain_db`, `ac_passband_db`, `ac_bandwidth_hz`, `ac_unity_gain_hz`, `ac_phase_margin_deg`, and `extract_metrics` with `MetricSpec`.

Bandwidth defaults to the half-power point, `HALF_POWER_DB` = 3.0103 dB. A round 3.0 dB puts a single-pole corner 0.23 % low — a systematic error that does not shrink with sweep density.

## Backends

Names: `ngspice` (aliases `ngspice-subprocess`, `ngspice-shared`), `ltspice`, `vacask` (alias `vacask-shared`), `spectre`, `espice` and every simulator registered with `ps.register_simulator` (next paragraph); no Xyce. `with_backend()` / `simulator()` raise `ValueError` on any other name. `tb.netlist(name)` also writes `hspice`, `xyce`, `cdl` and `verilog`, which no backend runs. Pick one with `tb.with_backend(name)`, or for every run that names none with `SPICERACK_BACKEND=<name>`. LTspice: `ltspice` on PATH, the macOS app, or a Wine install under `WINEPREFIX` (default `~/.wine`); `SPICERACK_LTSPICE=<path>` names the executable (a `.exe` runs under Wine). On `espice`, `veriloga()` becomes `.hdl` (VerA, no OpenVAF) and `osdi()` is refused. `ngspice` is the default and the only one assumed present. Declared features gate backend selection, so a bench that asks for `.measure` support will not route to vacask, and a run lands only on a backend whose writer writes the deck (`tb.check_backend(name)` names a writer's refusal code too). Check with `tb.check_backend(name)` and `ps.CircuitSimulator.available_backends()`.

Your own simulator: `ps.register_simulator(ps.Simulator("mysim", dialect="ngspice", command=["mysim", "-b", "-r", "{raw}", "{deck}"], output="ngspice-raw", analyses=["op", "tran"], features=["measures"]))` returns `"mysim"`, the name `with_backend()`, `simulator()` and `SPICERACK_BACKEND` then take. `command` is the argument list (no shell); `{deck}`, `{raw}`, `{dir}` become absolute paths and one word must hold `{deck}`. `output`: `ngspice-raw`, `ngspice-ascii`, `nutascii`, `raw-dir`, `ltspice-raw`. `analyses` (`op`, `dc`, `ac`, `tran`, `noise`, `tf`, `sens`, `sens_ac`, `pz`, `disto`, `pss`, `hb`, `sp`, `stb`, `trannoise`, `pac`, `pnoise`, `pxf`, `pstb`) and `features` (`xspice`, `osdi`, `measures`, `step`, `control`, `laplace`, `verilog`; or `verilog=True`, `verilog_a=True`) are what it declares, and it is routed only for those, after the built-ins. It is its command only (a built-in's run does more): an ngspice command with `-r {raw}` fails on a deck with a `.measure` or a Fourier analysis (that deck runs from `.control`; the built-in appends a `write`), and a `raw-dir` command runs in the scratch directory, where a relative `load` or `include` path is not found (the built-in VACASK makes it absolute first). `raw-dir` with `dialect="vacask"` is read as the built-in's run (the deck's analysis order, a step's sweep column last, noise spectra added); with another dialect, every `*.raw` in file-name order. A bad descriptor is `ValueError` `R0040` naming the field. A run it cannot take is `RuntimeError` opening `W0180` (an analysis it does not declare), `W0181` (a feature), `W0182` (no simulator runs every analysis of the deck) or `R0041` (its program is not on PATH), with `exc.diagnostic == {"code", "stage", "missing"}`. `tb.netlist()` takes dialect names: `tb.netlist(sim.dialect)`. The same descriptor as a file: `ps.load_simulator("mysim.toml")` (TOML: `schema = "spicerack.simulator/1"`, then `Simulator`'s arguments as keys) returns the checked `Simulator`, unregistered. Provisional (listed in `ps.provisional`, may change in a minor release): `read=fn` in `output`'s place reads the results yourself: after the command ran, `fn(outdir)` (the scratch directory, `{dir}`) returns `(raw, measures)`, `raw` a raw file's path, a `RawData` or a list of them, `measures` `None` or a `spicerack.measures/1` dict; a function that raises or returns anything else fails the run (`RuntimeError` opening `Simulation failed: read hook: `).

## Dialect plugins

A dialect of your own is a `ps.Dialect` subclass whose `define(d)` makes `spicerack.def_.DialectDef` calls (the Rust definition API 1:1), usually `d.extends(Lang.NGSPICE)` (or `class Mine(ps.dialects.HSPICE)` with `super().define(d)`) plus a few rows; `ps.register_dialect(Mine("mine"))` registers it, and `"mine"` then works in `tb.netlist(...)`, `Testbench.from_source(text, dialect=...)` and `extra_line(..., dialect=...)`. Its own refusals are `code(id, Kind.Refusal, msg)` + `refuse_code(Construct.X, id)`, raised as `W0201 mine:<id> <msg>`. A `hook(head, f)` rewrites the written cards of one head (`f(cards) -> [Answer.Text(...) | Answer.Refuse(id)]`, once per deck). A plugin is netlist-only unless it declares `runs_on(sim)`: a built-in (`Simulator.Ngspice`) or a simulator registered first with `ps.register_simulator` (the `ps.Simulator` or its name), allowed only when the plugin extends that simulator's dialect; `Lang.runs_on()` is the simulator's name, and the run is routed as that simulator's. Re-registering an equal definition is a no-op; a different one under the same name raises `ValueError` (restart the interpreter to change a plugin). Full page: `docs/dialects.md`.

→ Element signatures per class, analysis signatures, the unit list, and the backend capability matrix: [`REFERENCE.md`](REFERENCE.md)

→ PDK libraries, corners, Monte Carlo, PDK devices with parameters, Verilog-A devices, external (post-layout) netlists as DUT, printing a bare `.subckt`: [`PDK_AND_FLOW.md`](PDK_AND_FLOW.md)

→ The 13 built-in benches, the declared-vs-computed metric contract, cell characterization (pin cap, NLDM tables, setup/hold), the corner/MC runner, and how to write a new bench: [`TESTBENCHES.md`](TESTBENCHES.md)
