# Amendment A12: what the VACASK plan asks of the IR and the spine

Source: `rewrite/DIALECT-VACASK.md` on branch `dialect-vacask` (5b29a697), §5 and §10; every fact
below is a run on VACASK 36be560 recorded in its Appendix A. Rulings R16–R22 (`RULINGS.md`).

Each owner applies its part on its own branch, tests red first, updates its spec file
(`IR-KERNEL.md`, `IR-CIRCUIT.md`, `IR-RUN.md`, `SPINE.md`) in the same commit and reports the
hash. Everything here is additive: no existing row, field or member changes its meaning, except
the two sentences of `IR-CIRCUIT.md` that C2 and C4 withdraw.

## Kernel

### K1. Typed arithmetic (R16)

VACASK computes with integers and reals: `1/3` is 0, `-7/2` is −3, `x/2` is 1 or 1.5 by the type
of the value an instance passes for `x` (runs i1, t3, f2). The type follows the value, so no
reader can decide it statically for a library.

- `NumForm::Int`: a whole-number literal of integer type (`frac == 0`, `exp10 == 0`). Every
  other form is real.
- A value carries a tag, Int or Real; the reference evaluator computes it:

  | Node | Tag of the result |
  |---|---|
  | `Num` | Int iff its form is `Int` |
  | `Param`, `UserCall` | the tag of the value it evaluates to |
  | `Neg` | its operand's |
  | `Add`, `Sub`, `Mul`, `Min`, `Max` | Int iff every operand is Int |
  | comparisons, `And`, `Or`, `Not`, the bit operators and shifts | Int |
  | `Ternary` | the chosen branch's |
  | `ToInt` | Int |
  | everything else (`Div`, `DivTyped` of a real, every other `Func`, every leaf, `Draw`) | Real |

- `BinOp::DivTyped(a, b)`: both Int → the quotient truncated toward zero, Int (`b == 0` a domain
  error); otherwise `a / b`, Real. `BinOp::Div` is unchanged: real division whatever the tags.
- `Func::ToInt(x)`: `x` truncated toward zero, Int. `Trunc` stays Real.
- An Int value beyond ±2⁵³ is a domain error (R12: no integer overflow is carried).
- Features: `Binary(DivTyped)`, `BinaryDyn(DivTyped)`, `Func(ToInt)`, `FuncDyn(ToInt)` follow from
  the enums; new `Extra::IntLiteral` (a literal of form `Int` anywhere in the design).
- IR text: a spelling for an `Int` literal distinct from every real form (the kernel owner's
  choice, stated in `text.rs`'s grammar).
- The evaluator's API is the kernel owner's choice; a caller that wants a number keeps one call.
- Tests: each row of the table at hand-worked points; `DivTyped` at (7, 2), (−7, 2), (7, 2.),
  (1, 3), (1., 3); the values of Appendix A rows i1, t3 and f2 as spec examples (the oracle is
  what VACASK printed); the generator makes `Int` literals, `DivTyped` and `ToInt` only when the
  set has them; the text round trip.

### K2. Two features of scope (R18)

- `Extra::EnclosingRef`: an item holds a `Ref` bound to a definition that lives in the body of a
  subcircuit enclosing the item's own subcircuit (neither the item's own subcircuit nor the top:
  crossing one or more `Subckt` scope boundaries outward and stopping short of a file's top
  scope). VACASK resolves in the own scope, then the top, never an enclosing subcircuit (runs
  n1, m1).
- `Extra::FormalReadsParam`: a `Subckt` head default holds a `Param` reference. VACASK lets an
  instance override only a formal whose default folds to a constant (runs c1, t2).

### K3. `OpaqueClass::Hint` (R19)

Tunes its own simulator's solver (convergence aids, step-control knobs) and names no accuracy
target. Its own family copies it; another family's `legalize` drops it with a note of its own
(not `Inert`'s: the numbers may move within the solver's tolerance).

### K4. Two item kinds, added by their owners

`ItemKind::DeviceLibrary` (circuit, C5) and `ItemKind::Elaborate` (run, R1). Each owner adds its
own member to the kernel's enum, `ALL` and the dispatch on its branch (delegated for these two);
the kernel owner resolves the one conflict when merging both.

## Circuit

### C1. `Pulse.law: PulseLaw::{Spice, Vacask}`

`Spice`: SPICE's pulse. `Vacask`: `rise > 0` is required; `fall <= 0` is no falling edge (the
value holds `v2` to the end of the period and is `v1` again at the start of the next);
`period <= 0` is one pulse (runs p1, p2). One feature per law.

### C2. `Exp.td2_from: TdFrom::{Zero, Td1}`

Withdraws IR-CIRCUIT §3's "VACASK counts `td2` from `td1`: its reader adds": the sum puts the
delay in two fields (R13 b). One feature per member.

### C3. `Sffm.pre: PreDelay`

As `Sin` and `Am` have it (VACASK holds the value at `delay` before it; ngspice is 0).
`SffmLaw::Ngspice` then states only the clamp and the defaults. Features as for `Sin`.

