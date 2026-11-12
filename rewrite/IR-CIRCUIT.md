# IR, circuit half: elements, sources, waveforms, models (slice S1, Definition phase)

For the circuit owner of `crates/ir`. `IR-KERNEL.md` is binding (conventions C1–C14, the element
header of §6, Addendum A1); this file adds the decisions for your tables. Inventories:
`INV-constructs.md` §D (devices), §E (waveforms), §F (models), §B10/B16/B17 (multiplicity, naming,
references); `INV-semantics.md` categories 5 (sources), 6 (instance semantics), 7 (models).

## 1. The rule that decides every field

**A native meaning is a field or a variant, never an idiom.** A reader transliterates one
statement into rows using only what that statement says; a writer transliterates rows back; on
one dialect the two are inverse (law L5). Conversions between meanings are passes outside this
crate. So when two simulators give one keyword two meanings, the row must be able to state both,
as data:

- a **law field** (a `#[repr(u8)]` enum) when the formula differs: `SffmLaw::{Spice3, Ngspice}`
  (ngspice clamps the index to FC/FM and is 0 until the delay, at the operating point too);
  `TableLaw::{Exact, NgspiceRounded}`; `PreDelay::{Offset, HoldDelayValue, Zero}` (what a delayed
  wave is before its delay);
- an **explicit field** when a default differs: a pulse's rise is always present, and ngspice's
  "a given zero means TSTEP" is the leaf `TStep` in that field (never a number the reader looked
  up); where a source's DC comes from is the field `dc_rule` (A12 C4: `Given`, or `WaveAtZero` with `dc` `NONE`);
- a **mode field** when the same value applies differently: `IcMode::{UicOnly, InOp}` (ngspice
  applies a device `IC=` only under UIC, Xyce and LTspice in the operating point);
- a **superset row with restrictions** when dialects have sub-cases of one formula: one `Am` row
  holds the general formula; each writer's features say which field values it can spell.

Do not write a conversion into the IR (no "ngspice order" comment on a positional list, no
expression that hides a clamp). If you cannot state a simulator's meaning with a field, add the
field and say which dialect needs it in its doc.

## 2. Elements

Header as IR-KERNEL §6, with A13 (R23): `name` is the element's whole name as its dialect
spells it (ngspice: letter included, folded), unique in its scope across kinds; the SPICE card
letter is the neutral fact `letter::spice_letter(design, element)` (the canonical letter, which a
rename pass uses); `letter::spice_letters` lists every letter a SPICE card may give the element
(a voltage `Behav` B or E and a current one B or G, as ngspice and HSPICE read `E … vol=` and
`G … cur=`; a `Port` P or V), and a name that begins with none of them is
`CircuitExtra::NameWithoutLetter` (a SPICE-card writer lacks it; a pass
renames). Two elements of one name in one scope are `Violation::DuplicateElement`
(`element::design_check`). For every `ElemKind` document the terminal roles in order, the allowed
terminal count (fixed or a range), whether `model` is required, and its detail row.

