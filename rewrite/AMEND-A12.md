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
