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
letter is the neutral fact `letter::spice_letter(design, element)`, and a name that does not
begin with it is `CircuitExtra::NameWithoutLetter` (a SPICE-card writer lacks it; a pass
renames). Two elements of one name in one scope are `Violation::DuplicateElement`
(`element::design_check`). For every `ElemKind` document the terminal roles in order, the allowed
terminal count (fixed or a range), whether `model` is required, and its detail row.

| Family | Kinds | Detail row (owner file) | Decisions |
|---|---|---|---|
| passives | resistor, capacitor, inductor | `Passive` (`passive.rs`) | `value` (Ω, F, H; `NONE` when only the model gives it), `ic` + `ic_mode`, `tc1`, `tc2`, `temp_abs`, `dtemp` (two fields: Xyce and VACASK have both, HSPICE only the offset), `noisy`, `parasitics` (row or `NONE`: `rser lser rpar cpar rlshunt`, LTspice's; the builders take them too) |
| charge capacitor | charge-defined capacitor | `Behav` with `OutKind::Charge` | one construct, five spellings (`INV-constructs` D5) |
| coupling | coupling | `Coupling { inductors: NameList, k: ExprId }` | no terminals; every listed pair is coupled by `k`; `k` may be an expression |
| sources | voltage source, current source, RF port | `Source` (`source.rs`) | three independent slots: `dc` (what `.op`/`.dc` read) with `dc_rule: DcRule::{Given, WaveAtZero}` (A12 C4: `dc` is `NONE` exactly under `WaveAtZero`, the wave's t = 0 value; ngspice has `WaveAtZero` for SIN, PULSE, EXP, PWL, VACASK for every wave, and VACASK lacks `Given` with a wave: features `DcGivenWithWave`, `DcAtZero<Wave>`), `ac_mag` + `ac_phase_deg` (`NONE` = no AC stimulus), `wave` (`NONE` = the transient holds `dc`); `distof1`, `distof2`; `tc1`, `tc2` (Spectre scales the whole waveform); `parasitics`; port fields `port_index`, `z0`, with the value convention pinned as the EMF behind `z0` (Spectre's user value is half of it: its reader doubles) |
| linear controlled | VCVS, VCCS, CCCS, CCVS | `Controlled { gain, sense: NameId }` | terminals (out+, out−, ctl+, ctl−) or (out+, out−) + the sensing element's name |
| polynomial | one kind, `output: V \| I`, `control: Voltage \| Current` | `Poly { dimension, controls: NameList, coeffs: ExprList }` | stays `Poly`; expanding it is a pass |
| table | one kind | `Table { input: ExprId, points: ExprList, law: TableLaw, output }` | linear between points, ends held; ngspice's native TABLE rounds corners: its own law |
| behavioural | one kind | `Behav { output: OutKind::{Voltage, Current, Charge}, expr }` | positive current flows from the first terminal through the source to the second |
| Laplace | one kind | `Laplace { num: ExprList, den: ExprList, output }` | terminals (out+, out−, in+, in−); coefficients in ascending powers of `s`, absolute gain already applied |
| semiconductors | diode, BJT, MOSFET, JFET, MESFET | `Semi { area, flags, ic, temp_abs, dtemp }` or none | terminal ranges: BJT 3–5, MOSFET 3–7, diode 2–3 (thermal); everything else is a named instance parameter |
| switches | voltage switch, current switch | `Switch { initial: SwitchState, sense: NameId }` | the initial state seeds the operating point only |
| lines | lossless, lossy, RC, single lossy, coupled | `Line { z0, td, freq, nl, length }` or none | lossless: either `z0` + `td` or `freq` + `nl` as written (do not convert) |
| modules | code model, Verilog-A module, subcircuit instance, unresolved master | none | `model` names the master; `Ref.def == NONE` for the unresolved kind is legal and is what a Spectre reader produces without the kit |

`mult` is the header's field for every kind: a reader lowers `m=` (and VACASK `$mfactor`) to it,
never to a parameter named `m`.

## 3. Waveforms (`wave.rs`)

`WaveKind::{Pulse, Sin, Exp, Pwl, Sffm, Am, TrNoise, TrRandom, Pattern}`, one table each,
referenced from `Source.wave` as `{kind, index}`. Every field is an `ExprId`; times in seconds,
frequencies in hertz, phases in degrees.

| Wave | Fields | Decisions (evidence in `INV-semantics.md` §5) |
|---|---|---|
| `Pulse` | `v1 v2 delay rise fall width period cycles law` | `cycles == NONE` means unlimited; a zero `rise` means an ideal edge (5.04, 5.05); `law: PulseLaw::{Spice, Vacask}` (A12 C1: VACASK needs `rise > 0`, `fall <= 0` is no falling edge, `period <= 0` one pulse) |
| `Sin` | `offset amplitude freq delay damping phase_deg cycles pre` | `phase_deg` is the phase at `t = delay`; `pre` says what the source is before the delay (5.06, 5.07) |
| `Exp` | `v1 v2 td1 tau1 td2 tau2 td2_from` | `td2` counts from `td2_from: TdFrom::{Zero, Td1}` (A12 C2: SPICE from 0, VACASK from `td1`; the reader no longer adds) (5.08) |
| `Pwl` | `points` (t, v pairs, absolute times), `delay`, `repeat_from`, `repeat_join` | relative times are accumulated by the reader; a deck with no `t = 0` point gets the one its simulator implies (HSPICE ramps from the DC value, ngspice holds the first value: the reader writes the point) (5.09–5.11) |
| `Sffm` | `offset amplitude carrier_freq mod_index signal_freq delay phase_c_deg phase_m_deg law pre` | named, never positional; `law` as §1 states the clamp and the defaults only; what it is before the delay is `pre: PreDelay` as for `Sin` and `Am` (A12 C3: VACASK holds its value at `delay`, ngspice is 0) (5.01, 5.02) |
| `Am` | the general formula of 5.03 with every term a field, `pre` | each dialect's AM is a restriction of it |
| `TrNoise`, `TrRandom` | ngspice's fields | time domain only |
| `Pattern` | `bits levels delay rise fall sample repeats repeat_start` | 5.22 |

## 4. Models (`model.rs`)

`Model { name, family: ModelFamily, polarity: Polarity, params: ParamList, bin_of: NameId, ako:
Ref }`.

- `ModelFamily` is the **equation family**, a closed enum (`Resistor`, `Capacitor`, `Diode`,
  `GummelPoon`, `Vbic`, `Hicum`, `Mos1`, `Mos2`, `Mos3`, `Mos6`, `Mos9`, `Bsim3`, `Bsim4`,
  `BsimSoi`, `Hisim2`, `HisimHv`, `Psp103`, `Vdmos`, `JfetShichman`, `JfetParkerSkellern`,
  `Mesfet`, `Hfet`, `Switch`, `CurrentSwitch`, `Ltra`, `Txl`, `Cpl`, `Urc`, `Core`, `CodeModel`,
  `Module`, …: fix the list from `INV-constructs` §F and D68). A level number is a dialect's
  spelling of a family: readers map (type word, level) → family, writers map back. There is no
  "neutral level".
- `ModelFamily::Other` carries the type word as written for a family the IR does not know; a
  writer of another dialect refuses it.
- Parameter names in `params` are lowercase and are the family's own names (the model's
  published names); a dialect's renames live in that dialect's tables, not here.