| Family | Kinds | Detail row (owner file) | Decisions |
|---|---|---|---|
| passives | resistor, capacitor, inductor | `Passive` (`passive.rs`) | `value` (Ω, F, H; `NONE` when only the model gives it), `ic` + `ic_mode`, `tc1`, `tc2`, `temp_abs`, `dtemp` (two fields: Xyce and VACASK have both, HSPICE only the offset), `noisy`, `parasitics` (row or `NONE`: `rser lser rpar cpar rlshunt`, LTspice's; the builders take them too); for a value that reads the circuit, `tc_base: TcBase::{Tnom, Fixed27}` (ngspice measures its `tc1`/`tc2` from 27 °C whatever `tnom` is, `asrcload.c:41`) and, on a capacitor, `tc_law: TcLaw::{Direct, Reciprocal}` (ngspice divides by the polynomial, `inpcom.c:6747`); `Fixed27` and `Reciprocal` elsewhere are `FieldDomain` (A15 C2) |
| charge capacitor | charge-defined capacitor | `Behav` with `OutKind::Charge` | one construct, five spellings (`INV-constructs` D5) |
| coupling | coupling | `Coupling { inductors: NameList, k: ExprId }` | no terminals; every listed pair is coupled by `k`; `k` may be an expression |
| sources | voltage source, current source, RF port | `Source` (`source.rs`) | three independent slots: `dc` (what `.op`/`.dc` read) with `dc_rule: DcRule::{Given, WaveAtZero}` (A12 C4: `dc` is `NONE` exactly under `WaveAtZero`, the wave's t = 0 value; ngspice and VACASK have `WaveAtZero` for every wave (R36: ngspice's operating point runs the wave's own law at t = 0, `vsrcload.c:73-86`; SFFM and AM give 0 there because their law is 0 up to the delay, a random wave its offset), and VACASK lacks `Given` with a wave: features `DcGivenWithWave`, `DcAtZero<Wave>`), `ac_mag` + `ac_phase_deg` (`NONE` = no AC stimulus), `wave` (`NONE` = the transient holds `dc`); `distof1`, `distof2`; `tc1`, `tc2` (Spectre scales the whole waveform); `parasitics`; port fields `port_index`, `z0`, with the value convention pinned as the EMF behind `z0` (Spectre's user value is half of it: its reader doubles) |
| linear controlled | VCVS, VCCS, CCCS, CCVS | `Controlled { gain, sense: NameId }` | terminals (out+, out−, ctl+, ctl−) or (out+, out−) + the sensing element's name |
| polynomial | one kind, `output: V \| I`, `control: Voltage \| Current` | `Poly { dimension, controls: NameList, coeffs: ExprList }` | stays `Poly`; expanding it is a pass |
| table | one kind | `Table { input: ExprId, points: ExprList, law: TableLaw, output }` | linear between points, ends held; ngspice's native TABLE rounds corners: its own law |
| behavioural | one kind | `Behav { output: OutKind::{Voltage, Current, Charge}, expr, tc_base }` (`tc_base: TcBase`, A15 C2: `Fixed27` only with a coefficient; `tc_law: TcLaw`, A17: `Reciprocal` only on a `ChargeCapacitor` with a coefficient, ngspice `C … q=` dividing the charge by the polynomial; a `Behav` whose expression reads a parameter outside `ToReal`/`ToInt` sets `BehavUntypedParam`, A33, R56: VACASK types every identifier there) | positive current flows from the first terminal through the source to the second |
| Laplace | one kind | `Laplace { num: ExprList, den: ExprList, output }` | terminals (out+, out−, in+, in−); coefficients in ascending powers of `s`, absolute gain already applied |
| semiconductors | diode, BJT, MOSFET, JFET, MESFET | `Semi { area, flags, ic, temp_abs, dtemp }` or none | terminal ranges: BJT 3–5, MOSFET 3–7, diode 2–3 (thermal); everything else is a named instance parameter |
| switches | voltage switch, current switch | `Switch { initial: SwitchState, sense: NameId }` | the initial state seeds the operating point only |
| lines | lossless, lossy, RC, single lossy, coupled | `Line { z0, td, freq, nl, length }` or none | lossless: either `z0` + `td` or `freq` + `nl` as written (do not convert) |
| modules | code model, Verilog-A module, subcircuit instance, unresolved master | none | `model` names the master; `Ref.def == NONE` for the unresolved kind is legal and is what a Spectre reader produces without the kit |

`mult` is the header's field for every kind: a reader lowers `m=` (and VACASK `$mfactor`) to it,
never to a parameter named `m`. How it multiplies is the header's `mult_law` (AMEND-A12 A15 C1,
R31): `MultLaw::Parallel` (every kind: `mult` whole copies in parallel; through the hierarchy
multiplicities compose by multiplication) or, on an `Instance` with a `mult` only,
`MultLaw::NgspiceAppend` (ngspice's `m` on an X line, ngspice-45 `inpcom.c:4124-4198`
`inp_fix_subckt_multiplier` and `4208-4250`; exact list and runs from the ngspice elements
slice): when the subcircuit does not declare `m`, the instance's `mult` is appended as `m={m}`
to every element of the body, after its own `m=`, so it replaces that element's own `mult` (the
last one wins: an inner `R1 p q 1k m=5` under `X … m=2` runs as 500 Ω, v(f) = 0.6667), and
nested `Instance`s pass it on the same way. ngspice leaves untouched the cards whose letter is
in "vehaknopstuwy" and a B card whose output is `v=`: by kind `VSource` and `Port` (V), `Vcvs`,
`Delay` and the voltage-output `Poly`, `Table`, `Laplace`, `FreqTable` (E), `Ccvs` and the
voltage-output current-controlled `Poly` (H), `CodeModel` (A), `Coupling` (K), `ModuleModel`
(N), `LossyLine` (O), `CoupledLines` (P), `VSwitch` (S), `LosslessLine` (T), `RcLine` (U),
`ISwitch` (W), `SingleLossyLine` (Y), and `Behav` with `OutKind::Voltage`. Every other kind is
multiplied: R, C, L (with the charge- and flux-defined forms and a circuit-valued R), `ISource`
(`i` is not in the list), `Vccs`, `Cccs` and every current-output G/F form, a current `Behav`,
D, Q, M, J, Z (`Mesfet`, `Igbt`), and nested `Instance`s (X). `Module` and `Unresolved` are no
ngspice cards. Runs (ngspice-45): `X1 … m=2` over `R1 p q 1k` gives v(b) = 0.6667 (500 Ω). A
subcircuit that declares `m` in its head (`params: m=1`) triggers nothing: `m` is an ordinary
parameter (v(d) = 0.5, R stays 1k), so the ngspice reader puts it in the instance's `params`
and leaves `mult` `NONE`. HSPICE compatibility mode (`newcompat.hs`) writes `m={m*<old>}`, a
product: that is `Parallel`, not this law. Features: `MultInstanceParallel`,
`MultInstanceNgspiceAppend` (they replace `MultInstance`).

