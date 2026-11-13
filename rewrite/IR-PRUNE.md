# IR pruning for goal 1 (R100): the decision per member

The IR usage census (`rewrite/IR-USAGE.md` on `differences`, diffs-2) lists the features no reader
produces. Goal 1 deletes each unless a goal-2 dialect (Xyce, HSPICE, LTspice, Spectre, ESPice)
needs it, citing that dialect's spec. This file is the IR owner's record, one row per member, one
group per commit; the census file takes the rows when it next regenerates.

## Group 1: the eight members their own docs marked "removed if no reader produces it"

| member | decision | reason |
|---|---|---|
| `Binary(IntDiv)`, `BinaryDyn(IntDiv)` | deleted | "no speaker yet"; ngspice's `\` is `Trunc(Abs(Div))` (A11); no reader produces it |
| `Func(LnAbs)`, `FuncDyn(LnAbs)` | deleted | "speaker to be confirmed by the HSPICE and LTspice slices"; no reader produces it; ngspice-45's B-source `ln` is not `ln|x|` (A11) |
| `Func(SqrtAbs)`, `FuncDyn(SqrtAbs)` | deleted | as `LnAbs`; ngspice-45's B-source `sqrt` errors below zero (A11) |
| `Func(Step)`, `FuncDyn(Step)` | deleted | "speaker to be confirmed by the LTspice and Xyce slices"; ngspice's `u` is `StepHalf` (A11) |

A goal-2 slice that finds a speaker brings the member back with its run (the members cost one
row each to restore).

## Group 2: the 91 members no dialect's doc names (census list "No dialect named in its doc")

Read against `rewrite/INV-constructs.md` (IC) and `rewrite/INV-semantics.md` (IS), line numbers as
of 2026-11-13. Goal 2 lists every simulator, ngspice and VACASK included, so a construct either of
them has is kept too (a reader gap, not dead breadth).

**Deleted in this commit (no simulator has it):**

| member | reason |
|---|---|
| `Run(VaryMean)` (`Vary::mean`) | IC:547 J2 gives Spectre's `vary` no centre; J18 (IC:563) is a design row with no dialect |

