# IR kernel: conventions and file map (slice S1, Definition phase)

Binding for every agent that writes `crates/ir`. Read `REWRITE-PLAN.md` §2–§4 first; this file
turns them into rules. The Definition phase writes every type, field and signature, with bodies
`todo!()`. No tests, no bodies. Gate: `cargo check` and `cargo doc` clean, `check_style.py
crates/ir/src` clean, `cargo fmt --check` clean, no old type name (`Ast`, `Tokens`, `Rd`, `Card`,
`Val`, `Lang`, `Behaviours`, `Construct`) in any signature.

## 1. Crate

- `crates/ir/Cargo.toml`: package `spicerack-ir`, lib `spicerack_ir`, edition 2024, version
  0.2.0, **no dependencies**, its own empty `[workspace]` table (it is wired into the root crate
  in slice S2; do not touch the root `Cargo.toml` or `Cargo.lock`).
- `#![forbid(unsafe_code)]`. No `std::collections::HashMap` iteration that can reach an output:
  maps are for lookup only, ids come from append order.
- House style: each file opens with a `//!` header (what goes in → what comes out, `Cites:`,
  `Contents:` naming every `pub` item), order imports / pub types / private types / pub fns /
  `// ---- private ----` / private fns / tests, at most 500 code lines, a doc comment on every
  `pub` item that says what the signature cannot. Skills: `rust`, `data-oriented-design`,
  `compiler-construction`, `doc-comments`, `code-style`, `api-design`, `assertions`.
- Every build inside `nix develop <worktree root> -c …`; before a build `free -g` must show at
  least 6 GB available. One cargo process per agent.

## 2. What the IR is (and is not)

