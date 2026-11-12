# SpiceRack rewrite plan: one IR, one contract per dialect

Status: plan for approval (2026-11-11). Nothing here is implemented. Written against `main`
(f7fbf3b, 0.2.0) and the integration head `p7-integrate` (1059821, 596 commits ahead of `main`,
never pushed). Method: the house `rewrite-clean` (old code kept alive as the oracle, one slice at
a time, four gated phases per slice) and `codebase-clean-code` (data, owners, spines, seams,
drills). Every count below was measured on 1059821 unless it says otherwise.

## 0. Summary

**Verdict.** Rewrite the core (`src/syntax/render`, `def`, `dialects`, `behaviour`, `lang`, and the
deck half of `src/python`), slice by slice, around one owned semantic IR. Keep the front end
(tokenizer, lexicons, number reading, kit loader), the run side (`backend`, `simulation`, `rawfile`,
`result`, `measure`), the Python testbench package, the simulators-in-the-loop tests and the
cross-simulator harness. Roughly 47k of 65k Rust lines are in scope; about 30k replace them.

**Shape.**

```
text(A) ──read_A──▶ IR ──check──▶ legalize(features of B) ──check──▶ write_B ──▶ text(B)
                     ▲
     Python / Rust builders (the same ir::Builder every reader uses)
```

- `read_A` knows dialect A and the IR. It never learns the target.
- `write_B` knows dialect B and the IR. It never sees the source text, tokens or source dialect.
- The IR states meaning explicitly (a pinned normal form), so no rule is ever a fact of two
  dialects together.
- What a target cannot say is removed by dialect-neutral IR→IR passes, or refused with a code.
- Every dialect implements the same two traits and must pass the same law suite, run from the
  registry, so a new dialect is tested the moment it is listed.

**Why this isolates bugs.** A wrong output is then exactly one of: the reader made the wrong IR
(checked against the manual, no writer involved), a pass changed meaning (checked IR→IR), the
writer misspelled valid IR (checked against the real simulator, no reader involved), or the IR
lacks a fact (the only cross-dialect class, and each one becomes a normal-form rule). §7.4 is the
triage procedure; each step is one command.

**What I need from you** (§10): freeze the row team on the old architecture, confirm the base
branch, the dialect set, the fate of the `DialectDef` plugin API, and the byte-fidelity policy.

## 1. Diagnosis: how it drifted

| When | What happened |
|---|---|
| 2026-10-24 | "IR is the only circuit model; one IR → codegen → backend run path" (5d016cf). Schemify adopts it the same day (its D22) and still pins that tree (0fc42bd: `spicerack::ir::*`, `codegen::xschem`). |
| 2026-11-03/04 | The "transpiler rewrite", phases 0–6: a Zig-style token-bound AST replaces the PEG reader, gated byte for byte against the old pipeline (9d1d6bb). Phase 6 (f9a8f1d) deletes `CircuitIR`; the Python API now prints ngspice text for the parser to read. |
| 2026-11-07 → today | ZERO-LEDGER: 588 rows, each a `src→tgt` cell. Dialects become `DialectDef` rows (about 90 calls) selecting code paths in two family monoliths. 22 batches landed on 2026-11-11 alone; the golden baseline rotated four times that day (fx15 → fx18). |

The intent recorded in `tests/fixtures/transpiler/DIALECT-PLUGIN.md` ("decided by the user") is the
one in your message: every dialect goes through the same interface, and a dialect "both reads its
dialect's text into the one AST and writes any deck in its dialect". The implementation made a
dialect a set of configuration rows over shared family code instead of a reader and a writer.

**Root cause: the hub is syntax, not meaning.** `syntax::ast::Ast` is the concrete syntax of the
*source* dialect, rows pointing at source tokens. Meaning is computed late, inside the writer
stage, by `render/read` ("views" over the source AST), per target. Everything else follows:

| Symptom | Evidence (1059821) |
|---|---|
| Writers read the source | 347 calls from `render/{spice,named}` into the reader `Rd`, 123 of them straight at tokens and AST fields; 43 reads of the *source's* behaviours inside writers |
| The reader knows the target | `Rd::write_for(target)`; `rd.param_winners(&mut deck, target.behaviours())` (`render/mod.rs:326-327`) |
| Rules are facts of pairs | `render/spice/pair.rs`: 36 source×target rules ("a fact of the two rows together, never of either alone"); expression rewrites keyed source × target |
| Meaning is carried as text | 122 `String` fields in the views (`Val::Expr(String)`, `params: Vec<(String, String)>`); 518 `format!` in the writers; token "fixups" to respell values; a text post-pass over the written deck (`def_pass`, `globals::xyce_globals`) |
| The old pipeline's accidents are load-bearing | 37 `// quirk:` rules, 126 comments citing "the old reader/writer": the byte-identical gate was an oracle of sameness, so structure accidents were preserved as behaviour |
| No builder without text | every Python builder call formats an ngspice card (`format!` at ~80 sites in `src/python`); `syntax::build::Build::text(lines)` tokenizes and parses that text; the "neutral expression" design is `Lang::SPICERACK`, a tenth source dialect in the same matrix |
| The binding is a third writer | `python/deck.rs::bench` and `analysis_region.rs` hold their own `match Dialect` (Fourier, `pss`, log sweeps), raise W0040 themselves, and sniff their own text back in 12 places (`laplace(` substring, `portnum` word, first word of a card) |
| Side channels | `Numbers::{Read,Given}`; a `*spicerack ngbehavior=` comment the backend lifts; `LTSPICE_EACH` comment; `Source::loads`; the run path re-scans the written deck with line scanners in 8 files' worth of places (`.control` contents, `.dc` fields, VACASK `analysis` names) |
| Routing renders | choosing a simulator renders the whole deck once per candidate (`backend::select`'s `writes` filter), then once more for the text |
| Change radius | one new lexical fact = 10 edit sites, one new behaviour = 8 (the doc comments on `Lexicon` and `Behaviours` list them); streams share spine files under append-only rulings (SPEC-BRIEF R1, R2) |
| Size | `src` 33.7k lines on `main` → 64.6k non-blank on the head in a week; `render` alone 30.2k (99 files) |
| Downstream | Schemify cannot move past 0fc42bd (its IR is gone). Interoperate pins `main` and reaches into `syntax::{tokenize, ast::Node, resolve::Kit, device::Primitive}` because there is no model to query |

None of this is carelessness: each step obeyed "refactors keep goldens byte-identical". That rule
is right for a refactor and wrong for this change, because here the structure of the old output
*is* the thing being replaced (§7.3 says what replaces it).

## 2. Target architecture

### 2.1 Spines

One function per pipeline, every branch on *which* work in the spine:

```rust
// transpile: text in one dialect → text in another
pub fn transpile(src: &Sources, from: &DialectRow, to: &DialectRow) -> Result<Written, Diags> {
    let mut design = ir::Design::new();
    let mut diags = Diags::new();
    from.read.read(src, &mut design.builder(), &mut diags);   // text(A) → IR
    ir::check(&design, &mut diags);                           // a violation is a reader bug
    passes::legalize(&mut design, to.write.features(), &mut diags);   // IR → IR legal for B
    ir::check_legal(&design, to.write.features(), &mut diags);        // a violation is a pass bug
    let mut text = String::new();
    to.write.write(&design, design.entry(), &mut text, &mut diags);   // IR → text(B)
    diags.finish(text)                                        // refusal if any Refusal row
}
```

`netlist` (Python/Rust builders → text) is the same spine without the first line. `run` is
`netlist` + `backend` + `rawfile`, unchanged.

### 2.2 Five rules that isolate bugs

1. **One-sided knowledge, by signature.** `read(&Sources, &mut ir::Builder)`; `write(&ir::Design)`.
   A `Design` holds no source text, no tokens and no source-dialect tag, so a writer cannot
   depend on where the IR came from even by accident.
2. **Explicit over implicit.** Any quantity a simulator fills in by default, and on which two
   simulators differ, is explicit in the IR after `read` (§3.3). A writer spells what the IR
   states; a conversion between two meanings is a pass. A pair rule has nowhere to live.
3. **Closed world with an honest escape.** A statement the reader cannot represent is an
   `Opaque{dialect, text}` item with a coded diagnostic, never a guess. A writer copies an
   `Opaque` of its own text family and refuses any other. Opaque count per corpus is the coverage
   metric.
4. **Validity is checked, not assumed.** `ir::check` runs after every read and every pass,
   `ir::check_legal` before every write. Invalid IR never reaches a writer, so "garbage out" is
   attributable to whoever produced the IR.
5. **Dialects never import each other.** A layering test over `use` lines fails the build on a
   forbidden edge (§2.3). With it, a diff confined to `src/dialect/xyce/` cannot change an output
   whose source and target are both not Xyce; the golden gate checks exactly that ("blast radius",
   §7.2).

### 2.3 Module map (owners and the DAG)

```
crates/ir/   package `spicerack-ir`, re-exported as `spicerack::ir`. Owns every IR table: Builder,
             typed rows, check, features, IR text (dump/parse), neutral expressions + reference
             evaluator, the diagnostic rows and catalogue, a test-only generator. No dependency.
src/
  front/     dialect-agnostic syntax toolkit: tokens, tokenizer(Lexicon), numbers, CST parser,
             expression parser → ir::Expr, kit walk. Today's `syntax::{token, tokenize, scan,
             number, expr, parse, lexicon, resolve}`, used in place until the old writers are
             retired, then renamed. Imports ir.
  passes/    IR → IR legalization, one transform per file + the pass table. Imports ir.
  dialect/   mod.rs: traits Read/Write, the BUILTIN table, transpile/read/write spines.
    ngspice/ hspice/ xyce/ ltspice/ spectre/ vacask/   reader + writer + tables each
    espice.rs rspice.rs        flavours of ngspice (data only; the one allowed sibling import)
    cdl.rs verilog.rs          writers only
    spice/                     helpers shared by SPICE-card dialects (free functions, no dialect
    named/                     parameter); likewise for Spectre/VACASK statements
  backend/ simulation.rs rawfile.rs result.rs measure*.rs normalize.rs unit.rs veriloga.rs lint.rs
             unchanged; they take deck text + run hints read from the IR
  python/    PyO3: builders → ir::Builder; netlist → write; results unchanged
  bin/       spice2rack (CLI), golden (record / diff / census / meaning-diff)
```

Dependency direction: `spicerack-ir` ← `front`, `passes` ← `dialect/*` ← crate root, `python`,
`bin`. `backend` and friends depend on `ir` (run facts) only. Two enforcements:

- `ir` is its own crate (`HIERARCHY.md` reason 1, single ownership): its tables are private, a
  reader gets `&mut Builder`, a writer `&Design`, a pass the edit API, so a stray write from a
  dialect does not compile. It also cuts the build where the edits land: the crate has no
  dependency (no pyo3), builds in seconds, and Schemify can depend on it alone.
- Everything else stays in the `spicerack` crate, with `tests/layering.rs` (a forbidden-edge list
  over `use crate::…` lines): a dialect importing a sibling other than its declared parent, or a
  pass importing a dialect, fails the build.

### 2.4 Caller code first

```rust
// 1. Text in, text out (ESPice through spice2rack, D55)
let out = spicerack::transpile_text(text, "hspice", "espice")?;   // Written{text, notes} | Diags

// 2. A kit (Interoperate pdk-models). The caller owns file I/O: requests, no callback.
let mut src = Sources::new();
let mut todo = vec![entry];
while let Some(path) = todo.pop() {
    let text = my_load(&path)?;
    todo.extend(spectre.read.references(&text).into_iter().map(|r| r.resolve(&path)).filter(|p| !src.has(p)));
    src.add(path, text);
}
let mut design = ir::Design::new();
spectre.read.read(&src, &mut design.builder(), &mut diags);
for def in design.definitions() { /* kind, name, file, ports: the model Interoperate lacks today */ }
for file in design.files()     { ngspice.write.write(&design, file, &mut out, &mut diags); }

// 3. Built in Rust (Schemify): no text anywhere
let mut b = design.builder();
let inv = b.subckt("inv", &["a", "y", "vdd", "vss"]);
b.within(inv).mosfet("mp", ["y", "a", "vdd", "vdd"], "pfet").param("w", 1e-6);
b.vsource("vdd", ["vdd", "0"]).dc(1.8);
b.analysis(Tran { stop: 1e-9.into(), ..Tran::default() });

// 4. Python: Subcircuit/Circuit/Testbench call the same builder; tb.netlist("xyce") is `write`.
```

Tiers: `transpile_text` (diagonal) decomposes into `read`, `check`, `legalize`, `write`
(orthogonal). `Sources::load(entries, loader)` is the convenience twin of loop 2. The caller
owns every buffer (`&mut String` out, a reusable `Design`).

## 3. The IR

### 3.1 Principles

1. **Meaning, not spelling.** Values are `f64` or expression trees; devices are kinds with
   terminal roles and named fields; waveforms are structs with named fields (`Sffm{offset,
   amplitude, carrier, mod_index, signal}`), never "the card's values in the card's order".
2. **A pinned normal form** (§3.3): defaults, units, operator meaning, grouping, argument order,
   name identity. One node per meaning, the most specific one: a `POLY` source stays `Poly`;
   lowering it to an expression is a pass, not a reader's choice. Two meanings behind one keyword
   are two nodes (`Pow` with C's sign rule and `PowAbs`, ngspice's `|x|**y`; `Ln` and `Log10`;
   SFFM's two laws).
3. **Readers and writers recognize and spell; they never convert.** On its own dialect a reader
   and its writer are inverse maps between text and IR nodes. Every conversion between meanings
   (`PowAbs(x, y)` → `Pow(Abs(x), y)`, a cycle-counted pulse → a PWL) is a pass (§4.4): neutral
   code with its own oracle. This is what makes law L5 exact and keeps dialect code dumb.
4. **Owned and self-contained.** Interned names, arenas, typed `u32` handles. It outlives the
   source text, is built without text, and has a lossless text form (§3.6).
5. **Order is data.** One ordered item list per scope (card order is matrix order and result
   order). The writer never reorders; a target that needs loads first hoists exactly those.
6. **Provenance, not dependence.** Each item has a span (file, byte range) in a cold table, used
   only to anchor diagnostics.
7. **Identity is the interned name.** A reader of a case-folding dialect hands the builder one
   spelling per name (the first seen), so two IR names that differ are two things, as in Spectre.
   A target whose folding would merge two names gets a rename pass, not a wrong netlist.
   References to definitions carry the name and the resolved definition id.

### 3.2 Tables (the six questions, answered once here; frozen in the Definition phase)

In: appends from a reader or a builder. Out: linear scans per scope by writers and passes,
lookups by handle. Count: kits reach ~10⁶–10⁷ statements (sky130's combined library and the 190
files it reaches); every handle is `u32`, files `u16`. Lifetime: one arena per `Design`, freed
whole. Parallelism: none inside a design (decks are parallelized by the caller, D11).

| Table | Row | Notes |
|---|---|---|
| `names` | interned bytes, `NameId` in first-seen order; `NameId(0)` is ground | deterministic ids |
| `files` | path, role (deck/library), root scope | caps as today (`FILES_MAX` 4096, depth 16) |
| `scopes` | kind (top, subckt, section, branch), parent, ordered `Vec<ItemRef>` | `ItemRef = {kind: u8, index: u32}` |
| one table per item kind | a `Copy` row of handles and scalars, no heap; lists are ranges into shared pools (`params`, `name_lists`, `expr_lists`) | existence-based: an item is a row in its kind's table. Elements share a header (kind, name, terminals, model, params, multiplicity) and index a detail table only when the kind has typed fields (passives, sources, controlled sources, lines) |
| `exprs` | `{tag: u8, a: u32, b: u32}` SoA; a number's bits live in `(a, b)` | the row technique of today's `Ast`; children have lower ids, so trees are acyclic by construction |
| `defs` | kind, name, scope, item | subckts, models (+bins), params, funcs, sections, modules |
| `spans` (cold) | file, start, len per item | diagnostics only |

Typed rows per kind rather than hand-packed words: fields are compiler-checked, writers read a
whole row at once, and adding a kind makes the compiler list every `match`. Every row struct has
a compile-time size assertion; every table a named capacity. Dialects reach the tables only
through `ir::Builder` and `&Design` accessors.

### 3.3 Content and normal form

The construct inventory (`INV-constructs.md`, built from the code and the seven research dossiers:
about 440 rows in twelve areas) is the coverage checklist. Each row maps to an IR node, a
normal-form rule, or a named `Opaque` class; a row with no home is a hole in the IR. The node set:

| Area | Nodes |
|---|---|
| Structure | `Title`, `Comment`, `Include{file, section?}`, `Section`, `Subckt{ports, params}`, `If{branches}`, `Global`, `Load{VerilogA \| Osdi}`, `Opaque` |
| Definitions | `Param`, `Func`, `Model{family, version, polarity, params, bins}`, `Statistics{process, mismatch, correlate, truncate}` |
| Circuit | `Element{kind, terminals, model?, fields, params}`; kinds: R, C (incl. charge-defined), L, coupling, V/I source, port, four linear controlled sources, `Poly`, `Table`, `Behavioural`, `Laplace`, diode, BJT, MOSFET, JFET, MESFET, V/I switch, lossless line, lossy/coupled lines, code model, Verilog-A module, subckt instance, instance of an unresolved master |
| Sources | `dc`, `ac{mag, phase_deg}`, wave: `Pulse`, `Sin`, `Exp`, `Pwl` (absolute times), `Sffm`, `Am`, `TrNoise`, `TrRandom`; port and parasitic fields |
| Expressions | the neutral language of `DESIGN-neutral-expressions.md` §2 as trees: numbers, parameters, probes `v(n[,m])`/`i(dev)`, `time`/`temp`/`freq`, one closed function enum with one meaning each, statistical draws as their own node, user function calls |
| Run | `Analysis` (closed enum, typed parameters: op, dc with axes, ac, tran, noise, tf, sens, pz, sp, hb, pss family, four, Monte Carlo…), `Sweep`/`Step`, the run plan of `DESIGN-control-epochs.md` (epochs and loops), `Option` (portable key or simulator-private), `Env{temp, tnom}`, `Ic`, `Nodeset`, `Save`, `Print`, `Measure` |

Normal form, by example. Each line replaces a pair rule or a behaviour switch that exists today
with one read rule and one write rule that never meet:

| Today (`pair.rs` / `Behaviours`) | Reader of the dialect that has it | IR states | Writer of any target |
|---|---|---|---|
| `sffm_swap` (ngspice `VO VA FM MDI FC`, others `VO VA FC MDI FS`) | maps its positions to named fields | `Sffm{…}` by name | orders the fields its own way |
| `log_to_ln`, `log_to_log10` | Xyce `log` → `Log10`; the rest `log` → `Ln` | two functions | spells each its own way |
| `hspice_elsewhere` (TNOM/TEMP 25 °C) | HSPICE sets `tnom=25, temp=25` when the deck is silent | `Env` always explicit | writes it when it differs from its own default |
| `caret_reread` (`^` power or XOR) | resolves `^` to `Pow` or `Xor` | the operator meant | spells it, or the legalizer refuses `Xor` |
| `spell_clamp`, `limit` two meanings | `limit(x,lo,hi)` → `Clamp`; ngspice `limit(nom,var)` → `Draw` | two nodes | `min(max())`, `limit`, … |
| `nint_to_xyce/ngspice` | `nint` → `RoundHalfEven` or `RoundHalfAway` | two functions | its own natively; the other through a pass |
| power law and grouping (`2**3**2`, `(-2)**3`) | parses with its own associativity; ngspice's `**` is `PowAbs`, the others' `Pow` | a tree; `Pow` (C's sign rule), `PowAbs` | minimal parentheses for its own grammar; the node it lacks through a pass |
| `device_ic_in_op` (Xyce applies `IC=` in the OP, ngspice only under UIC) | sets `ic.applies = InOp \| UicOnly` | the mode | has the mode, or pass `ic_to_dot_ic` runs |
| `r_model_multiplies` (Xyce model `R` scales, ngspice's is the resistance) | lowers to `r_scale` or `r_default` | two distinct parameters | spells its own; pass folds the other |
| `pulse_as_pwl`, `sin_cycles_as_b` | fills `ncycles` | optional field | feature `PulseCycles` or pass `pulse_cycles_to_pwl` |
| duplicate `.param` (ngspice/HSPICE last wins, Xyce first) | keeps the winner by its own rule | one definition | nothing to do |
| `Numbers::{Read,Given}`, ngspice arithmetic | ngspice reads with `INPevaluate` (kept: `unit::ngspice_value`) | the `f64` the source simulator computes | spells the shortest text its simulator reads back to that `f64` (`unit::ngspice_spelling`, kept) |
| LTspice PWL `+t` relative times | accumulates | absolute times | — |
| `xyce_m_rename`, ESPice ignoring `m` on X | lowers multiplicity to `Instance.mult` | a field, not a parameter named `m` | spells it; ESPice's writer lacks the feature |

The full list (every "silent wrong number" row of ledger Phase 3 and the settle files, with its
evidence and side) is `rewrite/INV-semantics.md`; it becomes the "Normal form" chapter of the IR
reference and each rule gets one reader fixture and one writer fixture per dialect it touches.

### 3.4 `Opaque` and strictness

`Opaque{dialect, text, class}` is first class. Classes follow the ledger's final verdicts:
`Unread` (the reader has no rule yet: work to do), `Private` (only its own simulator has it: Xyce
`YADC`, HSPICE `.stateye`), `Inert` (changes no number), `InvalidInSource` (its own simulator
rejects it; today's `P0010`/`P0011`), `Script` (data-dependent control flow, D1). `read` takes a
policy: lenient (kits, corpora) keeps them; strict (`from_spice`, D26) turns `Unread` into an
error. The zero gate Z1 becomes "no `Unread` in any corpus".

### 3.5 Features

`Feature` is a closed enum: one entry per IR variant or optional field that some target cannot
spell (today's 47 `Construct`s re-keyed on IR nodes instead of source constructs). Each writer has
one `const FEATURES: FeatureSet`. A test demands, for every writer and every feature it lacks,
either a registered pass that removes it or a citation row (target manual or source) in
`citations.toml`: the Z2 gate, by construction. The support matrix in the docs is generated from
these tables.

### 3.6 IR text: dialect zero

The IR has a canonical, lossless, hand-writable text form, implemented as a dialect through the
same traits. It is the format of reader fixtures (expected IR written by hand from a manual
clause), of `--dump-ir`, and of the Python store (replacing "ngspice text behind a header line").
Law: `parse(dump(x)) == x` exactly, on generated designs.

## 4. The contract

### 4.1 Traits

```rust
/// One row per language. CDL and Verilog have no reader.
pub struct DialectRow {
    pub name: &'static str,
    pub read: Option<&'static dyn Read>,
    pub write: Option<&'static dyn Write>,
}

pub trait Read: Sync {
    /// The files this text names (includes, library calls, loads). No I/O.
    fn references(&self, text: &str) -> Vec<Reference>;
    /// Every statement of every file becomes IR items, or an `Opaque` item with a coded
    /// diagnostic. Total: never panics on input, never drops a statement.
    fn read(&self, src: &Sources, out: &mut ir::Builder, diags: &mut Diags);
}

pub trait Write: Sync {
    /// The IR features this dialect spells.
    fn features(&self) -> &'static FeatureSet;
    /// Spells one file of a design that is legal for `features()` (asserted on entry).
    /// Total over legal IR: every item is spelled, or copied if it is this family's `Opaque`.
    fn write(&self, design: &ir::Design, file: FileId, out: &mut String, diags: &mut Diags);
}
```

Dispatch happens once per file, never per statement; inside a dialect everything is concrete
code. A plugin dialect (Rust, or a Python class adapted to these two traits) is another row.

### 4.2 Laws every dialect must pass

One suite, `tests/dialect_laws.rs`, iterates the registry. `Gen(F)` is the shared generator of
random valid designs restricted to feature set `F` (seeded, shrinking, in `ir`, test-only).

| Law | Statement | Finds |
|---|---|---|
| L1 totality | any bytes → `read` returns; every statement is accounted for as items or `Opaque` (statement count in = out) | crashes, dropped text |
| L2 validity | `ir::check(read(x))` reports nothing | reader making ill-formed IR |
| L3 determinism | same input → same ids → same bytes; 1 job = N jobs | order dependence |
| L4 writer totality | for `x ∈ Gen(features(d))`, `write_d(x)` emits no refusal | a writer lying about its features |
| L5 round trip | for `x ∈ Gen(features(d))`, `read_d(write_d(x)) ≅ x` | any reader/writer asymmetry in `d`, with no simulator |
| L6 corpus fixpoint | for each corpus text `t` of `d`: `read_d(write_d(read_d(t))) ≅ read_d(t)` | the same, on real decks (14,938 inputs today) |
| L7 feature honesty | `features_used(x) ⊆ features(d)` after `legalize`; a lacked feature has a pass or a citation | wrong refusals, missing passes |
| L8 isolation | output depends on `(design, file)` only: permuting unrelated files and registry order changes nothing | hidden inputs |
| L9 semantics | for each IR fixture with reference numbers, `sim_d(write_d(x))` matches within the oracle's tolerance (where a simulator exists; else the manual pin, D43) | a writer spelling the wrong meaning |

`≅` is equality of the canonical IR text with spans dropped and names compared under `d`'s case
rule. L5 is the oracle the current architecture cannot have (there is no IR to compare).

### 4.3 Anatomy of a dialect, and the sharing rule

`dialect/<name>/`: `lex.rs` (the `Lexicon` row, as today), `read.rs` (spine: CST statement →
lowering function, by a dispatch table) + `read_<area>.rs`, `write.rs` (spine: one exhaustive
`match` over item tags, no `_` arm) + `write_<area>.rs`, `tables.rs` (device rows, option names,
model family ↔ level/master, parameter names), `features.rs`.

Sharing follows semantic compression, not design up front: ngspice is written concretely; when the
second SPICE-card dialect needs the same function it moves to `dialect/spice/` **only if it takes
no dialect parameter and no flag**. A difference between two dialects is two functions in two
dialect directories, selected by each dialect's own dispatch table, never an `if` inside a shared
one. ESPice and RSpice are ngspice flavours: a constant struct handed to ngspice's reader and
writer (the one declared sibling import).

### 4.4 Passes

`passes::legalize` walks a static table `{removes: Feature, requires: FeatureSet, run: fn(&mut
Design, &mut Diags)}`; for each feature the design uses and the target lacks it runs the pass, or
records the refusal `Unsupported(feature)` at the items that carry it. Passes know features, never
dialects. First set, each lifted from code that exists today as token fixups or writer branches:

| Pass | Removes | Today |
|---|---|---|
| `stats_to_draws` / `draws_to_stats` | `Statistics` block / statistical draw calls | `render/draws.rs`, `named/lift.rs`, `read/unlift.rs` |
| `expand_parasitics` | element parasitics (`Rser`, `Cpar`, …) | `spice/parasitics.rs`, `named/parasitics.rs` |
| `pulse_cycles_to_pwl`, `wave_to_behavioural` | `Pulse.ncycles`, `Sin.ncycles`, out-of-domain `Sffm`, `Am` | `read/wave_rules.rs`, `spice/waves.rs` |
| `startup_ramp` | `Tran.startup` | `Rule::StartupRamp` |
| `table_to_expr` | `Table` source | `spice/table.rs` |
| `ic_to_dot_ic` | `ic.applies = InOp` | `spice/initial.rs` |
| `unroll_script` | statically bounded script loops | `read/unroll.rs` |
| `hoist_nested_subckts`, `rename_collisions` | nested definitions; names the target would merge or cannot spell | `read/blocks.rs`, `render/names.rs` |
| `inline_stat_funcs` | functions whose body draws | `Fix::Inline` |

Target-specific *forms* of the same meaning (ngspice writing a run plan as a `.control` script,
Xyce as `.STEP DATA=`, HSPICE as `.alter`) are writer spelling, not passes.

### 4.5 Diagnostics

Coded rows, as today (`Diag{code, span, arg}`; rendered once, sorted by position). The catalogue
is re-cut by owner instead of by work stream (today's W0140–W0399 blocks are per agent stream):
`L`/`P` reader syntax, `R` resolution, `S` statement read but not representable (the `Opaque`
classes), `X` invariant violations (a bug in us, never the user's deck), `U` unsupported feature on
the target (the feature rides in the row; its citation comes from the writer's table), `W` writer
notes. Each owner keeps its own table in its own numeric range (the IR crate: `X`, `U`, `S`; each
dialect: its `P`, `R`, `W`), and a test checks the assembled catalogue for collisions, so no two
owners append to one file. 0.2.0 is the only release and D24 freezes codes at 1.0, so this is the
last moment to renumber; a map file (old code → new code) keeps the differential harness honest,
and the facade maps back to today's codes until retirement (§7.2).

## 5. Seams and change drills

| Seam | Axis: cases today | Rung | Residue | One more case costs |
|---|---|---|---|---|
| S1 dialect | 9 built-ins + RSpice asked + plugins decided | 8: trait objects in a static table (7 would do without runtime plugins) | 2 indirect calls per file | 0 bodies: a directory + 1 table row |
| S2 IR kinds (device, wave, analysis, item) | dozens | 4: closed enum, exhaustive `match` | none | compiler-listed arms: 1 per writer, plus check, dump, parse, generator |
| S3 target capability | 9 writers | 3: `const FeatureSet` per writer | none | 1 row |
| S4 lexical rules | 6 lexicons | 3: `const Lexicon` rows (kept) | none | 1 row |
| S5 file loading | disk, memory, wasm, Interoperate's store | 6: request (`references`) answered by the caller | none | 0 |
| S6 legalization | ~12 passes | 3: pass table + spine loop | 1 call per needed pass per design | 1 row + 1 file |
| S7 SPICE-family sharing | 4 card dialects | 2: free functions over plain data, no dialect parameter | none | 0 |
| S8 simulators | 5 backends (registry exists) | as today | — | — |

Drills, from your message, the ledger and the decision log:

| ID | Change | Source | Edits | Radius |
|---|---|---|---|---|
| D1 | Add RSpice | your message | `dialect/rspice.rs` (flavour constants, `FEATURES`), `backend/rspice.rs`, 1 row in `BUILTIN`, 1 in the backend registry, fixtures. Laws L1–L9 run on it unasked | 0 |
| D2 | A construct every dialect lacks today (two-tone HB; Pareto: `.hb` 132 decks) | ledger P5/P6 | `ir`: variant, `Feature`, check, dump/parse, generator (IR owner). Each writer: 1 arm (the compiler lists all 9) + feature or citation row. Each reader that has it: 1 function + 1 dispatch row | 9 arms, compiler-listed |
| D3 | A wrong number in one dialect (SFFM order; ledger Phase 3 has 119 such rows) | ledger P3 | 1 function in `dialect/<d>/read_sources.rs` *or* `write_sources.rs`; blast-radius gate proves no other pair moved | 1 |
| D4 | A simulator release changes a rule (ngspice fixes its R/C/L → B rewrite) | `ngspice-source.md` §1 | 1 function in `dialect/ngspice/`, its fixture | 1 |
| D5 | A new Python builder keyword | ledger P8 | `src/python/<x>.rs`, docs, skill. Core untouched when the IR node exists | 0 in core |

Today D2 and D3 each touch the shared family spines plus a `pair` field plus capability cells plus
the Python mirror tables; D1 is a `DialectDef` that can only select existing paths.

## 6. Inventory: what is kept, reorganized, rewritten, deleted

Sizes are non-blank lines on 1059821. The full inventories are in `rewrite/` on the `ir-rewrite`
branch (`INV-constructs.md`, `INV-semantics.md`, `INV-tests.md`, `INV-python.md`), beside
`IR-KERNEL.md` (the conventions of the IR crate).

### 6.1 Code

| Area | Lines | Verdict |
|---|---|---|
| `syntax/{token, tokenize, scan, number, expr, parse, spice, named, fields, ast, device, statement}` + `lexicon/` | ≈5.6k | **Keep** as the readers' private CST (`front`). The 14,938-input token gate already proves it. `Lexicon` rows stay plain data |
| `syntax/resolve` (kit walk, declare, lookup) | 2.3k | **Reorganize**: the walk becomes `Sources`; declare/lookup become the readers' first pass; answers land in `Design.defs` instead of side tables keyed by node |
| `syntax/render/read` (views, placement, fixups) | 11.8k | **Rewrite** as per-dialect lowering, CST → IR. Placement rules of the old reader and token fixups are not carried |
| `syntax/render/{spice, named}` + `render/*.rs` | 18.4k | **Rewrite** as writers (spelling only) and passes (conversions) |
| `syntax/{def, dialects, behaviour, lang, build, check}`, `render/{capability, def_pass, spelling}` | ≈6.4k | **Delete**. What they hold moves into each dialect's directory as plain constants |
| `syntax/diag` | 1.4k | **Rewrite** in `spicerack-ir`, catalogue re-cut by owner, with a map file |
| `src/python`, deck half (builders, analyses, deck assembly, store, the 80 `format!` sites, the 12 text sniffers) | ≈6.1k | **Rewrite** onto `ir::Builder`: no text, no dialect logic, no sniffing |
| `src/python/def_*`, `python/spicerack/dialects.py` | 1.2k | **Delete** with the `DialectDef` plugin API (§10.4) |
| `src/python` results, export, errors, simulators, units | ≈2.0k | **Keep** |
| `backend`, `simulation`, `rawfile`, `result`, `measure_parse`, `normalize`, `unit`, `veriloga`, `lint` | ≈6.5k | **Keep**. Their line scanners over the written deck (`.control` contents, `.dc` fields, VACASK `analysis` names, the two marker comments) are replaced by run facts handed over beside the text: analyses in run order with kinds and written names, measure names in deck order, the ngspice compatibility mode, outer-sweep values |
| `python/spicerack` (86 testbench functions, remeasure, export, diagnostics) | 6.3k | **Keep**; method names follow D22/D26 when the binding moves |
| `bin/spice2rack`, `bin/golden` | 1.7k | **Keep**, re-pointed. `golden` gains `--pipeline old\|new`, `meaning-diff`, the `Opaque` census |

About 47k lines are in scope and an estimated 30k replace them. The number that matters is not
the total: it is that each rule has one home.

### 6.2 Entry points (the rewrite-clean inventory)

| Entry | Consumer | Status | After |
|---|---|---|---|
| `render::render_text`, `render_kit`, `import::kit`, `references`, `relink` | CLI, tests (33 Rust test files), Interoperate | live | `transpile_text`, `read` + `write`, `references`; one release of the old names behind them |
| `syntax::{tokenize, parse, lexicon::of, ast::Node, resolve::Kit, device::Primitive}` | Interoperate `pdk_scan`/`pdk_models` (pins `main` f7fbf3b), 14 test files | live, but a leak | Interoperate queries `Design` (definitions, kinds, files) instead |
| `spicerack::ir::*`, `codegen::xschem::*` | Schemify (pins 0fc42bd) | live there, deleted at head | `ir::Builder` + `write`; layout per §10.5 |
| `syntax::def::*`, `lang::*`, `render_pending`, `splice`, `card_fields`; Python `spicerack.def_` (114 methods, 45 classes) | tests and docs only; no example, no testbench, no outside consumer found | **unknown → dead** unless you say otherwise | a plugin implements `Read`/`Write` |
| CLI `spice2rack --dialect <in> --to <out>` | ESPice (D55) | live | same flags, plus `--dump-ir`, `ir` as a dialect on both sides |
| Python API (182 builder methods, 15 result classes) | users, 91 testbench names | live; only 17 snapshot lines are frozen, version 0.2.0 | same surface with the pending decisions (D15, D16, D18, D22, D25, D26, D48) landing with the new core |
| Formats: raw files, `spicerack.measures/1`, `spicerack.diagnostics/1`, the store ("ngspice text behind a header line") | users | live | unchanged, except the store becomes IR text, and diagnostics locate by element and Python call site instead of a line in a text the user never wrote |
| Simulators, PDKs, corpora pinned in `flake.nix` | tests | live | unchanged |

## 7. Test plan

### 7.1 Bug class → oracle → layer

| Bug class | Oracle (never the code's own method) | Layer |
|---|---|---|
| reader misreads a statement | spec example: expected IR text derived by hand from the manual clause, with a refusal twin | reader fixtures, per dialect, per clause |
| reader drops or duplicates text | accounting: statements in = items + `Opaque` out | L1 on corpora, fuzzed |
| ill-formed IR | invariants | `ir::check`, always on |
| writer changes meaning | the real simulator on IR fixtures with closed-form numbers; manual pins where no simulator runs (D43 order) | writer fixtures, L9 |
| reader and writer of one dialect disagree | inverse: `read_d ∘ write_d` on generated IR and on corpora | L5, L6 (no simulator, runs at merge) |
| a pass changes meaning | a simulator that runs both forms; the reference evaluator for expression rewrites | pass tests |
| the IR lacks a fact (same IR, two meanings) | cross-simulator numbers | the existing cross-sim lists (12 lists, 7,881 lines) |
| expression semantics | reference evaluator `neutral::eval` + generated expressions on every simulator (`DESIGN-neutral-expressions` N1–N4) | expression suite |
| regression against the old pipeline | differential in meaning space (§7.3) | rewrite harness, until retirement |
| wrong refusal | a citation per lacked feature; the simulator accepting or rejecting the written deck | L7, citations test (Z2) |
| a change leaking across dialects | path-scoped golden diff | blast-radius gate (§7.2) |
| nondeterminism, deep nesting | byte compare 1 job vs N; 10⁴-deep fixtures in Debug | existing gates, kept |
| result-side misreads, API contract | closed forms; API snapshot | existing tests, kept |

### 7.2 What exists and carries over

The seam is narrow. One function, `render::render_text(text, from, name, to) -> Result<Written,
Diags>`, is behind 43 of the 53 integration test binaries (81 call sites, most through
`tests/common/oracle.rs`); `render_kit` + `import::kit` carry the kit tests and the cross-sim
harness. Keep that facade (signatures, `Written`, `Diags{diags, messages}` and its line format,
`Code`, `Dialect`, the CLI contract) and route inside it, pair by pair:

| Asset | Size | Carries over |
|---|---|---|
| Integration binaries | 53 (546 tests) | 41 unchanged, 4 with one helper changed, 8 go with the internals they test (`test_syntax`, `test_rust_api`, the six dialect-definition binaries) |
| Tests named by ledger row | 307 (175 Rust `p<phase>_<nnn>_*`, 132 Python) | all: each has a simulator, closed-form or manual oracle. They are the executable form of the research and the porting checklist: red on the new pipeline until the rule is re-homed |
| Python test files | 30 (657 functions; 814 pass) | 29; `test_dialect_def.py` goes with the plugin API |
| Unit tests inside `src/syntax` | 238 | 188 are welded to today's tree shape; their clause + input pairs become reader fixtures (expected IR instead of a printed AST) |
| Cross-sim harness and strict lists | 13 lists, 7,881 lines; 154 `mismatch` + 213 `target-fails` are the open writer-side lines | all, re-blessed per flip (strict both ways: a deck the rewrite fixes goes red until its line is removed) |
| Corpus fixpoints, simulator suites, Spectre/HSPICE pins (22 + 21 byte-pinned decks) | — | all (§7.3 for the byte pins) |
| Dropped with the code they test | — | the definition parity tests (`builtin_dialects.py`, 1,687 lines), `migration.xfail` (4,103 entries), `api_parity.py` |

Two cautions from the inventory. "Carries over" is not "passes unchanged": many of these tests
assert exact lines, so **spelling conventions are inherited**: where a choice is arbitrary
(spacing, number form, keyword case, parameter order) the new writer writes what the old one
wrote. Byte equality with the old output is then the common case and a cheap first check; it is
never the requirement. And during the transition the facade maps the new diagnostic codes back to
today's (82 distinct codes are pinned by 39 binaries), so the tests are renamed once, mechanically,
at retirement.

What today's suite cannot see, and the IR makes testable: no test reads a dialect and asserts a
meaning (every reader is observed through some writer's text, so matching bugs cancel); no writer
test is free of a reader; 43 of the 54 source × target cells have no correctness oracle (36 are
sameness only); nothing is generated. Each of the 36 factors into "reader A to IR" and "IR to
writer B", tested once each. One live defect found on the way: `tests/api_parity.py` (about 1,000
parity cases and the only store round trip) matches no pytest pattern and has never run in the
gate.

New: the law suite (§4.2), reader and writer fixtures, pass tests, and the **blast-radius gate**:
`golden diff` takes the dialect directories a commit touched (`git diff --name-only`) and fails if
any output changed whose source and target are both outside them. A change under `crates/ir`,
`passes` or `front` may move anything and runs the full gate.

### 7.3 The oracle for the switch: the old pipeline, compared by meaning

Per `rewrite-clean`: the old code stays callable (`--pipeline old`, tag `oracle-ir0`) until every
pair is flipped. The harness compares, per (input, target):

1. **status**: written or refused, and the refusal class through the code map;
2. **meaning**: `read_B(old text) ≅ read_B(new text)`, no simulator, available as soon as dialect
   B has its reader;
3. **numbers**: both texts on simulator B (nightly; a fixed subset at merge).

Old against old runs first and must be green. That run is also the strongest acceptance test a new
reader can get for free: it must read every deck the old writer of its dialect produced (164k
machine-written files) with no `Unread`.

Every difference is a line in `REWRITE.md` (old behaviour, new behaviour, evidence) or it is a
bug. Most of ledger Phase 3 ends up there as "old output was wrong; new matches the simulator".

Why meaning and not bytes: D24 says netlist bytes are not frozen, and byte identity is the gate
that welded the last rewrite to the old structure. Inside the new pipeline the house rule holds
again: once a pair is flipped its golden record is the baseline, and refactors keep it identical.

**The byte-pinned Spectre evidence.** 21 decks are pinned byte for byte because real Spectre
(21.1, 25.1) ran exactly those bytes, one pin is keyed by a hash of the written text, and the host
is gone (D43). Re-blessing them would keep the tests green and void what they stood for. Rule: the
pinned bytes stay in the tree as the evidence; the new Spectre writer either reproduces them
(spelling is inherited, so this is the expected case) or its text must read back, through the
Spectre reader, to an IR equal to the pinned deck's. The claim then is "the same Spectre circuit
real Spectre ran", judged by a reader that is itself tested against the manual, and it is said
that way in the test.

### 7.4 Triage: which component is wrong

For a wrong result on deck D, A → B (each step is one `spice2rack` invocation):

1. `--to ir`: dump the IR after `read_A`. `check` reports → **reader A**.
2. Is the dump what A's manual says D means (compare with the nearest reader fixture)? No →
   **reader A**.
3. `--from ir --to A`, read back, compare (L5 on this deck). Differs → **dialect A**, reader or
   writer, nobody else.
4. `--dump-ir=legal --to B`: the IR after each pass. `check_legal` reports, or a pass fixture
   disagrees → **that pass**.
5. Write B from the legal IR, run simulator B against the IR's reference numbers → **writer B**.
6. All green and simulator A ≠ simulator B → **the IR lacks a fact**, or the simulators differ
   (with proof). Add the normal-form rule and one fixture on each side.

### 7.5 Tiers and exit criteria

| Tier | Budget | Contents |
|---|---|---|
| pre-commit | < 10 s | fmt, style, `cargo test -p spicerack-ir`, layering |
| merge | < 15 min | laws at moderate counts for every dialect, fixtures, pass tests, row tests, pytest, golden record + blast radius + determinism, corpus fixpoints |
| nightly | hours | cross-sim lists, differential numbers, high-count properties, L1 fuzzing |

A pair A → B flips to the new pipeline when, compared by name and never by count: the laws are
green for A and B; every row test that names the pair is green; the pair's cross-sim failures are
a subset of the old pipeline's; the differential has no unledgered difference; `Unread` on the
pair's corpus is no more than the old W0001 + W0032. The old code is deleted one release after
the last pair flips and the Python API runs on the IR.

## 8. Execution

### 8.1 Slices (each runs Plan → Definition → Testing → Implementation, each phase gated)

| # | Slice | Seam | Gate to leave it |
|---|---|---|---|
| S0 | Pin and catalogue: tag `oracle-ir0`; `rewrite/RULES.md`: every ledger row, pair rule, behaviour, capability cell and `quirk` assigned a home (reader:d, normal form, pass, writer:d, run, python) or `drop` with a reason. It grows slice by slice: each slice's Plan phase takes its own rows | — | at the flip: no ledger id without a home (a script compares the id lists); old-vs-old differential green |
| S1 | `spicerack-ir`: tables, builder, check, features, IR text, expressions + evaluator, generator, diagnostics | `Builder`, `&Design` | definitions approved; tests red on stubs; then `parse(dump(x)) == x` on generated designs, each invariant has a violating fixture that is caught, 10⁴-deep expressions in Debug, throughput measured against today's 58 MB/s parse |
| S2 | Spine and seam: `Read`/`Write`, the registry, `transpile`, `passes::legalize` (empty table), the route table inside `render_text`/`render_kit` (the facade the tests already use; new diagnostics mapped back to today's codes), `spice2rack --pipeline`, the law suite, `tests/layering.rs`, the harness | the two traits; the facade | suite runs (and fails) on a stub dialect; route table sends every pair to the old pipeline; the whole existing gate is green and byte-identical |
| S3 | ngspice: reader + writer | traits | laws; spice-bench (6,050 decks): `ngspice(write(read(d)))` reproduces every plot and `.meas` of `ngspice(d)` (today's import-suite oracle, entry point swapped); differential ng → ng |
| S4 | VACASK: reader + writer, and the passes ng ↔ va needs | traits | laws; VACASK corpus + `cmos180n.lib`; cross-sim ng ↔ va ⊇ old. The second family comes second on purpose: it stresses the IR before it sets around SPICE |
| S5 | Xyce (second card dialect: `dialect/spice/` is compressed out of ngspice here) | traits | laws; 4,068 decks; cross-sim ng ↔ xy ⊇ old |
| S6–S10 | HSPICE, Spectre, LTspice, flavours (ESPice, RSpice), CDL + Verilog writers | traits | laws; own corpora and pins; ESPice reading the HSPICE writer's output stays an oracle |
| S11 | Python on the IR: builders, store as IR text, typed measures and probes, the pending API decisions | `ir::Builder` | pytest green; the conformance table (112 features × 9 targets) re-keyed on features |
| S12 | Run facts: backends take facts from the IR beside the text; scanners and marker comments go | `write` result | simulator suites green with the scanners deleted |
| S13 | Flip pair by pair (§7.5), then retire the old code and rename what is left of `syntax` to `front` | route table | no reference to `syntax::render`; docs, `AGENTS.md`, `TESTING.md`, `ARCHITECTURE.md` describe the new shape |

Passes are written inside the slice that first needs them. S3 is the tracer: after it the IR has
carried real decks end to end and you can read dumps before the fan-out.

### 8.2 Who works where

After S2, one owner per directory and no shared file: `crates/ir` (one owner; everyone else
sends a request: construct, manual clause, proposed node), `passes`, one per `dialect/<name>`,
`src/python`, the harness. S4–S10 and S11 run in parallel. Compare the ledger's shared-file
table, where three streams append to the same spines under rulings.

### 8.3 Phases for the agents

1. **Definitions** (S1 definition phase + S0 catalogue): the IR crate with every type and
   signature and `todo!()` bodies; `RULES.md`. Gate: it compiles, I review it against the
   inventories, you approve the types.
2. **The IR, working** (S1 testing + implementation; S2).
3. **Tracer** (S3, then S4).
4. **Fan-out** (S5–S12).
5. **Flip and retire** (S13).

## 9. Risks

| Risk | Mitigation |
|---|---|
| The normal form misses a fact and two simulators disagree silently | It is built from the 119 wrong-number rows and the settle files, which already carry evidence; each rule gets a fixture per side; cross-sim remains the net for the rest |
| Fixes of the last week get lost | Their 307 tests are the porting checklist; the differential and the cross-sim subsets gate each flip |
| The IR churns under parallel dialect work | Two families (ngspice, VACASK) go end to end before the fan-out; one IR owner; requests, not edits |
| The old architecture keeps moving while the new one is built | Freeze it (§10.1). The oracle is a tag; anything landed later is re-landed as a rule |
| Same-dialect output no longer byte-equal to its input | By decision (§10.5); meaning is held by L6 and by bit-identical simulator runs on PDK corners (tests that exist) |
| Spectre and HSPICE have no simulator here | As today (D43): pins, then manual clauses, then the equivalent deck on VACASK or ngspice. Their L5/L6 need no simulator |
| Speed on PDK-size kits | Budget stated in S1's gate and measured with `benches/perfbench.rs`; typed tables, interned names, no per-item allocation |
| The evidence is outside the repository | Under `/tmp/claude-1000/spicerack-golden/`: the ledger, dossiers and settle files (2.4 MB), the evidence tree (`research/impl/evidence/`, 207 directories, cited from 108 files in the tree), the golden baseline (2.7 GB). None is tracked. The four inventories are committed under `rewrite/`; the rest needs a durable home before anything is gated on it |

What stays hard whatever the architecture: `.control` scripts, model parameter tables per
simulator, and targets with no simulator to run. The plan gives each one home; it does not make
them small.

## 10. Decisions

Defaults are what I proceed on; say so to change one.

1. **Freeze the row team on the old architecture.** A session is landing batches on
   `p7-integrate` now. Every translate, refusal or wrong-number row landed there is implemented
   twice. Harness, oracle and backend rows are still worth landing. Default: the oracle is pinned
   at 1059821.
2. **Base.** Branch `ir-rewrite` from `p7-integrate` 1059821 in its own worktree
   (`../SpiceRack-ir`); `main` untouched; nothing pushed.
3. **Dialects.** Read + write: ngspice, VACASK, Xyce, HSPICE, Spectre, LTspice, in that order
   (simulators that run here first). Flavours of ngspice: ESPice, RSpice (JaimeHW/RSpice; not in
   the code today, its grammar is looked at in its slice). Write only: CDL, Verilog. Your list
   left LTspice, CDL and Verilog out; they exist and have users (Schemify), so they stay unless
   you drop them.
4. **The `DialectDef` plugin API is not ported.** 114 methods and 45 classes that mirror the
   internals being deleted, used by tests and docs only. A plugin dialect becomes an
   implementation of `Read`/`Write` (Rust now, a Python adapter after 1.0). The simulator registry
   (D38's other half) stays.
5. **Bytes.** Same-dialect output is not byte-identical to its input, and outputs differ from
   today's. Schemify's xschem parity: it builds the IR in xschem's order (order is data) and
   re-blesses its 126 fixtures once, or hands pre-spelled lines as `Opaque` of the target family.
6. **D8 is reversed.** `ir::Builder` is public Rust API (unstable until 1.0): readers need it
   anyway and Schemify is waiting for it.
7. **Diagnostic codes are renumbered once**, by owner, with a map file, before the 1.0 freeze.
8. **Python decisions land on the IR.** D15 (builders build the IR), D16 (expression strings are
   parsed in the neutral grammar at the Python edge), D26 (`from_spice` is `read` in strict
   mode). `add_spectre_analysis` (D19) becomes typed analyses. Diagnostics locate by element and
   Python call site; a job id hashes the canonical IR text; `measure(...)`, `save(...)` and probe
   strings are parsed at the edge into typed specs.