### C4. `Source.dc_rule: DcRule::{Given, WaveAtZero}`

`Given`: `dc` holds the value (the literal 0 where the simulator's rule is 0). `WaveAtZero`: the
DC value is the waveform's value at t = 0, and `dc` is `NONE`. Withdraws "`dc` always present"
(IR-CIRCUIT, DIALECT-NGSPICE §3): copying a field of the wave into `dc` repeats an operand
(R13 b) and hides whether a DC was given. Features so that each (rule, wave kind) a target
lacks can be refused or legalized: ngspice has `WaveAtZero` for SIN, PULSE, EXP and PWL and
only `Given` (0) for AM, SFFM and the noise kinds; VACASK has `WaveAtZero` for every wave.

### C5. `ItemKind::DeviceLibrary` and the elements that need a model (R17)

Row `DeviceLibrary { family: ModelFamily, path: NameId }`: the simulator's own implementation of
the family is made available here; `path` `NONE` is the target's default file, a variant path
(`spice/sn/…`) is kept as written. Features: `Item(DeviceLibrary)`; a design-level feature for
"a model of a family with no `DeviceLibrary` of that family"; an item-level feature for "an
element with no model" (VACASK instantiates every device through a model, sources included: run
n2). The circuit owner and the VACASK agent agree on how a parameterless model of a kind that
has no family today (independent and controlled sources, couplings) is named, and state it in
IR-CIRCUIT.

## Run

### R1. `ItemKind::Elaborate`

Row `Elaborate { circuits }` (references of `DefKind::Subckt`): the named subcircuits' bodies
form the top circuit for every run item after it, until the next `Elaborate`
(COVERAGE-run G-ELABORATE; 49 corpus files).

### R2. `RunExtra::LoopOverOp` (R20)

A `Loop` of kind `Sweep` whose body is exactly one `Op`. VACASK spells a `Dc` as `sweep` + `op`,
so this loop has no spelling of its own there.

## Spine

### S1. `Write::admits` (R21)

```rust
/// Appends a refusal for everything in `design`, legal by `features()`, that this dialect cannot
/// spell for a reason no feature states (a model parameter its simulator's module does not have,
/// a name it cannot quote). Never for something a feature could state. Default: nothing.
fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {}
```

`dialect::legalize` calls it last. Law L4 reads: `write` is total over a design `legalize`
accepted. The law harness skips and counts generated designs `admits` refuses, and fails when a
dialect admits none of a kind its `FEATURES` claims.

# Amendment A13: an element's name is its whole name (R23)

Source: the ngspice scaffold (`rewrite/S3-AREAS.md` on `dialect-ngspice`): with names stored
without their SPICE letter, `i(vdd)` reads as `I("dd")`, and a current through `L1` cannot be
told from one through `V1`: every by-name reference to an element (`Expr::I`, a coupling's
inductors, a controlled source's sensing element, a sweep or alter target, a probe) would need a
kind beside the name.

- **Rule.** `Element.name` is the element's whole name as its dialect spells it (ngspice: the
  letter included, folded: `r1`, `vdd`, `x1`). Element names are unique in their scope across
  kinds; a by-name reference names exactly one element. `IR-KERNEL.md` §6's "without any type
  letter" and `DIALECT-NGSPICE.md` §4's "prefixes an element name with its kind's letter" are
  withdrawn; the worked example of `text.rs` already spells whole names.
- **Circuit.** `spice_letter(design, element) -> Option<u8>`: the SPICE card letter of an
  element (its kind, and its detail where the letter depends on it; `None` for a kind SPICE
  cards do not have). Feature `CircuitExtra::NameWithoutLetter`: an element whose name does not
  begin with its SPICE letter (ASCII case ignored): a SPICE-card dialect cannot spell it, and a
  pass renames it and every reference to it. The generator makes letter-led names when the set
  lacks the feature.
- **Kernel.** `Violation::DuplicateElement` (two elements of one name in one scope; `arg`: the
  second), reported through a design-level check hook of the circuit owner
  (`pub(crate) fn design_check(design, out)` in `element.rs`, called once by `check`, as
  `design_features` is).

# Amendment A14: state that holds for the whole run, and what order means (R24–R26)

Source: the ngspice run area (`ng-run`), from `cktdojob.c`, `cktnewan.c`, `analysis.c`, `inp.c`
and runs: ngspice's batch mode runs the analyses of a deck by type rank (AC, DC, OP, TRAN, PZ,
TF, DISTO, NOISE, SENS, PSS, SP), reverse card order only inside one type (so
`INV-semantics.md` 8.04 is wrong); every state card (`.options`, `.temp`, `.ic`, `.nodeset`)
holds for the whole deck wherever it stands; a run card inside a `.subckt` body is the deck's.