**Kept: a goal-2 simulator has it** (68, citation per member): `Family(DiodeCmc)` Xyce's own
regression corpus runs it (`DIODE_CMC/DiodeClipper.cir`, trunk `tests/fixtures/cross_sim/xyce-ngspice.txt:269`),
though IC:304 D120 lists Xyce's D levels as 1, 2, 200 only (an inventory gap); `Analysis(Hb)` IC:416 G16 xy hs;
`Analysis(Net)` IC:494 H15 lt, IC:437 G37 hs; `Analysis(Pac)` IC:469 G69 sp, IS:239 8.17 es;
`Analysis(Pnoise)` IC:418 G18 sp; `Analysis(Pstb)` IC:418 G18 sp; `Analysis(Pxf)` IC:418 G18 sp;
`Analysis(TimedOp)` IC:442 G42 hs sp; `Circuit(BehavCapacitance)` IS:173 6.16 sp, IC:277 D93 hs;
`Circuit(BehavCapacitanceCharge)` IS:173 6.16 sp; `Circuit(BehavConductance)` IC:314 D130 sp;
`Circuit(BehavInductance)` IC:283 D99 hs, IS:173 6.16 sp; `Circuit(BehavPower)` IC:267 D83 lt;
`Circuit(BehavResistance)` IC:267 D83 lt, IC:314 D130 sp, IC:277 D93 hs;
`Circuit(DcAtZeroPattern)` IS:132 5.13 hs with IC:358 E32; `Circuit(DelaySampled)` IS:149 5.30 hs;
`Circuit(FreqDbLogFreq)` IS:147 5.28 hs; `Circuit(IcInOp)` IC:190 D6 xy lt;
`Circuit(LaplaceExprInput)` IC:267 D83 lt; `Circuit(LaplacePoleZero)` IC:275 D91 hs sp;
`Circuit(ModelAko)` IC:378 F8 lt; `Circuit(TableExact)` IC:205 D21 xy, IS:144 5.25 lt;
`Circuit(TableSmoothed)` IC:276 D92 hs; `Circuit(TcBaseTnom)` IS:172 6.15 hs;
`Elem(FluxInductor)` IC:264 D80 lt; `Elem(FreqTable)` IC:245 D61 lt; `Elem(Igbt)` IC:287 D103 lt;
`Elem(Laplace)` IC:207 D23 lt hs es; `Extra(SectionLibrary)` IC:40 A15 sp; `Family(Other)` IC:316
D132 sp; `Func(Clamp)`/`FuncDyn(Clamp)` IC:124 C22 xy lt; `Func(Ddt)` IC:126 C24 xy sp, IC:159 C57
lt; `Func(Pwl)`/`FuncDyn(Pwl)` IC:165 C63 xy, IC:159 C57 lt; `FuncDyn(Fmod)` IS:72 2.12 xy sp;
`FuncDyn(RoundHalfAway)` IS:70 2.10 xy lt hs; `FuncDyn(Trunc)` IS:71 2.11 lt hs sp;
`Item(Statistics)` IC:546 J1 sp; `Run(HbDiamond)` IC:467 G67, IS:237 8.15 hs; `Run(HbIntermod)`
IC:451 G51 xy, IS:237 8.15 sp; `Run(HbMultiTone)` IC:451 G51 xy; `Run(IcDevice)` IS:243 8.21 xy;
`Run(LoopTable)` IC:583 K14 xy hs; `Run(McMismatch)` IC:417 G17 sp; `Run(NoiseCurrentOutput)`
IC:466 G66 sp; `Run(PartIm)` IS:88 2.28 xy; `Run(PartPhaseDeg)` IS:80 2.20 xy;
`Run(PeriodicRelative)` IS:239 8.17 sp; `Run(PrintHbTime)` IC:489 H10 hs xy; `Run(ProbeAllI)`
IS:258 9.05 hs lt; `Run(ProbeAllPinI)` IS:258 9.05 lt; `Run(ProbeAllVTop)` IS:256 9.03 lt;
`Run(ProbeExpr)` IC:482 H3 xy; `Run(ProbeITerminal)` IC:506 H27 xy hs lt sp; `Run(ProbeNoise)`
IS:344 15.04 xy sp; `Run(ProbePower)` IC:502 H23 xy, IC:497 H18 hs; `Run(PssArgumentOmitted)`
IC:433 G33 sp, IS:245 8.23 es; `Run(PssDriven)` IS:238 8.16 hs; `Run(PssOscDiff)` IC:468 G68 sp;
`Run(PzSourceInput)` IC:413 G13 hs sp; `Run(SavePerAnalysis)` IC:480 H1 hs, IC:481 H2 xy;
`Run(StatsCorrelation)` IC:548 J3 sp; `Run(StatsMismatch)` IC:546 J1 sp; `Run(StatsTruncate)`
IC:549 J4 sp; `Run(VaryLogNormal)` IC:547 J2 sp; `Run(VaryPercent)` IC:547 J2 sp;
`Wave(Pattern)` IC:358 E32 hs xy.

**Kept: ngspice or VACASK has it, their readers do not produce it yet (a reader gap):**
`BinaryDyn(Shl)`, `BinaryDyn(Shr)` (IC:115 C13, IC:173 C71 va); `Expr(TStep)`, `Expr(TStop)`
(IS:124 5.05, IS:121 5.02 ng defaults); `Extra(FormalOuterRead)` (R62, ngspice runs);
`Extra(ListValue)` (IS:101 3.08 va vectors); `Run(AlterSubcktParam)` (IC:472 G72 ng
`alterparam sname`); `Run(FourGrid)` (IC:508 H29 ng va); `Run(MeasDeriv)` (IC:493 H14 ng);
`Run(ProbeAllSourceI)` (IS:256 9.03 va default); `Extra(DeckWithoutTitle)` (goal 3: a Python
design need not hold a title).

