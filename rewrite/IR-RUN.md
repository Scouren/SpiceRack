# IR, run half: analyses, the run plan, options, outputs, statistics (slice S1, Definition phase)

For the run owner of `crates/ir`. `IR-KERNEL.md` is binding (conventions C1–C14, Addendum A1),
and so is the rule of `IR-CIRCUIT.md` §1: **a native meaning is a field or a variant, never an
idiom**; readers and writers transliterate, passes convert. Inventories: `INV-constructs.md` §G
(analyses), §H (outputs), §I (options and state), §J (statistics), §K (control and scripting);
`INV-semantics.md` categories 4, 8–10, 12, 13; design notes under
`/tmp/claude-1000/spicerack-golden/research/`: `DESIGN-analysis-kinds.md`,
`DESIGN-control-epochs.md`, `DESIGN-mc-lowering.md`, `DESIGN-probe-terminals.md`.

## 1. Run items live in scopes, in order

A deck's run is the ordered run items of its top scope: `Analysis`, `Alter`, `Loop`, and the
state items (`Option`, `Env`, `NodeSet`) and output items (`Save`, `Print`, `Measure`). Order is
meaning: an `Alter` changes the circuit for every run item after it. This is the run plan of
`DESIGN-control-epochs.md` (epochs and loops) as data, not a side table.

