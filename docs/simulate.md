# Simulating

Analyses run immediately and return a result. They exist on `Testbench` and on
the simulator a `Circuit` returns (`circuit.simulator()`).

```python
import spicerack as ps

rc = ps.Subcircuit("rc", ["vin", "vout"])
rc.R("1", "vin", "vout", 1e3)
rc.C("1", "vout", rc.gnd, 1e-9)

tb = ps.Testbench(rc)
tb.V("in", "vin", "0", 1.0, ac=1.0)

op   = tb.operating_point()
dc   = tb.dc(Vin=slice(0, 2, 0.1))                       # sweep source "in": name it V<name>
ac   = tb.ac(variation="dec", number_of_points=20, start_frequency=1e2, stop_frequency=1e8)
tran = tb.transient(step_time=1e-8, end_time=5e-6, max_time=1e-8)
nz   = tb.noise("vout", "0", "Vin", variation="dec", points=20, start_frequency=1e2, stop_frequency=1e8)
```

## Analyses

| Method | Returns |
| --- | --- |
| `operating_point()` | `OperatingPoint` |
| `dc(Vsrc=slice(start, stop, step), ...)` | `DcAnalysis`; two keywords give a nested sweep: the first is the inner one (`dc.sweep`, once per outer value), the second the outer one (`dc["sweep1"]`). `temp=slice(...)` sweeps temperature |
| `ac(variation, number_of_points, start_frequency, stop_frequency)` | `AcAnalysis` |
| `transient(step_time, end_time, start_time=None, max_time=None, use_initial_condition=False)` | `TransientAnalysis` |
| `noise(output_node, ref_node, src, variation, points, start_frequency, stop_frequency)` | `NoiseAnalysis`. `output_node` and `ref_node` are node names (`"out"`; `"v(out)"` names the same node); any other form raises `ValueError` |
| `tf(outvar, insrc)` | `TransferFunctionAnalysis` (gain, input and output resistance). `outvar` is `"v(out)"`, `"v(a,b)"`, `"i(vload)"` or a node name (`"out"` is written `v(out)`); any other form raises `ValueError` |
| `dc_sensitivity(var)`, `ac_sensitivity(var, ...)` | `SensitivityAnalysis` |
| `polezero(n1, n2, n3, n4, tf_type, pz_type)` | `PoleZeroAnalysis` |
| `distortion(...)` | `DistortionAnalysis` (ngspice; its input is the source built with `distof1=(mag, phase)`, else every vector is 0) |
| `pss`, `s_param`, `harmonic_balance`, `stability`, `network_params` | RF and periodic analyses; each runs only where a backend supports it (see below) |
| `transient_noise(step_time, end_time)` | ngspice's transient noise: a `.tran` whose noise is the bench's `TransientNoise`/`Random` sources (no device noise) |
| `spectre_sweep`, `spectre_monte_carlo` (`spectre_montecarlo` is a deprecated alias), `spectre_pac`, `spectre_pnoise`, `spectre_pxf`, `spectre_pstb` | Spectre only |