## 3. Waveforms (`wave.rs`)

`WaveKind::{Pulse, Sin, Exp, Pwl, Sffm, Am, TrNoise, TrRandom, Pattern}`, one table each,
referenced from `Source.wave` as `{kind, index}`. Every field is an `ExprId`; times in seconds,
frequencies in hertz, phases in degrees.

| Wave | Fields | Decisions (evidence in `INV-semantics.md` §5) |
|---|---|---|
| `Pulse` | `v1 v2 delay rise fall width period cycles law given_zero` | `cycles` above 0 is a count, `NONE` or any other value unlimited (A17); a zero `rise` under `AsGiven` is an ideal edge (5.04, 5.05); `law: PulseLaw::{Spice, Vacask}` (A12 C1: VACASK needs `rise > 0`, `fall <= 0` is no falling edge, `period <= 0` one pulse) |
| `Sin` | `offset amplitude freq delay damping phase_deg cycles pre given_zero` | `phase_deg` is the phase at `t = delay`; `pre` says what the source is before the delay: ngspice `HoldDelayValue` (A17, from `vsrcload.c`); Xyce and HSPICE as their slices confirm (5.06, 5.07) |
| `Exp` | `v1 v2 td1 tau1 td2 tau2 td2_from given_zero` | `td2` counts from `td2_from: TdFrom::{Zero, Td1}` (A12 C2: SPICE from 0, VACASK from `td1`; the reader no longer adds) (5.08) |
| `Pwl` | `points` (t, v pairs, absolute times), `delay`, `repeat_from`, `repeat_join` | relative times are accumulated by the reader; before the first point the first value holds, after the last the last value, unless `repeat` says otherwise; no reader writes a point (A22, R42: it would repeat an operand and give two t = 0 points when the first time evaluates to 0); a simulator with another rule before the first point gets a mode member when its slice shows it (none today); `PwlJoin::Extrapolate` (VACASK `period=`): on [xN, period) and on [0, x1) of the wrapped time τ, the line through (xN, yN) and (period + x1, y1) evaluated at τ itself; each join of a periodic PWL sets a feature, `Jump` there `PwlPeriodJump` (A24: VACASK only extrapolates) (VACASK 36be560, `src_pwl.sim`: (1u, 1) (2u, 3) with period 4u gives 4.333 at 0, 3.833 at 0.75u, 1 at 1u, 2.667 at 2.5u, 1.733 at 3.9u, 4.267 at 4.1u; (0, 0) (1u, 1) with period 2u gives 0.5 at 1.5u and 3.5u; a delay shifts τ and the operating point reads the line at τ = 0; VACASK refuses x1 = 0 with xN = period and y1 ≠ yN, and xN > period: its writer's refusals) (5.09–5.11) |
| `Sffm` | `offset amplitude carrier_freq mod_index signal_freq delay phase_c_deg phase_m_deg law pre given_zero` | named, never positional; `law` as §1 states the clamp and the defaults only; what it is before the delay is `pre: PreDelay` as for `Sin` and `Am` (A12 C3: VACASK holds its value at `delay`, ngspice is 0 for t ≤ delay, the delay included) (5.01, 5.02) |
| `Am` | the general formula of 5.03 with every term a field, `pre` | each dialect's AM is a restriction of it; a `mod_offset` other than the literal 1 sets `AmModOffset` (A31: VACASK's AM is `sinedc + ampl·sin(carrier)·(1 + modindex·sin(mod))`) |
| `TrNoise`, `TrRandom` | ngspice's fields | time domain only, superposed on a given DC value; a `TrRandom` before its first draw and at the operating point is its offset (A17) |
| `Pattern` | `bits levels delay rise fall sample repeats repeat_start` | 5.22 |

