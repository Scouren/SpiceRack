# API reference

Signatures as the module reports them. In every element method the name and
nodes are positional; everything after `*` is keyword-only.

## Module `spicerack`

| Name | What it is |
| --- | --- |
| `Circuit(title)` | flat circuit; `str(circuit)` is its netlist |
| `Subcircuit(name, ports=None, **params)` | a block with ports |
| `Testbench(dut)` | stimulus and analyses around a `Subcircuit` |
| `Section(name)` | a library section, `.lib name` … `.endl name`, holding `parameter(name, value)`, `model(name, kind, **params)`, `include(path)`, `lib(path, section)` and `subcircuit(subckt)` lines; `Circuit.section(s)` adds it to a library file |
| `CircuitSimulator` | what `Circuit.simulator()` returns; `CircuitSimulator.available_backends()` lists installed simulators |
| `ModelLibrary(path, corner=None, setup_includes=None, **backend_paths)` | a PDK model file and corner; `.name`, `.path`, `.corner` |
| `lint(netlist, backend=None)` | `{"errors": [...], "warnings": [...]}` |
| `compile_veriloga(source_or_path)` | compiles Verilog-A with OpenVAF, returns the `.osdi` path |
| `read_raw(path)` | every plot of a `.raw` file, as `RawData` |
| `unit` | unit constants, used as `10 @ u_kOhm` |
| `Dialect(name)`, `register_dialect(obj)` | a dialect plugin's base class and its registration ([Dialect plugins](dialects.md)); returns its `def_.Lang` |
| `Simulator(name, dialect=, command=, output=, analyses=, features=, verilog=, verilog_a=)` | a simulator's descriptor, checked when built ([Your own simulator](simulate.md#your-own-simulator)); `.name`, `.dialect`, `.command`, `.output`, `.analyses`, `.features`, `.verilog`, `.verilog_a` |
| `register_simulator(sim)`, `simulators()` | registers a `Simulator` and returns the name `with_backend()` takes (an equal one twice is one); every simulator's name in route order |
| `load_simulator(path)` | the `Simulator` a `spicerack.simulator/1` TOML file describes, checked and not registered; `ValueError` `R0040` names the key at fault |
| `dialects` | the built-ins as base classes: `NGSPICE`, `ESPICE`, `HSPICE`, `XYCE`, `LTSPICE`, `SPECTRE`, `VACASK`, `CDL`, `VERILOG` |
| `def_` | the definition API: `DialectDef(name)` (one method per Rust call, each returning the definition), `register(d)`, `Lang`, `Card`, `Answer`, and the vocabulary enums (`Tag`/`Keyword`, `Construct`, `Alt`, `Rule`, `Code`, `Kind`, `Scale`, ...) |

## Elements (`Circuit`, `Subcircuit`)

| Method | Signature |
| --- | --- |
| `R` | `(name, positive, negative, value, *, raw_spice=None, **params)`; `raw_spice` is the text after the nodes (`"1k tc=0.01"`); when it opens with a parameter (`"tc=1e-3"`) or is empty, `value` is written before it. In every `**params`: `True` is a bare flag (`off`), `False` and `None` are left out |
| `C`, `L` | `(name, positive, negative, value, **params)` |
| `C` by its charge | `(name, positive, negative, *, charge="expr", **params)`: the charge as an expression of the capacitor's voltage (`charge="1n*v(a,b)"`), in place of `value`; VACASK refuses it (`W0105`) |
| `K` | `(name, *inductors, coupling)`: two inductors of the same body by their full names (`K("1", "L1", "L2", coupling=0.9)`), each added before the `K`; any other name or count raises `ValueError` |
| `V`, `I` | `(name, positive, negative, value, *, ac=None, ac_phase=None)`; `V` also takes `portnum=None, z0=50.0`: an RF port for `s_param`, with `z0` in series in every analysis (ngspice and ESPice; Spectre writes its `port`, `dc`/`mag` the voltage across the terminated port, half the source behind `z0`; VACASK writes the source behind a series resistor `z0`, the pair its `acsp` names; other targets refuse it, `W0007`) |
| `BV`, `BI` | `(name, positive, negative, *, expression)` |
| `NonLinearVoltageSource`, `NonLinearCurrentSource` | `(name, positive, negative, *, expression)`: the E and G cards `E<name> n+ n- value={expression}`, the same behavioural source as `BV` / `BI` under the E/G letter |
| `E` | `(name, positive, negative, *, control_positive, control_negative, voltage_gain)` |
| `G` | `(name, positive, negative, *, control_positive, control_negative, transconductance)` |
| `F` | `(name, positive, negative, *, vsense, current_gain)` |
| `H` | `(name, positive, negative, *, vsense, transresistance)` |
| `POLY` | `(name, kind, positive, negative, controls, coefficients)`; `kind` is `"E"`, `"G"`, `"F"` or `"H"` |
| `D` | `(name, anode, cathode, *, model, **params)` |
| `Q` / `BJT` | `(name, collector, base, emitter, *, model, substrate=None, thermal=None, **params)` |
| `M` / `MOSFET` | `(name, drain, gate, source, bulk=None, *, model, extra_nodes=None, **params)`; `bulk=None` is a three-node device (ngspice VDMOS): Spectre and VACASK refuse it (`W0105`) |
| `J`, `Z` | `(name, drain, gate, source, *, model, **params)` |
| `S` | `(name, positive, negative, *, control_positive, control_negative, model)` |
| `W` | `(name, positive, negative, *, vcontrol, model)` |
| `T` | `(name, input_positive, input_negative, output_positive, output_negative, *, Z0, TD)` |
| `X` | `(name, subcircuit_name, *nodes, **params)` |
| `instance` | `(subckt, name, *nodes, **params)`: like `X`, from the object |
| `A` | `(*, name, connections, model)`: XSPICE code model; ngspice and ESPice only, other targets refuse it (`W0001`) |