- **R24, run owner.** `Option`, `Env` and `NodeSet` rows gain `applies: Applies::{FromHere,
  WholeRun}`. `FromHere`: the item holds for the run items after it (a script, a control block).
  `WholeRun`: it holds for every run item of its deck wherever it stands; among `WholeRun` items
  that set the same thing (an option key, `temp`, `tnom`, one node's condition) the last in
  listed order holds. "Listed order" reads an `Include` as the items of the file it names, in
  place. Features, so that each target can refuse or legalize what it lacks: a `FromHere` state
  item after a run item (today's `OptionMidRun`, and its `Env` and `NodeSet` twins); a
  `WholeRun` state item after a run item (a target whose state is positional cannot spell it in
  place; a pass moves it before the first run item when both are in one file). `Env`'s `temp`
  and `tnom` are each optional: an `Env` sets what it names.
- **R25, IR-RUN §1.** Run items are listed in the order their source lists them. Analyses with
  no `Alter`, no loop boundary and no `FromHere` state item between them are independent: their
  order changes no number, only the order of the results, which is a run fact of the dialect
  that ran them (S12), not a property of the design. No reader permutes run items; no writer
  reorders them. `DIALECT-NGSPICE.md` §3's "run items in the order ngspice runs them" is
  withdrawn.
- **R26, run owner.** A run item is its deck's wherever it stands: a scope does not scope an
  analysis, an option or an `Env`. Feature `RunExtra::RunItemNested`: such an item listed
  outside its file's top scope and the `If` chains of it. (A `NodeSet` inside a subcircuit keeps
  its own, different meaning, `NodeSetInSubckt`: nodes local to each instance.)

# Amendment A15: what the ngspice scaffold and its first areas found (R27–R31)

Sources: the S3.0 final report (laws L3, L4, L5 and L8 ran on nothing for the `ngspice` row:
its `admits` refused every generated design for forms no feature states); `ng-elements`
(`inpcom.c:4124-4198`, `asrcload.c:41-52`, runs); `ng-outputs` (`com_measure2.c`, runs).

## Kernel

- **K1. Placement is a feature class (R27).** `Feature::Placed(ScopeKind, ItemKind)`: an item of
  that kind listed directly in a scope of that kind. It replaces the placement members of
  `Extra` (`NestedSubckt`, `LocalModel`, `LocalFunc`, `ParamInBranch`, `LoadInBranch`) and, with
  the run owner, `NodeSetInSubckt`, `LoopNested` and A14's `RunItemNested`; the generator's
  placement table reads the set. A placement every target has (an item in a file's top scope)
  need not be a feature. A helper builds sets by scope kind (`placed(scope, &[kinds])`).
- **K2.** `Extra::FormalWithoutDefault`: a `Subckt` head parameter with no default.
  `Extra::IfConditionDynamic`: an `If` condition of class `Temp` or `Circuit`.
- **K3. Two invariants.** A `Title`'s and a `Comment`'s text holds no line break
  (`FieldDomain`). `ProbeContext` follows the class: a `Handle::Expr` field whose expression is
  of class `Circuit` (through a parameter or function too) is a violation; the generator
  respects both.
- **K4. Literal forms and equality (R28).** `eq_meaning` compares two real literals by value
  (`eq_value`), whatever their forms; an `Int` literal equals only an `Int` of the same value.
  `eq_ignoring_spans` stays exact. Stated in the docs of both; changed if it is not so.

## Circuit

- **C1. The multiplier law of a subcircuit instance (R31).** `MultLaw::{Parallel,
  NgspiceAppend}` on an `Instance`: `Parallel` is `mult` whole copies in parallel (it composes
  by multiplication through the hierarchy); `NgspiceAppend` is ngspice's: the instance's `m` is
  given to every element of the subcircuit's body that takes a multiplier, replacing that
  element's own, and passed on the same way to nested instances; an element of a kind ngspice
  does not multiply (independent V sources, E, H, code models, couplings, OSDI instances, the
  line kinds, switches, a voltage-valued behavioural source: the letters of
  `inpcom.c`'s exclusion list) is left as it is. One feature per law (they replace
  `MultInstance`).
- **C2. The base of a behavioural temperature coefficient.** `TcBase::{Tnom, Fixed27}` on
  `Behav` and on a `Passive` whose value reads the circuit (ngspice measures a B source's
  `tc1`/`tc2` from 27 °C whatever `tnom` is: `asrcload.c:41`), and `TcLaw::{Direct,
  Reciprocal}` for a circuit-valued capacitor (ngspice divides by the polynomial:
  `inpcom.c:6747`). Features per member.

## Run

- **R1.** `ProbeKind::AllBranchI`: the branch current of every element that has a branch
  unknown (ngspice's `all`: every node voltage at every depth and these; `AllSourceI` is only
  the voltage sources).
- **R2. Measures (R30).** `Stat::MinAt`, `Stat::MaxAt` (where the extremum is). `Event.until`
  (an upper limit of the search; none when absent). A pick rule beside the count for "the
  simulator's default first event" where it is not the first by count (ngspice's default skips
  a crossing in the first interval, an explicit `CROSS=1` counts it): the run owner names the
  member.
- **R3. Open, not typed now (R29).** What a deck saves when it names nothing: today the IR is
  silent and the target's default applies. Decided with the run side (S12); `output.rs`'s
  sentence that the reader expands the default is withdrawn until then.