- Meaning, not spelling: no token, no source text, no dialect tag on any node except `Opaque`.
- One node per meaning, the most specific one. Two meanings behind one keyword are two nodes
  (`Func::Pow` with C's sign rule, `Func::PowAbs` = `|x|^y`; `Ln`, `Log10`; `RoundHalfEven`,
  `RoundHalfAway`; SFFM's two laws). A reader and its writer are inverse maps between their own
  text and these nodes; conversions between meanings are passes outside this crate.
- Explicit over implicit: anything a simulator fills in by default and on which two simulators
  differ has a field. "Absent" is a value only when absence means the same on every simulator.
- Order is data: one ordered item list per scope.
- A statement with no node is `Opaque{dialect, class, text}`.

## 3. Conventions

| # | Rule |
|---|---|
| C1 | Handles are `#[repr(transparent)]` newtypes over `u32` with `const NONE` = `u32::MAX`, `index()`, and a short `Debug` (`n12`, `e7`). `FileId` is over `u16`. A handle is never a pointer or a reference. |
| C2 | Rows are `#[derive(Clone, Copy, Debug, PartialEq)]` structs of handles and scalars. No `String`, `Vec`, `Box`, `&`, `Option<T>` (use the handle's `NONE`, or a `u8` enum with a `None` variant). Every row has `const _: () = assert!(size_of::<Row>() == N);`. |
| C3 | Existence tables: one table per item kind. An item is `ItemRef { kind: ItemKind, index: u32 }`: a row in that kind's table. No `active` flag, no `Option` field that means "this row is really another kind". |
| C4 | A table is `Table<R> { rows: Vec<R>, spans: Vec<Span> }` (hot rows, cold spans, equal length); `push(row, span) -> u32`. |
| C5 | Lists are typed ranges into shared pools: `List<T> { start: u32, len: u32 }` (`NameList`, `ExprList`, `ParamList` of `(NameId, ExprId)` pairs, `ItemList` where needed). A scope's items are its own growable `Vec<ItemRef>` (passes rewrite them). |
| C6 | Names: exact bytes, interned once (`NameId`), ids in first-seen order. `NameId::GROUND` is id 0, the text `0`. The IR never folds case: a reader of a folding dialect passes one spelling per name. |
| C7 | A value that may be a number or an expression is an `ExprId` (a literal is one `Num` row). Optional value: `ExprId::NONE`. A string value (model `version="4.8"`, a path) is a `NameId` through `Expr::Str` or a `NameId` field. |
| C8 | A reference to a definition is `Ref { name: NameId, def: DefId }`; `def == NONE` means unresolved (legal: the kit may be absent). |
| C9 | Numbers are `f64`, finite. NaN and infinities are refused at `push` (the NaN gate). Units are SI base units; angles in degrees where SPICE uses degrees (AC phase), stated on the field. |
| C10 | Capacities are named constants (`NAMES_MAX`, `EXPRS_MAX`, `FILES_MAX = 4096`, …), asserted at `push`. |
| C11 | No recursion on input-controlled depth anywhere (evaluator, dump, parse, check, feature scan): explicit stacks. |
| C12 | Boundary enums (`ItemKind`, `ElemKind`, `WaveKind`, `AnalysisKind`, `ExprTag`, `Func`, `BinOp`, `UnOp`, `Dist`, `ScopeKind`, `DefKind`, `OpaqueClass`) are `#[repr(u8)]`, carry `ALL` and `COUNT`, and are matched without `_` arms everywhere. |
| C13 | Doors: readers and builders get `Builder<'_>` (append only). Writers get `&Design` (read only). Passes get `Editor<'_>` (replace a row, replace a scope's item list, append through its builder). Table fields are private. |
| C14 | Diagnostics are rows: `Diag { code: Code, span: Span, arg: u32 }` appended to `Diags`; nothing returns early on a diagnostic, nothing prints. |

## 4. Tables of a `Design`

| Table | Row |
|---|---|
| `names` | interner (byte buffer + offsets + lookup map) |
| `files` | `File { path: NameId, role: Role, top: ScopeId }` |
| `scopes` + `scope_items` | `Scope { kind: ScopeKind, parent: ScopeId, owner: ItemRef, file: FileId }`; `Vec<Vec<ItemRef>>` |
| `defs` | `Def { kind: DefKind, name: NameId, scope: ScopeId, item: ItemRef }` |
| `exprs` | SoA `{tag: ExprTag, a: u32, b: u32}`; a `Num`'s bits are `(a, b)`; children have lower ids |
| pools | `name_pool: Vec<NameId>`, `expr_pool: Vec<ExprId>`, `param_pool: Vec<(NameId, ExprId)>`, `text_pool` for `Opaque`/`Comment` text |
| one `Table` per `ItemKind` | see §5 |
| detail tables | per element family with typed fields (§6) |

## 5. Item kinds (`ItemKind`, the complete list)

| Group | Kind | Row (owner file) |
|---|---|---|
| structure | `Title`, `Comment`, `Include`, `Global`, `Load`, `Opaque` | `item.rs` (kernel) |
| structure, opens a scope | `Subckt`, `Section`, `If` (branches: condition + scope each) | `item.rs` (kernel) |
| definitions | `Param`, `Func` | `item.rs` (kernel) |
| definitions | `Model` | `model.rs` (circuit) |
| definitions | `Statistics` | `stats.rs` (run) |
| circuit | `Element` | `element.rs`, `source.rs` (circuit) |
| run | `Analysis` | `analysis.rs` (run) |
| run, opens a scope | `Loop` (a sweep over a range, a list or a table of states; its body holds run items) | `plan.rs` (run) |
| run | `Alter` (the circuit state changes from here on) | `plan.rs` (run) |
| run | `Option`, `Env` (temperature, nominal temperature), `NodeSet` (initial conditions and nodesets) | `option.rs` (run) |
| run | `Save`, `Print`, `Measure` | `output.rs` (run) |

`Opaque { dialect: NameId, class: OpaqueClass, text: TextId }` with `OpaqueClass::{Unread,
Private, Inert, InvalidInSource, Script}` (REWRITE-PLAN §3.4).

## 6. Elements

One header for every element, a detail row only where the kind has typed fields:

```rust
pub struct Element {
    pub kind: ElemKind,
    pub name: NameId,          // without any type letter
    pub terminals: NameList,   // in the kind's role order (documented per kind)
    pub model: Ref,            // model or subcircuit master; NONE def when the card names none
    pub params: ParamList,     // instance parameters with no typed field
    pub mult: ExprId,          // multiplicity; NONE = 1
    pub detail: u32,           // row in the kind's detail table, or NONE
}
```

`ElemKind` (circuit owner fixes the list from `rewrite/INV-constructs.md` §D): resistor,
capacitor, charge-defined capacitor, inductor, coupling, voltage source, current source, RF port,
VCVS, VCCS, CCCS, CCVS, polynomial source, table source, behavioural source, Laplace source,
diode, BJT, MOSFET, JFET, MESFET, voltage switch, current switch, lossless line, lossy line, RC
line, single lossy line, coupled lines, code model, Verilog-A module instance, subcircuit instance,
instance of an unresolved master.

## 7. Expressions (`expr.rs`, kernel)

The neutral language of `DESIGN-neutral-expressions.md` §2
(`/tmp/claude-1000/spicerack-golden/research/`), as rows. Tags: `Num`, `Str`, `Param`, `Call`
(a `Func`), `UserCall` (a `.func`), `Unary`, `Binary`, `Ternary`, `V` (one or two nets), `I`
(branch current of an element), `Time`, `Temp`, `Freq`, `Draw` (a statistical draw: `Dist` +
arguments), `List` (a vector of values). Each tag's doc says how to read `a` and `b`.

`Func` is closed and every member has one meaning, stated in its doc with its domain: `Abs Sqrt
Exp Ln Log10 Pow PowAbs Min Max Floor Ceil Trunc RoundHalfAway RoundHalfEven Sgn Sin Cos Tan Asin
Acos Atan Atan2 Sinh Cosh Tanh Asinh Acosh Atanh Hypot Fmod Clamp Step Ramp Ddt Idt Pwl`. Add a
member only for a meaning some dialect has natively and no tree of the others states exactly
(`INV-constructs.md` §C, `INV-semantics.md` categories 1–2). `BinOp`: arithmetic, `IntDiv`,
comparisons, logical and bitwise, shifts. `UnOp`: `Neg`, `Not`, `BitNot`.

`eval.rs`: the reference evaluator `eval(&Design, ExprId, &dyn Bindings) -> Result<f64, Domain>`
(C `f64` semantics, the ternary evaluates only its chosen branch, explicit stack). It is the
oracle for every expression rule; it is not used by writers.

## 8. Features (`feature.rs`, kernel)

Every kind is a feature: `enum Feature { Item(ItemKind), Elem(ElemKind), Wave(WaveKind),
Analysis(AnalysisKind), Func(Func), Binary(BinOp), Unary(UnOp), Dist(Dist), Extra(Extra) }` with a
`const fn index(self) -> u16`. `Extra` is the closed list of features that are fields, not kinds
(`PulseCycles`, `PwlRepeat`, `NestedSubckt`, `LocalModel`, `LocalFunc`, `ParamInBranch`,
`IcInOp`, `IcUicOnly`, `InstanceMult`, `ElementParasitics`, `CaseSensitiveNames`, …); the circuit
and run owners append theirs in their marked regions. `FeatureSet` is a fixed bitset with `const`
constructors (`FeatureSet::EMPTY.with(f)`), `contains`, `minus`, `iter`. `features_used(&Design)
-> FeatureSet` is one scan.

## 9. Check, text, generator (signatures only in this phase)

- `check.rs`: `check(&Design, &mut Diags)`, `check_legal(&Design, &FeatureSet, &mut Diags)`, and
  `enum Violation` with one documented variant per invariant: handles in range; scope tree well
  formed and owners match kinds; expression children below their parent and arities right;
  terminal counts within the kind's range; a detail row present exactly when the kind has one; a
  resolved `Ref` names a definition of the right kind with that name; no non-finite number; names
  non-empty; probes only where an expression may read the circuit; no duplicate definition of one
  kind and name in one scope; `Opaque` has a dialect and text.
- `text.rs`: `dump(&Design, FileId, &mut String)`, `parse(&str, &mut Builder, &mut Diags)`. The
  format is canonical (one spelling per design), lossless (`parse(dump(d))` rebuilds equal tables),
  line oriented, indented by scope, writable by hand. Put the grammar and one worked example in
  the file header.
- `gen.rs` (behind `cfg(any(test, feature = "gen"))`): `generate(seed, &FeatureSet, Size) ->
  Design`, always valid under `check` and legal for the set.
- `diag.rs`: `Code { letter: u8, number: u16 }`, `Kind::{Note, Refusal}`, `Info { title, cite,
  explain, kind }`, `Diag`, `Diags`, and this crate's own table: `X` codes (one per `Violation`),
  `U` (unsupported feature; `arg` is the feature index), `S` (one per `OpaqueClass`). Other owners
  keep their own tables in numeric ranges; a test in the root crate checks they do not collide.

## 10. File map and owners

| File | Owner | Holds |
|---|---|---|
| `lib.rs`, `id.rs`, `names.rs`, `span.rs`, `table.rs`, `design.rs`, `scope.rs`, `build.rs`, `edit.rs`, `item.rs`, `expr.rs`, `eval.rs`, `feature.rs`, `check.rs`, `diag.rs`, `text.rs`, `gen.rs` | kernel | everything above |
| `element.rs`, `passive.rs`, `source.rs`, `wave.rs`, `controlled.rs`, `line.rs`, `model.rs`, `build_circuit.rs` | circuit | `ElemKind`, detail rows, `WaveKind` and wave rows, `Model`, `ModelFamily`, the builder's circuit methods |
| `analysis.rs`, `plan.rs`, `option.rs`, `output.rs`, `stats.rs`, `build_run.rs` | run | `AnalysisKind` and analysis rows, `Loop`/`Alter`, options, environment, node sets, probes, save/print/measure, statistics, the builder's run methods |

The kernel owner creates every file of the other two owners as a stub (header, the type names
`design.rs` needs, declared empty with a `// circuit:` or `// run:` note) so the crate compiles
and the other two never edit a kernel file except their marked region of `feature.rs`.