**No simulator has it, but it is a combination of kept members, so deleting it means a new
invariant (the lead's decision):** `Circuit(AmPreOffset)` (IS:122 5.03: ng 0, va the t=delay value),
`Circuit(SinPreZero)` (`PreDelay::Zero` is ng's AM/SFFM only; IS:125 5.06 lt sp va hold),
`Extra(IfConditionDynamic)` (IS:289 11.08: `.if` decided once from parameters),
`Extra(FormalWithoutDefault)` (no row found), `Run(LoopSubcktParam)`, `Run(DcAxisSubcktParam)`
(a subckt default swept: no row; the alter of one is ng's G72).

**Unsure, waiting for a row in the inventory:** `Circuit(FreqComplexLinear)` (IS:147 5.28 ng
XSPICE only; lt's `Freq=` law unrecorded), `Circuit(SffmPreOffset)` (IS:121 5.02 xy, ambiguous
without a delay), `Circuit(SinPreOffset)` (wave.rs:61 says xy hs, no inventory code),
`Run(McNominal)` (plan.rs:143 says sp `donominal=yes`, no inventory row), `Run(TranCapNone)`
(IS:223 8.01 records no hs or lt default ceiling).

The two inventories disagree on HSPICE's `FREQ` (IS:147 5.28 has it, IC:245 D61 says hs lacks
it) and HSPICE's E `DELAY` (IS:149 5.30 sampled, IC:274 D90 pure): both kept on the IS rows,
which cite the HSPICE manual.

## Group 3: the combinations no simulator has, made refusals (the lead's rule, 2026-11-13)

Each was a feature of a combination of members that stay; it is now a `FieldDomain` row (the IR
text reader refuses it like every row `check` names), and the generator draws none. Xyce's spec
(`rewrite/DIALECT-XYCE.md` on `differences`) was read first: it keeps `Run(DcAxisSubcktParam)`
(`.dc X1:…`, its `.dc` row), so that one stays a feature.

| member deleted | now refused | why |
|---|---|---|
| `Circuit(SinPreZero)` | a `Sin` with `PreDelay::Zero` (`circuit_check::wave_ok`) | `PreDelay::Zero` is ngspice's AM and SFFM only; IS:125 5.06 lt sp va hold the value, wave.rs xy hs the offset |
| `Circuit(AmPreOffset)` | an `Am` with `PreDelay::Offset`; AM's IR-text default is now `zero` | IS:122 5.03: ng 0, va the t = delay value; xy lt have no AM |
| `Extra(IfConditionDynamic)` | an `If` condition of class `Temp` or above (`check_scopes::dynamic_conditions`) | IS:289 11.08: `.if` decided once from parameters |
| `Extra(FormalWithoutDefault)` | a `Subckt` formal with no default (the row's check) | no row names a formal without one; Xyce's `params: w=1` (DIALECT-XYCE.md, subcircuit parameters) |
| `Run(LoopSubcktParam)` | a sweep or table loop over `TargetKind::SubcktParam` (the loop rows' `fault`) | no row; the DC axis keeps it (Xyce) and the alter keeps it (ng G72) |

A goal-2 spec that shows one of these brings it back as a member.

## Group 4: the five unsure members, settled by diffs-2's runs (differences f78efcca)

Runs on ngspice-45, VACASK, Xyce 7.10 and ESPice in HSPICE mode (decks:
`rewrite/upstream/decks/members/` and `rewrite/DIALECT-HSPICE.md` §10 on `differences`).

| member | decision | the runs |
|---|---|---|
| `Circuit(FreqComplexLinear)` | kept | ngspice speaks it (XSPICE `xfer`: 1∠0° at 1k, 1∠−90° at 3k give −3.01 dB, −45° at 2k); its `E … FREQ` applies a tenth of the phase (F69, an ngspice bug) |
| `Circuit(SinPreOffset)` | deleted, with `PreDelay::Offset` | all four hold the t = delay value (`sin(1 2 1k 0.3m 0 90)` reads 3 at 0.1 ms); a sine now has only `HoldDelayValue` (`Zero` and `Offset` refused) |
| `Circuit(SffmPreOffset)` | deleted | ngspice and ESPice 0, VACASK's fm the held value, Xyce has no delay |
| `Run(McNominal)` | kept | ESPice-HSPICE `sweep monte=3` gives a nominal plot and three drawn |
| `Run(TranCapNone)` | deleted, with `StepCap::None` | ngspice and ESPice min(print step, span/50), Xyce span/10, VACASK about span/50 |

`PreDelay::Offset` had no wave left (AM's and SFFM's offset refused in groups 3 and 4), so the
member is gone; every wave's IR-text default `pre` is now `hold_delay_value`.

## Group 5: the census rerun (2026-11-13), what the newer specs and rounds left

Census: diffs-2's run of `tests/ir_usage.rs` on `differences` 683297fd (trunk 3c7fdb98 merged, 618
ngspice and 280 VACASK files), each dead member marked per goal-2 branch (dialect-xyce 8e0e8d38,
dialect-ltspice 2d2491d1, HSPICE on the trunk): R = its reader names it, w = only its writer. 247
dead members: 58 placements (combinations, not members: listed at the end); about 130 named by a
goal-2 reader (live by the lead's rule); 119 with no R. Of those 119, 12 are already deleted on
`ir-rewrite` (groups 1 and 4), and the other 107 were decided in groups 2–4 or are checked here
against the goal-2 specs `rewrite/DIALECT-{XYCE,HSPICE,LTSPICE,SPECTRE,ESPICE,NGSPICE,VACASK}.md` on
`differences` (ba636e25), then, where no spec row names it, against the member's own doc and the
inventories, as groups 2 and 4 did. The law points new on `ir-rewrite` are live by their law tests.
**None is deleted**: every member still in the IR has a goal-2 citation. `Family(BsimBulk)` and
`Family(Psp103)`, newly dead in this census: **kept: user decision 3** (the OSDI modules compiled).

**A spec row states it (50):** `BinaryDyn(BitAnd)`, `BinaryDyn(BitOr)`, `BinaryDyn(BitXor)`,
`UnaryDyn(BitNot)` VACASK:126, :293; `Circuit(ElementParasitics)` LTSPICE:58;
`Circuit(MultInstanceParallel)` XYCE:71; `Circuit(PwlJoinRampWidth)`, `Circuit(PwlPeriodJump)`
SPECTRE:106; `Circuit(SinCycles)` LTSPICE:60; `Circuit(SinSecondTone)` SPECTRE:103;
`Circuit(SourceNoisePsd)` SPECTRE:117; `Circuit(SourceParasitics)` LTSPICE:58; `Circuit(SourceTc)`
SPECTRE:105; `DrawPer(Instance)`, `DrawPer(Run)` SPECTRE:76; `Elem(Delay)` SPECTRE:111; `Expr(List)`
SPECTRE:45; `Family(BsimBulk)`, `Family(Psp103)` VACASK:101 (kept: user decision 3); `Family(BsimSoi)`, `Family(Gaas)`,
`Family(Hisim1)`, `Family(Mos0)` SPECTRE:92; `Family(CapacitorMultiplier)`,
`Family(InductorMultiplier)`, `Family(ResistorMultiplier)` XYCE:74; `Family(DiodeLtspice)`
LTSPICE:62; `Family(DiodePspice)` XYCE:86; `Family(DiodeSpectre)` SPECTRE:93; `Family(Ekv)`,
`Family(HbtUcsd)`, `Family(Mextram)` HSPICE:84; `Family(JfetShockley)` XYCE:87; `Family(Relay)`
SPECTRE:114; `Family(SwitchHystSmooth)` LTSPICE:61; `Func(Atan2)`, `Func(Ramp)` XYCE:58;
`Func(PowRe)`, `FuncDyn(PowRe)` XYCE:49; `Func(PowSigned)` XYCE:50; `FuncDyn(ExpLin14)` NGSPICE:63;
`Run(AlterString)` VACASK:153; `Run(SweepCenterSpan)` SPECTRE:128; `Run(SweepDcLog)`,
`Run(SweepFreqLog)`, `Run(SweepLoopLog)` SPECTRE:126; `Run(SweepEndNgDecade)` HSPICE:91;
`Run(SweepEndOnStop)` LTSPICE:71; `Run(TranCapSpan10)` XYCE:84; `Run(TranStartup)` LTSPICE:69.

**A spec names it as a lead not yet run (25), kept until its run.** Each is tagged with the dialect
whose owner confirms, when that dialect's row lands, that its reader produces the member; if none
does, the member is deleted then (the lead's ruling, 2026-11-13).
- **Xyce:** `Circuit(CouplingCore)`, `Circuit(SwitchControlExpr)`, `Family(CoreJilesAtherton)`,
  `Family(CurrentSwitchLogCubic)`, `Family(SwitchLogCubic)` XYCE:131 (and R103's runs for the
  switch); `Family(Hicum0)` XYCE:132; `Family(VdmosRpi)` XYCE:133; `Run(IcEveryOp)` XYCE:76 (also
  VACASK:296); `Run(NoiseRefSourceAc)` XYCE:97.
- **LTspice:** `Circuit(BehavRpar)` LTSPICE:99; `Circuit(PwlRepeatCount)`,
  `Family(CurrentSwitchHystSmooth)` LTSPICE:100.
- **HSPICE:** `Family(HspiceW)` HSPICE:123; `Option(GminDc)`, `Option(MaxStep)`, `Option(Search)`
  HSPICE:126; `Elem(Module)` HSPICE:128 (also SPECTRE:143).
- **Spectre:** `Circuit(AmModCarrierPhase)` SPECTRE:103; `Circuit(LineLengthVel)` SPECTRE:112;
  `Family(DiodePspiceTemp)` SPECTRE:93; `Func(Idt)`, `Func(ToReal)` SPECTRE:62 (also VACASK:115,
  :514).
- **ngspice:** `Func(ExpLin14)`, `Func(StepHalf)`, `FuncDyn(Sqr)` NGSPICE:63, :71.

**No spec row yet, kept on the rulings or the member's own inventory citation (19):**
`Extra(StringCompare)` (the strings runs, `crates/ir/tests/fixtures/strings/`: VACASK computes it);
`Run(SweepEndStretch)`, `Run(SweepCountNgLin)` (R99/R99a: VACASK's and ngspice's sweep laws, runrules'
runs); `Circuit(PortNoiseTemp)` (sp, INV-semantics 5.17); `Circuit(PortPower)` (ng, INV-constructs
E24); `Circuit(PwlJoinRamp)` (lt); `Circuit(SemiInitialOn)` (Spectre `region=on`, A24);
`Circuit(SourcePac)` (sp, D129); `Family(DiodeIdeal)` (lt, 7.19); `Family(Igbt)` (lt, D103, as
`Elem(Igbt)` in group 2); `Family(JfetHspice2)` (hs, 7.03); `Family(Juncap200)` (xy, D120);
`Option(GminTran)` (hs `gmin`); `Run(FourPeriods)` (lt `Np`); `Run(HbHybrid)` (xy);
`Run(McSampleList)`, `Run(NoiseAcSweep)`, `Run(TranSegments)` (hs); `DistDyn(Limit)` (the dynamic form
of ngspice's `limit` draw, a member of the form grid, not one of its own).

**Placements (58), not members:** decided in group 6.

## Group 6: the placements (the lead's ruling, 2026-11-13)

A placement `Placed(S, K)` the census found unproduced follows the members' rule: one no dialect
expresses (trunk or goal-2 spec) is `Violation::Misplaced` (X0030, `check_scopes::misplaced`), at
any depth; one some dialect expresses stays legal and each target that cannot write it refuses it.
Runs: `crates/ir/tests/fixtures/placed/` (`run.sh`, `runs.out`, ngspice-45): a card in
`.if (sel==1)` that the branch governs reads differently at sel=0 and 1; one ngspice keeps as if
unconditional reads the same. ngspice-45 governs `.temp`, `.ic`, `.tran` and `.meas` by the branch,
and keeps `.options`, `.func` (the last definition wins, like F77's `.param`), `.global`, `.print`,
`.save` and `.control` whatever the condition.

**Already no design (4):** `Placed(Branch|Loop|Section|Subckt, Title)`: `Builder::title` asserts a
deck's top, IR text refuses it (P9001).

**Already `RunOrderInSubckt` (5, R98):** `Placed(Subckt, Alter|Loop|Reset|Print|Measure)`.

**Made `Misplaced` (31):**
- Branch (12): `Section` (no dialect's branch holds a definition of a section: VACASK:100, :508,
  SPECTRE:75, INV-constructs B13); `Func` (ngspice keeps it unconditional, `br_func`; Spectre's
  functions are top level only, SPECTRE:65); `Global` (VACASK P2008, VACASK:348; ngspice keeps it
  unconditional, `br_global`); `DeviceLibrary` (VACASK P2008: `load` in a branch); `Statistics`
  (INV-constructs J1: refused in a branch); `Option` (ngspice unconditional, `br_options`; Spectre's
  and VACASK's branches hold no options, SPECTRE:75, VACASK:100); `Save`, `Print` (ngspice
  unconditional, `br_save`, `br_print`; the same rows); `Alter`, `Loop`, `Reset`, `Elaborate` (script
  commands: ngspice runs a `.control` in a branch whatever the condition, `br_control`; no other
  dialect has a run command in a branch).
- Loop (12): `Section`, `Subckt`, `If`, `Include`, `Load`, `DeviceLibrary`, `Global`, `Param`,
  `Func`, `Model`, `Statistics`, `Element`: a loop body holds run items only (`plan::Loop`): an
  ngspice `foreach` body is script commands, Spectre's and VACASK's `sweep` wraps analyses
  (SPECTRE:138, VACASK:276).
- Section (2): `Section` (sections do not nest: VACASK:508 lists a section's contents without one;
  a `.lib` inside a `.lib` section is a call, an `Include`); `Elaborate` (VACASK's command, never in a
  section, VACASK:508).
- Subckt (5): `Section` (as above); `Load`, `DeviceLibrary` (VACASK P2008, VACASK:348; no other
  dialect loads inside a subcircuit); `Statistics` (INV-constructs J1: refused in a subcircuit);
  `Elaborate` (as above).

**Kept (18), with the dialect that expresses each:**
- Branch: `Analysis`, `Env`, `NodeSet`, `Measure` (ngspice-45 governs them by the branch: `br_tran`,
  `br_temp`, `br_ic`, `br_meas`); `Param` (Spectre SPECTRE:75, HSPICE INV-semantics 3.07); `Load`
  (Spectre, INV-constructs B14, A17); `Include`, `Subckt` (VACASK:508, pl/br and pl/brs).
- Loop: `Measure`, `Print` (ngspice: a `meas` or `print` command in a `foreach` body, the loop's
  name being its variable, `plan::Loop`).
- Section: `Alter`, `Loop`, `Reset` (ngspice: a `.control` in a `.lib` section runs where the
  section is included, `lib_sections`); `Measure`, `Print` (the same run: `.meas`, `.print` in a
  section act); `Save` (ngspice's `.lib` is textual, as the same run shows); `Statistics` (Spectre,
  INV-constructs J1: at the top or in a section).
- Subckt: `Save` (a whole-run `.save` in a `.subckt` saves `x1.m` on ngspice-45, `sub_save`; one
  holding from here is `RunOrderInSubckt`).