`variation` is `"dec"`, `"oct"`, `"lin"` or `"log"` (N log-spaced points in total, Spectre's `log=N-1`, its steps; every other target refuses it, `W0040`), in any
case; any other name raises `ValueError` when the analysis is built. The same
holds for the other keywords: `polezero`/`add_polezero` `tf_type` (`vol`, `cur`)
and `pz_type` (`pol`, `zer`, `pz`), `step_sweep` (`lin`, `dec`, `oct`, `log`),
and the Spectre `inner_type` (`dc`, `ac`, `tran`, `noise`, `xf`, `sp`, `stb`,
`pss`, `pac`, `pnoise`, `pxf`, `pstb`, `hb`).

Set `max_time` on every transient you measure from: without it the simulator
picks its own grid, and edges and peaks move with it.

## Reading results

Every result reads the same way: index by node or branch name.

```python
print(op["vout"])                 # a float for an operating point
vout = tran["vout"]               # list[float] for a sweep, one per time point
t = tran.time                     # the axis: .time, .frequency or .sweep
mag = ac.magnitude_db("vout")     # AC results are complex:
ph = ac.phase("vout")             # degrees, wrapped to (-180, 180]
lin = ac.magnitude("vout")        # ac["vout"] alone is only the real part
four = tran.fourier               # a run's .four tables (add_fourier; [] without one)
density = nz["onoise_spectrum"]   # V/sqrt(Hz)
total = nz["onoise_total"][0]     # V rms over the band (ngspice, ESPice)
```

Branch currents are not saved unless you ask: `tb.save("I(Vin)")`, or
`sim.save_currents = True` on a `Circuit` simulator. `res.nodes` and
`res.branches` list what came back, on every result (`op.nodes` is sorted).
An operating point reads a vector by the simulator's spelling too:
`op["v(vout)"]` is `op["vout"]`, `op["I(Vin)"]` is `op["i(vin)"]`.

## Simulator settings

```python
tb.options(RELTOL=1e-4, ABSTOL=1e-12)         # .options
tb.temperature = 85.0                         # .temp
tb.nominal_temperature = 27.0                 # tnom
tb.initial_condition(vout=0.0)                # .ic (use with use_initial_condition=True)
tb.node_set(vout=0.5)                         # .nodeset, helps a stuck .op converge
tb.save("V(vout)", "I(Vin)")                  # only these vectors
tb.measure("tran", "vmax", "max", "v(vout)")  # .meas, lands in result.measures
res = tb.transient(step_time=1e-8, end_time=5e-6, max_time=1e-8)
print(res.measures["vmax"])
```

A measure that gives no value (a `WHEN` never reached, a measure on an
analysis that did not run) is in `res.measures` with the value `None`.

`options()` takes portable keys only: the names of ngspice's `.options`
(its manual, §11.1: `reltol`, `abstol`, `vntol`, `gmin`, `method`, `seed`, ...),
in any letter case. Each target writes its own spelling of a key; a target with
none refuses the deck (`W0017`) or notes that it dropped the key (`W0024`).
`True` sets a flag (`tb.options(KLU=True)` writes `.options KLU`); `None` and
`False` are a `ValueError` (on an element's `**params` `True` is a bare flag too,
and `False` and `None` write nothing). Any other key is a `ValueError`: another
simulator's own option (a Xyce package, VACASK's `tran_method`) is a line of
that simulator's text, `tb.extra_line(line, dialect="xyce")`.

`tb.step(param, start, stop, step)` and `tb.step_sweep(param, start, stop, n, "dec")`
emit `.step`: LTspice, VACASK and Spectre run it, ngspice and ESPice do not.
On LTspice a stepped run returns a list, one result per step in step order, and
each result's `params` names its step (`{"r": 2000.0}`; `{}` when nothing steps),
so `[op.params["r"] for op in tb.operating_point()]` lists the step values
(`tests/test_ltspice_backend.py` runs it).

On VACASK a stepped result holds every step's points one after another on the
analysis's own axis (`dc.sweep`, `ac.frequency`), with the stepped parameter a
vector beside them (`dc["step_r"]`); an `OperatingPoint` holds one point, so
reading a stepped one raises `ValueError`. LTspice runs one analysis a deck, so
`tb.run()` on LTspice runs each queued analysis in a deck of its own.

### ngspice compatibility mode

PDKs written for HSPICE (sky130 among them) ship an ngspice setting that tells
it how to read them, and ngspice takes it only before it parses the deck. Set
it on the bench; SpiceRack runs `ngspice -D ngbehavior=<mode>`, and every other
simulator ignores it:

```python
tb.ngbehavior = "hsa"   # sky130's own setting; "hs", "ps", "lt", "ki", "spe", "a" for the whole deck
tb.ngbehavior = None    # plain ngspice
```

ngspice runs with `-n`: a `.spiceinit` in your working directory or home
directory never changes a SpiceRack result. A mode ngspice does not recognise
fails the run.

## Netlists without simulating

```python
print(tb.netlist("ngspice"))      # any dialect: "ltspice", "vacask", "spectre", "espice",
                                  # "hspice", "xyce", "cdl", "verilog"
```

To write a deck with several analyses, queue them with the `add_*` twins: every
run method has one with the same arguments (`add_operating_point`, `add_ac`,
`add_transient`, `add_noise`, `add_tf`, `add_fourier`, `add_pss`,
`add_s_param`, `add_harmonic_balance`, `add_stability`, `add_transient_noise`,
`add_network_params`, `add_spectre_pac`, ...) and print `tb.netlist(...)`. HSPICE, Xyce, CDL and Verilog
are netlist-only: `netlist()` writes them, `run()` needs one of ngspice,
LTspice, VACASK, Spectre or ESPice.

A Spectre analysis with no method of its own goes through one door,
`add_spectre_analysis(kind, name=None, nodes=(), **params)`: the statement
`name (nodes) kind key=value ...` of the Spectre reference, checked against the
analyses Spectre names (`ValueError` for any other word). Spectre writes it; a
target with no such analysis refuses it (`W0015`).

