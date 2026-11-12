# Coverage, circuit half (Definition phase)

One line per row of `INV-constructs.md` §D, §E, §F (and B8–B10, B16, B17, B24) and of
`INV-semantics.md` categories 5, 6, 7 (and 3.09, 9.02, 9.06): `<row> → <home>` or `gap: <why>`.
Homes name the type and field in `crates/ir/src` (`Passive.tc1` = field `tc1` of `passive::Passive`),
an `ElemKind`/`ModelFamily`/law variant, an `Extra` feature, an `Opaque` class, or another owner.
"Reader" means the dialect reader states its simulator's meaning in the named field (no
conversion); "pass" means a conversion outside this crate. The gaps and proposals close the file.

## INV-constructs §B (rows the circuit owns)

- B8 → `ElemKind::Instance`, `Element.model` (`DefKind::Subckt`), `Element.params`
- B9 → `ElemKind::Instance` / `Module` / a device kind by what the reader binds the master to; `Unresolved` otherwise
- B10 → `Element.mult`; `Extra::MultInstance` (ESPice lacks it); a subckt that declares `m` keeps it in `params`
- B16 → `Element.name` is the whole name (A13, R23), unique in its scope; the kind is `ElemKind`, never the letter; `letter::spice_letter` and `CircuitExtra::NameWithoutLetter` (a pass renames for a SPICE-card target)
- B17 → names: `Coupling.inductors`, `Controlled.sense`, `Switch.sense`, `Poly.controls` (current control); `Expr::I` (kernel)
- B24 → `ElemKind::ModuleModel` (ng `N`, va) and `ElemKind::Module` (sp, hs); `ModelFamily::Module` + `Model.module`; `$mfactor` → `Element.mult`

## INV-constructs §D (devices)

