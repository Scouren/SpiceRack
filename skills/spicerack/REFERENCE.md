# SpiceRack reference

Signatures verified against the built module. Element arguments are keyword-only
except where noted.

## Elements

Available on `Circuit` and `Subcircuit`. `Testbench` carries only the rows marked **TB**.

| Method | Emits | Signature |
|---|---|---|
| `V` **TB** | `V` | `(name, positive, negative, value, ac=None, ac_phase=None, portnum=None, z0=50.0, distof1=None, distof2=None)`; `portnum` makes it an RF port for `s_param` (ngspice/ESPice; Spectre `port`, its `dc`/`mag` halved; VACASK a source behind a series resistor; elsewhere `W0007`); `distof1=(mag, phase)` / `distof2=` make it a `distortion` input at F1 / F2 (ngspice `DISTOF1`/`DISTOF2`, degrees; ngspice/ESPice, elsewhere `W0001`) |
| `I` **TB** | `I` | `(name, positive, negative, value, ac=None, ac_phase=None, distof1=None, distof2=None)`; `distof1`/`distof2` as `V`'s |
| `R` **TB** | `R` | `(name, positive, negative, value, raw_spice=None)` — `raw_spice` on `Circuit`/`Subcircuit` only: the text after the nodes; `value` is written before it when it opens with a parameter (`"tc=1e-3"`) |
| `C` **TB** | `C` | `(name, positive, negative, value)` |
| `C` (charge) | `C` | `(name, positive, negative, charge="1n*v(a,b)")` on `Circuit`/`Subcircuit`: the charge as an expression of its voltage, in place of `value` (`ValueError` with both or neither) |
| `L` | `L` | `(name, positive, negative, value)`; `R`, `C`, `L` on `Circuit`/`Subcircuit` also take instance parameters (`tc1=1e-3`) and `model="rm"`, a `.model` card's name written after the value (`R1 a b 1k rm`) |
| `K` | `K` | `(name, *inductors, coupling)`: two or more inductors by full name, added before the `K`: `K("1", "L1", "L2", coupling=0.9)`; three or more couple every pair; `coupling` a number or `"{k}"`; else `ValueError`. Three or more, and an expression, net for ngspice/ESPice only (`W0001` elsewhere) |
| `D` | `D` | `(name, anode, cathode, model)` |
| `Q` / `BJT` | `Q` | `(name, collector, base, emitter, model)` |
| `M` / `MOSFET` | `M` | `(name, drain, gate, source, bulk=None, model, extra_nodes=None, **kwargs)` — e.g. `L=`, `W=`; `bulk=None` is a three-node device (ngspice VDMOS), written on VACASK (`sp_vdmos`), refused on Spectre (`W0105`) |
| `J` | `J` | `(name, drain, gate, source, model)` |
| `Z` | `Z` | `(name, drain, gate, source, model)` |
| `E` | `E` | `(name, positive, negative, control_positive, control_negative, voltage_gain)` |
| `G` | `G` | `(name, positive, negative, control_positive, control_negative, transconductance)` |
| `F` | `F` | `(name, positive, negative, vsense, current_gain)` — `vsense` names a V source |
| `H` | `H` | `(name, positive, negative, vsense, transresistance)` |
| `BV` / `BI` | `B` | `(name, positive, negative, expression)` |
| `ac`, `ac_phase`, `dc` | — | on `V`, `I` and the waveform sources a number or an expression (`ac="{amp}"`; `dc=` on waveform sources). A `dc` expression nets everywhere; an `ac`/`ac_phase` expression for ngspice/ESPice only (`W0001` elsewhere) |
| `I`, `G`, `F`, `BV`, `BI` parameters | — | `**params` last on the card: `m=2` (the current, on `BV` the voltage, times 2), on `BV`/`BI` also `tc1`, `tc2`, `temp`, `dtemp`. `G`/`F` fold `m` into the gain everywhere; `I … m=` and a B source with parameters net for ngspice/ESPice only (`W0001` elsewhere). Not on `V`, `E`, `H` |
| `initial_condition`, `node_set` | `.ic`, `.nodeset` | `(**nodes)` on `Testbench`/`CircuitSimulator` (top level) and on `Subcircuit`/`Circuit` (inside its `.subckt`); a value is a number or an expression (`out="{vdd/2}"`), an instance's node is `**{"x1.n": 0.5}`. Inside a subcircuit it nets for ngspice/ESPice only (`W0001` elsewhere) |
| `NonLinearVoltageSource` / `NonLinearCurrentSource` | `E` / `G` | `(name, positive, negative, expression=...)` — `E<name> n+ n- value={expr}`: `BV` / `BI` under the E/G letter (PySpice's names); give an E and a G different names |
| `T` | `T` | `(name, input_positive, input_negative, output_positive, output_negative, Z0, TD)` |
| `S` | `S` | `(name, positive, negative, control_positive, control_negative, model)` |
| `W` | `W` | `(name, positive, negative, vcontrol, model)` |
| `A` | `A` | `(name, connections, model)` — `connections` is a `list[str]`; an XSPICE code model: ngspice, ESPice (written), every other target refuses it (`W0001`) |
| `X` | `X` | `X(name, subcircuit_name, *nodes, **params)` — name and nodes positional; `params` become `k=v` on the card |

Current-controlled sources (`F`, `H`) sense through a named voltage source; add a
0 V source in the branch you want to measure.

`**kwargs` values (`M`, `X`, `model`, `Subcircuit(..., **params)`, `options`): a
`UnitValue` is written as its SI number (`1 @ u_MOhm` -> `1e6`); `True` is a bare flag
(`D(..., off=True)` -> `off`, `options(KLU=True)` -> `.options KLU`); `False` and `None`
write nothing (in `options` they raise `ValueError`); anything else is its
`str()`, so pass expressions as `"{w*2}"`. A PDK device that is a subcircuit (all sky130
and gf180 FETs) is an `X` with parameters:

```python
dut.X("MN1", "sky130_fd_pr__nfet_01v8", "d", "g", "s", "b", W=1, L=0.15, nf=2)
# -> XMN1 d g s b sky130_fd_pr__nfet_01v8 W=1 L=0.15 nf=2
```

`Subcircuit.instance(subckt, name, *nodes, **params)` is the same card from the object. `subckt.name` and `subckt.ports` (declaration order) read a DUT back, e.g. to wrap it: `harness.instance(dut, "dut", *dut.ports)`.

`Subcircuit.subcircuit(child)` defines `child` inside that subcircuit (a nested `.subckt`: only its own instances see it, two parents can each hold a `buf`); every target writes it as a top-level `<parent>__<child>`. A definition every block shares goes to the top: `tb.add_subcircuit(child)` / `Circuit.subcircuit(child)`.

Waveform sources, all **TB**:

```python
tb.SinusoidalVoltageSource(name=, positive=, negative=, dc_offset=0, offset=0,
                           amplitude=1, frequency=1000, ac=, ac_phase=)   # no delay/damping/phase
tb.PulseVoltageSource(name=, positive=, negative=, initial_value=0, pulsed_value=1,
                      pulse_width=50e-9, period=100e-9, rise_time=1e-9, fall_time=1e-9)  # no delay_time
tb.PieceWiseLinearVoltageSource(name=, positive=, negative=, values=[(t, v), ...],
                               repeat_time=None, delay_time=None)  # ngspice r=/td=; ngspice + ESPice; Spectre delay, and r=0 as pwlperiod when the points start at 0 and end on their first value; VACASK the same (delay, period)
tb.TransientNoiseVoltageSource(name=, positive=, negative=, noise_rms=, time_step=)  # ngspice TRNOISE only
tb.RandomVoltageSource(name=, positive=, negative=, random_type="uniform", duration=,
                       time_delay=0, parameter1=1, parameter2=0)              # ngspice TRRANDOM only
```
The noise and random sources add to the DC value (default 0); every dialect but ngspice
(ESPice included) refuses them.
Current variants replace `Voltage` with `Current`. `dc_offset` is the DC (operating-point)
value; `offset` is the SIN offset. `PieceWiseLinearCurrentSource` exists on `Testbench`
only, and `ac=`/`ac_phase=` on the sinusoid only on `Testbench`/`Subcircuit`.

Neither waveform takes a delay: for an edge at a chosen time, use a PWL. `rise_time` /
`fall_time` default to 1 ns — set them to the edge rate the spec assumes, since charge
injection, propagation delay and pedestal all track them.

## Directives

`Circuit` / `Subcircuit` only: `model(name, kind, **params)`, `include(path)`,
`lib(path, section)`, `parameter(name, value)`, `raw_spice(line)`, `veriloga(...)`,
`osdi(...)`, `verilog(...)`. `temp(value)` and `options(**kw)` are `Circuit` only.

A corner library is a `Circuit` of sections: `s = ps.Section("tt")`, then `s.parameter(...)`, `s.model(...)`, `s.include(...)`, `s.lib(...)`, `s.subcircuit(sub)`, then `circuit.section(s)`. `str(circuit)` is the library file (`.lib tt` … `.endl tt`); a deck selects a section with `lib(path, "tt")`. `Testbench(circuit).netlist(target)` writes the library in another dialect (LTspice refuses sections, `W0012`). `Testbench` has no `section`: a deck that runs holds none.

`Subcircuit.add_function(name, args, body)` is a `.func` inside the subcircuit (`cell.add_function("scaled", ["x"], "x*k")`; `tb.add_function` is the top-level one). A function reading a parameter of its subcircuit stays local to it, which LTspice, Spectre and VACASK refuse (`W0106`: functions at the top only).

`Testbench`: `extra_line(text)`, `options(**kw)`, `temperature`, `nominal_temperature`,
`initial_condition(**nodes)`, `node_set(**nodes)`, `save(*signals)`, `use_pdk(...)`,
`add_subcircuit(sub)`. `tb.temperature` / `tb.nominal_temperature` are write-only.
`options(**kw)` (`Testbench`, `CircuitSimulator`, `Circuit`) takes portable keys only: the
names of ngspice's `.options` (manual §11.1: `reltol`, `abstol`, `vntol`, `gmin`, `method`,
`seed`, ...), any letter case. `klu=True` writes the bare flag `.options klu`; `None`, `False`
and any other key (a Xyce package, VACASK's `tran_method`, a typo) raise `ValueError`: write
another simulator's own option with `extra_line(line, dialect=...)`. A target with no
spelling for a key refuses the deck (`W0017`) or drops it with a note (`W0024`).
`tb.ngbehavior = "hsa"` (read/write, `None` clears) is ngspice's compatibility mode,
passed as `ngspice -D ngbehavior=hsa`; lowercase letters only (`ValueError` otherwise),
a mode ngspice does not know fails the run, other backends ignore it. ngspice always
runs with `-n`: no `./.spiceinit` or `~/.spiceinit` reaches a run.

## Analyses

Queue on a `Testbench` with `add_*`; run on a `Testbench` or `CircuitSimulator` directly.
`tb.run()` runs everything queued as one deck and returns a list, one result per
analysis in queue order, typed as the run method's (`None` for `add_fourier`):
`op, tran = tb.run()`. Results are matched to analyses by plot name (ngspice
reorders analyses); `pss`, `hb`, `stb` and Spectre-only analyses
queued beside others raise instead.

| Run method | Queue method | Signature | Result |
|---|---|---|---|
| `operating_point()` | `add_operating_point()` | — | `OperatingPoint` |
| `dc(**sweeps)` | `add_dc(**sweeps)` | `Vin=slice(start, stop, step)`; a second keyword is an outer sweep, read as `dc["sweep1"]` | `DcAnalysis` |
| `ac(...)` | `add_ac(...)` | `(variation, number_of_points, start_frequency, stop_frequency)` | `AcAnalysis` |
| `transient(...)` | `add_transient(...)` | `(step_time, end_time, start_time=, max_time=)` | `TransientAnalysis` |
| `noise(...)` | `add_noise(...)` | `(output_node, ref_node, src, variation, points, start_frequency, stop_frequency)`; the two nodes by name (`"out"`, or `"v(out)"`), else `ValueError` | `NoiseAnalysis` |
| — | `add_fourier(...)` | `(fundamental_frequency, [vars], num_harmonics=)` | see note |
| `polezero(...)` | — | — | `PoleZeroAnalysis` |
| `transfer_function(...)` | `add_tf(...)` | `(outvar, insrc)`; `outvar` is `"v(out)"`, `"v(a,b)"`, `"i(vload)"` or a node name, else `ValueError` | `TransferFunctionAnalysis` |
| `distortion(...)` | — | — | `DistortionAnalysis` (2F1; all 0 unless a source has `distof1=(mag, phase)`) |

`variation` is `"dec"`, `"oct"`, `"lin"` or `"log"` (N log-spaced points in total, Spectre's `log=N-1`, its steps; every other target refuses it, `W0040`), any
case; any other name raises `ValueError` at the call, before anything runs. So
do the `.pz` types (`vol`/`cur`, `pol`/`zer`/`pz`), a `step_sweep` type and a
Spectre `inner_type` (`dc`, `ac`, `tran`, `noise`, `xf`, `sp`, `stb`, `pss`, ...).
`noise`'s `points_per_summary` is ngspice's, HSPICE's and Xyce's last field;
LTspice's `.noise` has none, so it is left out there with a note (W0033).

`add_fourier` emits ngspice's `.four`. Its slot in `tb.run()` is `None`; the
tables are `tran.fourier` on the same run's transient result (ngspice,
LTspice): one dict per output, `t["thd"]` in percent and `t["magnitude"][n]`,
`t["phase"][n]`, `t["frequency"][n]`, `t["norm_magnitude"][n]`,
`t["norm_phase"][n]` for harmonic `n` (0 is DC). `[]` when the run printed no
table. `num_harmonics=n` (harmonics past DC; the simulators' default is 9)
is `.options nfreqs=n+1` on ngspice (one count per deck), LTspice's
`.four f n …`, Spectre's `fourier … harms=n`; Xyce and HSPICE refuse it with
`W0040` (their `.four` takes no count), VACASK refuses any `.four` (`W0015`).

Every run method has an `add_` twin on `Testbench` with the same arguments,
so any deck is netlisted before it runs: also `add_pss`, `add_s_param`
(add the RF ports first, `V(..., portnum=n)`: with none it raises `ValueError`),
`add_harmonic_balance`, `add_stability`, `add_transient_noise`,
`add_network_params`, `add_spectre_sweep`, `add_spectre_monte_carlo`,
`add_spectre_pac`, `add_spectre_pnoise`, `add_spectre_pxf`, `add_spectre_pstb`.
Any other Spectre analysis (`dcmatch`, `acmatch`, `qpss`, `envlp`, `tdr`, `psp`, `hbsp`, `hbstb`, `pdisto`, ...) is
`tb.add_spectre_analysis(kind, name=None, nodes=(), **params)`, the statement `name (nodes) kind key=value ...`
(`tb.add_spectre_analysis("dcmatch", "dcmm3", ("out", "0"), mth=1e-3, stats="yes")`): written on Spectre, `W0015` on
a target with no such analysis, `ValueError` for a kind Spectre does not name; netlist only (`run()` returns nothing for it).
`spectre_montecarlo` warns, deprecated. `transient_noise(step, stop)` is
ngspice's transient noise: a `.tran` whose noise is the bench's `TRNOISE`/
`TRRANDOM` sources (no device noise). `pss(..., sc_iter=, steady_coeff=,
uic=)` on Spectre is `W0040` (no such `pss` fields).

## Results

Every result type has `result[node]`, `result.measures` (`{name: value}`; `None` for a failed
measure), and `.nodes` and `.branches` (the names it holds). `op["v(out)"]` and `op["out"]` are one node.

| Type | Axis | Extra |
|---|---|---|
| `OperatingPoint` | — | scalar per node |
| `DcAnalysis` | `.sweep` | `.nodes`, `.branches` |
| `AcAnalysis` | `.frequency` | `.magnitude(n)`, `.magnitude_db(n)`, `.phase(n)` |
| `TransientAnalysis` | `.time` | `.fourier`: the run's `.four` tables, `[{"output", "thd", "magnitude", ...}]` |
| `NoiseAnalysis` | `.frequency` | `inoise_spectrum`, `onoise_spectrum`; `inoise_total`, `onoise_total` (V rms over the band, one-element lists; ngspice and ESPice) |

`ac[node]` is the real part. Use `.magnitude()` for `|H|`.

A `transfer_function` result has the same keys on every backend:
`tf["transfer_function"]`, `tf["input_impedance"]`, `tf["output_impedance"]`
(one-element lists; VACASK reports no output impedance: `KeyError`).

## Re-measuring (`spicerack.remeasure`)

Recompute a deciding number from the raw file instead of trusting `.meas`.
`raw = rm.read(path)` keeps every plot; each function finds its vector in the
first plot that has it (or `plot=`).

| Call | Returns |
|---|---|
| `rm.sndr(raw, "v(out)", fs, fin, n, window=None, t0=None)` | `sndr_db`, `sfdr_db`, `enob`, `thd_db`, bins. Raises on an incoherent record (`fin*n/fs` integer, prime to `n`) unless `window="blackmanharris"` |
| `rm.settling(raw, vec, final, tol, t0)` | seconds; `tol` relative to the step (or `abs_tol=` in volts); `inf` if not settled |
| `rm.slew(raw, vec, lo=0.1, hi=0.9)` | `{"rising", "falling"}` V/s |
| `rm.loop_margins(raw, plot="Loop Stability Analysis")` | `pm_deg`, `ugf_hz`, `gm_db`, `pcf_hz`, `unity_crossings` |
| `rm.noise_total(raw)` | V rms from `onoise_spectrum` |
| `rm.yield_ci(passes, n, conf)` | Clopper-Pearson `(low, high)` |
| `rm.compare(meas_json, recomputed, rtol)` | disagreements with ESPice `--measures` JSON; `[]` = agree |

Sample a transient for `sndr` with a max step well under `1/fs` (or ngspice
`.options interp`): interpolating uneven time points adds distortion.

## Exporting results

`raw, meas = ps.export(results, stem, raw_format="binary")` writes `<stem>.raw` and `<stem>.meas.json` and returns both paths. `results` is a sequence: `tb.run()`'s list, `[tran]`, `[stepped_list]`, or `ps.read_raw(...)`; a `None` item is left out. `ValueError`: a `raw_format` other than `"binary"`/`"ascii"`, no plot among the results; `TypeError`: an item that is no result.

| File | Holds |
|---|---|
| `<stem>.raw` | one ngspice raw file: the plots in the order given, a stepped analysis's one after the other; every vector under its canonical name (`out`, `i(vin)`, `time`) whichever simulator ran |
| `<stem>.meas.json` | `spicerack.measures/1`: `schema`, `spicerack`, `backend`, `raw`, `analyses[{index, kind, plot, steps}]`, `measures[{name, analysis, step, value, status, reason?}]` |

`analyses[].index` is the number of its first plot in the raw file; `kind` is `tran`, `ac`, ... (`null` for a plot read from a file); `steps` is `null` or one `{param: value}` per step. `measures[].analysis` is a place in `analyses`, `step` a place in its `steps` (or `null`); `value` is `null` exactly when `status` is `"failed"`. Order: analysis, step, the deck's `.meas` order.

## Units

`from spicerack.unit import u_V, u_kOhm, ...` then `10 @ u_V`.

The set is curated, not a full prefix x base cross product — there is no `u_kV`,
`u_pA` or `u_nW`.

| Quantity | Constants |
|---|---|
| Voltage | `u_V u_mV u_uV` |
| Current | `u_A u_mA u_uA u_nA` |
| Resistance | `u_Ohm u_kOhm u_MOhm` and `u_Ω u_kΩ u_MΩ` |
| Capacitance | `u_F u_mF u_uF u_nF u_pF u_fF` |
| Inductance | `u_H u_mH u_uH u_nH` |
| Time | `u_s u_ms u_us u_ns u_ps` |
| Frequency | `u_Hz u_kHz u_MHz u_GHz` |
| Power | `u_W u_mW u_uW` |
| Angle | `u_Degree` |

Micro is ASCII `u_uF`. Names spelled with U+00B5 MICRO SIGN (`u_µF`, `u_µs`, …)
do exist in the module dict and show up in `dir()`, but Python normalises source
identifiers to U+03BC, so neither `import` nor attribute access can reach them.
The `u_Ω` family is registered with U+03A9 and does work.

`UnitValue` exposes `.value`, `.str_spice()`, and `float()`.

## Dialect plugins (`spicerack.def_`, `spicerack.dialects`)

| Name | What it is |
|---|---|
| `ps.Dialect(name)` | base class; override `define(self, d) -> DialectDef` |
| `ps.register_dialect(obj)` | `DialectDef(obj.name)`, one-argument class attributes as calls, `obj.define(d)`, register; returns `Lang` |
| `ps.dialects.NGSPICE` ... `VERILOG` | the built-ins as base classes (`define` is `d.extends(Lang.X)`) |
| `def_.DialectDef(name)` | the Rust calls 1:1, chained: `extends`, `renamed`, `comment_line`, `keyword`, `scale`, `write_head`, `option`, `option_to`, `model_master`, `spell`, `refuse`, `code`, `refuse_code`, `hook`, `runs_on`, `kept_word(word, Construct)`, `dot_rule(word, DotRule.Note(Code) \| DotRule.SParams)`, `no_infix(tag)`, `no_scale(text)`, `nonassoc(tag)`, `ternary_nesting(Nesting.Explicit)`, ... |
| `def_.register(d)` | registers a `DialectDef`; `ValueError` names the rule it breaks |
| `def_.Lang` | `Lang.NGSPICE` ... `Lang.VERILOG`, `Lang.from_name(n)`, `.name()`, `.dump()`, `.runs_on()` (the simulator's name, what `with_backend()` takes; `None`: netlist only) |
| `def_.Card`, `def_.Answer` | a hook's card (`head`, `words`, `params`, `text`) and answer (`Answer.Text(s)`, `Answer.Refuse(id)`) |
| enums | the Rust enums by name: `Tag` (= `Keyword`), `Construct`, `Alt` (`Alt.Omit(Code.W0030)`), `Rule`, `Code`, `Kind`, `Scale` (`Scale.Exp(6)`, `Scale.Mil`), `Simulator`, the behaviour enums; `None` variants are `None_` |

## Custom simulators

| Name | What it is |
|---|---|
| `ps.Simulator(name, dialect=, command=, output=, analyses=, features=None, verilog=False, verilog_a=False)` | a descriptor, checked when built (`ValueError` `R0040 ...: <field>: ...`); getters of the same names; `==` compares descriptors |
| `ps.register_simulator(sim)` | registers it, returns its name; an equal `Simulator` twice is one; a different one under a held name (any case) is `R0040`, a 33rd simulator `R0042` |
| `ps.simulators()` | every simulator's name in route order: `ngspice`, `ltspice`, `vacask`, `spectre`, `espice`, then the registered ones |
| `ps.load_simulator(path)` | a `spicerack.simulator/1` TOML file (`schema = "spicerack.simulator/1"` required, then `Simulator`'s arguments as keys; an unknown key is `R0040`) as a `Simulator`, not registered; Python 3.11+ |
| `exc.diagnostic` | on a `RuntimeError` with code `W0180`/`W0181`/`W0182` (`stage: "route"`, `missing: {"analyses": [...], "features": [...]}`) or `R0041` (`stage: "run"`, `missing: None`) |

## Backend capabilities

Declared in `src/backend/mod.rs`; these gate automatic backend selection, and so does the writer: a run never lands on a backend whose writer refuses the deck.

| | ngspice | ltspice | vacask | spectre | espice |
|---|---|---|---|---|---|
| XSPICE | yes | no | no | no | no |
| OSDI | yes | no | yes | `.va` (`ahdl_include`) only; `.osdi` refused | `.va` via VerA only; `.osdi` refused |
| `.measure` | yes | yes | **no** | yes | yes |
| `.step` params | **no** | yes | yes | yes | **no** |
| control blocks | yes | no | no | no | no |
| Laplace sources | yes | yes | no | no | yes |
| Verilog co-sim | yes | no | no | no | no |

`ps.lint(netlist, backend=None)` takes the **netlist text**, not a `Circuit` —
pass `str(circuit)` or `tb.netlist(backend)`. It returns
`{"errors": [...], "warnings": [...]}`, each entry a dict with `line`,
`message`, `suggestion` and `backends_affected`:

```python
report = ps.lint(str(circuit))
for issue in report["errors"] + report["warnings"]:
    print(issue["line"], issue["message"], "->", issue["suggestion"])
```

It catches missing models, dangling nodes and backend-incompatible constructs
before a run. Pass a backend name to scope the check to it.