```python
mismatch = ps.Testbench(rc)
mismatch.V("in", "vin", "0", 1.0)
mismatch.add_spectre_analysis("dcmatch", "dcmm3", ("vout", "0"), mth=1e-3, where="rawfile", stats="yes")
print(mismatch.netlist("spectre"))   # ... dcmm3 (vout 0) dcmatch mth=0.001 where=rawfile stats=yes
```

`tb.run()` runs the queued analyses as that one deck and returns one result per
analysis, in the order they were queued, each the type its immediate twin
returns (`None` for `add_fourier`: a `.four` writes no plot, its tables are
`tran.fourier` on the same run's transient result, one dict per output with
`output`, `thd` in percent, and `frequency`, `magnitude`, `phase` (degrees),
`norm_magnitude`, `norm_phase` as lists indexed by harmonic, 0 the DC
component; read from ngspice's stdout and LTspice's log). ngspice runs a
deck's analyses in its own order, so results are matched to analyses by plot
name, never by position. `pss`, `hb`, `stb` and the Spectre
analyses run alone; queued beside others, `run()` raises rather than guess
which plot is theirs. On the Spectre backend `hb` is written (`netlist()`) but
not run: Spectre has no nutmeg output for it and its PSF output is not read, so
`run()` raises before Spectre starts. A Spectre `montecarlo` (`add_spectre_monte_carlo`) is also
written for VACASK, as an `mc` loop around the deck's analysis of its inner
type. VACASK's `mc` draws everywhere, so the draws its `variations` (Spectre's
default, `process`) leaves out are written at their nominal values; two
montecarlos of one deck that leave out different scopes are refused (`W0115`).

```python
both = ps.Testbench(rc)
both.V("in", "vin", "0", 1.0)
both.add_transient(step_time=1e-8, end_time=5e-6, max_time=1e-8)
both.add_operating_point()
tran, op = both.run()
print(op["vout"], tran.time[-1])   # 1.0 5e-06
```

An ngspice deck with a `.meas` or a `.four` (from `measure`, `add_fourier`, or a
raw or extra line) runs its first `ac`, `tran` or `dc` from a `.control` block,
with a `fourier` command for each `.four`, because ngspice refuses `.measure` and
ignores `.four` when it writes a rawfile with `-r`. Run such a deck by hand with
`ngspice -b deck.cir`; the results print to stdout. A bench with a `.meas` or
`.four` and none of those three analyses is refused when it runs, rather than
returning without its results.

## Choosing a simulator

```python
print(ps.CircuitSimulator.available_backends())   # what is installed
tb.with_backend("ngspice")                        # pin one for this bench
```

Without a choice, SpiceRack picks an installed backend that supports what the
circuit and the analysis need and whose writer writes the deck: a run never
lands where `netlist()` would refuse it. `tb.check_backend(name)` lists the
writer's refusal (its code) beside the missing features. `SPICERACK_BACKEND=vacask` in the environment
sets the default for every run. A flat `Circuit` takes it as
`circuit.simulator(simulator="vacask")`.
LTspice is found as `ltspice` on PATH, the macOS app, or a Wine install under
`WINEPREFIX` (default `~/.wine`); `SPICERACK_LTSPICE=<path>` names the
executable outright (a `.exe` runs under Wine).

| Feature | ngspice | LTspice | VACASK | Spectre | ESPice |
| --- | --- | --- | --- | --- | --- |
| Verilog-A | yes (OpenVAF) | no | yes (OpenVAF) | yes | yes (compiles `.va` itself) |
| XSPICE `A` elements, control blocks | yes | no | no | no | no |
| `.meas` | yes | yes | no | yes | yes |
| `.step` sweeps | no | yes | yes | yes | no |
| Laplace sources | yes | yes | no | no | yes |
| Digital Verilog | yes | no | no | no | no |

The same design runs on several simulators with a loop:

```python
for backend in ps.CircuitSimulator.available_backends():
    if backend in ("ngspice", "vacask"):
        b = ps.Testbench(rc)
        b.V("in", "vin", "0", 1.0)
        b.with_backend(backend)
        print(backend, b.operating_point()["vout"])
```

## Linting

Catch problems before a run fails:

```python
print(tb.check_backend("ltspice"))          # list of reasons this bench will not run there
report = ps.lint(tb.netlist("ngspice"), backend="ngspice")
print(report["errors"], report["warnings"])
```

`lint` flags missing models, floating nodes and constructs the target cannot
take; each warning names the other backends it affects.

## Raw files

`ps.read_raw("out.raw")` reads a simulator's `.raw` file you already have
(ngspice, VACASK, ESPice, LTspice, Spectre nutbin; ASCII or binary) and returns
every plot in it. To recompute specs from one, see [Re-measuring](remeasure.md).
