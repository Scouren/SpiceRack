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
| `dc(Vsrc=slice(start, stop, step), ...)` | `DcAnalysis`; two keywords give a nested sweep. `temp=slice(...)` sweeps temperature |
| `ac(variation, number_of_points, start_frequency, stop_frequency)` | `AcAnalysis` |
| `transient(step_time, end_time, start_time=None, max_time=None, use_initial_condition=False)` | `TransientAnalysis` |
| `noise(output_node, ref_node, src, variation, points, start_frequency, stop_frequency)` | `NoiseAnalysis` |
| `tf(outvar, insrc)` | `TransferFunctionAnalysis` (gain, input and output resistance) |
| `dc_sensitivity(var)`, `ac_sensitivity(var, ...)` | `SensitivityAnalysis` |
| `polezero(n1, n2, n3, n4, tf_type, pz_type)` | `PoleZeroAnalysis` |
| `distortion(...)` | `DistortionAnalysis` |
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
density = nz["onoise_spectrum"]   # V/sqrt(Hz)
```

Branch currents are not saved unless you ask: `tb.save("I(Vin)")`, or
`sim.save_currents = True` on a `Circuit` simulator. `dc.nodes` and
`dc.branches` list what came back.

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
returns (`None` for `add_fourier`, whose table goes to stdout). ngspice runs a
deck's analyses in its own order, so results are matched to analyses by plot
name, never by position. `pss`, `hb`, `stb` and the Spectre
analyses run alone; queued beside others, `run()` raises rather than guess
which plot is theirs. On the Spectre backend `hb` is written (`netlist()`) but
not run: Spectre has no nutmeg output for it and its PSF output is not read, so
`run()` raises before Spectre starts. A Spectre `montecarlo` (`add_spectre_monte_carlo`) is also
written for VACASK, as an `mc` loop around the deck's analysis of its inner
type, unless its `variations` (Spectre's default, `process`) leaves out draws
the deck makes inside a subcircuit (`W0115`).

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

### Your own simulator

A simulator SpiceRack does not ship is one descriptor: the dialect its decks
are written in, the command that runs one, how its results are read, and what
it declares it can run.

```python
mysim = ps.Simulator(
    "mysim",
    dialect="ngspice",                                  # any dialect name, a plugin's too
    command=["mysim", "-b", "-r", "{raw}", "{deck}"],   # the argument list; no shell
    output="ngspice-raw",
    analyses=["op", "dc", "ac", "tran"],
    features=["measures"],
)
print(ps.register_simulator(mysim))    # mysim
print(ps.simulators())                 # the built-ins, then mysim
mine = ps.Testbench(rc)
mine.with_backend("mysim")             # or SPICERACK_BACKEND=mysim
```

- `command` is one word per entry and is run as written, with no shell. `{deck}`, `{raw}` and `{dir}` become
  absolute paths: the deck file, the file the results are read from, and the run's scratch directory, which
  holds both and is deleted after the read. The first word is the program, found on `PATH`; one word holds
  `{deck}`.
- `output` says how the results are read: `ngspice-raw`, `ngspice-ascii` or `nutascii` (one raw file at
  `{raw}`), `raw-dir` (every `*.raw` the command leaves in `{dir}`, where it then runs, as VACASK does), or
  `ltspice-raw` (`{raw}` and the `.log` beside it).
- `analyses` are the kinds it runs: `op`, `dc`, `ac`, `tran`, `noise`, `tf`, `sens`, `sens_ac`, `pz`, `disto`,
  `pss`, `hb`, `sp`, `stb`, `trannoise`, `pac`, `pnoise`, `pxf`, `pstb`. `features` are what a deck may need
  of it: `xspice`, `osdi`, `measures`, `step`, `control`, `laplace`, `verilog`; `verilog=True` and
  `verilog_a=True` declare `verilog` and `osdi`.
- The descriptor is checked when it is built: a bad field raises `ValueError` with code `R0040` and the
  field's name. Registering an equal `Simulator` again changes nothing; a different one under a held name
  is refused. A process holds at most 32 simulators (`R0042`), the five built-ins among them.
- `mysim.name`, `.dialect`, `.command`, `.output`, `.analyses`, `.features`, `.verilog` and `.verilog_a`
  read the descriptor back.

The same descriptor as a file is `spicerack.simulator/1`: TOML whose keys are `Simulator`'s arguments, plus
the required `schema`. An unknown key is refused (`R0040`), so a typo cannot drop a declaration.
`load_simulator` reads and checks the file and registers nothing (Python 3.11 or later, for `tomllib`):

```python
from pathlib import Path