Waveform sources, each a `...VoltageSource` and a `...CurrentSource`, also on
`Testbench`; all take `dc=None, ac=None, ac_phase=None`:

| Method | Keyword arguments |
| --- | --- |
| `Sinusoidal` | `offset, amplitude, frequency, delay, damping_factor, phase` |
| `Pulse` | `initial_value, pulsed_value, delay, pulse_width, period, rise_time, fall_time` |
| `PieceWiseLinear` | `values=[(t, v), ...], repeat_time, delay_time` (ngspice `r=`, `td=`: ngspice and ESPice only; ngspice on a voltage source only) |
| `Exponential` | `initial_value, pulsed_value, rise_delay_time, rise_time_constant, fall_delay_time, fall_time_constant` |
| `SingleFrequencyFM` | `offset, amplitude, carrier_frequency, modulation_index, signal_frequency` |
| `AmplitudeModulated` | `offset, amplitude, modulating_frequency, carrier_frequency, signal_delay` |
| `TransientNoise` | `noise_rms, time_step, flicker_exponent=0, flicker_rms=0, rts_amplitude=0, rts_capture_time=0, rts_emission_time=0`: ngspice `TRNOISE`, added to `dc`; other dialects refuse it |
| `Random` | `random_type, duration, time_delay=0, parameter1=1, parameter2=0`: ngspice `TRRANDOM`, `random_type` one of `uniform`, `gaussian`, `exponential`, `poisson`; added to `dc`; other dialects refuse it |

## Directives and models (`Circuit`, `Subcircuit`)

| Method | Emits |
| --- | --- |
| `model(name, kind, **params)` | `.model` |
| `include(path)` | `.include` |
| `add_function(name, args, body)` | `.func name(args) {body}` in the subcircuit (`cell.add_function("scaled", ["x"], "x*k")`). One that reads a parameter of the subcircuit is local to it: LTspice, Spectre and VACASK refuse it (`W0106`) |
| `subcircuit(subckt)` | on a `Subcircuit`: `subckt` defined inside it (a nested `.subckt`), seen by its own instances only; each target writes it as a top-level definition named `<parent>__<name>`. `ValueError` when `subckt` holds a `verilog()` block |
| `lib(path, section)` | `.lib` |
| `parameter(name, value)` | `.param` |
| `raw_spice(line)` | the line as written |
| `veriloga(source_or_path)` | a Verilog-A model, compiled per backend; returns the `.va` path |
| `osdi(path)` | a compiled OSDI model |
| `verilog(*, source, mode="simulate", instance_name, connections, pdk=None, liberty=None, spice_models=None)` | a digital Verilog module (co-simulated or synthesized) |
| `name`, `ports`, `gnd` | read-only; `gnd` is `"0"` |
| `to_source()`, `save(path)`, `Subcircuit.from_source(text, dialect=None)`, `Subcircuit.load(path)` | the store: the deck's source text behind a `* spicerack-source ngspice` line (`dialect=None` reads that line; another dialect is written as ngspice first); the JSON methods raise `NotImplementedError` naming these |