- Bins: one `Model` per bin, `bin_of` naming the base; a Spectre model group is a run of them.

## 4a. Device libraries (`library.rs`, AMEND-A12 C5, R17)

`ItemKind::DeviceLibrary` with row `DeviceLibrary { family: ModelFamily, path: NameId }`: the
simulator's own implementation of `family` is available from here on; `path` `NONE` is the
target's default file, a variant path (`spice/sn/…`) is kept as written. IR text:
`device_library <family> [path="<path>"]`. Features: `Item(DeviceLibrary)`;
`CircuitExtra::ModelWithoutLibrary` (design-level, `element::design_features`: a reachable model
whose family has no reachable library of that family); `CircuitExtra::ElementWithoutModel`
(item-level: an element whose `model.name` is `NONE`; VACASK instantiates every device through a
model, run n2). Neutral passes add and remove libraries and models; readers and writers stay
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
`DeviceLibrary`, so it never sets `ModelWithoutLibrary` (assumed from DIALECT-VACASK: the
builtins are not loaded; to be confirmed by the VACASK agent). Features: `ElementWithoutModel`
(an element with no master: VACASK lacks it) and `BuiltinMaster` (a source or linear controlled
source naming a model, or a coupling bound to `Mutual`: ngspice lacks it). The two conversions
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
