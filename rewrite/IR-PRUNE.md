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
