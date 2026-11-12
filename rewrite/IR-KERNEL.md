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

**A9 (2026-11-12): a row's facts take its position.** The bodies showed three things the trait
above could not say, so its final shape is (as implemented in `facts.rs`):

```rust
fn handles(&self, design: &Design, out: &mut Vec<Handle>);
fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind));
fn refs(&self, design: &Design, at: ItemRef, out: &mut Vec<(Ref, DefKind)>); // M5: read twin
fn features(&self, design: &Design, at: ItemRef, scope: ScopeId, set: &mut FeatureSet);
fn check(&self, design: &Design, at: ItemRef, scope: ScopeId, out: &mut Diags);
fn dump(&self, design: &Design, ctx: &text::Ctx, out: &mut String);
fn parse(words: &mut Words<'_>, scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ItemRef;
fn generate(g: &mut Gen, scope: ScopeId, b: &mut Builder<'_>, features: &FeatureSet) -> ItemRef;
```

`map_refs` is an associated function: it visits every `Ref` the item owns, in its header, its
detail rows and its pools (a sweep's target, a `Vary`'s parameter), in a fixed order, and stores
them back. `Editor::map_refs` dispatches to it, `link` binds through it in two passes (record,
look up, write the k-th answer to the k-th slot), and `check` reads it from one copy of the design.
`dump` takes a `text::Ctx { scope, item, formals }`: the scope a bare reference is looked up from,
the item (a measure's reads fall back to measures), and the function whose body is being written
(its formal arguments). The kernel's dump loop builds it; a row passes it to the writers. Features
of the whole design (several analyses in one deck, an option after a run item) are not a row's:
`features_used` calls one `pub(crate) fn design_features(&Design, &mut FeatureSet)` hook per owner
(`element.rs`, `analysis.rs`), and `check_legal` reports what they add at `Span::NONE`. `text`
gives owners the writers that mirror the `Words` readers: `write_name`, `write_names`,
`write_string`, `write_literal`, `write_value`, `write_expr`, `write_exprs`, `write_params`,
`write_ref(design, r, kind, ctx, out)` (bare when lookup from `ctx.scope` gives what the reference
holds, `name?` for an unresolved reference the lookup would bind, `name#k` otherwise).

**A10 (2026-11-12): a subcircuit's head defaults are looked up from its body.** The neutral lookup
scope of a `Subckt` row's expressions is its body scope, not the scope that lists it, so
`r=rval w=(2*r)` binds `r` to the sibling head parameter and then looks outward. `link`, the
writer's bare-or-`#k` test and `parse` (through `link`) use the body for that row; every other row
keeps its listing scope. Why: ngspice sorts head parameters by dependency, HSPICE and Spectre read
them in the body, and a lookup from the parent would dump the common case as `r#k`.

**A11 (2026-11-12): from the ngspice tracer (ngspice-45 source and runs).**
1. Three `Func` members: `StepHalf` (0 for x < 0, 0.5 at 0, 1 for x > 0: ngspice B-source `u`,
   `PTustep`), `PowSigned` (−pow(−x, y) for x < 0, else pow(x, y), `0` to a negative power a
   domain error: B-source `pwr`, `PTpwr`), `Sqr` (x·x: numparam `sqr`). `Func::COUNT` is 43 and
   `Feature::COUNT` 502.
2. Attributions corrected, no behaviour change: `Step` is not ngspice's `u`; `SqrtAbs` and `LnAbs`
   are not ngspice-45's B-source `sqrt`/`ln` (those error below zero), and are removed if no reader
   produces them; `ExpLin14` is ngspice's B-source `exp` under `ngbehavior=ps` only; `IntDiv` is not
   ngspice's `\` (which is `trunc(|a / b|)`, read as `Trunc(Abs(Div))`), no speaker yet.
3. The dynamic class (R14): `pub enum ExprClass { Constant, Temp, Circuit }` (ordered) and
   `pub fn expr_class(&Design, ExprId) -> ExprClass`. An expression is `Circuit` when it, or a
   parameter or user function it reads (transitively), holds V, I, Time, Freq, Ddt or Idt; else
   `Temp` when one of them holds `Temp`; else `Constant`. One class per `Param`/`Func` definition,
   computed once per design state (Kahn's worklist; a definition on or behind a reference cycle is
   `Circuit`) and cached in the design, cleared by every `Builder`/`Editor` mutation. The `*Dyn`
   features are `class >= Temp`. `ProbeContext` stays the direct test.
4. `Builder::bind_ref(item, k, def)` sets the k-th `Ref` of `item`, in `map_refs` order (asserts `k`
   in range and the slot's kind), so a reader binds a row reference to a definition it pushes later.

**A12 K1 (2026-11-12): typed arithmetic (R16).** `NumForm::Int` (a whole number of integer type:
`frac == 0`, `exp10 == 0`; the Int zero is the second permitted zero); `BinOp::DivTyped` (both
Int: the quotient truncated toward zero, Int; else `a / b`, Real; `Div` unchanged); `Func::ToInt`
(`x` truncated, Int; `Trunc` stays Real); `Extra::IntLiteral`. The reference evaluator tags every
value Int or Real (`eval::eval_typed -> Value { number, int }`; `eval` keeps returning the
number; `Bindings::param_value` passes a typed parameter, defaulting to `param` as Real):
`Num` Int iff its form is; a read its value's; `Neg` its operand's; `Add`, `Sub`, `Mul`, `Min`,
`Max` Int iff every operand is; comparisons, logic, bit operators and shifts Int; the ternary its
chosen branch's; `ToInt` Int; `SgnZeroPos` (−1 for `x < 0`, +1 otherwise, −0.0 included:
VACASK's `sgn`, runs e1, f4) its operand's; `ToReal` (A21, R40: the value of `x`, VACASK run
b1) Real; everything else Real, `Sgn` (−1, 0, 1) included. An Int beyond ±2⁵³ is a
domain error. IR text: an Int literal is `3i`, `DivTyped` is `/:`, `ToInt` is `to_int`, `SgnZeroPos` is `sgn_zero_pos`, `ToReal` is `to_real`. An Int
and a real literal of one value are not `eq_value` (they mean different things).

**A12 K2 (2026-11-12): two features of scope (R18).** `Extra::EnclosingRef`: an item holds a
resolved `Ref` (a row reference, or a `Param`/`UserCall` in one of its expressions) bound to a
definition that lives in the body of a subcircuit enclosing the item's own subcircuit: on the
item's scope chain, past the nearest `Subckt` scope, with a `Subckt` scope still at or past it
(so neither the own subcircuit nor a file's top). An item in no subcircuit never has it.
`Extra::FormalNotConstant` (was `FormalReadsParam`; widened by A15): a `Subckt` head default that is not a constant, holding a `Param` read anywhere in its tree or of class `Temp`. The
generator binds an enclosing definition only when the set has `EnclosingRef`, and puts a `Param`
leaf or a `$temp` in a head default only when it has `FormalNotConstant` (VACASK runs n1, m1, c1, t2).
`Extra::DeckWithoutTitle` (a design-level feature, reported at `Span::NONE`): a `Deck` file whose
top scope lists no `Title`. The generator titles every deck when the set lacks it, and makes
only libraries when the set has neither it nor `Item(Title)`.

**A12 K3 (2026-11-12): `OpaqueClass::Hint` (R19).** Tunes its own simulator's solver (a
convergence aid, a step-control knob) and names no accuracy target. Appended after `Script`, so
its code is `S0006` and no earlier code moves; IR text word `hint`. Its note says its own family
copies it and another family leaves it out, the numbers moving at most within the solver's
tolerance (not `Inert`'s note: an `Inert` statement moves no number). Dropping it is the spine's
`legalize`, not the kernel's.

**A15 K2 (2026-11-12): two more extras.** `Extra::FormalWithoutDefault`: a `Subckt` head
parameter with no default (an instance must pass it). `Extra::IfConditionDynamic`: an `If`
condition of class `Temp` or `Circuit` (transitive, as the `*Dyn` features are). The generator
gives every formal a default and makes every condition `Constant` unless the set has the
feature.

**A15 K3 (2026-11-12): two invariants.** A `Title`'s or `Comment`'s text holds no line break
(`\n` or `\r`): `FieldDomain`. `ProbeContext` follows the class: a constant-in-time field
(`Handle::Expr`, a list of them, a head default) whose expression is of class `Circuit` through a
parameter or function it reads is a violation, as a direct probe is. That pass runs last and only
on a design with no other violation (the class table reads definitions through their rows).
The generator writes one-line titles and comments, and a tree under `Reads::Constant` reads no
`Circuit`-class definition.

**A15 K4 (2026-11-12): literal equality (R28).** `Design::eq_meaning` compares two real
literals by exact value whatever their forms (`1k` = `1e3` = `1000` = `1000.0`); an `Int` literal
equals only an `Int` of the same value. `Design::eq_ignoring_spans` stays exact: forms are
compared. Both were already so; `kernel_literal` pins it.

**A15 K1 (2026-11-12): placement is a feature class (R27, ruled "enclosed").**
`Feature::Placed(ScopeKind, ItemKind)`: an item of that kind with a scope of that kind on its
scope chain, at any depth; `File` is never the scope kind (every target has a file's top), so the
group is 4 × `ItemKind::COUNT`. A subcircuit in an `If` branch in a subcircuit sets
`Placed(Branch, Subckt)` and `Placed(Subckt, Subckt)`. The kernel sets it for every item in
`item_features`; no owner sets a placement member. `FeatureSet::placed(scope, &[kinds])` builds a
writer's set. It replaces, with this mapping:

| Was | Now |
|---|---|
| `Extra::NestedSubckt` | `Placed(Subckt, Subckt)` |
| `Extra::LocalModel` | `Placed(Subckt, Model)` |
| `Extra::LocalFunc` | `Placed(Subckt, Func)` |
| `Extra::ParamInBranch` | `Placed(Branch, Param)` |
| `Extra::LoadInBranch` | `Placed(Branch, Load)` |
| `RunExtra::NodeSetInSubckt` | `Placed(Subckt, NodeSet)` |
| `RunExtra::LoopNested` | `Placed(Loop, Loop)` |
| `RunExtra::RunItemNested` | `Placed(Subckt, K)` and `Placed(Section, K)` for the run kind `K` |

The generator's `fill` admits a kind in a scope only when every placement it would make is in
the set (`gen::placeable`, also for a row's own generator that pushes into a body it opened);
sections may now be nested where the set allows. `Placed(_, Title)` never occurs: a title stands
only in a deck's top, which `check` now enforces too (`FieldDomain`; the builder asserts it and IR
text refuses it, so only a pass moving the item reaches the check).

**A19 (2026-11-12): a field that may read the circuit counts the dynamic form (R38).** An
expression a row lists as `Handle::ExprDyn` counts its functions and operators as `FuncDyn`,
`BinaryDyn` and `UnaryDyn` whatever its class; elsewhere the form follows the class as before
(dynamic when `Temp` or `Circuit`). A constant in a B source's expression is the dynamic form,
because its simulator reads that field with the behavioural parser (`B1 a 0 V={2**3}`). The
generator picks operators from the dynamic features for every tree under `Reads::Circuit`, and
for a `Handle::ExprDyn` field it builds with constant reads (`Gen::dyn_form`, a measure's
threshold).

**A23 (2026-11-12): the passes' door for expressions.** A fourth row fact beside `handles`,
`map_refs` and `refs`:

```rust
pub enum ExprUse { Constant, Circuit }
fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)); // default: no root
```

It calls `f` on exactly the roots `handles` lists, in its order (`NONE` included): a single
`Expr` (`Constant`) or `ExprDyn` (`Circuit`) field, each `Exprs` element and each `Params` value
(`Constant`), and stores back what `f` changed. `Editor::map_exprs` dispatches it;
`Design::item_handles` dispatches `handles` for readers. `rewrite_exprs(design, rule) -> u32`
(`rewrite.rs`) rewrites every root of every listed item bottom-up: `rule(b, node)` sees a node
whose children are rewritten and returns `Some(new)`; a node whose child changed is appended
again (`Builder::rebuild`), an unchanged tree keeps its ids, a shared subtree is rewritten once
(a memo by old id), the walk is an explicit stack, and only rows that existed before the pass are
visited. Properties (`tests/kernel_rewrite.rs`): `map_exprs` against `handles` for every
generated item (every kind of all three owners, since round 4), an identity
rule changes no id, a wrapping rule changes every visited root and no `eval` value, and `check`
finds nothing after either.

**A18 (2026-11-12): a literal keeps digits its mantissa cannot hold (R37).** `NumForm::Long`: the
significand as written (`[-]digits[.digits]`) lives in the design's long-literal pool beside the
literal table; the row (still 16 bytes) holds its pool index in `mantissa`, the power in `exp10`,
and the form that power was written in (`Plain`, `Exponent`, `Scale`) in `frac`. One bound
decides long against short, `literal::exceeds_mantissa(significant_digits(..))`: more than 18
significant digits, leading zeros not counted and trailing zeros as written counted (`0.000123`
is 3 digits, `1.2300` is 5), the count a short mantissa holds; the lexer, `Builder::long_literal`
and `check` call it, so no value has two rows. Value: the correctly rounded double of the digits
(`Design::literal_value`). Equality: exact decimal equality, digits and power after stripping
zeros, never the doubles (`Design::literal_eq`, and `eq_meaning` through it); an `Int` equals
only an `Int`; `eq_ignoring_spans` compares the digits as written. The bare row's
`Literal::value`, `eq_value` and `text::write_literal` assert "not Long", and `is_valid` is
false for it; every generic caller goes through the design (`literal_value`, `literal_eq`,
`long_digits`, `text::write_num`). IR text spells the digits as written, then the power in its
form. `Extra::LongLiteral` marks one; the generator makes them only when the set has it.

**A4: references are bound by a linker, not by the row's author.** `RowFacts` also has
`map_refs` (each `Ref` the item holds, with the kind it expects; final signature in A9 above). `link(&mut Design, &mut Diags)` (`link.rs`) binds every unresolved `Ref` by
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
Analysis(AnalysisKind), Expr(ExprTag), Func(Func), FuncDyn(Func), Binary(BinOp), BinaryDyn(BinOp),
Unary(UnOp), UnaryDyn(UnOp), Dist(Dist), DrawWhen(DrawWhen), DrawPer(DrawPer), Extra(Extra),
Circuit(CircuitExtra), Run(RunExtra), Option(OptionKey) }` with a `const fn index(self) -> u16`
(496 features at the freeze). The features that are fields, not kinds, are three closed enums,
one per owner and each in its owner's file, so adding one never touches a kernel file: `Extra`
(kernel, `feature.rs`: `IncludeSection`, `CaseSensitiveNames`, …; placement is
`Feature::Placed` since A15 K1), `CircuitExtra` (`circuit_extra.rs`: pulse cycle counts, PWL repeat
forms, each law variant, IC modes, parasitics, multiplicity per kind class, …), `RunExtra`
(`run_extra.rs`: a simulator-chosen step, start-up, list and table sweeps, sweep ends, loop
targets, measure and probe kinds, …). `FeatureSet` is a fixed bitset with `const` constructors
(`FeatureSet::EMPTY.with(f)`), `contains`, `minus`, `iter`. `features_used(&Design) ->
FeatureSet` is one scan.

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