A PWL never pairs `Ramp` with `Period`, nor `RampWidth` or `Extrapolate` with `From` (`check`: no
simulator states them; A30 finding 4).

**The reference evaluator** (AMEND-A12 A30, `wave_eval.rs`): `source_value(design, source,
At::{OperatingPoint, Time(t)}, bindings)` and `wave_value(design, wave, t, bindings)` give a
source's value by its wave's law and modes, on `eval`'s `Bindings` (`TSTEP`/`TSTOP` for the
`SpiceDefault` defaults, `temp`/`tnom` for the source's temperature factor), or a
`WaveDomain` (a field's `eval` domain, `Unbound`, `Random` after t = 0, `File`, `Undefined`).
`wave_value`'s doc is the specification of every law below, each boundary written out; a law
no run pins is marked unmeasured there (the LTspice ramp join and sine cycles, the Spectre
ramp width, the bit pattern). Its proof is the areas' runs: 48 ngspice-45 values
(`dialect-ngspice` read/sources decks) and 205 VACASK 36be560 values (`vc-sources` settle
`src_*`, `dcw`) reproduced to 1e-12 (`tests/wave_eval.rs`). Writers and readers never call it.

**The zero rule** (AMEND-A12 A17, R35). `Pulse`, `Sin`, `Exp`, `Sffm`, `Am` and `TrRandom`
carry `given_zero: GivenZero::{AsGiven, SpiceDefault}` (features `GivenZeroAsGiven`,
`GivenZeroSpiceDefault`). Under `AsGiven` every field below is present and means what it holds.
Under `SpiceDefault` (ngspice-45; table and source lines from the ngspice sources slice,
`vsrcload.c` unless named, `isrcload.c` the same but for SFFM) a field below that the card omits
is `NONE` and means its default; a **both** field also means its default when it evaluates to
zero after parameter substitution, an **omit** field keeps a given 0.

| Wave | Field | Rule | Default | Source |
|---|---|---|---|---|
| `Pulse` | `delay` | omit | 0 | :106 |
| `Pulse` | `rise`, `fall` | both | TSTEP | :108, :111 |
| `Pulse` | `width`, `period` | both | TSTOP | :114, :117 |
| `Sin` | `freq` | both | 1/TSTOP | :181 |
| `Sin` | `delay`, `damping`, `phase_deg` | omit | 0 | :184, :186, :173 |
| `Exp` | `td1`, `tau1`, `tau2` | both | TSTEP | :204, :207, :213 |
| `Exp` | `td2` | both | `td1` (after its own rule) + TSTEP | :210 |
| `Sffm` | `signal_freq` | omit | 5/TSTOP | :237 |
| `Sffm` | `mod_index` | omit | 90, then clamped to [0, fc/fm] | :239, :256-266 |
| `Sffm` | `carrier_freq` | both | 500/TSTOP | :241 |
| `Sffm` | `delay`, `phase_m_deg`, `phase_c_deg` | omit | 0 | :244-248 |
| `Am` | `mod_index` | omit | 1 | :290 |
| `Am` | `mod_freq`, `carrier_freq` | omit | 5/TSTOP, 500/TSTOP | :292, :294 |
| `Am` | `delay`, `phase_m_deg`, `phase_c_deg` | omit | 0 | :296-300 |
| `TrRandom` | `td`, `p1`, `p2` | omit | 0, 1, 0 | `vsrcpar.c:262-278` |

`Pulse.cycles` is a count above 0 and unlimited otherwise (`:124-138`); PWL has no defaults;
TRNOISE's optional fields are 0 when omitted and already optional (`vsrcpar.c:223-249`). A field
holds what the card holds: an expression, a literal 0, or `NONE` when omitted. No reader copies
a default in; a pass that converts `SpiceDefault` to `AsGiven` writes the defaults with the
`TStep` and `TStop` leaves (a both field `x` becomes `x == 0 ? TStep : x`). ngspice's
I-source SFFM has no delay (`isrcload.c:205-252`; its 6th value is PHM, the 7th PHC): the
ngspice reader writes `delay` 0 and `pre: HoldDelayValue`.

## 4. Models (`model.rs`)

`Model { name, family: ModelFamily, polarity: Polarity, params: ParamList, bin_of: NameId, ako:
Ref }`.

A model of a versioned family of a model standard (`circuit_tables::needs_version`: every
versioned family but `Module` and `Other`) states its `version` (`check`, round 7: "no version"
would be each simulator's own default). The generator draws it from `circuit_tables::VERSIONS`,
ngspice-45's accepted versions per family with their source lines (its vocabulary, not a rule).

- `ModelFamily` is the **equation family**, a closed enum (`Resistor`, `Capacitor`, `Diode`,
  `GummelPoon`, `Vbic`, `Hicum`, `Mos1`, `Mos2`, `Mos3`, `Mos6`, `Mos9`, `Bsim3`, `Bsim4`,
  `BsimSoi`, `Hisim2`, `HisimHv`, `Psp103`, `Vdmos`, `JfetShichman`, `JfetParkerSkellern`,
  `Mesfet`, `Hfet`, `Switch`, `CurrentSwitch`, `Ltra`, `Txl`, `Cpl`, `Urc`, `Core`, `CodeModel`,
  `Module`, …: fix the list from `INV-constructs` §F and D68). A level number is a dialect's
  spelling of a family: readers map (type word, level) → family, writers map back. There is no
  "neutral level".
- Every model sets `Feature::Family(family)` (A15 K5, R32): a target that has no such family
  lacks the feature, so its refusal is a feature, not an `admits` check; a version of a family
  a target lacks stays the dialect's own check.
- `ModelFamily::Other` carries the type word as written for a family the IR does not know; a
  writer of another dialect refuses it.
- Parameter names in `params` are lowercase and are the family's own names (the model's
  published names); a dialect's renames live in that dialect's tables, not here.
- Bins: one `Model` per bin, `bin_of` naming the base; a Spectre model group is a run of them.
  A bin group has a definition (AMEND-A12 A15 C4, R33): pushing the first `Model` with
  `bin_of = X` into a scope where no `DefKind::Model` `X` exists also defines `X` there, its
  `Def.item` the first bin's row (`Builder::model` and the IR-text reader both do it, so the round
  trip holds). An instance that names the group (`nch`, where only `nch.1`, `nch.2`, … exist)
  binds to that definition, and the simulator picks the bin by geometry. A consumer tells a group
  from a plain model by its definition: `design.row::<Model>(def.item.index).name != def.name` (the
  row it points at is a bin whose `bin_of` is the definition's name). A plain model of the
  group's name in the same scope is a duplicate definition: a reader keeps its simulator's
  winner before it pushes.
- `ModelFamily::Mesa` (A15 C3): ngspice's MESA device (`nmf`/`pmf`/`nhfet`/`phfet` levels 2 to 4,
  `inpdomod.c`); the level is a parameter of the device.