`Circuit` only: `options(**kw)`, `temp(t)`, `node(name)`, `element(name)`,
`set_source_value(name, value)`, `subcircuit(subckt)`, `simulator(simulator=None)`.

`Circuit.section(s)` adds a `Section` at the circuit's top. The circuit is then a
library file: `str(circuit)` in ngspice's dialect, `Testbench(circuit).netlist(target)`
in another, and a deck selects one of its sections with `lib(path, name)`. LTspice has
no library sections (`W0012`). A `Testbench` takes no section: a deck that runs holds
none (ngspice refuses one outside a library file).

## Testbench

| Method | Does |
| --- | --- |
| `V`, `I`, `R`, `C`, waveform sources | stimulus (no other elements) |
| `add_subcircuit(subckt)` | registers a child definition |
| `use_pdk(lib)` | attaches a `ModelLibrary` |
| `with_backend(name)` | pins the simulator |
| `netlist(backend="ngspice")` | the deck in any dialect the writer has (ngspice, ESPice, HSPICE, Xyce, LTspice, Spectre, VACASK, CDL, Verilog); a refused construct raises `RuntimeError` naming its code; each note on the written deck (an output or option the target does not get, e.g. `W0023`) is a `UserWarning` `spicerack <deck>:<line>:<col>: <code> <title>`, on a built-in or a registered dialect alike (a plugin's `omit_code` note is `W0202 <dialect>:<id> <message>`) |
| `to_source()`, `save_source(path)`, `Testbench.from_source(text, dialect=None)`, `Testbench.load(path)` | the store, as for `Subcircuit` (`save(*outputs)` stays the `.save` card): a deck read back writes the same netlists, a `ModelLibrary` at each target's path (a `* spicerack-library` line keeps them); its analyses are its text, and `run()` runs those queued on it |
| `check_backend(name)` | reasons it will not run on `name`: missing features, and the writer's refusal with its code |
| `global_nodes(*nodes)` | `.global` |
| `extra_line(line, dialect="ngspice")` | a line of `dialect`'s text: copied as written on a target of that dialect's family, refused (`W0001`) on any other |
| `add_spectre_analysis(kind, name=None, nodes=(), **params)` | any other analysis of Spectre's own (`dcmatch`, `acmatch`, `qpss`, `envlp`, `tdr`, `psp`, `hbsp`, `hbstb`, `pdisto`, ...) as its statement `name (nodes) kind key=value ...`: `add_spectre_analysis("dcmatch", "dcmm3", ("out", "0"), mth=1e-3, stats="yes")`. `name` defaults to `<kind>1`; a value is written as given, so a device it names is spelled as `netlist("spectre")` spells it. Written on Spectre, refused (`W0015`) on a target with no such analysis; a kind Spectre does not name is a `ValueError`. `run()` returns no result for it |
| `add_operating_point()`, `add_dc(**sweeps)`, `add_ac(...)`, `add_transient(...)`, `add_noise(...)`, `add_tf(...)`, `add_fourier(f0, outputs, num_harmonics=None)`, `add_pss(...)`, `add_s_param(...)`, `add_harmonic_balance(...)`, `add_stability(...)`, `add_transient_noise(...)`, `add_network_params(...)`, `add_spectre_sweep(...)`, `add_spectre_monte_carlo(...)`, `add_spectre_pac(...)`, `add_spectre_pnoise(...)`, `add_spectre_pxf(...)`, `add_spectre_pstb(...)`, ... | queue an analysis instead of running it: each run method's twin, same arguments |
| `run()` | runs every queued analysis as one deck: a list, one result per analysis in queue order (`None` for `add_fourier`) |

## Settings (`Testbench`, `CircuitSimulator`)

`options(**kw)`, `save(*vectors)`, `measure(*parts)`, `initial_condition(**v)`,
`node_set(**v)`, `step(param, start, stop, step)`,
`step_sweep(param, start, stop, n, kind)`; attributes `temperature`,
`nominal_temperature`, `ngbehavior` (ngspice's compatibility mode, e.g.
`"hsa"`; other simulators ignore it), and `save_currents` (`CircuitSimulator`).
`options` (here and on `Circuit`) takes portable keys only, the names of
ngspice's `.options` (manual §11.1) in any letter case; `True` is a bare flag;
`None`, `False` and any other key raise `ValueError`.

## Analyses (`Testbench`, `CircuitSimulator`)

| Method | Returns |
| --- | --- |
| `operating_point()` | `OperatingPoint` |
| `dc(**{source: slice(start, stop, step)})` | `DcAnalysis` (`.sweep`, `.nodes`, `.branches`) |
| `ac(variation="dec", number_of_points=10, start_frequency=1.0, stop_frequency=1e9)` | `AcAnalysis` (`.frequency`) |
| `transient(step_time, end_time, start_time=None, max_time=None, use_initial_condition=False)` | `TransientAnalysis` (`.time`) |
| `noise(output_node, ref_node, src, variation="dec", points=10, start_frequency=1e3, stop_frequency=1e8, points_per_summary=None)` | `NoiseAnalysis` |
| `transfer_function(outvar, insrc)` / `tf` | `TransferFunctionAnalysis` |
| `dc_sensitivity(var)`, `ac_sensitivity(var, ...)` | `SensitivityAnalysis` |
| `polezero(node1, node2, node3, node4, tf_type, pz_type)` | `PoleZeroAnalysis` (`pz["pole(1)"]`) |
| `distortion(variation="dec", points=10, start_frequency=100.0, stop_frequency=1e8, f2overf1=None)` | `DistortionAnalysis` |
| `pss(fundamental_frequency, stabilization_time, observe_node, points_per_period=128, harmonics=10, sc_iter=None, steady_coeff=None, uic=False)` | `PssAnalysis`; the last three are ngspice's `sciter`, `steadycoeff`, `uic` (Spectre's `pss` has none: `W0040`) |
| `s_param(variation="dec", number_of_points=10, start_frequency=1e6, stop_frequency=1e10)` | `SParamAnalysis` |
| `harmonic_balance(fundamental_frequencies, num_harmonics=None)` | `HarmonicBalanceAnalysis` |
| `stability(probe, variation="dec", number_of_points=10, start_frequency=1.0, stop_frequency=1e10)` | `StabilityAnalysis` |
| `transient_noise(step_time, end_time)` | `TransientNoiseAnalysis`: ngspice's transient noise, a `.tran` whose noise is the bench's `TransientNoise`/`Random` sources |
| `network_params(output_current, input_source, z_in=50.0, z_out=50.0, ...)` | `SParamAnalysis` (LTspice `.net`) |
| `spectre_sweep`, `spectre_monte_carlo` | `RawData` (Spectre); `spectre_montecarlo` is a deprecated alias that warns |
| `spectre_pac`, `spectre_pnoise`, `spectre_pxf`, `spectre_pstb` | periodic small-signal (Spectre) |

