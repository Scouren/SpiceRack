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