## 3a. Invariants every simulator holds (AMEND-A12 A19, R38)

`check` reports `FieldDomain` (the generator never makes one): a passive with no value and no
model; `ac` on a capacitor or inductor; `noisy == false` on a capacitor or inductor whose value
does not read the circuit; an instance parameter whose key names one of the row's own typed
fields (`circuit_check::typed_key`: the detail row's IR field names from the field table, `m`
and `mult`, and `temp` where the row has `temp_abs`; a kind without a detail row, such as a
subcircuit instance with a declared `m`, has none); a model parameter `level`, `type` or
`version` on a family that fixes it (all but `Other`, `Module`, `CodeModel`, and `level` on
`Mesa`); a lossless line outside `Line`'s forms (no `z0`; not exactly one of `td`,
`freq`+`nl`, `length`+`vel`; half of a pair); a `version` on a family with one version everywhere
(`circuit_check::versioned`). A model value that reads the circuit is the kernel's
`ProbeContext` (model values are fields constant in time). "Reads the circuit" is
`expr_class == Circuit` (A11): through a parameter or user function too.

## 4a. Device libraries (`library.rs`, AMEND-A12 C5, R17)

`ItemKind::DeviceLibrary` with row `DeviceLibrary { family: ModelFamily, path: NameId }`: the
simulator's own implementation of `family` is available from here on; `path` `NONE` is the
dialect's default file for the family, the one proven over the family's whole typed parameter
set; a path names another proven file, kept as written (R45). The path is spelling, never
meaning: no `check` or feature reads it, and a pass that drops libraries drops the row whatever
its path. IR text:
`device_library <family> [path="<path>"]`. Features: `Item(DeviceLibrary)`; `Family(family)`, as a
model of it (A25);
`CircuitExtra::ModelWithoutLibrary` (design-level, `element::design_features`: a reachable model
whose family has no reachable library of that family; `Module`, `CodeModel` and `Other` are
exempt beside the builtins, R44: their code comes from a `Load` or is unbound, never from a
family's library); `CircuitExtra::ElementWithoutModel`
(item-level: an element of a kind with a master, `master_kind` `Some`, whose `model.name` is
`NONE`; VACASK instantiates every device through a model, run n2; a masterless kind such as
`Behav`, `ChargeCapacitor` or `FluxInductor` never sets it). Neutral passes add and remove libraries and models; readers and writers stay
inverse.

**Parameterless models of kinds with no family today** (agreed with the VACASK agent,
2026-11-12; VACASK fails "Master 'vsource' not found." without a model line, run n2). One
`ModelFamily` member per builtin: `SourceV SourceI Vcvs Vccs Ccvs Cccs Mutual`, each "the
simulator's built-in device for that element kind, with no parameters". `ElemKind::master_kind`
of `VSource ISource Vcvs Vccs Cccs Ccvs` is an optional `Model` (as `Coupling`'s; `Port` keeps
none: VACASK spells a port as a vsource and a resistor). `check` (`FieldDomain`): a resolved
master of a builtin family serves only its kind; a source or linear controlled source names
only its builtin (a coupling may also name a core, and a coupling bound to `Mutual` does not set
`CouplingCore`); a builtin model has no parameters and no polarity. A builtin family needs no
`DeviceLibrary`, so it never sets `ModelWithoutLibrary` (confirmed by the VACASK agent: the builtins `vsource isource vcvs vccs ccvs cccs mutual` need a
`model` line and no `load`, VD cir-loading, run n2; VACASK has no port module and its `b` source
takes no model, VD dev-builtin-behavioral; model names are the deck's choice). The ideal transmission line (A20, R39) is one more builtin: `ModelFamily::TlineIdeal`
(VACASK `tline_ideal`, a loaded OSDI module), parameterless and without polarity, the optional
master of a `LosslessLine` (`master_kind` says so; `check` accepts only that family). Unlike
the others a target may need a `DeviceLibrary` for it, so it is not exempt from
`ModelWithoutLibrary` (`circuit_tables::library_free`). Features: `ElementWithoutModel`
(an element with no master: VACASK lacks it) and `BuiltinMaster` (a source, linear controlled
source or lossless line naming a model, or a coupling bound to `Mutual`: ngspice lacks it; its
T card names none). The two conversions
are the dialect slices' neutral passes (`src/passes/`), not this crate's; the rule they follow:
one model per scope and family, in the scope that first needs it, named by the family's
IR-text word (`source_v`, `vcvs`, …) and renamed on a collision; the reverse pass drops the
models and sets `Element.model` to `NONE`. Names: the VACASK agent's `SourceV`/`SourceI`
(not `VSource`/`ISource`, which read as the element kinds).

## 5. What to deliver

The kernel is committed (read `rewrite/IR-DECISIONS.md` first: it says how the kernel's doors
work). Your tables go in `CircuitDetails` (`element.rs`); `Element.detail` stays one `u32` read by
kind, behind typed accessors (`design.passive(&element)`, asserting the kind). Implement
`RowFacts` (IR-KERNEL §7.2) for `Element` and `Model` in their files, and the constant facts
`ElemKind::terminals()`, `has_detail()`, `needs_model()`.

The files of the circuit owner in IR-KERNEL §10, complete (types, fields, enums, builder methods
in `build_circuit.rs`, read accessors), bodies `todo!()`, your `Extra` features in your region of
`feature.rs`, and `rewrite/COVERAGE-circuit.md`: one line per row of `INV-constructs.md` §D, §E,
§F and of `INV-semantics.md` categories 5, 6, 7, naming the field, variant, feature or `Opaque`
class that holds it (or `gap: <why>`). A row with no home is a finding, not a failure: list it.