`variation` is `"dec"`, `"oct"`, `"lin"` or `"log"` (N log-spaced points in total, Spectre's `log=N-1`, its steps; every other target refuses it, `W0040`), in any
case; any other name raises `ValueError` when the analysis is built.
`noise`'s `points_per_summary` is ngspice's, HSPICE's and Xyce's last field;
LTspice's `.noise` has none, so it is left out there with a note (W0033).

## Results

Every result: `res["node"]` or `res.node` (a float for `OperatingPoint`, a list
otherwise), `res.measures` (`.meas` values by name, `None` for a measure
that failed; of a stepped run, that step's values) and `res.params` (the
`.step` point it is, `{param: value}`; `{}` when nothing steps). A run that
LTspice steps returns a list of results, one per step. AC-type results add
`magnitude(name)`, `magnitude_db(name)` and `phase(name)` (degrees). `RawData`
has `title`, `plot_name`, `is_complex`, `variable_names`, `measures`, `params`
and `complex(name)`.

## Units (`spicerack.unit`)

`u_V u_mV u_uV`, `u_A u_mA u_uA u_nA`, `u_Ohm u_kOhm u_MOhm` (`u_Ω u_kΩ u_MΩ`),
`u_F u_mF u_uF u_nF u_pF u_fF`, `u_H u_mH u_uH u_nH`, `u_Hz u_kHz u_MHz u_GHz`,
`u_s u_ms u_us u_ns u_ps`, `u_W u_mW u_uW`, `u_Degree`, plus `u_µ...` aliases.
A `UnitValue` has `.value` (SI float) and `.str_spice()` (`"10k"`).

Testbench and re-measuring functions: see [Testbenches](testbenches.md) and
[Re-measuring](remeasure.md).