- D1 → `ElemKind::Resistor`, `Passive.value`, optional `Element.model`
- D2 → `Passive.{value, ac, temp_abs, dtemp, tc1, tc2, tce, noisy, scale}`, `Element.mult`; `l w bv_max noise` → `Element.params`
- D3 → `Passive.value` reading the circuit (instantaneous R); `Extra::PassiveDynamicValue`
- D4 → `ElemKind::Capacitor`, `Passive`
- D5 → `ElemKind::ChargeCapacitor` + `Behav{output: Charge}`
- D6 → `Passive.ic` + `IcMode::{UicOnly, InOp}`; `Extra::IcUicOnly`, `IcInOp`; va: run owner's transient `ic` list
- D7 → `Element.mult` (n parallel units scale C and the parasitics)
- D8 → `Parasitics` via `Passive.parasitics`, `Source.parasitics`, `Behav.rpar`; ratings → gap G1
- D9 → `ElemKind::Inductor`, `Passive`; `ModelFamily::Inductor`
- D10 → `ElemKind::Coupling`, `Coupling{inductors, k}`; `Extra::CouplingMany`
- D11 → `Coupling` + `Element.model` → `ModelFamily::CoreJilesAtherton`; `Extra::CouplingCore`
- D12 → `ElemKind::VSource`, `Source{dc, ac_mag, ac_phase_deg, wave}`
- D13 → `ElemKind::ISource`, `Source`
- D14 → `ElemKind::VSource` with `dc` 0 (sp reader's `iprobe`; its writer spells a 0 V vsource back as `iprobe` only by its own table)
- D15 → `ElemKind::Port`, `Source` + `PortSpec{index, z0, noise_temp}`, slots as EMF
- D16 → `ElemKind::Vcvs`, `Controlled.gain`
- D17 → `ElemKind::Vccs`, `Controlled.gain`; `m=` → `Element.mult`
- D18 → `ElemKind::{Cccs, Ccvs}`, `Controlled{gain, sense}`
- D19 → `ElemKind::Poly`, `Poly{dimension, controls, coeffs, output, control}`; xy `B … POLY` read as the same row
- D20 → `ElemKind::Behav`, `Behav{Voltage|Current, expr}`; hs `SCALE MAX MIN` → `Behav.expr` (product, `Min`/`Max`/`Clamp`)
- D21 → `ElemKind::Table`, `TableSource` with `TableLaw::{Exact, NgspiceRounded}`; `Extra::Table*`
- D22 → `ElemKind::Behav`; ESPice limits are its writer's features
- D23 → `ElemKind::Laplace`, `Laplace{num, den, input, form: Polynomial}`; ng `s_xfer` lift is a pass
- D24 → `ElemKind::Diode`, `Semi{area, ic, temp_abs, dtemp, initial}`; `pj perim w l …` → `params`
- D25 → `ElemKind::Bjt` (3–5 terminals); node/model split is the reader's
- D26 → `ElemKind::Mosfet` (3–7); va `instance_<p>` renames are its tables; `$mfactor` → `mult`
- D27 → `ElemKind::Jfet`
- D28 → `ElemKind::Mesfet`; `ModelFamily::{Mesfet, Hfet1, Hfet2}`
- D29 → `ElemKind::VSwitch`, `Switch.initial`
- D30 → `ElemKind::ISwitch`, `Switch{sense, initial}`
- D31 → `ElemKind::LosslessLine`, `Line{z0, td | freq, nl | length, vel}`; `Extra::LineFreqNl`, `LineLengthVel`; sp loss terms → gap G2
- D32 → `ElemKind::LossyLine`, `ModelFamily::Ltra`
- D33 → `ElemKind::RcLine`, `Line{length, lumps}`, `ModelFamily::Urc`
- D34 → `ElemKind::SingleLossyLine`, `ModelFamily::Txl`, `Line.length`
- D35 → `ElemKind::CoupledLines` (4–18 terminals), `ModelFamily::Cpl`
- D36 → `Opaque` `Private` (hs W element); `ModelFamily::HspiceW` holds its model → gap G2
- D37 → `ElemKind::CodeModel`, `ModelFamily::CodeModel` + `Model.word`
- D38 → `Opaque` `Private` (lt A devices)
- D39 → `ElemKind::ModuleModel`
- D40 → `Opaque` `Private` (xy Y devices)
- D41 → `Opaque` `Private` (xy U)
- D42 → `Opaque` `Private` (lt `@`, `&`)
- D43 → `Semi.initial` (`InitialState::{Off, On}`); `thermal` → the terminal count (`Extra::DiodeThermal`)
- D44 → `ElemKind::terminals` (fixed, range; Any/ByModel = 0–255 with the master's port count checked by the reader)
- D45 → the reader's position table; the IR holds `Element.model` and `Passive.value` separately (6.10)
- D46 → `ElemKind` per master (sp reader/writer tables)
- D47 → `ElemKind` + `ModelFamily` (va tables)
- D48 → reader: `Opaque` `Unread`/`InvalidInSource` with its P code
- D49 → reader syntax (no IR content)
- D50 → writer (cdl) refusal of a kind; no IR content
- D51 → `ElemKind::CodeModel` (`d_cosim`) + `Load`; Yosys path is a builder concern
- D52 → `Passive.value` + `Element.model`; `l`/`w` in `params`
- D53 → `Passive.value` reading the circuit, `noisy`; `Extra::PassiveDynamicValue`
- D54 → kernel expression trees
- D55 → `Coupling.k` (expression)
- D56 → `Coupling.inductors` (all pairs)
- D57 → `Switch.initial`; `Extra::SwitchInitial`
- D58 → `Source.distof` → `Distof`; `Extra::SourceDistof`
- D59 → gap G3 (ngspice `external`)
- D60 → `ElemKind::{Vcvs, Vccs}` (keyword is ng's spelling)
- D61 → `ElemKind::FreqTable`, `FreqTable{law, unit}`
- D62 → `Behav` over `TableSource` (two-point clamp-linear, min/max of inputs) as each reader writes it; hs smoothing → `TableLaw::Smoothed` (see 5.29)
- D63 → `Behav.{tc1, tc2, temp_abs, dtemp}`; `Extra::BehavTc`
- D64 → `Semi.area`; `Extra::SemiArea`
- D65 → `ElemKind::Diode` 3 terminals; `Extra::DiodeThermal`
- D66 → `ModelFamily::DiodeLevel3` (its law over `w`, `l`, `m`)
- D67 → `ElemKind::Bjt` 3–5 terminals; padding is the writer's
- D68 → `ModelFamily::{Mos1, Mos2, Mos3, Mos6, Mos9, Bsim3, Bsim4, B4Soi, B3SoiFd, B3SoiDd, B3SoiPd, Soi3, Hisim2, HisimHv, Vdmos}`; bins `Model.bin_of`
- D69 → `ElemKind::CodeModel`; private ones `Opaque` `Private`
- D70 → `Opaque` `Private` (digital U gates)
- D71 → `ElemKind::RcLine`; lowering to a ladder is a pass
- D72 → `ModelFamily::{Ltra, Txl, Cpl}` with params; mapping to W/mtline is a pass
- D73 → `ElemKind::LosslessLine` (va reader maps `tline_ideal`)
- D74 → `ElemKind::ModuleModel`, `Model.module`
- D75 → `ElemKind::Behav` current (lt reader writes I = max(I0(t), 0)·g(V)); see 5.23
- D76 → `ElemKind::Table` with `input` V(n+, n−), `output: Current`; `step(…)` → run owner's `.step` (gap G4); `R=` → `ElemKind::Resistor`
- D77 → `Opaque` `Private`
- D78 → `Controlled.gain` (expression)
- D79 → gap G5 (lt hysteretic core inductor)
- D80 → `ElemKind::FluxInductor` + `Behav{output: Flux}`
- D81 → `Passive.value` (expression; `temp` is `Expr::Temp`)
- D82 → `Passive.noisy`; `Extra::PassiveNoiseless`
- D83 → `Behav{output: Resistance | Power}`, `Behav.rpar`; `laplace=` → `ElemKind::Laplace` with `input`
- D84 → `Line.{z0, td}` (spelling)
- D85 → `Parasitics.rser` written by the lt reader
- D86 → `Parasitics.cpar` written by the lt reader
- D87 → `Parasitics.rlshunt`
- D88 → run owner (`NodeSet` scope)
- D89 → reader syntax
- D90 → `ElemKind::Delay`, `DelayLaw::Sampled`
- D91 → `ElemKind::Laplace`, `LaplaceForm::{Polynomial, PoleZero}`
- D92 → `TableLaw::Smoothed` + `TableSource.delta`
- D93 → VCR: `Behav{Resistance}`; VCCAP: `Behav{Capacitance}` (law unsettled in the inventory)
- D94 → OPAMP: `ElemKind::Vcvs` gain 1e14 (hs reader); TRANSFORMER: gap G6
- D95 → `ModelFamily::HspiceW` params
- D96 → gap G7 (N-port data element)
- D97 → `ElemKind::Port`; `hblin` → gap G8
- D98 → reader rule: ESPice `b… v=` → `Behav`; real hs B (IBIS) → `Opaque` `Private`
- D99 → gap G9 (C/L `POLY`)
- D100 → gap (structure, kernel): `.connect` → the reader merges nets or writes a 0 V `VSource`
- D101 → `Behav.rpar`; `ic`, `tripdv`, `tripdt` → gap G10
- D102 → `ElemKind::Table` (`TableLaw::Exact`)
- D103 → `ElemKind::Igbt`, `ModelFamily::Igbt`
- D104 → `Passive.value` expression + `tc1 tc2 dtemp`, `Element.mult`; `SCALE` → `Passive.scale`
- D105 → gap G5 (magnetic winding/core)
- D106 → per form: `Behav`, `Poly`, `Table`, `Laplace`, `Delay`, gates as D62; `PWL(1)` → `TableSource`; `ABS` → `Behav.expr`; `IC` → gap G10
- D107 → `U` line: gap G2; `U` digital: `Opaque` `Private`
- D108 → `Opaque` `Private`
- D109 → xy reader leaves `Element.mult` `NONE`; `Extra::MultVSource`, `MultISource`
- D110 → reader syntax
- D111 → `ElemKind::Port` with a wave
- D112 → `Passive.{tc1, tc2, temp_abs, dtemp, tce}`, `tce` a resistor's only (R74: Xyce 7.10 refuses `TCE` on C and L, "Unrecognized parameter TCE for device C1", against the inventory row); `AGE`/`D` → `Element.params`, refusal elsewhere (no field: gap G11)
- D113 → `Semi.area`
- D114 → `CouplingCore` + `ModelFamily::CoreJilesAtherton`
- D115 → `Coupling.k` expression of `Time`
- D116 → `Behav` (reader syntax)
- D117 → `ModelFamily::SwitchLogCubic`, `CurrentSwitchLogCubic`
- D118 → `Switch.control`; `Extra::SwitchControlExpr`
- D119 → `Opaque` `Private` (undocumented 7-node BJT)
- D120 → `ModelFamily` per level where established (D 1/2/200, J 1/2, M 1/2/3/6/9/10/14/54/18/103, Q 1/11/12/230/234/504/505); M 77, 102, 107, 110, 301, 2000, 2001 and Q 23 → `ModelFamily::Other` (gap G12)
- D121 → `ElemKind::Module` (xy plugin) or `Opaque` `Private` when no module is loaded
- D122 → `YDELAY` → `ElemKind::Delay` (`Ideal`); `YLIN` → gap G7; `YTRANSLINE` → gap G2
- D123 → `Opaque` `Private`
- D124 → `Behav{Voltage | Current}` (`potential`/`flow` are spellings)
- D125 → `Pwl` (`period` → `PwlRepeat::Period`; `scale offset stretch` folded into the points; `file` → `Pwl.file`)
- D126 → `ElemKind::Mosfet` 5 terminals, `ModelFamily::Vdmos`
- D127 → `Element.mult`
- D128 → `ElemKind::ModuleModel` / `ModelFamily::{Psp103, BsimBulk, Vbic}`; `opamp` module → `ModelFamily::Module`
- D129 → `Sin.{amplitude2, freq2}`, `Am` (sp), `Source.{pac_mag, pac_phase_deg, tc1, tc2, noise_psd}`, `Element.mult`, `Pwl.file`; `xfmag` → gap G13
- D130 → `Behav{Charge, Resistance, Conductance, Capacitance}`; `ddt`/`idt` kernel `Func`
- D131 → `delay` → `ElemKind::Delay`; `relay` → `VSwitch` + `ModelFamily::Relay`; rest → `Opaque` `Private` (gap G14)
- D132 → `ModelFamily::{Bsim1, Bsim2, Bsim3, BsimSoi, B3SoiPd, Vbic, Mextram, JfetShichman, Gaas, Mos0, Ekv, Hisim1}`; Philips/UCSD/TFT → `Other`
- D133 → `ElemKind::Unresolved`
- D134 → `Line` + model families; the unified record is a pass's view (gap G2)
- D135 → builder: `ElemKind` + `ModelFamily` by the PDK binding pass (not a reader)
- D136 → `ElemKind` role order (documented per kind)

## INV-constructs §E (waveforms)

- E1 → `Source.dc` (always present)
- E2 → `Source.{ac_mag, ac_phase_deg}`
- E3 → `Pulse`; `Pulse.cycles`, `Extra::PulseCycles`
- E4 → `Sin`; `Sin.cycles`, `Extra::SinCycles`
- E5 → `Exp` (`td2` absolute)
- E6 → `Pwl.points`
- E7 → `Pwl.{delay, repeat: From, repeat_at}`; `Extra::PwlDelay`, `PwlRepeatFrom`
- E8 → `Pwl.points` (absolute, accumulated by the lt reader as `Add` trees)
- E9 → `Pwl.file`; `Extra::PwlFile`
- E10 → `Sffm` named fields + `SffmLaw::{Spice3, Ngspice}`
- E11 → `Am` (general formula), `PreDelay`, `AmModTime`
- E12 → `TrNoise`
- E13 → `TrRandom` + `TrRandomKind`
- E14 → `Pattern`
- E15 → reader syntax
- E16 → every wave field is an `ExprId`
- E17 → writer (W0026 cut) or a pass; nothing in the IR
- E18 → run owner (`Tran.startup`, pass `startup_ramp`)
- E19 → sp reader table onto `Pulse`, `Sin`, `Sffm`, `Exp`, `Pwl`, `Am`
- E20 → va reader table, same rows
- E21 → `Sffm.{delay, phase_m_deg, phase_c_deg}`
- E22 → `Pwl.file` + `delay`, `repeat`
- E23 → reader syntax (points are expressions)
- E24 → `PortSpec.{power, freq}`
- E25 → `SffmLaw::Ngspice`, `Am.pre = PreDelay::Zero`
- E26 → `Source.ac_mag` expression
- E27 → gap G15 (lt `wavefile=`)
- E28 → `REPEAT` → `PwlRepeat::From` + `repeat_count` + `PwlJoin::Ramp`; scale factors folded; `file=` → `Pwl.file`; `Trigger=` → gap G16
- E29 → `Pwl.{delay, repeat}` (hs reader)
- E30 → `Pwl.points` with the explicit (0, dc) point (hs reader)
- E31 → `Pwl.points` (hs reader swaps PL pairs)
- E32 → `Pattern{repeats, repeat_start}`; xy repeat semantics unsettled (gap G17)
- E33 → gap G18 (`.vec`)
- E34 → gap G19 (data-driven PWL)
- E35 → `PwlRepeat::From` with `repeat_at` between points
- E36 → reader syntax
- E37 → `Source.ac_phase_deg` expression
- E38 → `Am.pre`, `Sin.phase_deg` (`tdphase`), `PreDelay::HoldDelayValue`; DC at t = 0 → `Source.dc` written by the va reader

## INV-constructs §F (models)

- F1 → `Model{name, family, polarity, params, version}`
- F2 → `ModelFamily` + `Polarity` (reader table type word → family)
- F3 → no neutral level: `ModelFamily` look-alikes (`JfetShockley` vs `JfetParkerSkellern`, `HbtUcsd`, …); ng level > 99 → writer refusal
- F4 → sp reader/writer table onto `ModelFamily`
- F5 → va reader/writer table onto `ModelFamily`; `model_load` → kernel `Load`
- F6 → `Model.polarity`
- F7 → `Model.bin_of`; `Extra::ModelBin`
- F8 → `Model.ako`; `Extra::ModelAko`
- F9 → `ModelFamily::{Resistor, Capacitor, Inductor}` vs `*Multiplier` (xy)
- F10 → `ModelFamily::Resistor` params (family names); Spectre renames → its tables
- F11 → `ModelFamily::Capacitor` params
- F12 → `ModelFamily::DiodeLevel3` / `DiodeSpectre` params; aliases in reader tables
- F13 → dialect tables (writer answers per parameter), not IR
- F14 → dialect tables; a name the target lacks → writer refusal
- F15 → va tables
- F16 → `Model.params` values (`Expr::Str`, `Expr::List`, numbers); `version` → `Model.version`
- F17 → reader rule: a built-in name is a `ModelFamily`, never `Module`
- F18 → `ModelFamily::Module` + `Model.module` (va), `ModelFamily::Other` + `Model.word` (sp masters the IR lacks)
- F19 → writer refusal by family; `DiodePspice` vs `Diode` are separate families
- F20 → `ModelFamily::CodeModel` + `Model.word = s_xfer`
- F21 → linker / reader binding (kernel)
- F22 → lt reader table (`DiodeLtspice`, `Igbt`, …); ratings → gap G1
- F23 → gap G20 (`.MALIAS`)
- F24 → run owner (`.alter` with `.del lib`)

## INV-semantics category 5

- 5.01 → `Sffm` named fields
- 5.02 → `SffmLaw::Ngspice`; defaults as `TStop` expressions
- 5.03 → `Am` general formula, `Am.pre`, `AmModTime`, `Extra::Am*`
- 5.04 → `Pulse.cycles` (`NONE` unlimited)
- 5.05 → `TStep`/`TStop` leaves in `Pulse.{rise, fall, width, period}`, `Exp` (kernel A1). The inventory's `given` bit is not needed: a given zero is a literal 0, ngspice's default is the leaf
- 5.06 → `Sin.{phase_deg, delay, cycles, pre}`
- 5.07 → `Sin.{damping, amplitude2, freq2}`
- 5.08 → `Exp.td2` absolute
- 5.09 → `Pwl.points` absolute
- 5.10 → `Pwl.points` explicit t = 0
- 5.11 → `Pwl.{delay, repeat, repeat_at, repeat_count, join, join_width}`
- 5.12 → `Pwl.file` (the inventory proposes inline points: gap G21)
- 5.13 → `Source.{dc, ac_mag, wave}` independent
- 5.14 → `Source.dc` always, `ac_mag` 1 written by the reader
- 5.15 → run owner (`Tran.startup`)
- 5.16 → `Element.mult` (kept, not folded: IR-CIRCUIT §2); `Extra::MultVSource`, `MultISource`, `MultControlled`; sp per-copy current → gap G22
- 5.17 → `ElemKind::Port`, `PortSpec`, EMF convention
- 5.18 → `TrNoise`/`TrRandom` vs `Source.noise_psd`
- 5.19 → `Source.{tc1, tc2}`, `Behav.{tc1, tc2}`
- 5.20 → gap G8 (HB drive)
- 5.21 → `Distof`
- 5.22 → `Pattern`
- 5.23 → `Behav` current (lt reader)
- 5.24 → sign rule in `controlled.rs` header
- 5.25 → `TableLaw`
- 5.26 → `Poly` (kept; expansion is a pass, per IR-CIRCUIT over the inventory's proposal)
- 5.27 → `Laplace` ascending, absolute gain; sp `gainfactor` undone by its reader
- 5.28 → `FreqTable{law, unit}`
- 5.29 → gates: `Behav`/`TableSource`; OPAMP: `Vcvs`; TRANSFORMER: gap G6; modifiers in `Behav.expr`
- 5.30 → `ElemKind::Delay`, `DelayLaw`
- 5.31 → `OutKind` (v, i, q, φ, r, g, c, c·v, l, p)
- 5.32 → `Source.dc` written with the phase
- 5.33 → folded into `Pwl.points` as expressions

## INV-semantics category 6

- 6.01 → `Element.mult`, composed down the hierarchy (pass semantics); ng override vs xy multiply: the ng reader writes what ng simulates (gap G23 for the override of an inner `m`)
- 6.02 → `Extra::MultCapacitor`, `MultInductor`
- 6.03 → `Element.mult`; ratings gap G1
- 6.04 → `Parasitics`; `Extra::ElementParasitics`, `SourceParasitics`
- 6.05 → `Parasitics` written by the lt reader
- 6.06 → run owner (option `scale`) or the reader folds it into geometry params (gap G24)
- 6.07 → `Passive.scale`
- 6.08 → `ModelFamily::ResistorMultiplier`
- 6.09 → `Passive.value` (reader fills)
- 6.10 → `Passive.value` and `Element.model` separate
- 6.11 → model params default-complete (reader), `ModelFamily::Resistor`
- 6.12 → `Passive.{tc1, tc2}`
- 6.13 → `Passive.tce`; `AGE` gap G11
- 6.14 → `Passive.value` dynamic + `noisy`; ng's dropped `dtemp` → the ng reader writes `dtemp` `NONE`
- 6.15 → `Passive.{tc1, tc2, dtemp}`, `mult`; `CTYPE`/`LTYPE` → `OutKind::{Capacitance, CapacitanceCharge}` on a `Behav`
- 6.16 → capacitor `Passive.value` = incremental c(v); `ChargeCapacitor`; `OutKind::{Capacitance, CapacitanceCharge}`; `FluxInductor`
- 6.17 → `Passive.{ac, noisy}`
- 6.18 → `Coupling`
- 6.19 → `ModelFamily::{SwitchHyst, SwitchHystSmooth, SwitchLogCubic, Relay, CurrentSwitch*}`, `Switch.{sense, control, initial}`
- 6.20 → `Line` + model families; per-writer refusals
- 6.21 → `ModelFamily::Cpl` (its own matrix order); conversion is a pass
- 6.22 → `ModelFamily::HspiceW` params `rs`, `gd`
- 6.23 → `ElemKind::RcLine`, `Line.lumps`; hs U → gap G2
- 6.24 → `Line.{nl}` written by the reader
- 6.25 → reader materialises elements (Xyce `smoothbsrc`)
- 6.26 → `ElemKind::terminals` + role order per kind
- 6.27 → `ModelFamily::DiodeLevel3` (its law); `Semi.area`
- 6.28 → `Semi.initial`; va's anonymous instances are its reader's
- 6.29 → gap G7
- 6.30 → R level 2: gap G25; U gates `Opaque`; LTRA hints → `Element.params`
- 6.31 → `Behav.expr` with the device's own `V`/`I` probe
- 6.32 → gap G26 (xy 0 Ω supernode)
- 6.33 → `Semi.area`, `Coupling` (pairs, `k` expression)

## INV-semantics category 7

- 7.01 → `ModelFamily` (no level)
- 7.02 → `ModelFamily::Other` / writer refusal
- 7.03 → `JfetShichman`, `JfetParkerSkellern`, `JfetShockley`, `JfetHspice2`
- 7.04 → `Model.version`
- 7.05 → `Bsim1`, `Bsim2`; per-parameter units are the family's (gap G27 for hs's µm units)
- 7.06 → `Diode` vs `DiodePspice` vs `DiodeSpectre`
- 7.07 → `DiodePspiceTemp`
- 7.08 → `DiodeLevel3`, `DiodeSpectre` params; writer checks domains
- 7.09 → reader writes the effective `lambda`; (family × analysis) table is a writer table
- 7.10 → reader alias tables
- 7.11 → reader drops with a note; writer refuses
- 7.12 → `Model.params` family names; `Element.mult` vs a grading coefficient
- 7.13 → reader writes defaults; the inventory's `given` bit → see 3.09
- 7.14 → `Model.bin_of`, `Model.ako` (kept, not expanded: expansion is a pass)
- 7.15 → `Load` (kernel) + `ModelFamily::Module`; define set → gap G28
- 7.16 → `Model.module` (a `Ref` to a `Module` definition); define set gap G28
- 7.17 → writer tables (family × analysis)
- 7.18 → separate families (`Vdmos`/`VdmosRpi`, `Mos0`, `Gaas`, `CoreJilesAtherton`)
- 7.19 → `DiodeLtspice`, `DiodeIdeal`
- 7.20 → model params (`selft`, `sw_et`) as written by the reader
- 7.21 → `Model.polarity`
- 7.22 → run side (comparison policy); no IR content

## INV-semantics 3.09, 9.02, 9.06

- 3.09 → no `given` bit: absence is `NONE` where every simulator means the same, otherwise the reader writes its simulator's value (IR-KERNEL §2 "explicit over implicit"); see 5.05
- 9.02 → run owner (probes); the element side supplies role order (`ElemKind`) and the sign rule
- 9.06 → run owner (operating-point quantities); depends on `ModelFamily`

## AMEND-A12 (VACASK)

- A12 C1 → `Pulse.law: PulseLaw::{Spice, Vacask}`; `CircuitExtra::{PulseSpice, PulseVacask}`; `check`: a literal `rise <= 0` under `Vacask` is `FieldDomain`
- A12 C2 → `Exp.td2_from: TdFrom::{Zero, Td1}`; `CircuitExtra::{ExpTd2FromZero, ExpTd2FromTd1}` (E5, 5.08: the reader no longer adds)
- A12 C3 → `Sffm.pre: PreDelay`; `CircuitExtra::{SffmPreOffset, SffmPreHold, SffmPreZero}`
- A12 C4 → `Source.dc_rule: DcRule::{Given, WaveAtZero}` (`dc` `NONE` exactly under `WaveAtZero`, which needs a wave); `CircuitExtra::DcGivenWithWave` and `DcAtZero{Pulse, Sin, Exp, Pwl, Sffm, Am, TrNoise, TrRandom, Pattern}` (5.13, E1, E38)
- A15 (R31) → C1 `Element.mult_law: MultLaw::{Parallel, NgspiceAppend}` (`CircuitExtra::{MultInstanceParallel, MultInstanceNgspiceAppend}`, the kinds ngspice leaves untouched in IR-CIRCUIT §2); C2 `TcBase::{Tnom, Fixed27}` on `Passive` and `Behav`, `TcLaw::{Direct, Reciprocal}` on `Passive` (`CircuitExtra::{TcBaseTnom, TcBaseFixed27, TcLawDirect, TcLawReciprocal}`; gf180's 324 behavioural-resistor decks)
- A24 ruling → `SemiInitial` split into `SemiInitialOff` and `SemiInitialOn` (ngspice semis take `off` only; Spectre's diode `region=on`); `SwitchInitial` stays one
- Unmeasured, no feature now (A24 ruling, R18): `TrRandomKind` per member, `FreqUnit` per member, `AmModTime::Delay`: check at Xyce, HSPICE, Spectre, LTspice
- R44's shape (vc-devices) → `ElementWithoutModel` only for a kind with a master (`master_kind` `Some`); the masterless kinds need no feature in the generator
- R61/A36 → `Violation::InstancePortCount` (X0022): a subcircuit instance's terminal count equals its bound master's port count (`circuit_check::master_ports`; an unbound master is not reported); the generator draws an instance's terminals from its master's ports
- A32 → `RowFacts::map_element_names` for `Element`: `Coupling.inductors`, `Controlled.sense`, `Poly.controls` under current control, `Switch.sense` (one use per name, in field order); no circuit row holds a path
- A31 → `CircuitExtra::AmModOffset` (an AM whose `mod_offset` is not the real literal 1, an Int 1 included); without it the generator draws the real literal 1
- Round 7 → `ModelFamily::word`, `ModelFamily::library_free` public (A32); a versioned standard's model states its version (`check`), the generator's vocabulary `VERSIONS` (ngspice-45, ng-models); `CircuitExtra::BehavUntypedParam` (A33, R56; without it the generator builds each parameter read of a behavioural expression as `ToReal`/`ToInt` of the read, `Gen::typed_params`); PWL `Ramp`/`Period` and `RampWidth`/`Extrapolate` under `From` refused (A30 finding 4)
- Unmeasured: specified from the document only (A30): the LTspice ramp join (`PwlJoin::Ramp`), the LTspice sine `cycles`, the Spectre `PwlJoin::RampWidth`, the bit pattern; the PWL `Period` wrap takes `fmod`, which the runs cannot tell from x − P·⌊x/P⌋ at 1e-12
- A30 → `wave_eval.rs`: `source_value`, `wave_value`, `At`, `WaveDomain`; every law of §3 specified on `wave_value`; 48 ngspice and 205 VACASK run values reproduced to 1e-12; properties (periodic repeat, PWL ends, op = value at 0, finite or domain)
- A25 → a `DeviceLibrary` uses `Feature::Family(family)`; the generator draws a library only of a family the set holds
- A24 (R44, R45) → `library_free` exempts `Module`, `CodeModel`, `Other`; `DeviceLibrary.path` is spelling (no check or feature reads it); `CircuitExtra::PwlPeriodJump`
- A22 (R42) → `Pwl` holds its first value before the first point and its last after the last; no reader writes a point; `PwlJoin::Extrapolate` is VACASK's periodic law (docs; `check` and the generator never assumed a point at 0)
- A20 (R39) → `ModelFamily::TlineIdeal`, an optional `Model` master on `LosslessLine`, `BuiltinMaster` for a line naming one; the family is not `library_free`
- A19 (R38) → the invariants of IR-CIRCUIT §3a (`check` `FieldDomain`; a circuit-valued model value is `ProbeContext`); the finer element-feature grain deferred as A19 records
- A17 (R35, R36) → `wave::GivenZero::{AsGiven, SpiceDefault}` on `Pulse`, `Sin`, `Exp`, `Sffm` (`CircuitExtra::{GivenZeroAsGiven, GivenZeroSpiceDefault}`; table in IR-CIRCUIT §3); ngspice `WaveAtZero` for every wave; wave-law docs from `vsrcload.c` (5.04–5.06, 5.13, 5.18)
- A15 C3, C4 (R32, R33) → `ModelFamily::Mesa`; a bin group's definition made by its first bin (`Builder::define_model`; IR-CIRCUIT §4); `Feature::Family` with the kernel owner
- A13 (R23) → whole names; `letter::spice_letter(design, element)`; `CircuitExtra::NameWithoutLetter`; `Violation::DuplicateElement` through `element::design_check` (with the kernel owner)
- A12 C5 → `ItemKind::DeviceLibrary`, `library::DeviceLibrary { family, path }`; `CircuitExtra::ElementWithoutModel` (item), `CircuitExtra::ModelWithoutLibrary` (design-level); parameterless builtin models: `ModelFamily::{SourceV, SourceI, Vcvs, Vccs, Ccvs, Cccs, Mutual}`, an optional `Model` master on the source and linear controlled kinds, `CircuitExtra::BuiltinMaster` (the direction ngspice lacks) (IR-CIRCUIT §4a)

## Gaps and proposals

| Gap | Rows | Proposal |
|---|---|---|
| G1 | D8, 6.03, F22 | Ratings with no simulation meaning (`Irms`, `mfg`, `Vpk`): keep in `Element.params`/`Model.params`; lt writer only. No field |
| G2 | D31 (sp loss terms), D36, D107, D122 `YTRANSLINE`, D134, 6.23 | A multi-conductor RLGC `Line` record (D134) as its own kind with a matrix side table, after the line slice; `Opaque` `Private` until then |
| G3 | D59 | ngspice `external`: `Opaque` `Private` |
| G4 | D76 | lt `step(…)` source values: a run-owner sweep target; ask the run owner |
| G5 | D79, D105 | Hysteretic cores and windings: `Opaque` `Private` |
| G6 | D94, 5.29 | Ideal transformer: proposed `ElemKind::Transformer` (ratio, direction) if a second dialect needs it; hs reader lowers to E+F otherwise |
| G7 | D96, D122 `YLIN`, 6.29 | N-port data element: `ElemKind::NPort` with a file and per-target analysis list; `Opaque` until then |
| G8 | D97 `hblin`, 5.20 | HB per-tone drive: a `Source` side row `HbDrive{tone, harmonic, mag, phase}` once the run owner fixes HB |
| G9 | D99 | hs C/L `POLY`: a `Passive` polynomial value (`coeffs: ExprList`); `Opaque` until needed |
| G10 | D101, D106 | B-source `ic`, `tripdv`/`tripdt`, E `IC`: proposed `Behav` side row; `Element.params` today |
| G11 | D112, 6.13 | xy `AGE`/`D` ageing: `Element.params` (xy only) |
| G12 | D120 | xy MOSFET levels 77, 102, 107, 110, 301, 2000, 2001 and BJT 23: family not established in the inventory; `ModelFamily::Other` until researched |
| G13 | D129 | sp `xfmag`: an analysis-side field (run owner) |
| G14 | D131 | sp `transformer`, `switch`, `nport`, z-domain, `msline`, `cktrom`, … : `Opaque` `Private` |
| G15 | E27 | lt `wavefile=`: `Opaque` `Private` (or decode to a PWL at import) |
| G16 | E28 | lt PWL `Trigger=`: `Opaque` `Private` |
| G17 | E32 | xy PAT repeat semantics unsettled: the xy reader refuses non-default `RB` |
| G18 | E33 | hs `.vec`: `Opaque` `Private` (run-owner stimuli) |
| G19 | E34 | hs data-driven PWL: `Opaque` until `.DATA` has a run-owner home |
| G20 | F23 | `.MALIAS`/`.ALIAS`: a reader lowers to a second `Model` with `ako`; else `Opaque` |
| G21 | 5.12 | The inventory wants inline points; readers do no I/O, so `Pwl.file` keeps the path and a pass inlines it |
| G22 | 5.16 | sp V source current per copy under `m`: a probe-side rule (run owner), not an element field |
| G23 | 6.01 | ngspice overriding an inner `m` (instead of multiplying): ng reader writes the effective per-element `mult` |
| G24 | 6.06 | Deck `scale`: run owner's option; the reader keeps geometry as written |
| G25 | 6.30 | xy thermal resistor (R level 2): `ModelFamily::Other` until a second dialect has it |
| G26 | 6.32 | xy 0 Ω supernode: the xy reader merges the nets (its rule); no field |
| G27 | 7.05 | Per-parameter units of BSIM1/2 cards (hs µm vs m): the family's unit table, a writer table |
| G28 | 7.15, 7.16 | Module artifact identity (source, compiled artifact, define set): a kernel `Load` field |
