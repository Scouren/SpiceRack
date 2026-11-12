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
  up); a source's `dc` is always present (each reader fills its simulator's rule for a missing DC);
- a **mode field** when the same value applies differently: `IcMode::{UicOnly, InOp}` (ngspice
  applies a device `IC=` only under UIC, Xyce and LTspice in the operating point);
- a **superset row with restrictions** when dialects have sub-cases of one formula: one `Am` row
  holds the general formula; each writer's features say which field values it can spell.

Do not write a conversion into the IR (no "ngspice order" comment on a positional list, no
expression that hides a clamp). If you cannot state a simulator's meaning with a field, add the
field and say which dialect needs it in its doc.

## 2. Elements

Header as IR-KERNEL §6. For every `ElemKind` document the terminal roles in order, the allowed
terminal count (fixed or a range), whether `model` is required, and its detail row.

| Family | Kinds | Detail row (owner file) | Decisions |
|---|---|---|---|
| passives | resistor, capacitor, inductor | `Passive` (`passive.rs`) | `value` (Ω, F, H; `NONE` when only the model gives it), `ic` + `ic_mode`, `tc1`, `tc2`, `temp_abs`, `dtemp` (two fields: Xyce and VACASK have both, HSPICE only the offset), `noisy`, `parasitics` (row or `NONE`: `rser lser rpar cpar rlshunt`, LTspice's; the builders take them too) |
| charge capacitor | charge-defined capacitor | `Behav` with `OutKind::Charge` | one construct, five spellings (`INV-constructs` D5) |
| coupling | coupling | `Coupling { inductors: NameList, k: ExprId }` | no terminals; every listed pair is coupled by `k`; `k` may be an expression |
| sources | voltage source, current source, RF port | `Source` (`source.rs`) | three independent slots: `dc` (what `.op`/`.dc` read; always present), `ac_mag` + `ac_phase_deg` (`NONE` = no AC stimulus), `wave` (`NONE` = the transient holds `dc`); `distof1`, `distof2`; `tc1`, `tc2` (Spectre scales the whole waveform); `parasitics`; port fields `port_index`, `z0`, with the value convention pinned as the EMF behind `z0` (Spectre's user value is half of it: its reader doubles) |
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
| `Pulse` | `v1 v2 delay rise fall width period cycles` | `cycles == NONE` means unlimited; a zero `rise` means an ideal edge (5.04, 5.05) |
| `Sin` | `offset amplitude freq delay damping phase_deg cycles pre` | `phase_deg` is the phase at `t = delay`; `pre` says what the source is before the delay (5.06, 5.07) |
| `Exp` | `v1 v2 td1 tau1 td2 tau2` | `td2` is absolute (VACASK counts it from `td1`: its reader adds) (5.08) |
| `Pwl` | `points` (t, v pairs, absolute times), `delay`, `repeat_from`, `repeat_join` | relative times are accumulated by the reader; a deck with no `t = 0` point gets the one its simulator implies (HSPICE ramps from the DC value, ngspice holds the first value: the reader writes the point) (5.09–5.11) |
| `Sffm` | `offset amplitude carrier_freq mod_index signal_freq delay phase_c_deg phase_m_deg law` | named, never positional; `law` as §1 (5.01, 5.02) |
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