Path("mysim.toml").write_text('''
schema = "spicerack.simulator/1"
name = "mysim"
dialect = "ngspice"
command = ["mysim", "-b", "-r", "{raw}", "{deck}"]
output = "ngspice-raw"
analyses = ["op", "dc", "ac", "tran"]
features = ["measures"]
verilog = false
verilog_a = false
''')
assert ps.load_simulator("mysim.toml") == mysim
```

A registered simulator is routed like a built-in. It is a candidate for every analysis it declares, after
the built-ins, in registration order, and it is held to what it declares. A run that cannot be routed
raises `RuntimeError` opening with its code, and `exc.diagnostic` holds the same as data:

| code | when | `exc.diagnostic` |
| --- | --- | --- |
| `W0180` | the simulator chosen does not declare an analysis of the deck | `{"code": "W0180", "stage": "route", "missing": {"analyses": ["tran"], "features": []}}` |
| `W0181` | it does not declare a feature the deck needs | `{"code": "W0181", "stage": "route", "missing": {"analyses": [], "features": ["measures"]}}` |
| `W0182` | no simulator runs every analysis of the deck | `{"code": "W0182", "stage": "route", "missing": {"analyses": ["hb"], "features": []}}` |
| `R0041` | the simulator's program is not on `PATH` | `{"code": "R0041", "stage": "run", "missing": None}` |

`netlist(name)` and `check_backend(name)`'s writer line take a dialect's name: a registered simulator's
deck is `mine.netlist(mysim.dialect)`.

## Linting

Catch problems before a run fails:

```python
print(tb.check_backend("ltspice"))          # list of reasons this bench will not run there
report = ps.lint(tb.netlist("ngspice"), backend="ngspice")
print(report["errors"], report["warnings"])
```

`lint` flags missing models, floating nodes and constructs the target cannot
take; each warning names the other backends it affects.

## Exporting results

`ps.export(results, stem)` hands a run's results to another tool as two files
and returns their paths.

```python
raw, meas = ps.export(both.run(), "bench")    # ("bench.raw", "bench.meas.json")
print(ps.read_raw(raw)[0].variable_names)     # ['time', 'vin', 'vout', 'i(vin)']
```

`results` is a sequence: what `tb.run()` returns, single-analysis results in a
list (`[tran]`, or `[tb.transient(...)]` for a stepped run), or `read_raw()`'s
plots. A `None` in it (a Fourier's place) is left out. The stem's directory
must exist.

`bench.raw` is one ngspice raw file (`raw_format="binary"`, the default, or
`"ascii"`): the plots in the order given, a stepped analysis's one after the
other. Whichever simulator ran, every vector has its canonical name, the one a
result is indexed by: `vout`, `i(vin)`, `time`. ngspice's `load` reads the
file, and its `v(vout)` finds `vout`. A result whose vectors are not all one
length is written as one plot per length.

`bench.meas.json` is the `spicerack.measures/1` sidecar, what a raw file has no
place for:

```json
{
  "schema": "spicerack.measures/1",
  "spicerack": "0.2.0",
  "backend": "ngspice",
  "raw": "bench.raw",
  "analyses": [
    {"index": 0, "kind": "tran", "plot": "Transient Analysis", "steps": null},
    {"index": 1, "kind": "op", "plot": "Operating Point", "steps": null}
  ],
  "measures": [
    {"name": "vmax", "analysis": 0, "step": null, "value": 0.993, "status": "ok"},
    {"name": "tdel", "analysis": 0, "step": null, "value": null, "status": "failed"}
  ]
}
```

- `backend`: the simulator family that wrote every plot; `null` when they
  differ or were read from a file.
- `analyses[].index`: the number of the analysis's first plot in the raw file,
  from 0. `kind`: `op`, `dc`, `ac`, `tran`, `noise`, `tf`, `sens`, `pz`,
  `disto`, `pss`, `sp`, `hb`, `stb` or `trannoise`, by the result's class;
  `null` for a plot read from a file. `steps`: `null`, or one
  `{parameter: value}` per step in step order (step `k` is plot `index + k`).
- `measures[]`: ordered by analysis, step, then the deck's `.meas` order.
  `analysis` is a place in `analyses`, `step` a place in its `steps` (`null`
  when it is not stepped). `value` is `null` exactly when `status` is
  `"failed"`; NaN is never written, and a value JSON cannot hold (an infinity)
  is `failed` with a `reason`. A measure of an analysis that is not exported
  is listed once, under the first analysis of its run.

The same results give the same bytes: no dates, a fixed order.

## Raw files

`ps.read_raw("out.raw")` reads a simulator's `.raw` file you already have
(ngspice, VACASK, ESPice, LTspice, Spectre nutbin; ASCII or binary) and returns
every plot in it. To recompute specs from one, see [Re-measuring](remeasure.md).