| Item | Row | Decisions |
|---|---|---|
| `Analysis` | header `{ kind: AnalysisKind, name: NameId, detail: u32 }` + one detail table per kind | `name` is Spectre's and VACASK's statement name, `NONE` in SPICE |
| `Loop` | `{ target: Target, over: SweepSpec or table, body: ScopeId }`; opens a scope of kind `Loop` holding run items | covers `.step` (the first `.step` is the innermost loop), Spectre `sweep { }`, VACASK `sweep`, an unrolled ngspice loop, and a table of states (HSPICE `.alter` blocks, Xyce `.DATA` + `.STEP DATA=`): a table is a list of parameter names and rows of values |
| `Alter` | `{ target: Target, value: ExprId }` | everything after it runs on the changed circuit |
| `Elaborate` (A12 R1) | `{ circuits: RefList }` (`DefKind::Subckt` references) | the named subcircuits' bodies are the top circuit for every run item after it, until the next `Elaborate` (VACASK repeated `elaborate circuit(…)`) |
| `Target` | a small row `{ kind, a: NameId, b: NameId }` | `Temp`, `Param(name)`, `SourceDc(element)`, `ElementValue(element)`, `DeviceParam(element, param)`, `ModelParam(model, param)`, `Option(key)`. Temperature is its own kind on every simulator, never a parameter named `temp` |
| `SweepSpec` | `{ kind: Lin \| Dec \| Oct \| List, start, stop, step, points, values: ExprList, end: SweepEnd }` | a set of values with one pinned reading: `Lin` by `step` or by `points` = the number of values (Spectre's `lin=4` is 5 values: its reader adds one, its writer subtracts: spelling, not conversion); `Dec`/`Oct`: `points` per decade or octave; `end: Grid \| OnStop` (LTspice ends a log sweep on the stop value even off the grid, `INV-semantics.md` 8.06, 8.07). Shared by `Loop`, DC axes, AC, noise |

A `Loop` of kind `Sweep` whose body is exactly one `Op` uses `RunExtra::LoopOverOp` (A12 R2,
RULINGS R20): it is not a `Dc` (one result over the grid, each point continued from the last,
against one result per value); a target that spells both the same lacks the feature and a pass
converts.

## 2. Analyses (`analysis.rs`)

`AnalysisKind` is closed. Start from the union of `INV-constructs.md` §G: `Op`, `Dc` (ordered
axes, the first innermost: `{ target: Target, sweep: SweepSpec }`), `Ac`, `Tran`, `Noise`, `Tf`,
`Sens`, `SensAc`, `Pz`, `Sp`, `Four`, `Disto`, `Hb`, `Pss`, `Pac`, `Pnoise`, `Pxf`, `Pstb`, `Stb`,
`Xf`, `MonteCarlo` (opens a scope holding its inner analyses), VACASK's `DcInc`, `DcXf`, `AcXf`.
An analysis only one simulator has and whose parameters are not worth typing yet is an `Opaque`
of class `Private`; say which ones you left there and why in the coverage file.

Decisions:

- `Tran { stop, record_start, print_step, max_step, initial_step, uic: bool, startup: bool }`
  (`INV-semantics.md` 8.01–8.03): every field named, none inferred from another, `NONE` is not 0.
  Each simulator reads the positional `.tran` fields differently (Xyce's first is an initial-step
  guess, ngspice's a print step that also caps the step, HSPICE's extra fields are segments): the
  reader puts each number in the field its simulator means, and fills the cap its simulator would
  apply by default where two simulators differ. `print_step == NONE` means "the simulator
  chooses" (Xyce, LTspice); ngspice has no such thing and its writer lacks the feature.
  `startup` is LTspice's ramp of the DC sources; lowering it to PWL sources is a pass.
- `Ac`/`Noise`/`Sp` hold a `SweepSpec` over frequency. An AC or DC analysis over something else
  (Spectre `ac dev= param=`) is a `Loop` over that target around the analysis.
- `Noise { out_p, out_n, source: NameId, sweep, points_per_summary }`; `Tf { output: Probe,
  source }`; `Pz { in_p, in_n, out_p, out_n, transfer: Vol \| Cur, find: Poles \| Zeros \| Both }`.
- `Four { fundamental, outputs: probes, harmonics }` (the harmonic count is a field here, not an
  option).
- `Hb { tones: ExprList, harmonics: ExprList }`: several tones from the start (the one-tone
  restriction is a writer's feature).
- Each simulator's "what the same analysis means" differences in `INV-semantics.md` category 8
  become fields or `Extra` features, by the §1 rule of `IR-CIRCUIT.md`.

## 3. State (`option.rs`)

- `Env { temp: ExprId, tnom: ExprId }`, degrees Celsius, both always present. A reader fills its
  simulator's defaults (HSPICE 25/25, the others 27/27). A list of temperatures is a `Loop` over
  `Target::Temp`.
- `OptionRow { key: OptionKey, private: NameId, dialect: NameId, value: ExprId }`. `OptionKey` is
  the closed list of options that mean one quantity on every simulator that has them (start from
  the 60 portable keys of `INV-constructs.md` I2 and the name maps I3–I5; document each member's
  meaning and unit). `OptionKey::Private` keeps a simulator's own option by name, with its
  dialect; only that dialect's writer spells it. ngspice's compatibility mode is a private option
  (the backend reads it from the IR, not from a comment in the text).
- `NodeSet { kind: Ic \| Nodeset, node: NameList, value: ExprId }`: `node` is a hierarchical
  path, instance names then the net.

## 4. Outputs (`output.rs`)

- `Probe { kind, a: NameList, b: NameList, part }`: `V(node)`, `Vdiff(node, node)`,
  `I(element)`, `ITerminal(element, terminal)`, `Param(element, parameter)`, `Power(element)`,
  `Noise(…)`, `All`, `AllV`, `AllI`, `Expr(ExprId)`; `part: None | Re | Im | Mag | PhaseDeg | Db`.
  Paths are name lists (no separator character in the IR). The sign convention of a current probe
  is pinned in its doc (positive into the first terminal).
- `Save { probes }`, `Print { analysis: AnalysisKind or none, probes }`.
- `Measure { name, analysis, kind, … }` typed: trigger/target, when, find-at, the statistics over
  a window (`avg min max pp rms integ`), `param` (an expression of other measures), derivative.
  A measure form you cannot type is an `Opaque` of class `Private` naming its dialect. Thresholds
  and edges are fields; do not keep a word list.

## 5. Statistics (`stats.rs`)

`Statistics { process: list of Vary, mismatch: list of Vary, correlations, truncate }` with `Vary
{ param: NameId, dist: Dist, spread: ExprId, percent: bool }` (Spectre's block), and the draw
node of `expr.rs` for the SPICE idiom (`agauss` and friends). The two are different native
meanings: converting one to the other is a pass (`DESIGN-mc-lowering.md`). State per `Dist`
exactly what its arguments mean (absolute or relative spread, the sigma divisor, the nominal
outside sampling is a simulator rule the reader makes explicit with a field, `INV-semantics.md`
category 12).

## 6. What to deliver

The kernel is committed (read `rewrite/IR-DECISIONS.md` first: it says how the kernel's doors
work). Your tables go in `RunDetails` (`analysis.rs`). Implement `RowFacts` (IR-KERNEL §7.2) for
every run row in its own file. Three answers to the kernel owner's notes: `Vary` has its own
`VaryDist::{Gauss, Unif, LogNormal}` (a block's variation is not a draw call; `Dist` stays the
kernel's); a field that is "an analysis kind or none" is a `bool` beside the kind, never an
`Option`; a draw's when and how often are the kernel's `DrawWhen`/`DrawPer`.

The files of the run owner in IR-KERNEL §10, complete (types, fields, enums, builder methods in
`build_run.rs`, read accessors), bodies `todo!()`, your `Extra` features in your region of
`feature.rs`, and `rewrite/COVERAGE-run.md`: one line per row of `INV-constructs.md` §G, §H, §I,
§J, §K and of `INV-semantics.md` categories 4, 8, 9, 10, 12, 13, naming the field, variant,
feature or `Opaque` class that holds it (or `gap: <why>`).
