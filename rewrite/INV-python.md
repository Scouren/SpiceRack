# INV-python: the Python API and binding, as a contract on a rewritten core

Read-only inventory, 2026-11-11. Nothing was built, run or imported.

- **batch-22** = `/home/omare/Documents/Projects/EDA-Tools/Schematic/SpiceRack-merge` (branch `batch-22`, head `1059821`, crate and wheel version `0.2.0`, wheel name `fast-spicerack`).
- **main** = `/home/omare/Documents/Projects/EDA-Tools/Schematic/SpiceRack` (branch `main`, head `f7fbf3b`).
- Paths below are relative to batch-22 unless they say `main:`.
- Counts come from reading the sources and from `ast`/`grep` over them. "not found" means I looked and it is absent.

## 0. The five facts that matter most

1. **batch-22 builders are still text emitters.** Every builder call formats an ngspice card (`Card { text }`, `Vec<String>`) at call time; analyses are the one typed value kept until write time, and they are printed as ngspice cards (or Spectre statements) at write time. `syntax::build::Build` exists (D15 step 1, commit `4b4c681`, ledger row P8-140-s1) but its only content method is `Build::text(lines)`, which **runs the tokenizer and the parser over the text** and relocates their rows (`src/syntax/build.rs:99-145`). No builder appends an AST row of its own; the only hand-made rows are the title's two tokens and its `Title` node (`build.rs:83-86`).
2. **The binding is a third writer with its own dialect logic.** `src/python/deck.rs::bench` and `analysis_region.rs` hold `match Dialect` code (LTspice and Spectre Fourier forms, `pss` on Spectre, log-sweep refusal) and raise a catalogue code (W0040) themselves, before the renderer sees the deck.
3. **The approved D15 design is not "IR, no text".** `DESIGN-python-ast-builders.md` §2 has every builder call write *source text + token rows + AST rows together*, because an `Ast` borrows its source through `Tokens` (a token's end is re-lexed from its start). Constructs with no ngspice spelling are stored as their "home dialect" text inside a `simulator lang=<d>` region, and the guard is "the built rows equal `parse(tokenize(text))` row for row". A dialect-neutral IR built directly makes that design, its differential oracle and its store format (`to_source()` = the buffer verbatim) obsolete.
4. **Almost nothing of the builder surface is frozen.** The D13 snapshot (`tests/fixtures/api_snapshot.txt`) holds 17 lines: `Simulator` and its 8 properties, `register_simulator`, `simulators`, `load_simulator`, `def_.DialectDef.runs_on`, `def_.Lang.runs_on`, and `parameter` on three classes. Version is 0.2.0. D18/D22/D25/D26/D48 renames and removals are all still pending, so the rewrite can land them with the new core.
5. **The run path needs very little from `syntax`**: a dialect id, a diagnostic code, two marker constants, and deck text. But it re-scans that deck text with ad-hoc line scanners in five places (§5.3), which is a hidden contract on the writers' output shape.

---

## 1. Public surface

Marking: **R** = implemented in Rust (file under `src/python/`), **P** = pure Python (file under `python/spicerack/`).

### 1.0 Totals

| group | count |
|---|---|
| classes in `spicerack._native` (R) | 26 + the `Error` exception |
| public native functions (R) | 5 (`lint`, `read_raw`, `compile_veriloga`, `register_simulator`, `simulators`) |
| private native names (R) | `_export`, `_written`, `_route`, `_version` |
| native submodules (R) | 2: `spicerack.unit` (42 `Unit` constants), `spicerack.def_` (49 classes + `register` + alias `Keyword`) |
| builder method definitions (R) | 182: `Subcircuit` 69, `Circuit` 11 (3 override), `Section` 5, `CircuitSimulator` 33, `Testbench` 64; plus 8 builder properties |
| result classes (R) | 15 |
| top-level pure-Python names (P) | `Dialect`, `register_dialect`, `export`, `load_simulator`, `provisional`, modules `diagnostics`, `dialects` |
| `spicerack.testbenches` (P) | 91 exported names: 5 classes, 86 functions |
| `spicerack.remeasure` (P) | 8 functions |
| diagnostic codes exposed as `def_.Code` | 115 (84 W, 13 R, 18 P) |
| conformance matrix | 112 features × 9 targets (`tests/test_python_conformance.py` `FEATURES`/`EXPECTED`) |

`python/spicerack/__init__.py` (11 lines) is `from ._native import *` plus the pure-Python re-exports. `spicerack.def_` and `spicerack.unit` are not files: the native module inserts them into `sys.modules` (`def_.rs:98-100`, `unit.rs:37-39`).

### 1.1 Circuit building

Classes (all R): `Subcircuit` (`subcircuit.rs`, base class), `Circuit(Subcircuit)` (`circuit.rs`), `Block(Subcircuit)` (`block.rs`), `Section` (`section.rs`), `Testbench(CircuitSimulator)` (`testbench.rs`), `CircuitSimulator` (`simulator.rs`, no constructor: `Circuit.simulator()` returns one).

Constructors: `Subcircuit(name, ports=None, **params)`, `Circuit(title)`, `Block()`, `Section(name)`, `Testbench(dut)` where `dut` is a `Subcircuit` (becomes the deck's top, **copied** at construction) or a `Circuit` (its definitions, options and temp come along).

Value shapes: `value` = `float | UnitValue | str` (`args.rs:27 PyValueArg`); a `str` is an expression, one pair of `{}` stripped. A waveform field or `ac=` is the same union (`args.rs:37 Num`). `**params` values go through `str(v)`; `True` is a bare flag, `False`/`None` are dropped, a `UnitValue` is written `{:e}` (`args.rs:191-203`).

**`Subcircuit` element methods** (`subcircuit.rs`, `lines.rs`; inherited by `Circuit` and `Block`):

| method | signature shape | file:line |
|---|---|---|
| `R` | `(name, positive, negative, value, *, raw_spice=None, **params)`; `model=` in params is the card's model word | subcircuit.rs:106 |
| `C` | `(name, positive, negative, value=None, *, charge=None, **params)`; exactly one of `value`, `charge` | :121 |
| `L` | `(name, positive, negative, value, **params)` | :131 |
| `K` | `(name, *inductors, coupling)`; inductor names validated against the body's card text | :143 |
| `V` | `(name, positive, negative, value, *, ac=None, ac_phase=None, portnum=None, z0=50.0, distof1=None, distof2=None)` | :167 |
| `I` | `(name, positive, negative, value, *, ac=None, ac_phase=None, distof1=None, distof2=None, **params)` | :176 |
| `BV`, `BI` | `(name, positive, negative, *, expression, **params)` | :186, :194 |
| `NonLinearVoltageSource`, `NonLinearCurrentSource` | `(name, positive, negative, *, expression)` | :204, :211 |
| `E` | `(name, positive, negative, *, control_positive, control_negative, voltage_gain: float)` | :216 |
| `G` | `(…, *, control_positive, control_negative, transconductance: float, **params)` | :223 |
| `F` | `(name, positive, negative, *, vsense, current_gain: float, **params)` | :232 |
| `H` | `(name, positive, negative, *, vsense, transresistance: float)` | :240 |
| `D` | `(name, anode, cathode, *, model, **params)` | :247 |
| `Q` / `BJT` (alias) | `(name, collector, base, emitter, *, model, substrate=None, thermal=None, **params)` | :255, :270 |
| `M` / `MOSFET` (alias) | `(name, drain, gate, source, bulk=None, *, model, extra_nodes=None, **params)` | :280, :297 |
| `J`, `Z` | `(name, drain, gate, source, *, model, **params)` | :305, :312 |
| `S` | `(name, positive, negative, *, control_positive, control_negative, model, initial_state=None)` | :319 |
| `W` | `(name, positive, negative, *, vcontrol, model, initial_state=None)` | :327 |
| `T` | `(name, input_positive, input_negative, output_positive, output_negative, *, Z0, TD)` | :335 |
| `POLY` | `(name, kind, positive, negative, controls: list[str], coefficients: list[float])`; `kind` ∈ E G F H | :344 |
| `A` | `(*, name, connections: list[str], model)` (XSPICE) | :370 |
| `O` | `(name, in+, in-, out+, out-, *, model)` (LTRA) | lines.rs:30 |
| `Y` | `(…, *, model, length=None)` (TXL) | lines.rs:38 |
| `P` | `(name, inputs, ground_in, outputs, ground_out, *, model, length=None)` (CPL) | lines.rs:47 |
| `U` | `(name, n1, n2, n3, *, model, length, lumps=None)` (URC) | lines.rs:69 |
| `X` | `(name, subcircuit_name, *nodes, **params)` | subcircuit.rs:386 |
| `instance` | `(subckt, name, *nodes, **params)`; uses only `subckt.name` | :379 |

**`Subcircuit` structure and directives** (subcircuit.rs): `subcircuit(subckt)` nested definition (:403); `conditional(branches=[(cond: str, Block)], otherwise=None)` (:422); `model(name, kind, **params)` (:430); `initial_condition(**node=value)` (:439); `node_set(**)` (:446); `raw_spice(line)` (:450); `include(path)` (:454); `lib(path, section)` (:458); `osdi(path)` (:463); `parameter(name, value)` (:467); `add_function(name, args, body)` (:477); `veriloga(source_or_path) -> str` (:485); `verilog(*, source, mode="simulate", instance_name, connections, pdk=None, liberty=None, spice_models=None)` (:507). Properties: `gnd` (`"0"`), `name`, `ports`.

**`Circuit` only** (circuit.rs): `options(**kw)` (:92), `temp(t)` (:97), `subcircuit(subckt)` (:101, a top-level definition: overrides the base's nested one), `section(section)` (:110), `__getitem__(name)` / `element(name)` (:115, :119), `set_source_value(name, value)` (:123), `node(name)` (:136), `__str__` (:141), `netlist(backend="ngspice")` (:151), `to_source()` (:158), `save(path)` (:163), `simulator(simulator=None)` (:172).

**`Section`** (section.rs): `parameter(name, value)`, `model(name, kind, **params)`, `include(path)`, `lib(path, section)`, `subcircuit(subckt)`; property `name`.

**`Testbench` own builder methods** (testbench.rs): `add_subcircuit(subckt)` (:51), `use_pdk(lib)` (:58), `parameter(name, value)` (:66, replaces by name), stimulus `V` (:81), `I` (:89), `R(name, p, n, value)` (:97), `C(name, p, n, value)` (:103), `with_backend(name)` (:110), `extra_line(line, dialect="ngspice")` (:119), `add_print(analysis, *outputs)` (:128), `add_function(name, args, body)` (:134), `global_nodes(*nodes)` (:140).

### 1.2 Sources and waveforms

There is **no waveform object**: `V()`/`I()` take a value only. A waveform is a method of its own. Sixteen methods (eight waveforms, each as `…VoltageSource` and `…CurrentSource`), generated once by `pymethods_with_waves!` (`source.rs:37-164`) for both `Subcircuit` and `Testbench` (R). All take `(name, positive, negative, *, …, dc=None, ac=None, ac_phase=None)`.

| waveform | keyword fields | source.rs |
|---|---|---|
| `Sinusoidal` | `offset=0, amplitude=1, frequency=1000, delay=0, damping_factor=0, phase=0` + aliases `dc_offset`, `ac_magnitude` | :48 |
| `Pulse` | `initial_value=0, pulsed_value=1, delay=0, pulse_width=50e-9, period=100e-9, rise_time=1e-9, fall_time=1e-9` + aliases `delay_time`, `dc_offset` | :58 |
| `PieceWiseLinear` | `values=[(t, v)…], repeat_time=None, delay_time=None` | :70 |
| `Exponential` | `initial_value, pulsed_value, rise_time_constant, fall_delay_time, fall_time_constant, rise_delay_time=0` | :80 |
| `SingleFrequencyFM` | `offset, amplitude, carrier_frequency, modulation_index, signal_frequency` | :90 |
| `AmplitudeModulated` | `offset, amplitude, modulating_frequency, carrier_frequency, signal_delay=0, signal_phase=None, carrier_phase=None` | :104 |
| `TransientNoise` | `noise_rms, time_step, flicker_exponent=0, flicker_rms=0, rts_amplitude=0, rts_capture_time=0, rts_emission_time=0` | :134 |
| `Random` | `random_type, duration, time_delay=0, parameter1=1, parameter2=0` | :150 |

### 1.3 Models, libraries, PDK

- R: `Subcircuit.model`, `Section.model`, `include`, `lib`, `osdi`, `veriloga`, `verilog`; `ModelLibrary(path, corner=None, setup_includes=None, **backend_paths)` with properties `name`, `path`, `corner` (testbench.rs:451-507); `Testbench.use_pdk(lib)`; `compile_veriloga(source_or_path) -> str` (mod.rs:164).
- P: `testbenches.pdk_corners(library, sections, temperatures, vdds)` builds `ModelLibrary` objects (`testbenches/analysis.py:488`).
- **Not found on batch-22:** D39's generic devices (`nmos`, `pmos`, `res`, `cap`, `ind`, `bjt`, `diode`), `comment()`, `load_pdk`, `prove_pdk`, `netlist(pdk=, corner=)`.

### 1.4 Analyses

Run-now on `CircuitSimulator` (simulator.rs, spectre.rs; inherited by `Testbench`) and queued `add_*` twins on `Testbench` (testbench.rs). Both build one `Analysis` value through `card.rs`. `add_*` return `None`.

| run-now (returns) | queued twin | arguments |
|---|---|---|
| `operating_point()` → `OperatingPoint` | `add_operating_point()` | — |
| `dc(**{source: slice})` → `DcAnalysis` | `add_dc(**)` | key = source name or `temp`; `slice(start, stop, step)` |
| `ac` → `AcAnalysis` | `add_ac` | `variation="dec", number_of_points=10, start_frequency=1.0, stop_frequency=1e9` |
| `transient` → `TransientAnalysis` | `add_transient` | `step_time, end_time, start_time=None, max_time=None, use_initial_condition=False` |
| `noise` → `NoiseAnalysis` | `add_noise` | `output_node, ref_node, src, variation="dec", points=10, start_frequency=1e3, stop_frequency=1e8, points_per_summary=None` |
| `transfer_function` / `tf` (alias) → `TransferFunctionAnalysis` | `add_transfer_function` / `add_tf` (alias) | `outvar, insrc` |
| `dc_sensitivity` → `SensitivityAnalysis` | `add_dc_sensitivity` | `output_variable` |
| `ac_sensitivity` → `SensitivityAnalysis` | `add_ac_sensitivity` | `output_variable, variation, number_of_points, start_frequency=100, stop_frequency=1e5` |
| `polezero` → `PoleZeroAnalysis` | `add_polezero` | `node1..node4, tf_type, pz_type` |
| `distortion` → `DistortionAnalysis` | `add_distortion` | `variation, points, start_frequency=100, stop_frequency=1e8, f2overf1=None` |
| `pss` → `PssAnalysis` | `add_pss` | `fundamental_frequency, stabilization_time, observe_node, points_per_period=128, harmonics=10, sc_iter=None, steady_coeff=None, uic=False` |
| `s_param` → `SParamAnalysis` | `add_s_param` | `variation, number_of_points, start_frequency=1e6, stop_frequency=1e10`; `ValueError` with no RF port |
| `network_params` → `SParamAnalysis` | `add_network_params` | `output_current, input_source, z_in=50, z_out=50, variation, points=100, start_freq=1e3, stop_freq=1e9` |
| `harmonic_balance` → `HarmonicBalanceAnalysis` | `add_harmonic_balance` | `fundamental_frequencies, num_harmonics=None` (int or list) |
| `stability` → `StabilityAnalysis` | `add_stability` | `probe, variation, number_of_points, start_frequency=1.0, stop_frequency=1e10` |
| `transient_noise` → `TransientNoiseAnalysis` | `add_transient_noise` | `step_time, end_time` |
| — | `add_fourier` | `fundamental_frequency, outputs, num_harmonics=None`; result is the transient result's `.fourier` |
| `spectre_sweep` → `RawData` | `add_spectre_sweep` | `param, start, stop, step, inner_analysis: str, inner_type="ac"` |
| `spectre_monte_carlo` → `RawData`; `spectre_montecarlo` (deprecated alias, warns) | `add_spectre_monte_carlo` | `num_iterations, inner_analysis: str, inner_type="ac", seed=None` |
| `spectre_pac` → `AcAnalysis` | `add_spectre_pac` | `pss_fundamental, pss_stabilization, pss_harmonics=10, variation, points=100, start_freq=1, stop_freq=1e9, sweep_type="relative"` |
| `spectre_pnoise` → `NoiseAnalysis` | `add_spectre_pnoise` | `pss_fundamental, pss_stabilization, output_node, ref_node, …` |
| `spectre_pxf` → `AcAnalysis` | `add_spectre_pxf` | `…, output_node, source` (`source` accepted and not written) |
| `spectre_pstb` → `StabilityAnalysis` | `add_spectre_pstb` | `…, probe` |
| — | `add_spectre_analysis` | `kind, name=None, nodes=None, **params`; a Spectre statement, text only, no result |

`Testbench.run()` (testbench.rs:328) runs every queued analysis as one deck and returns a list, one result per analysis (`None` for a Fourier; a list of one result per step where the run stepped).

Internal analysis table: `enum Analysis` has 21 variants (analysis.rs:36-145), mapped to 22 `backend::Kind`s (`Sensitivity` is `Sens` or `SensAc`).

### 1.5 Outputs and measures (R, simulator.rs)

`save(*vectors)` (:91), `measure(*words)` (:96; the words of an ngspice `.meas` card, joined with blanks), property `save_currents` (:104-117), `Testbench.add_print(analysis, *outputs)`, `Testbench.add_fourier(...)`.

### 1.6 Options and settings (R, simulator.rs, circuit.rs)

`options(**kw)` (portable keys only, D17: `args.rs:80 kw_options` against `syntax::render::portable_option`), `initial_condition(**v)`, `node_set(**v)`, `step(param, start, stop, step)`, `step_sweep(param, start, stop, step, sweep_type)`; write-only properties `temperature`, `nominal_temperature` (setters, no getters); read-write `ngbehavior`, `save_currents`. `Circuit.options(**kw)`, `Circuit.temp(t)`.

### 1.7 Simulators, backends, registry

- R: `CircuitSimulator.available_backends()` (static), `check_backend(name) -> list[str]`, `Testbench.with_backend(name)`, `Circuit.simulator(simulator=None)`; `Simulator(name, **fields)` with fields `dialect, command, output, read, analyses, features, verilog, verilog_a` and the same nine read-only properties (sims.rs); `register_simulator(sim) -> str`; `simulators() -> list[str]`.
- P: `load_simulator(path)` (`simulator_file.py`, TOML `spicerack.simulator/1`, Python ≥ 3.11); `provisional = ("Simulator.read",)` (`__init__.py:11`).
- Environment: `SPICERACK_BACKEND` (backend/mod.rs:455), `SPICERACK_LTSPICE` (backend/ltspice.rs:61).
- Backend names accepted: the 5 runnable built-ins (`ngspice`, `ltspice`, `vacask`, `spectre`, `espice`), 3 aliases (`ngspice-subprocess`, `ngspice-shared`, `vacask-shared`; `src/syntax/dialect.rs:74`), any registered simulator, any plugin dialect with a `runs_on`.

### 1.8 Results (R, results.rs)

15 classes. `RawData`: `title`, `plot_name`, `is_complex`, `variable_names`, `measures`, `params`, `complex(name)`, `raw[name]`, `raw.name`. `OperatingPoint`: `op[name] -> float`, `op.name`, `measures`, `params`, `nodes`, `branches`. Thirteen swept classes from one macro (`analysis_class!`, results.rs:40-106): `DcAnalysis(.sweep)`, `AcAnalysis(.frequency)`, `TransientAnalysis(.time)`, `NoiseAnalysis(.frequency)`, `TransferFunctionAnalysis`, `SensitivityAnalysis`, `PoleZeroAnalysis`, `DistortionAnalysis(.frequency)`, `PssAnalysis(.time)`, `SParamAnalysis(.frequency)`, `HarmonicBalanceAnalysis(.frequency)`, `StabilityAnalysis(.frequency)`, `TransientNoiseAnalysis(.time)`; each has `res[name]` / `res.name` (a list of floats, the real part), `measures`, `fourier`, `params`, `nodes`, `branches`, `magnitude(name)`, `magnitude_db(name)`, `phase(name)`.

Module functions: `read_raw(path) -> list[RawData]` (R, mod.rs:181); `export(results, stem, *, raw_format="binary") -> (raw_path, meas_path)` (P `export.py` over R `_export`, export.rs:62).

### 1.9 Dialect definition and plugin API

- R: `spicerack.def_` (def_.rs, def_enums.rs, def_tag.rs): `DialectDef(name)` with 114 chained methods, `register(d) -> Lang`, `Lang` (11 class constants, `from_name`, `name`, `dump`, `runs_on`), `Card` (`head`, `words`, `params`, `text`), `Answer.Text(s)` / `Answer.Refuse(id)`, 45 vocabulary classes, alias `Keyword = Tag`.
- P: `Dialect(name)` base class and `register_dialect(obj)` (`dialects.py:18-48`); `spicerack.dialects.{NGSPICE, ESPICE, HSPICE, XYCE, LTSPICE, SPECTRE, VACASK, CDL, VERILOG}` (9 classes, each `d.extends(Lang.X)`).
- Detail in §6.

### 1.10 Units (R, unit.rs)

`Unit` (`value @ unit` through `__rmatmul__`), `UnitValue` (`value`, `str_spice()`, `__float__`, `__str__`), submodule `spicerack.unit` with 42 constants from `crate::unit::UNITS` (`src/unit.rs:37`).

### 1.11 Lint (R)

`lint(netlist: str, backend=None) -> {"warnings": [...], "errors": [...]}` (mod.rs:124 over `src/lint.rs`, 584 lines). It takes deck text, not a builder object.

### 1.12 Export, import, store

| call | where | what it does today |
|---|---|---|
| `Testbench.netlist(backend="ngspice")`, `Circuit.netlist(backend="ngspice")` | R testbench.rs:396, circuit.rs:151 | the deck in any of 9 dialects or a plugin; notes raised as `UserWarning` |
| `Testbench.netlist_with_notes(dialect) -> (text, [Diagnostic])` | R testbench.rs:403 | same deck, notes returned |
| `str(circuit)` | R circuit.rs:141 | the ngspice deck with `.title` |
| `Circuit["R1"]`, `Circuit.element(name)` | R circuit.rs:115 | one element's ngspice card (renders a one-element deck) |
| `to_source()`, `save(path)` / `save_source(path)` | R subcircuit.rs:528/544, circuit.rs:158/163, testbench.rs:343/365 | the stored ngspice text behind `* spicerack-source ngspice` |
| `Subcircuit.from_source(text, dialect=None)`, `.load(path)`; `Testbench.from_source(text, dialect=None)`, `.load(path)` | R subcircuit.rs:537/551, testbench.rs:354/372 | text read back; another dialect's text is first **written as ngspice** (`store.rs:359-379`) |
| `to_json`, `from_json`, `save_json`, `load_json` (Subcircuit and Testbench, 8 methods) | R | stubs raising `NotImplementedError` |
| `from_spice(text, dialect)` (D26) | — | **not found** |
| `variants`, `netlist_all`, `run_all`, manifest (D11, D23) | — | **not found** |
| `python -m spicerack` (D14 exit codes) | — | **not found** (no `__main__.py`) |

Outside Python: `src/bin/spice2rack.rs` (295 lines, the D55 CLI: `--dialect <in> --to <out> --kit --check -o`), `src/import.rs` (180 lines: `decode`, `parse`, `kit`, `dump`), and the Rust text door `syntax::render::render_text(text, d, name, target)` (D8).

### 1.13 Errors and diagnostics

- R (error.rs): `Error` (a `RuntimeError`) with `.status` (`"refused"` | `"failed"`) and `.diagnostics`; a backend error also carries `.diagnostic` (`{"code", "stage", "missing"}`) and `._simulator`. `Diagnostic` (frozen, 10 fields: `code`, `kind`, `title`, `cite`, `message`, `file`, `line`, `column`, `statement`, `element`). `ValueError`/`TypeError`/`KeyError`/`OSError` for edge errors.
- P (`diagnostics.py`): `diagnostics.netlist(testbench, dialect, out_dir, variant) -> (status, text)`, `diagnostics.run(testbench, out_dir, variant) -> (status, text, results)`, `diagnostics.SCHEMA = "spicerack.diagnostics/1"`. They write `<variant>.diag.json`, `<variant>.source.cir` and `<variant>.log`. Native halves: `_written(sim, dialect, queued)`, `_route(sim)` (stage.rs).
- JSON schema fixtures: `tests/fixtures/schemas/spicerack.diagnostics-1.schema.json`, `spicerack.measures-1.schema.json`.

### 1.14 Pure-Python layer

- `spicerack.testbenches` (15 files, 5 562 lines): classes `CornerCase`, `MetricSpec`, `MonteCarloPlan`, `ValidationRule`, `DesignBench`; 86 functions (65 bench recipes and helpers over 11 modules, 21 metric/corner helpers in `analysis.py`). Every recipe takes the `spicerack` module as its first argument `ps` and a DUT.
- `spicerack.remeasure` (371 lines): `read`, `sndr`, `settling`, `slew`, `loop_margins`, `noise_total`, `yield_ci`, `compare`.

### 1.15 Surface that already names a dialect

For the rewrite's "dialect-neutral" goal, these public names or arguments carry one simulator's meaning today: `ngbehavior`; the 13 `spectre_*`/`add_spectre_*` methods and `add_spectre_analysis`; `network_params` (LTspice `.net`); `raw_spice`, `R(raw_spice=)`, `extra_line(dialect=)`; `pss(sc_iter, steady_coeff, uic)`; `TransientNoise*Source`, `Random*Source` (ngspice TRNOISE/TRRANDOM); `A` (XSPICE); `O`, `Y`, `P`, `U` (ngspice line models); `V(portnum=, z0=)`, `distof1/2`; `noise(points_per_summary=)`; `measure(*words)` (ngspice `.meas` grammar); `save("V(out)")` (SPICE probe spelling); `options()` ("portable" is defined as the names of ngspice manual §11.1); `ModelLibrary(**backend_paths)` keyed by dialect family name; `step`/`step_sweep` (`.step`); the store header `* spicerack-source ngspice`.

---

## 2. How a builder call becomes deck content today (batch-22)

### 2.1 Answer in one paragraph

**Text that is re-parsed, for every call.** A builder call appends a `Card { name, text, source, needs }` or a `String` line to `Body`/`Bench` (`model.rs:31-96`). At write time `Deck::source(d)` (`deck.rs:444`) prints the whole deck as ngspice text; `Deck::build` (`deck.rs:429`) passes the title to `Build::new` and everything else to **one** `Build::text(rest)` call, which tokenizes and parses it (`build.rs:114-115`). The `(text, TokRows, Ast)` triple is rendered by `syntax::render` with `lang: Lang::NGSPICE`, `Numbers::Given`, `kit: None`. In debug builds only, `Ast::check` and `build::unlike_parse` re-read the text and compare (`deck.rs:406-411`). No call builds AST rows directly. The typed exception is `enum Analysis`, which is kept as data until `bench()` prints it.

### 2.2 Trace A: `sub.R("1", "a", "b", 1e3)` → bytes of `tb.netlist("spectre")`

1. `PySubcircuit::R` (`subcircuit.rs:106`): `PyValueArg::Float(1000.0)` → `PyValueArg::value` (`args.rs:130`) → `Value::Num(1000.0)`.
2. `PySubcircuit::two('R', …)` (`subcircuit.rs:636`): `nets` (:594), `kw_params` (`args.rs:43`), `Value::text` (`model.rs:200`) → `num` (`model.rs:211`) → `unit::format_spice_number` → `"1k"`; `format!("{letter}{name} {nets} {}{model}{ps}")` (`subcircuit.rs:652`) = `R1 a b 1k`; `push` (:607) → `Card::element` (`model.rs:136`) → `Body.elements`. Nothing is validated at call time.
3. `Testbench(sub)` (`testbench.rs:42`) clones `sub.inner` into `Deck.top`.
4. `PyTestbench::netlist` (`testbench.rs:396`) → `Deck::netlist` (`deck.rs:76`) → `written` (:96) → `written_from` (:107): the name → `Dialect::from_backend_name` filtered by `render::writes`, else a plugin `Lang`; `lowered()` (:310) clones the deck and lowers Verilog blocks; `wrote(lang)` (:383) → `write_with(lang, def_::render)` (:394).
5. `write_with`: `build(builtin_of(lang))` (:429) → `source(Dialect::Spectre)` (:444) prints the ngspice text (`body()` :527 writes `R1 a b 1k`) → `Build::new(title, NGSPICE_GIVEN lexicon)` (`build.rs:70`) + `Build::text(rest)` (`build.rs:99`: `tokenize` :114, `parse` :115, rows relocated by `Shift::rows` :204) → `finish()` (:150) → `Tokens::from_rows` (`deck.rs:402`) → `render::Source { ast, tokens, lang: Lang::NGSPICE, name, kit: None, loads: &top.osdi, numbers: Numbers::Given }` (`deck.rs:412-420`).
6. `def_::render(&src, lang)` (`def_.rs:135`) → `render::render_pending` (`src/syntax/render/mod.rs:156`) → `write_lang` (:324) → `read::read(ast, tokens, lang, name, kit)` (`render/read/mod.rs:359`) gives the plan `(Rd, Deck)` → `named::write` (`render/named/mod.rs:94`) → `W::elements` (:425) → `W::card(n)` (`render/named/cards.rs:29`), which asks `rd.card_view(n)` (`render/read/elements.rs:163`) for `Card::Rcl { letter: 'R', n1, n2, value, model, params }` (`cards.rs:34`) and writes `r1 (a b) resistor r=1k`.
7. `render::splice` (`render/mod.rs:189`) → `Written { text, notes }` → `error::locate` (`error.rs:92`) → `Wrote`. Back in `Deck::netlist`, each note is a `UserWarning` (`deck.rs:78-86`) and the text is returned.

### 2.3 Trace B: a PULSE/SIN source on the testbench

`tb.V("in", "in", "0", <wave>)` does not exist; the call is `tb.PulseVoltageSource("in", "in", "0", initial_value=0, pulsed_value=1.8, …)` (or `SinusoidalVoltageSource`).

1. Generated method (`source.rs:58-67` pulse, `:48-56` sin) inside `pymethods_with_waves!`, instantiated for `PyTestbench` at `testbench.rs:35`. Each field is extracted as `Num` (`args.rs:146`).
2. `Wave::new("PULSE", &[initial_value, pulsed_value, delay, rise_time, fall_time, pulse_width, period], zero = initial_value)` (`source.rs:168`): `format!("{form}({})")` (:171), each field by `Num::text` (`args.rs:162`: a number in plain decimal by `dec`, `model.rs:216`; an expression braced).
3. `PyTestbench::place_wave` (`testbench.rs:426`) → `wave_source` (`source.rs:235`: DC value is `dc=`, else the waveform's value at t = 0) → `source(...)` (`source.rs:200`: tail ` AC m [p]`, ` distof1 …`, ` portnum n z0 z`, the wave text) → `Card::source(name, "Vin in 0", &value, tail)` (`model.rs:147`) = `Vin in 0 0 PULSE(0 1.8 0 0.000000001 …)` → `PySimulator::stim` (`simulator.rs:43`) → `Bench.stimulus`.
4. `tb.V("in", "in", "0", 1.0, ac=1.0)` is `PyTestbench::V` (`testbench.rs:81`) → the same `source(...)` → `stim`.
5. At write: `deck.rs::bench` (:539-541) prints the card into the ngspice text; parser → `read` → `Card::Source { …, wave: Some(Wave { kind: WaveKind::Pulse, args }) }`; Spectre: `W::card` `Card::Source` arm (`render/named/cards.rs:125`) → `W::wave` (`render/named/sources.rs:73`) → `vin (in 0) vsource dc=… type=pulse …`.

### 2.4 Trace C: an analysis

Queued, then written: `PyTestbench::add_transient` (`testbench.rs:165`) → `card::transient` (`card.rs:82`) → `Analysis::Transient { step, stop, start, max_step, uic }` → `PySimulator::tb()` (`simulator.rs:38`) `.analyses.push`. At write, the loop in `deck.rs::bench` (:594-617) asks `a.log_sweep()` (W0040 refusal made here for a non-Spectre target), handles a counted Fourier per target, and prints `a.card()` (`analysis.rs:205`; `.tran` at :228 = `format!(".tran {} {}", num(step), num(stop))`) unless `a.in_spectre_region(d)` (`analysis.rs:357`), in which case `a.spectre()` (`analysis_region.rs:84`) goes into a `simulator lang=spectre` region (`deck.rs:618`, `region` :628). The parser reads `.tran 1n 1u`; the reader makes `Ana::Tran` (`render/read/analyses.rs:63-79`); the Spectre writer's `W::analysis` (`render/named/analyses.rs:31`, arm at :82) writes `tran1 tran step=1n stop=1u`.

Run-now: `PySimulator::transient` (`simulator.rs:204`) → `PySimulator::typed` (:425) → `PySimulator::deck(analysis)` (:416, **clones the whole `Deck`** and sets `analyses = [a]`) → `Deck::run_each` (`deck.rs:141`) → `for_run` (:270): `lowered()`, `route()` (:279) → `backend::select(&kinds, backend, needs, writes)` (`src/backend/mod.rs:435`) where `writes(sim)` is `self.write_to(sim.row().dialect).is_ok()` (**one full render per candidate simulator**), then `write_to(lang)` once more for the text → `simulation::run_each(sim, &text, &kinds)` (`src/simulation.rs:84`) → `run_all` (:32) → `Sim::run_all` → the built-in's `run` (e.g. `backend/ngspice.rs:40`) → `rawfile::parse_raw_all` → measures (`measure_parse::parse_measures`, `in_deck_order`, `at_step`) → `results::typed_result` (`results.rs:167`) → `PyTransientAnalysis { inner: Results::from_raw(raw) }`.

### 2.5 Every place the binding formats netlist text (file:line)

**Element cards, at call time**

| file:line | text |
|---|---|
| subcircuit.rs:652 | `R/C/L<name> n1 n2 value [model] params` (`two`) |
| subcircuit.rs:109, :124 | R's `raw_spice` tail; C's `q={expr}` |
| subcircuit.rs:156 | `K<name> L… k` |
| subcircuit.rs:218, :226, :235, :242 | `E`, `G`, `F`, `H` linear |
| subcircuit.rs:249, :265, :292, :307, :314 | `D`, `Q`, `M`, `J`, `Z` |
| subcircuit.rs:322, :330 | `S`, `W` (+ `args.rs:125` ` ON`/` OFF`) |
| subcircuit.rs:337 | `T… Z0= TD=` |
| subcircuit.rs:359 | `E/G/F/H… POLY(n) …` |
| subcircuit.rs:371 | `A<name> conns model` |
| subcircuit.rs:387-393 | `X<name> nodes master params` |
| subcircuit.rs:662 | `B<name> n1 n2 V=expr` / `I=expr` |
| subcircuit.rs:676 | `E/G<name> n1 n2 value={expr}` |
| lines.rs:32, :40, :61, :71-72, :83 | `O`, `Y`, `P`, `U`, ` LEN=` |
| source.rs:171, :180-187, :120 | `FORM(f0 f1 …)`, `PWL(…) r= td=`, AM's `(a)*(o)` product |
| source.rs:215-225, :231 | ` AC m p`, ` distof1/2 m p`, ` portnum n z0 z`, `V/I<name> np nm` |
| model.rs:148, :163, :172-174 | source card assembly, `set_value`, appended params |
| model.rs:203, :211-218, :221-232 | `{expr}`, `num`/`dec` number spelling, ` k=v` params |
| testbench.rs:98, :104 | Testbench `R`, `C` |
| args.rs:200-202 | a param value as text (`{:e}` or `str()`) |
| verilog.rs:78, :82-85, :96 | `.model … adc_bridge/dac_bridge`, `.model cosim_x d_cosim(…)`, the `A` card, synthesized `X` cards |

**Directive lines, at call time**

| file:line | text |
|---|---|
| model.rs:237-242 | `.model name kind (k=v …)` |
| model.rs:183-188 | `.lib path corner` / `.include path` for a `ModelLibrary` |
| subcircuit.rs:478 | `.func name(args) {body}` |
| subcircuit.rs:602 | `.ic V(n)=v` / `.nodeset V(n)=v` inside a body |
| section.rs:33-38, :69, :75, :81, :86, :100-101 | `.lib name … .endl name`, `.param`, `.model`, `.include`, `.lib path section`, a nested `.subckt` |
| block.rs:66-90, :98-118 | `.if (c)` / `.elseif (c)` / `.else` / `.endif` and the branch lines |
| store.rs:45-48, :57-85, :156-168, :239-252 | stored deck, `.subckt … .ends`, the header and `.control`/`pre_osdi`/`.endc`, the `* spicerack-library` marker |
| circuit.rs:143 | `.title` for `str(circuit)` |

**Analysis text, at write time**

| file:line | text |
|---|---|
| analysis.rs:205-329 | `.op` :211, `.dc` :213-218, `.ac` :220, `.tran` :228-241, `.noise` :250-258, `.tf` :260, `.sens` :261-264, `.pz` :269-274, `.disto` :276-280, `.pss` :292-304, transient-noise `.tran` :307, `.sp` :309-311, `.four` :319 |
| analysis.rs:378 | `.options nfreqs=N` |
| analysis.rs:388-396 | `snum`: Spectre scale letters |
| analysis_region.rs:50, :60 | LTspice `.four f n outs`; Spectre `fourh<i> (out 0) fourier fund= harms=` |
| analysis_region.rs:85-88, :115, :124-128, :133, :143-149, :157-161, :168-172, :185-190, :197, :203-208 | Spectre statements: PSS base, `pss`, `hb`, `stb`, `sweep { }`, `montecarlo { }`, `pac`, `pnoise`, `pxf`, `pstb` |
| spectre.rs:206-218 | Spectre `start= stop= dec=` / `values=[…]` |
| card.rs:300-335 | a generic Spectre analysis statement (`add_spectre_analysis`) |
| card.rs:338-345 | LTspice `.net … Rout= Rin=` |
| card.rs:130-136 | `v(node)` wrap for `.tf` |

**Deck assembly, at write time** (`deck.rs`)

| line | text |
|---|---|
| :450-473 | title, `.global`, `.func`, `.include`/`.lib` (`file_cards` :517), library setup includes and line, `.param`, models |
| :474-490 | `.subckt name ports PARAMS: …` … `.ends name` |
| :491-507 | a placed library's path rewritten inside raw lines; the top body (`body` :527) |
| :539-586 | stimulus, `.options`, `.options ngbehavior=`, `.temp`, `.options tnom=`, `.ic`, `.nodeset`, `.save`, `.print`, `.meas`, `.step` |
| :589-622 | the nfreqs card, the analyses, the Spectre and LTspice regions, each `extra_line` in its region |
| :628-637 | `simulator lang=<d>` … `simulator lang=spice` |
| :640-652 | the W0040 refusal text and its `spicerack.Error` |

**Pure Python**: `python/spicerack/testbenches/analysis.py:460-462` formats `parameters k=v` (Spectre) or `.param k=v` and passes it to `extra_line`.

### 2.6 Places the binding reads its own text back (sniffing)

| file:line | what is sniffed |
|---|---|
| deck.rs:186-194 | `has_rf_port`: the word `portnum` in any card or raw line |
| deck.rs:339-345 | `needs`: a raw line starting `.control` → `CONTROL_BLOCKS` |
| deck.rs:198-216 | `Deck::card`: renders a one-element deck and drops its first and last lines |
| deck.rs:154-161 | `to_source`: finds a library's line by string equality to insert its marker |
| subcircuit.rs:614-630 | `inductor`: an `L` name exists if some card's or raw line's first word equals it |
| subcircuit.rs:702-707 | `is_laplace`: the substring `laplace(` → `LAPLACE` feature bit |
| subcircuit.rs:712-717 | `lacks_value`: whether `raw_spice=` opens with `k=` |
| verilog.rs:76 | whether a `.model` of a kind exists, by the third word |
| store.rs:94-150, :180-232, :256-284, :313-354 | `from_source`, `read_deck`, `libraries`, `head`, `words`: tokenize + parse, then slice statements back to text |
| card.rs:128-141, :350-377 | `.tf`/`.noise` output: `v(…)`/`i(…)` recognised by prefix |
| analysis_region.rs:217-224 | `bare`: strips `V(…)` for Spectre terminals |
| block.rs:98-118 | a branch's raw lines starting with `.` |

---

## 3. The same trace on main, for contrast

1. `sub.R(...)` → `PySubcircuit::two` → `format!("{letter}{name} {nets} {}{ps}")` → `Card::element` (`main:src/python/subcircuit.rs:102`, `:482-493`): the same text card (no `model=` word, no flags).
2. `tb.netlist(b)` → `Deck::netlist` (`main:src/python/deck.rs:60`): target is a `Dialect`, only HSPICE or a runnable one (no Xyce, CDL, Verilog, no plugin `Lang`).
3. `Deck::write` (`main:deck.rs:229`): `source(d)` (:253) → `syntax::tokenize(&source, &lexicon::NGSPICE_GIVEN)` (:233) → `syntax::parse` (:234) → `render::Source { dialect: Dialect::Ngspice, kit: None, numbers: Given }` → `render::render(&src, d)` (:244).
4. A refusal is `PyRuntimeError(diags.messages)`: no `spicerack.Error`, no `Diagnostic` objects, notes dropped.
5. A run is `backend::select(first_kind, backend, needs) -> Dialect` (`main:deck.rs:170-176`): chosen by the first analysis only, no `Sim` registry, no `writes` filter; `run_each` returns `Vec<Option<RawData>>` (no steps).

Net difference: batch-22 adds a `Build` shim, `Lang` targets and plugins, typed errors with located diagnostics, the simulator registry and routing by every analysis, steps, export, diagnostics files, `Block`/`Section`, the `def_` module. The data path from a builder call to bytes is unchanged: text → parser → renderer. Binding size: main 16 files / 4 612 lines; batch-22 29 files / 9 210 lines.

---

## 4. What the pure-Python layer needs from the binding

Source: `python/spicerack/testbenches/*.py` (15 files, 5 562 lines) and `remeasure.py` (371 lines). Counts are `grep` hits.

**Construction**: `ps.Testbench(dut)` (48), `ps.Subcircuit(name, [])` (3, harness wrappers), `ps.ModelLibrary(path, corner=)` (1); DUT properties `dut.name`, `dut.ports`, `dut.gnd`.

**Builder methods**

| method | hits | notes |
|---|---|---|
| `V` | 49 | keyword form `V(name=, positive=, negative=, value=, ac=)` |
| `R` | 22 | on `Testbench` and on harness `Subcircuit`s |
| `C` | 12 | |
| `I` | 10 | |
| `PieceWiseLinearVoltageSource` / `…CurrentSource` | 14 / 2 | |
| `SinusoidalVoltageSource` | 9 | 2 use the `dc_offset=` alias (`amplifiers_2.py:342`, `timing.py:385`) |
| `PulseVoltageSource` / `…CurrentSource` | 4 / 1 | |
| `L`, `E`, `G`, `X`, `instance` | 2, 2, 3, 1, 2 | harness subcircuits only |
| `add_subcircuit` | 3 | |
| `save` | 37 | always `"V(node)"` / `"I(Vsrc)"` strings |
| `options` | 7 | `reltol`, `vntol`, `abstol`, `seed` |
| `measure` | 1 | `tb.measure("ac", "gain_at_1khz_db", "find", f"vdb({node})", "at=1k")` (`design.py:112`) |
| `initial_condition` | 1 | `**{node: value}` |
| `temperature`, `nominal_temperature` setters | 4, 4 | |
| `with_backend` | 3 | |
| `use_pdk`, `extra_line`, `add_spectre_monte_carlo` | 1, 1, 1 | all in `analysis.py` |
| `netlist` | 4 | |

**Analyses**: queued `add_operating_point` (10), `add_dc` (11), `add_ac` (13), `add_transient` (15), `add_noise` (1); run-now `dc` (8), `ac` (15), `transient` (11), `noise` (2). **`Testbench.run()` and `operating_point()` are never called**: a recipe queues `add_*` so `netlist()` shows the analyses, and its `runner` lambda calls the run-now twin (which replaces the queue with its one analysis).

**Result accessors**: `.time` (58), `.frequency` (33), `.sweep` (21), `res[name]`, `getattr(res, name)`, `.magnitude_db(name)` (12), `.magnitude(name)` (4), `.phase(name)` (3), `.measures` (1); vector names `i(v<name>)` (10 literal uses), `onoise_spectrum`, `inoise_spectrum`. `remeasure` uses `read_raw`, `RawData.plot_name`, `.variable_names`, `raw[name]`, `.complex(name)`.

Not used by the layer: `Circuit`, `Block`, `Section`, `conditional`, `raw_spice`, `step`, `add_fourier`, `.fourier`, `.nodes`, `.branches`, `.params`, any HB/PSS/S-parameter/stability/pz/sens/tf analysis, `export`, `diagnostics`, `def_`, the simulator registry.

**Is anything dialect-specific? Yes, six things.**

1. `analysis.py:460-462` (`CornerCase.apply_to`): `card = "parameters" if backend == "spectre" else ".param"` then `tb.extra_line(f"{card} {key}={value}", dialect=backend)`. Dialect text is formatted in Python and sent through an escape hatch D26 removes. `Testbench.parameter(name, value)` now exists and would replace it.
2. `analysis.py:547-569` (`MonteCarloPlan`): Spectre only, raises for any other backend; calls `add_spectre_monte_carlo(samples, "tran1", "tran", seed)` with the inner analysis named by a string.
3. `analysis.py:536-537` (`run_corners`): the seed is `options(seed=n)`, an ngspice option name.
4. `timing.py:300`: `if "trnoise" not in tb.netlist("ngspice").lower()`: a recipe inspects ngspice deck text to decide whether to build.
5. `design.py:112`: the `.meas` words are ngspice's (`find vdb(out) at=1k`); `save()` strings are SPICE probe spellings throughout.
6. `remeasure.compare` reads ESPice's own `espice-measures/1` JSON; `analysis.py:350-414` parses Spectre `mcdata`/`mcparam` scalar files and HSPICE `.mt0/.ms0/.ma0` names.

Also: backend defaults are the string `"ngspice"` in 9 recipe signatures (`characterize.py`, `analysis.py`).

---

## 5. Run path and results

### 5.1 Choosing a backend

`Deck::route` (`src/python/deck.rs:279-307`): the kinds of every queued analysis (`Analysis::kind`) and the deck's feature needs (`Deck::needs`, :327: `OSDI`, `VERILOG_COSIM`, `CONTROL_BLOCKS`, per-card `XSPICE`/`LAPLACE`, `MEASURES`, `STEP_PARAMS`) go to `backend::select(kinds, override, needs, writes)` (`src/backend/mod.rs:435`).

- Candidates: the first kind's route (`Kind::spec().route`, `backend/kind.rs:123`) then registered simulators that declare the kind, kept where every other kind's route has them.
- An explicit name (argument, else `$SPICERACK_BACKEND`) wins if it declares every kind and has every needed feature bit; else `W0180` / `W0181`.
- Otherwise the first installed candidate with every feature bit **whose writer writes the deck** (`writes` = a full render); a last resort lands on a built-in for universal analyses.
- A plugin dialect name that is no simulator's runs on its `runs_on` simulator and is written as the plugin (`deck.rs:284-302`).
- `PySimulator::run_spectre` (`simulator.rs:435`) forces `"spectre"` for the Spectre-only run-now methods.

### 5.2 Writing, invoking, reading

1. `Deck::for_run` (`deck.rs:270`) renders the deck for the chosen simulator's language. A default-routed run therefore renders once per tested candidate plus once for the text; `diagnostics.run` routes and writes a third time (`diagnostics.py:87`, marked `ponytail:`).
2. `simulation::run_each(sim, text, kinds)` (`src/simulation.rs:84`) → `run_all` (:32) → `Sim::run_all` (`backend/registry.rs`): `Runner::BuiltIn(fn)` (one of `ngspice.rs:40`, `ltspice.rs:19`, `vacask.rs:17`, `spectre.rs:29`, `espice.rs:24`), `Runner::Command { argv, output }` (`command.rs:143`) or `Runner::Hook { argv, read, id }` (`command.rs:167`, provisional). Each writes the deck to a scratch directory, runs one subprocess, reads the results.
3. Reading: `rawfile::parse_raw_all(bytes) -> Vec<RawData>` (`src/rawfile.rs:55`; nutmeg binary/ASCII, LTspice UTF-16, Spectre nutascii); LTspice also reads its `.log` (`ltspice.rs:47`, `log_steps` :120); VACASK reads one raw file per analysis (`vacask.rs:48`).
4. Measures: `measure_parse::parse_measures(text, family)` (`src/measure_parse.rs:99`) from stdout (LTspice: the log), then `in_deck_order(deck, rows, family)` (:136) and `at_step(rows, k)` (:180). `.four` tables: `parse_fourier` (:191). There is **no `src/measure.rs`**: D7's evaluator on raw data (VACASK `.meas`/`.four`) is not on batch-22.
5. Matching plots to analyses: by `Plotname` prefix (`Kind::spec().plot`: `Named`, `NamedWith`, `Table`, `Unmatched`), with ngspice's "last card of a kind runs first" rule (`simulation.rs:127-158`); VACASK by position; LTspice by running each analysis card alone (`ltspice_each`, :186).
6. Typing: `results::typed_result(py, analysis, plots)` (`src/python/results.rs:167`) → one result class per `Analysis`, or a list of one per step (D42). `result::Results::from_raw` (`src/result.rs`) builds the name → column maps through `normalize::canonical(name, backend_hint)` (`src/normalize.rs:24`).

Types: `result::RawData { title, plot_name, variables, real_data, complex_data, is_complex, stdout, log_content, measures, backend_hint, params }`, `VarInfo`, `VarType`, `MeasureResult { name, value, status, step, analysis }`, `Results { raw, nodes, branches }`.

### 5.3 What depends on `src/syntax` internals, and what only on "deck text for dialect X"

Imports of `crate::syntax` in the run path (everything else is independent):

| file | import | role |
|---|---|---|
| backend/mod.rs, kind.rs, command.rs, result.rs, normalize.rs, measure_parse.rs | `syntax::Dialect` | an id for the simulator family |
| backend/registry.rs | `syntax::{Dialect, Lang}`, `syntax::def::RunsOn`, `syntax::diag::Code` | a simulator's language; a plugin's `runs_on`; `R0040`–`R0042` |
| backend/mod.rs | `syntax::diag::Code` | `W0180`–`W0182`, `R0041` |
| backend/ngspice.rs | `syntax::render::NGBEHAVIOR` | the comment line carrying `ngbehavior` to the command line |
| simulation.rs | `syntax::render::LTSPICE_EACH` | the comment line before LTspice's analysis cards |

No AST, token, reader or writer type is used. The dependence is on **text shape**: the run path re-scans the written deck with line scanners.

| file:line | scanner | what it assumes about the writer's output |
|---|---|---|
| backend/ngspice.rs:79, :95, :103, :110, :128 | `runs_in_control`, `is_meas`, `is_four`, `ngbehavior`, `append_write_block` | a `.control` block that runs the analysis; the `*spicerack ngbehavior=` line |
| simulation.rs:186-231 | `ltspice_each` | analysis cards one per line after the `LTSPICE_EACH` line |
| simulation.rs:238-255 | `commands_run` | the ngspice writer's command names inside `.control` |
| simulation.rs:273-310 | `outer_sweeps` | a `.dc` card (or `dc` command) with eight plain-number fields |
| backend/vacask.rs:88, :116, :195, :222 | `analysis_names`, `outer_sweeps`, `raw_files_for`, `absolutize_paths` | VACASK `analysis <name> …` lines, `sweep` nesting, `load`/`include` paths |
| backend/spectre.rs:50 | `hb` | `<name> hb …` on one line |
| backend/ltspice.rs:27-34 | — | the title on line 1, options inserted after it |
| measure_parse.rs:462-507 | `cards`, `commands`, `subject`, `reads_files` | `.meas` cards and `meas` commands by first words |

A rewritten core can hand these facts over as data beside the text (the analyses in run order with their kinds and written names, the measure names in deck order, the ngspice compatibility mode, outer-sweep values, VACASK raw-file names) and delete the scanners.

Side effects inside "write": lowering a Verilog block runs Icarus Verilog or Yosys (`src/python/verilog.rs:67` → `veriloga::lower`), and writing an ngspice or VACASK deck with a Verilog-A load runs OpenVAF (`veriloga::osdi_loads`, called from `src/syntax/render/spice/mod.rs:325` and `render/named/vacask.rs:134`). So `netlist()` is not pure today.

### 5.4 The simulator registry (D38) surface

Rust (`src/backend/registry.rs`, 480 lines; `command.rs`, 304): `SIMS_MAX = 32`; `Sim(u8)` handle; `SimDef { name, dialect: Lang, runner, features: u8, analyses: u32, installed }`; `Runner::{BuiltIn, Command { argv, output }, Hook { argv, read, id }}`; `Output::{NgspiceRaw, NgspiceAscii, RawDir, LtspiceRaw, Nutascii}` (names `ngspice-raw`, `ngspice-ascii`, `raw-dir`, `ltspice-raw`, `nutascii`); `Piece::{Lit, Deck, Raw, Dir}`; `register_simulator`, `simulators`, `by_name`; seven feature bits (`xspice`, `osdi`, `measures`, `step`, `control`, `laplace`, `verilog`); 22 kinds with 19 distinct names.

Python: `Simulator(...)`, `register_simulator`, `simulators`, `load_simulator`, `provisional`, `DialectDef.runs_on(sim)`, `Lang.runs_on()`; codes `W0180`, `W0181`, `W0182`, `R0040`, `R0041`, `R0042`; `exc.diagnostic`; the `simulator` object of `spicerack.diagnostics/1`. Provisional and present: `Simulator(read=fn)` (`sims_read.rs`, 230 lines). Provisional and **not found**: `Dialect(emit=fn)` and its `View` (P8-208); the snapshot test already lists them as excluded names (`tests/test_python_api_snapshot.py:32`).

Tests: `tests/test_simulators.py` (400), `tests/test_simulator_oracles.py` (274), `tests/test_sim_registry.rs` (242), `tests/test_sim_command.rs` (334), fixture `tests/fixtures/sims/fake`.

---

## 6. Dialect definition API

### 6.1 What a user can declare

A `Dialect` subclass whose `define(d)` chains `DialectDef` calls and returns it; `register_dialect(obj)` registers it and returns a `Lang`. The calls are the Rust `syntax::def::DialectDef` calls one for one (`def_.rs:70-84`, macro `calls!`):

- identity: `extends`, `renamed`;
- lexicon (lines, words, operators, numbers): `title`, `fold_case`, `comment_line`, `comment_col1`, `blank_led_comment`, `cont_skip`, `comment_tail`, `block_comment`, `long_strings`, `dollar`, `plus`, `backslash`, `end`, `stop`, `join`, `quote`, `string`, `string_expr`, `name_escape`, `ident_start`, `ident_continue`, `directive`, `head_words`, `head_after`, `operators`, `operator`, `expr_braces`, `caret`, `infix`, `right`, `nonassoc`, `ternary_nesting`, `no_infix`, `prefix`, `prefix_bp`, `probe`, `index`, `lib_sections`, `exponent`, `scale`, `no_scale`, `tail`, `arithmetic`, `keywords`, `keyword`, `named`, `devices`, `device`, `master`, `analysis`, `region`;
- spelling: `comment`, `path_quote`, `expr`, `func`, `subckt_params`, `ngspice_numbers`, `write_lexicon`;
- capability cells per construct: `native`, `spell`, `lift`, `refuse`, `omit`, `copy`;
- behaviours: `writer`, `caret_power`, `hier_sep`, `number_reading`, `model_semantics`, `source_rules`, `card_rules`, `named_rules`, `options_form`, `load_form`, `control_block`, `analysis_form`, `end_rule`, `value_quoting`, `coupling`, `param_form`, `include_scope`, `named_params`, `analysis_masters`, `expr_eval`, `parasitics`, `netlist_rules`, `subckt_multiplicity`, `text_family`, `spectre_analysis`, `temp_name`, `builtin_constant`, `builtin_device`, `vacask_analysis`, `option_to`, `option_spell`, `option_method`, `option_cosmetic`, `model_load`, `spice_model_type`, `level_alias`, `model_master`, `kept_word`, `dot_rule`;
- plugin-only: `code`, `refuse_code`, `omit_code`, `write_head`, `write_master`, `option`, `hook(head, f)`, `runs_on(sim)`, `regions_registered`.

A `hook` receives the **written cards** of one head as text views (`Card.head`, `.words`, `.params`, `.text`) and returns `Answer.Text(str)` or `Answer.Refuse(id)` per card (`def_.rs:458-515`).

### 6.2 Size of the surface

- `DialectDef`: 114 chained methods (107 from the `calls!` list plus `renamed`, `extends`, `runs_on`, `model_master`, `end`, `device`, `hook`) and its constructor.
- 45 vocabulary classes: `Tag` (108 members), `Construct` (47), `Code` (115), `Analysis` (40), `Command` (11), `Simulator` (9, the built-in dialects: a different thing from `spicerack.Simulator`), `Alt` (28 units + `Omit`, `Copy`), `Rule` (7), plus `Join`, `Dollar`, `Backslash`, `Caret`, `Nesting`, `Arithmetic`, `Keywords`, `Operators`, `Devices`, `FuncForm`, `Kind`, `Writer`, `NumberReading`, `ModelSemantics`, `SourceRules`, `CardRules`, `NamedRules`, `OptionsForm`, `LoadForm`, `ControlBlock`, `AnalysisForm`, `EndRule`, `ValueQuoting`, `Coupling`, `ParamForm`, `IncludeScope`, `NamedParams`, `ExprEval`, `Parasitics`, `NetlistRules`, `Scale`, `Quote`, `Tail`, `DotRule`, `Statement`, `Quoting`, `End`.
- `Lang`: 11 constants (9 dialects, `NGSPICE_GIVEN`, `SPICERACK`), `from_name`, `name`, `dump`, `runs_on`.

The vocabulary is the current front end's internals by name: token tags, the construct list, the writers' alternative spellings and lift rules, and 20 behaviour enums, most with variants named after dialects (`CardRules.Hspice`, `LoadForm.Espice`, `Writer.Vacask`, …).

### 6.3 Who uses it

- **The built-ins themselves use the Rust API**: `src/syntax/dialects/*.rs` (12 files, 2 359 lines) declare every built-in with `DialectDef` calls, so the Rust definition layer is load-bearing for the core.
- **The Python binding of it** has these consumers in the two trees: tests (`tests/test_dialect_def.py` 15 tests, `tests/builtin_dialects.py`, and uses in `test_simulators.py`, `test_python_errors.py`, `test_python_diagnostics.py`, `test_python_api_snapshot.py`), docs (`docs/dialects.md`, executed by `tests/test_docs.py`), the skill pages (`skills/spicerack/SKILL.md:70`, `REFERENCE.md:262-272`). No file under `examples/` and no file under `python/spicerack/testbenches/` uses it. A consumer outside these two trees: not found (not searched beyond them).
- `tests/builtin_dialects.py` (1 687 lines) re-declares every built-in call by call in Python; it exists only as the parity oracle for the binding.

### 6.4 Code that exists only to support user-declared dialects

| part | lines |
|---|---|
| `src/python/def_.rs`, `def_enums.rs`, `def_tag.rs` | 533 + 437 + 129 = 1 099 |
| `python/spicerack/dialects.py` | 70 |
| `src/syntax/def/plugin.rs` (codes, heads, masters, hooks, `runs_on`, regions) | 158 |
| `src/syntax/render/def_pass.rs` (the plugin post-pass over written text) | 366 |
| `src/syntax/def/dump.rs` (`Lang.dump()`, the parity oracle's form) | 139 |
| product code subtotal | 1 832 |
| tests: `builtin_dialects.py` 1 687, `test_dialect_def.py` 264, `dialect_defs.rs` 295, `test_dialect_plugin.rs` 551, `test_dialect_full.rs` 27, `test_dialect_kit.rs` 52 | 2 876 |
| docs: `docs/dialects.md` 204, `tests/fixtures/transpiler/DIALECT-PLUGIN.md` 1 046 | 1 250 |

The rest of `src/syntax/def/` (`mod.rs` 452, `lex.rs` 308, `tables.rs` 224, `freeze.rs` 289, `copy.rs` 102) and `lang.rs` (421), `behaviour.rs` (527), `render/capability.rs` (483) serve the built-ins too. `render::render_pending` / `render::splice` (`render/mod.rs:156-216`) exist for external hooks.

---

## 7. User decisions that bind the API

Status is what I found on batch-22.

| # | decision, in one line | status on batch-22 |
|---|---|---|
| D8 | No Rust circuit builder; builders are Python-only; Rust consumers use text in / text out through `render_text` | **holds**: `syntax::build::Build` is `pub(crate)`; `render_text` is the public door (`render/mod.rs:254`) |
| D10 | A testbench's results hand-off is a standard raw file plus a JSON sidecar for `.meas` values | **implemented**: `export()` (`export.py`, `export.rs`), `rawfile::write_raw`, `spicerack.measures/1` |
| D11 | Batch path: `tb.variants(corners=, sweeps=, mc=N)` → `.netlist_all(dialect, out_dir)` / `.run_all(backend)` → a manifest, one Python process | **pending**: `variants`, `netlist_all`, `run_all`, manifest not found |
| D13 | Ships as fast-spicerack 1.0.0; builder API and results/diagnostics contracts semver-stable, enforced by a public-API snapshot test | **partial**: the snapshot test exists with 17 lines (D38 roots and `parameter`); version is 0.2.0 |
| D14 | A failed netlist/run exits nonzero and writes a JSON diagnostics file beside the outputs | **partial**: `spicerack.Error`, `Diagnostic`, `diagnostics.netlist/run`, `.diag.json` are there; the process entry (`python -m spicerack`, exit codes) is not found |
| D15 | Python builders build the AST directly, not text through the reader | **pending**: step 1 of 7 only (`Build::text` re-parses the whole deck) |
| D16 | Builder expression strings use a neutral SpiceRack syntax | **partial**: `Lang::SPICERACK` reads the grammar and notes `P0020`–`P0026` (`src/syntax/dialects/spicerack.rs`, 63 lines, "a reader only: written, it is ngspice"); no builder uses it; `ExprEval::Neutral` and the lowering table not found |
| D17 | Python options: portable keys only; simulator-only options through the text escape hatches | **implemented** (`args.rs:80`); the error text sends the user to `extra_line` |
| D18 | Renames before the freeze: `add_sweep`/`add_monte_carlo` with an analysis handle, `add_spectre_analysis(kind, **params)`, K and step signatures fixed, hatches kept and documented | **partial**: `K(name, *inductors, coupling)` and `add_spectre_analysis` done; `add_sweep`, `add_monte_carlo`, handles and the one `step` not found; the last clause is superseded by D26 |
| D19 | Spectre-only analyses through one generic `add_spectre_analysis`, validated against the Spectre analysis table | **implemented** (`testbench.rs:317`, `card.rs:300`): the kind is validated, the values are verbatim Spectre text |
| D20 | (superseded by D26) the escape hatches stay | n/a |
| D21 | Keep both `Circuit` and `Testbench`, made consistent | **partial**: both exist; `Circuit.netlist`, `to_source`, `save` exist; `add_subcircuit` vs `subcircuit`, `with_backend`, `run` still differ |
| D22 | No aliases survive 1.0; one name per method | **pending**: `BJT`, `MOSFET`, `add_tf`, `tf`, `spectre_montecarlo`, `instance`, `element`, `dc_offset`, `ac_magnitude`, Pulse's `delay_time`, the 8 JSON stubs are all present |
| D23 | `variants(mc=N)` generates N seeded decks on every simulator, now | **pending** (no `variants`) |
| D24 | 1.0 freezes signatures, JSON schemas and diagnostic codes; netlist bytes are not frozen | **partial**: two schema fixtures and 115 codes exist; signatures unfrozen except the 17 snapshot lines |
| D25 | `C`/`L`/`V` take `rser=`, `lser=`, `rpar=`, `cpar=`; LTspice writes them, others expand | **pending**: not found in the builders (the writers already expand parasitics read from LTspice text) |
| D26 | No escape hatches: `raw_spice`, `extra_line`, `R(raw_spice=)`, `from_source` removed; `from_spice(text, dialect=...)` reads any registered dialect straight into the AST | **pending**: all four hatches present, `from_spice` not found |
| D38 | Custom simulators, stable core: `Simulator` + `register_simulator`, TOML `load_simulator`, a plugin dialect whose `runs_on` takes a registered simulator, stable codes; `emit`/`read` provisional | **implemented** except the provisional `emit`/`View` |
| D39 | PDK-neutral builders, PDK bindings as data, bound and unbound dumps, comment/hint round trip, codes from S3's range (note 13) | **pending**: none of `nmos…diode`, `comment`, `load_pdk`, `spicerack.pdk/1`, `W0183`–`W0189`, `R0043`–`R0045`, `P0050` found |
| D42 | Stepped results: one `RawData` per step with `params`; Python results gain `.params`; a stepped run returns one result per step | **implemented** (`result.rs`, `results.rs:79`, `:167-180`) |
| D48 | Run-now twins renamed `CircuitSimulator.sweep(...)` / `monte_carlo(...)`, taking the analysis value the `add_*` builders make | **pending**: still `spectre_sweep`, `spectre_monte_carlo`, plus the deprecated `spectre_montecarlo` |
| D55 | SpiceRack ships `packages.<system>.spice2rack` (no Python); general CLI `--dialect <in> --to <out>`; ESPice's target is `espice` | **implemented** (`flake.nix:359-377`, `:520`; `src/bin/spice2rack.rs`) |

### 7.1 Conflicts

**With "Python builders construct a neutral IR directly":**

- **C1. D15 as designed is text-first.** `DESIGN-python-ast-builders.md` §2: the builder writes the source buffer, token rows and AST rows together; a construct with no ngspice spelling lives as home-dialect text in a `simulator lang=<d>` region; §3 makes "built rows == parse of the built text" the guard; §6 makes `to_source()` the buffer verbatim. An IR with no source bytes has none of these. The decision (builders do not go through the reader) survives; the design note does not.
- **C2. D14 + D24: diagnostics are located in a source text.** `Diagnostic.line`, `.column`, `.statement` and the file's `source` field point into `<variant>.source.cir`, "the deck's ngspice-given source text" (`api-parity.md` §6c; `diagnostics.py:26-41`; `error.rs:92-126` computes them from tokens). An IR-built deck has no such text. Either the core writes a canonical text for located diagnostics (and keeps a statement → line map), or the frozen schema changes to locate by element and Python call site (the design note's §5 `"python": {"file", "line"}` field is not implemented).
- **C3. D11's job id** is defined as a hash of "the ngspice-given source of the base deck" (`api-parity.md` §6a field rules). It needs a new definition over the IR.
- **C4. D19 is an escape hatch by another name.** `add_spectre_analysis` stores a Spectre statement line (`testbench.rs:319-320` pushes into `extra_lines` with `Lang::SPECTRE`); its values are "written as given, so a device it names is spelled as `netlist("spectre")` spells it" (`docs/reference.md:126`). A neutral IR needs an explicit "dialect-private statement" node to hold it, which is what D26 removes for every other dialect.
- **C5. `measure(*words)` stays ngspice grammar.** `api-parity.md` §6e keeps it word-level and PySpice-identical. D7 (the backend evaluates measures on raw data for VACASK) and a neutral IR both need a typed measure spec, so the ngspice words must be parsed at the Python edge. Same for `save()`/`add_print()`/`add_fourier(outputs=)` probe strings.
- **C6. D39's verbatim-parameter contract.** "A neutral expression is written verbatim, as the source text of the call: `w=W6 l=L5` stays `w=W6 l=L5`" (`DESIGN-pdk-binding.md` §4, oracle B8). An IR that normalizes expressions must keep the original spelling or have a printer that is a fixpoint on it. D39's comment round trip also requires positioned comment nodes in the IR.
- **C7. D38's plugin model is defined on today's internals.** The Python `DialectDef` mirrors the Rust lexicon/behaviour/capability calls 1:1, and a hook edits written card text. Two snapshot lines (`def_.DialectDef.runs_on`, `def_.Lang.runs_on`) pin the existence of `spicerack.def_.DialectDef` and `Lang`; nothing else of `def_` is in the snapshot. The not-yet-built `emit(view)` hook, by contrast, is specified over reader views with neutral-expression values and fits an IR.

**Between decisions:**

- **C8. D17 vs D26.** D17 routes simulator-only options "through the text escape hatches"; D26 deletes the hatches. After D26 a non-portable option has no builder door (`api-parity.md` Q3–Q5: "not built; `from_spice` of a source deck reads it"). `kw_options`'s message (`args.rs:87`) still names `extra_line`.
- **C9. D18 vs D26.** D18's last clause keeps `raw_spice`/`extra_line`/`from_source`; D26 supersedes it (the ledger marks D20, not D18, as superseded).
- **C10. D48 presupposes analysis values and handles.** "The same analysis value the `add_*` builders make" needs `add_*` to return something (`api-parity.md` A11); today they return `None`, the inner analysis is a string name (`"ac1"`), and no analysis type is exposed to Python.
- **C11. D21 + D22 on `subcircuit`.** One name `subcircuit()` on every class (`api-parity.md` §6d F2) gives two meanings: on `Subcircuit` a nested, scoped definition (`subcircuit.rs:403`); on `Circuit`, a subclass, a top-level definition (`circuit.rs:101`); on `Testbench` today `add_subcircuit`. The override already exists on batch-22.
- **C12. D22 vs the shipped layer.** The pure-Python benches use names D22/D26 remove: `instance`, `add_subcircuit`, `with_backend`, `extra_line`, `dc_offset=`, `add_spectre_monte_carlo`. They must move with the API.
- **C13. D13/D24 vs the plan itself.** The freeze is not in force (0.2.0, 17 snapshot lines), so nothing blocks a rewrite; but the docs already describe the D38 surface, the two JSON schemas and the codes as frozen at 1.0.

---

## 8. Contract for a new core (inferred)

The binding lives in the same crate (`src/python`), so D8 holds with a `pub(crate)` builder.

### 8.a IR-builder operations

Scopes: a deck top, a subcircuit definition (name, ports, parameter defaults), a conditional branch body, a library section, a bench.

| verb | data |
|---|---|
| open scope | `deck(title)`; `subckt(name, ports[], defaults[(name, value)])`; `block()`; `section(name)` |
| add element R/C/L | name, 2 nodes, value (number or expression) or model word, params; C alternative: a charge expression |
| add coupling K | name, ≥ 2 inductor references (by element name, validated), coupling (number or expression) |
| add source V/I | name, 2 nodes, DC value, optional AC magnitude and phase, optional distortion inputs `(mag, phase)` ×2, V: optional RF port `(num, z0)`, I: params, optional waveform |
| waveform | kind ∈ {SIN, PULSE, PWL, EXP, SFFM, AM, TRNOISE, TRRANDOM}; fields each a number or expression; PWL: points `[(t, v)]`, repeat, delay; TRRANDOM: a distribution keyword |
| add controlled source | E/G: 4 nodes, gain (G: params); F/H: 2 nodes, a sensing-source reference, gain (F: params) |
| add behavioural source | letter B (V or I) or E/G; 2 nodes; an expression over node voltages and branch currents; params (`tc1`, `tc2`, `temp`, `dtemp`, `m`) |
| add POLY | letter E/G/F/H; 2 nodes; controls (node pairs or source references); coefficients `f64[]` |
| add semiconductor | D: 2 nodes; Q: 3–5 nodes; M: 3, 4 or 4 + extra nodes; J/Z: 3 nodes; each with a model reference and params (flags allowed) |
| add switch | S: 4 nodes; W: 2 nodes + a control-source reference; model; optional initial state |
| add line | T: 4 nodes, Z0, TD; O: 4 nodes, model; Y: 4 nodes, model, optional length; P: n inputs + ground + n outputs + ground, model, optional length; U: 3 nodes, model, length, optional lumps |
| add code-model instance A | name, connection strings (XSPICE port syntax), model |
| add instance X | name, master name, nodes[], params |
| add model | name, kind word, params (values: number, flag, or opaque text such as a matrix) |
| add parameter | name, value; on a bench: replace an existing one by name |
| add function | name, args[], body expression |
| add include / lib / model library | path; (path, section); (path, corner, setup includes, per-dialect paths) |
| add load | an OSDI binary or a Verilog-A source (path, or inline source resolved to a path) |
| add Verilog block | source, mode (simulate/synthesize), instance name, port → net or bus, pdk, liberty, spice models |
| add definition | a subcircuit registered at the top, or nested in a subcircuit, or inside a section |
| add conditional | `[(condition expression, block)]`, optional otherwise block |
| add section | a section into a library deck |
| add initial condition / nodeset | scope (bench or body), node (may be hierarchical `x1.n`), value (number or expression) |
| add global nodes | names[] |
| bench: stimulus | V, I, R, C and the 16 waveform sources, on the top's nets |
| bench: option | key (portable set), value or flag; `save_currents` on/off; `ngbehavior` mode |
| bench: temperature, nominal temperature | `f64` each |
| bench: save / print / measure | signals[]; (analysis word, outputs[]); a measure spec (today: words) |
| bench: step | parameter, start, stop, step, spacing (lin/dec/oct/log) |
| bench: add analysis | a kind and its arguments (§1.4), returning a handle (needed by D18/D48) |
| bench: dialect-private statement | `add_spectre_analysis`; `extra_line(line, dialect)`; `raw_spice(line)` until D26 lands |
| mutate | set a source's DC value by name (`set_source_value`); set/unset an option; replace a parameter |
| query | one element's card by name (`Circuit["1"]`); whether an RF port exists; the feature needs mask; the analyses' kinds; name, ports |
| copy | clone a whole deck: `Testbench(dut)`, `Circuit.simulator()`, every run-now analysis, every future variant (D11) |

Node naming rule the binding applies itself: at the top level (`Circuit`, `Testbench` stimulus) `gnd` in any case is node `0` (`subcircuit.rs:37`, `:585`); inside a `Subcircuit` it is not.

State kept beside the text today that an IR must hold as fields: OSDI/Verilog-A loads (`Body.osdi`, passed as `render::Source.loads`), per-dialect library paths, per-card feature needs (`XSPICE`, `LAPLACE`), unlowered Verilog blocks, the ngspice compatibility mode, the dialect tag of each `extra_line`, the chosen backend.

### 8.b write(ir, dialect) → text + diagnostics

What the binding calls today: `render::render_pending(&Source, Lang) -> Result<Pending, Diags>`, `render::splice(&Source, Pending, &[Answer]) -> Result<Written, Diags>`, `render::render(&Source, Lang)` (no hooks), `render::writes(Dialect) -> bool`, `render::portable_option(&str) -> bool`, `render::card_fields(&str)`, `Lang` lookup by name (`Dialect::from_backend_name`, `Lang::from_name`, `Lang::runs_on`, `Lang::base`, `Lang::lexicon`, `Lang::spelling().comment`, `Lang::behaviours().text_family`).

What a new core must return:

- the deck text for any of the 9 built-in targets or a registered plugin language;
- notes (kind Note) on a written deck and every diagnostic on a refused one, each with: code, kind, title, cite, message, line, column, the statement's text, the element's name when the statement is an element (§7.1 C2 for what "line" means);
- a verdict "does this target write this deck" for routing, ideally without producing text (today: a full render per candidate);
- the facts the run needs beside the text (§5.3);
- for a plugin with external hooks: the cards to answer, then the spliced result;
- number spelling "as given" (`Numbers::Given`: SI suffix form) for built decks;
- a single element's card in ngspice spelling (`Circuit["1"]`);
- a store form (`to_source`/`save`) that reads back losslessly.

### 8.c read(text, dialect) → ir + diagnostics

Callers today: `Subcircuit.from_source` / `load` (the first `.subckt` of a deck, split back into elements, instances, models, includes, libs, raw lines: `store.rs:94-150`); `Testbench.from_source` / `load` (a whole deck, **kept as opaque raw lines**, so its analyses are text and `run()` does not return results for them: `store.rs:180-232`); foreign-dialect text is translated to ngspice text first (`store.rs:359-379`); `lint(text, backend)`; `spice2rack`; `render_text`. D26's `from_spice(text, dialect, *, base=None)` needs: any registered dialect, statements merged into an existing scope, duplicate-name checks, unreadable statements raised with codes and positions in the fragment, include paths made absolute.

### 8.d run and results

`backend::select(kinds, override, needs, writes) -> Result<Sim, BackendError>`; `simulation::run(sim, text) -> RawData`; `simulation::run_each(sim, text, kinds) -> Vec<Vec<RawData>>`; `backend::check(needs, elements, name) -> Vec<Issue>`; `backend::available()`, `simulators()`, `by_name`, `register_simulator`, `features()`, `Kind::ALL`/`name`, `Output::NAMES`, `parse_arg`; `rawfile::{parse_raw_all, write_raw}`; `result::{RawData, Results, MeasureResult}`; `normalize::canonical`; `measure_parse::{parse_fourier, at_step}`; `veriloga::{resolve, compile_osdi, lower}`; `lint::lint_netlist`; `unit::{UNITS, UnitValue, format_spice_number}`; `BackendError::{code, missing}`.

### 8.e Builder inputs passed as opaque strings today

These decide whether the IR needs an expression language, signal references and element references.

**Expressions (need a neutral expression type):**

| input | today's treatment |
|---|---|
| `value` of R/C/L/V/I as `str` | one `{}` pair stripped, written `{e}` (`args.rs:130-142`, `model.rs:203`) |
| `C(charge=)` | `q={expr}` (`subcircuit.rs:124`) |
| `K(coupling=)` as `str` | written as given |
| `ac=`, `ac_phase=`, `dc=` and every waveform field as `str` | braced (`args.rs:162`) |
| `BV`/`BI(expression=)` | **verbatim, unbraced**, after `V=`/`I=` (`subcircuit.rs:662`) |
| `NonLinear*Source(expression=)` | `value={expr}` |
| `conditional` conditions | verbatim between `.if (` and `)` |
| `add_function(body=)` | verbatim between `{}` |
| `parameter(value=)`, `Subcircuit(**params)` defaults | verbatim |
| `initial_condition` / `node_set` values | braced |
| `**params` values on every element, instance and model | `str(v)` verbatim (`"{w*2}"`, `"1k"`, a matrix string) |
| AM's `amplitude * offset` when either is an expression | a new expression `(a)*(o)` built by string concatenation (`source.rs:113-121`) |

All of these get ngspice's grammar and meaning (the deck is ngspice-given text); D16's neutral syntax is not applied.

**Signals and probes (need a signal reference type):** `save(*vectors)` (`"V(out)"`, `"I(Vsrc)"`), `add_print(outputs)`, `add_fourier(outputs)`, `transfer_function(outvar)` (`v(out)`, `v(a,b)`, `i(vload)` or a node), `dc_sensitivity`/`ac_sensitivity(output_variable)`, `noise(output_node, ref_node)` (a node or `v(node)`), `network_params(output_current="I(R1)")`, the probes inside behavioural expressions.

**Element and analysis references by name (need handles or validated references):** `F`/`H(vsense=)`, `W(vcontrol=)`, `POLY` F/H controls, `K` inductors (full card names such as `"L1"`), `noise(src=)`, `transfer_function(insrc=)`, `dc(**{source: slice})` keys (a source name or `temp`), `stability(probe=)`, `spectre_pstb(probe=)`, `step(param)`, `network_params(input_source)`, `spectre_sweep`/`spectre_monte_carlo(inner_analysis="ac1")` (a generated analysis name), every `model=`, `X(subcircuit_name)`, `lib(section)`, `ModelLibrary(corner)`.

**Keywords parsed at the edge (already closed enums, `keywords.rs`):** `variation`/`sweep_type` (dec, oct, lin, log), `tf_type` (vol, cur), `pz_type` (pol, zer, pz), `inner_type` (13 Spectre analysis words), `random_type` (4), `initial_state` (ON, OFF), `POLY(kind)`, `verilog(mode)`. Not validated: `spectre_pac(sweep_type="relative")` is written verbatim.

**Opaque by design (no interpretation; each is a dialect door):** `raw_spice(line)` (any ngspice text, multi-line, including whole `.subckt` blocks), `R(raw_spice=)`, `extra_line(line, dialect)`, `add_spectre_analysis(**params)` values, `measure(*words)`, `A(connections=)` port strings, `model(kind, **params)` for kinds the core does not know, `Section` lines, `veriloga(source)` and `verilog(source)` (a path or inline HDL), `Simulator(command=[…])` templates, a hook's `Answer.Text`, `lint(netlist)`, `from_source(text)`.

**Paths:** `include`, `lib`, `osdi`, `veriloga`, `ModelLibrary(path, setup_includes, **backend_paths)`, `verilog(pdk, liberty, spice_models)`, `save`/`load`/`save_source`, `read_raw`, `export(stem)`, `diagnostics(out_dir)`, `load_simulator`.

---

## 9. Size

### 9.1 `src/python` (batch-22): 29 files, 9 210 lines

| file | lines | responsibility |
|---|---|---|
| deck.rs | 1 054 | deck text, route, run (≈ 400 lines are tests) |
| subcircuit.rs | 717 | element/directive card builders |
| analysis.rs | 591 | analysis enum, ngspice cards |
| store.rs | 533 | source-text store, read-back |
| def_.rs | 533 | dialect-definition binding, hooks |
| testbench.rs | 507 | Testbench, ModelLibrary classes |
| results.rs | 451 | result classes, typing |
| simulator.rs | 438 | settings, run-now analyses |
| def_enums.rs | 437 | definition vocabulary classes |
| sims.rs | 386 | simulator descriptor, registry |
| card.rs | 377 | analysis argument checks |
| error.rs | 287 | Error, Diagnostic, locating |
| export.rs | 287 | results to raw |
| model.rs | 285 | Card, Body, Bench |
| source.rs | 268 | V/I cards, waveforms |
| sims_read.rs | 230 | provisional read hook |
| analysis_region.rs | 224 | Spectre/LTspice analysis text |
| spectre.rs | 218 | Spectre-only run methods |
| args.rs | 203 | Python argument conversion |
| mod.rs | 187 | module registration, lint |
| circuit.rs | 181 | Circuit class |
| def_tag.rs | 129 | token Tag class |
| keywords.rs | 126 | keyword enums |
| block.rs | 118 | conditional branch blocks |
| section.rs | 108 | library sections |
| verilog.rs | 103 | Verilog block lowering |
| lines.rs | 84 | O/Y/P/U lines |
| unit.rs | 84 | Unit, UnitValue |
| stage.rs | 64 | diagnostics native half |

By purpose: text building and assembly (subcircuit, lines, source, model, args, analysis, analysis_region, card, spectre, block, section, verilog, deck, store, circuit, testbench, simulator, keywords) ≈ 6 135 lines; dialect definition 1 099; simulators 616; results/export/errors/stage 1 089; mod and unit 271.

For contrast, main: 16 files, 4 612 lines (analysis 737, subcircuit 532, simulator 462, deck 419, testbench 377, store 362, results 264, source 234, model 225, spectre 196, card 180, mod 171, circuit 148, args 118, verilog 103, unit 84).

### 9.2 `python/spicerack` (batch-22): 21 files, 6 307 lines

| file | lines | responsibility |
|---|---|---|
| `__init__.py` | 11 | re-exports |
| dialects.py | 70 | Dialect base, register_dialect |
| export.py | 98 | raw + measures sidecar |
| diagnostics.py | 156 | diagnostics file writer |
| simulator_file.py | 39 | TOML simulator loader |
| remeasure.py | 371 | recompute from waveforms |
| testbenches/`__init__.py` | 88 | re-exports (91 names) |
| testbenches/_common.py | 42 | AC complex, Tian |
| testbenches/_digital.py | 18 | code reassembly |
| testbenches/analysis.py | 569 | metrics, corners, MC |
| testbenches/design.py | 787 | DesignBench, basic recipes |
| testbenches/timing.py | 558 | oscillators, clocks |
| testbenches/power.py | 493 | regulators, supplies |
| testbenches/converters.py | 479 | ADC, comparator |
| testbenches/amplifiers_2.py | 440 | CMRR, distortion, ports |
| testbenches/timing_2.py | 435 | PLL, DLL |
| testbenches/amplifiers.py | 434 | open loop, noise |
| testbenches/bias.py | 390 | references, mirrors |
| testbenches/converters_2.py | 345 | DAC, sample-hold |
| testbenches/characterize.py | 254 | cell timing tables |
| testbenches/converters_3.py | 230 | kickback, residue |

### 9.3 Tests of the Python surface (for scale)

`tests/test_python_*.py` 12 files, 7 604 lines; `tests/api_parity.py` 451; `tests/test_bench_*.py`, `test_design_testbenches.py`, `test_characterize.py`, `test_run_corners.py`, `test_remeasure.py` 3 106; `tests/test_*_backend.py` 578; `tests/test_simulators.py` + `test_simulator_oracles.py` 674; `tests/test_dialect_def.py` + `builtin_dialects.py` 1 951; `tests/test_docs.py` 111.

**One finding about these tests.** `tests/api_parity.py` (the build-versus-text-twin oracle of `api-parity.md` §3: 111 `PARITY` entries × 9 targets = 999 cases in `test_build_equals_text` alone, plus the store round trip) does not match pytest's default file patterns (`test_*.py`, `*_test.py`); `pyproject.toml` sets only `testpaths = ["tests"]`, `tests/conftest.py` sets no `python_files`, and no other file imports it. The batch-22 gate log ends `814 passed, 4 xfailed` (`/tmp/claude-1000/spicerack-golden/batch-22-pytest.log`), fewer than that one test's cases. So the parity oracle appears not to be collected. I did not run pytest to confirm.

A second, independent count agrees (raised by inv-tests, checked here by `grep`): the gate's `4 xfailed` is exactly the four `xfail` markers in the collected files (`tests/test_python_simulator.py:1200`, `:1253`, `:1270`; `tests/test_python_conformance.py:651`). `api_parity.py` calls `pytest.xfail` for its `PARITY_GAP` (13 entries), `STORE_HIDDEN` (4) and `STORE_DETOUR` (31) cases (`api_parity.py:321`, `:378`, `:437`), up to 48 more, none of which show in the log. inv-tests records the same finding in `INV-tests.md` §2.2 and §7.1.
