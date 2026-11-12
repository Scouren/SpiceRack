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

### 7.1 Addendum A1 (2026-11-11): native meanings, exactly

The round-trip law (`read_d(write_d(x))` equals `x`, REWRITE-PLAN §4.2 L5) must hold exactly, so
on its own dialect a reader and a writer are inverse tables between spellings and nodes. That
fixes four things:

1. **One member per native meaning; never an idiom.** ngspice's B-source `ln` is `|x|`'s
   logarithm. Encoding it as the tree `Ln(Abs(x))` breaks the law (written `ln(abs(x))`, it reads
   back as `LnAbs(Abs(x))`). So it is its own member. A reader may still expand *sugar* into a
   tree when its own writer spells that tree back to the same tree (Xyce `sign(x, y)` →
   `Mul(Abs(x), Sgn(y))`; LTspice's boolean `a ^ b` → `Ne(Gt(a, 0.5), Gt(b, 0.5))`).
2. **`Func` members this adds** (evidence: `rewrite/INV-semantics.md` 2.04–2.10, when it lands;
   `INV-constructs.md` C16, C20, C21, C53):
   - `Pow`: the signed real power; a negative base with a whole exponent is exact, with a
     fractional exponent it is a domain error (HSPICE, LTspice, Spectre, VACASK).
   - `PowAbs`: `|x|^y` (ngspice `**`, `^`, `pow` in a B source).
   - `PowRe`: the real part of the complex power, `|x|^y·cos(πy)` for `x < 0` (Xyce `**`, `pow`,
     `pwr`); equal to `Pow` wherever `Pow` is defined.
   - `Ln`, `Log10`, `Sqrt`, `Exp`: real functions, domain errors outside their domains.
   - `LnAbs`, `SqrtAbs`: of `|x|` (ngspice B source `ln`/`log`, `sqrt`).
   - `ExpLin14`: `exp(x)` up to 14, the tangent line beyond (ngspice B source `exp`).
   - `RoundHalfEven` (ngspice `nint`), `RoundHalfAway` (Xyce, HSPICE, LTspice, Spectre, VACASK),
     `Trunc`, `Floor`, `Ceil`; `Fmod` takes the dividend's sign; `Clamp(x, lo, hi)`.
   There is no `limit`, `pwr`, `pwrs`, `if`, `nint`, `log` or named constant in the IR: readers
   map each to the member (or draw, or literal) its own simulator means.
3. **Operators.** `And`, `Or`, `Not` read "nonzero is true" and yield 0 or 1. There is no logical
   XOR (each reader expands it with its own truth rule). `BitAnd`, `BitOr`, `BitXor`, `BitNot`,
   `Shl`, `Shr` are VACASK's integer operators. `IntDiv` truncates toward zero (ngspice `\`).
4. **Leaves for what a dialect leaves to the analysis.** `TStep` and `TStop` (the transient's
   step and stop: ngspice's meaning of a given-zero PULSE rise or a defaulted SFFM carrier) and
   `Tnom`, beside `Time`, `Temp`, `Freq`. A pass substitutes numbers when the design has one
   transient; the reader never looks for the analysis.

**Legality is per context.** Every simulator has functions and operators that are legal in a
parameter and illegal in an expression that reads the circuit (or the reverse). So the feature
enum also has `FuncDyn(Func)`, `BinaryDyn(BinOp)`, `UnaryDyn(UnOp)`: the member used inside an
expression that holds a `V`, `I`, `Time` or `Freq` leaf. `features_used` sets the plain or the
`Dyn` feature by that test.

### 7.2 Addendum A2 (2026-11-11): row facts, and what a draw means

**Row facts.** The kernel's generic code (`check`, `features_used`, `text::dump`/`parse`, `gen`,
passes that renumber) must read rows it does not own. Each row's owner states what the kernel
needs, in the row's own file, through one trait declared in the kernel (`facts.rs`):

```rust
/// What the kernel's generic code needs from an item row. Implemented by the row's owner, in
/// the row's file; the kernel reaches every kind through one exhaustive dispatch.
pub trait RowFacts: ItemRow {
    /// Appends every handle the row and its detail rows hold.
    fn handles(&self, design: &Design, out: &mut Vec<Handle>);
    /// Adds what the row uses beyond `Feature::Item(Self::KIND)`: its element, wave or analysis
    /// kind and its `Extra`s. The kernel adds the expression features from `handles`.
    fn features(&self, design: &Design, set: &mut FeatureSet);
    /// Appends the violations of the row's own invariants (counts, a detail row present exactly
    /// when the kind has one, field domains). Handle ranges are the kernel's check.
    fn check(&self, design: &Design, at: ItemRef, out: &mut Diags);
    /// Writes the row as IR text: its statement, without indentation and without its body.
    fn dump(&self, design: &Design, out: &mut String);
    /// Reads one IR-text statement of this kind and pushes it into `scope`.
    fn parse(words: &mut Words<'_>, scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ItemRef;
    /// Pushes one random valid row of this kind that uses only `features`.
    #[cfg(any(test, feature = "gen"))]
    fn generate(g: &mut Gen, scope: ScopeId, b: &mut Builder<'_>, features: &FeatureSet) -> ItemRef;
}

pub enum Handle { Expr(ExprId), Name(NameId), Ref(Ref), Names(NameList), Exprs(ExprList),
                  Params(ParamList), Scope(ScopeId), Text(TextId) }
```

`Words` is the IR text reader's cursor (`text.rs`), `Gen` the generator's state (`gen.rs`); both
are kernel types the owners use. Per-kind constant facts the owners' `check` needs
(`ElemKind::terminals() -> (u8, u8)`, `ElemKind::has_detail()`, `ElemKind::needs_model()`) are
`const fn`s in the owner's file.

**A draw states when and how often it is drawn.** The same `agauss(…)` is a fresh sample at
every reference on ngspice even in a plain run, the mean outside a sampling run on Xyce and
HSPICE, one sample per call site per scope on VACASK (`INV-semantics.md` 12.02, 12.03). Those are
three meanings, so the node states them: `Expr::Draw(Dist, DrawWhen, DrawPer, ExprList)` with
`DrawWhen::{Always, Sampling}` (outside a sampling run a `Sampling` draw is its nominal) and
`DrawPer::{Reference, CallSite, Instance, Run}`. Converting one to another (the two-layer
parameter idiom, Spectre's `statistics` block) is a pass.

**A4: references are bound by a linker, not by the row's author.** `RowFacts` also has
`map_refs(&mut self, f: &mut dyn FnMut(&mut Ref, DefKind))` (each `Ref` the row holds, with the
kind it expects). `link(&mut Design, &mut Diags)` (`link.rs`) binds every unresolved `Ref` by
exact name from the item's scope outward. It is the neutral rule: builders call it when they are
done, the IR text parser runs it after the whole text (so a hand-written fixture may name a
definition that comes later), and a dialect reader never calls it, because a reader binds by its
own simulator's rule. In IR text a reference is a bare `name`; `name?` is deliberately
unresolved; `name#k` appears only when the lookup would bind another definition.

### 7.3 Addendum A3 (2026-11-11): a number is a decimal literal, not a double

Ruling on `INV-semantics.md` 1.08 (the sources conflicted). Two simulators do not compute the
same double from the same digits: ngspice multiplies an integer mantissa by one power of ten
(`100u` is 9.999999999999999e-5, `0.050000` is not `0.05`), VACASK multiplies `stod(mantissa)` by
the scale, the others round the decimal correctly. Storing "the double" forces a choice between
the source simulator's image (exact on the same simulator, 17-digit monsters everywhere else) and
the rounded decimal (pretty, and one ulp off on the same simulator, which is enough to move
ngspice's time steps: 1.09). So the IR stores neither. It stores what the deck says:

```rust
/// A decimal literal, as written: `mantissa × 10^(exp10 − frac)` (`Mil`: that times 25.4e-6).
/// The IR states the number; the double is each simulator's own arithmetic on these digits.
pub struct Literal {
    pub mantissa: i64,   // every digit as one integer, sign included; |mantissa| < 10^18
    pub frac: u8,        // digits after the decimal point (`0.050000`: mantissa 50000, frac 6)
    pub exp10: i16,      // the exponent, or the scale suffix's power of ten
    pub form: NumForm,   // Plain (exp10 == 0), Exponent (`1e-4`), Scale (`100u`), Mil
}
```

- `Expr::Num(NumId)`; literals live in a side table of the design (`Design::num(NumId) ->
  Literal`), the expression row holding the index. `Literal::value() -> f64` is the correctly
  rounded value (what the reference evaluator and the passes compute with).
- `Builder::literal(Literal) -> ExprId` is what readers call: digits in, no floating arithmetic
  anywhere in a reader. `Builder::num(f64) -> ExprId` is for built numbers (Python, Schemify,
  passes): the shortest decimal that reads back to the double, in engineering form (`1e-4` →
  mantissa 100, `Scale`, exp10 −6), asserted finite (the NaN gate, C9). Zero is one literal
  (mantissa 0, frac 0, exp10 0, `Plain`).
- A writer spells the same digits and the same form in its own syntax (its suffix letter for a
  `Scale` it has; `e<exp10>` for one it lacks; `Mil` only where the dialect has `mil`). On the
  same simulator the text has the same digits as the original, so the simulator computes the same
  double: bit-identical runs without emulating anyone's arithmetic. Across simulators the reader
  sees `100u` and the target gets `100u`.
- Law L5 compares literals field by field. The differential harness (REWRITE-PLAN §7.3) compares
  them by value (`mantissa × 10^(exp10 − frac)` as exact decimals), since `1k` and `1e3` mean one
  number.
- A literal with more than 18 significant digits is rounded to 18 by the reader, with a note; a
  suffix followed by an exponent or the reverse (`1e3k`) is folded into `Exponent`.

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
