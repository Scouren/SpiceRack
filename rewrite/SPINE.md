# Slice S2: the spine and the seam

The contract every dialect implements, the pipeline that calls it, and the place where the old
pipeline and the new one meet. Read `REWRITE-PLAN.md` §2, §4, §7.2–§7.5 first. This slice adds no
dialect except the IR's own text; when it lands, every existing test still runs through the old
pipeline, byte for byte.

## 1. Workspace

- Root `Cargo.toml`: `[workspace] members = ["crates/ir"]`; dependency `spicerack-ir = { path =
  "crates/ir" }`; dev-dependency the same with `features = ["gen"]`. `crates/ir/Cargo.toml` loses
  its own `[workspace]` table and `crates/ir/Cargo.lock` is deleted (one lock at the root).
- `src/lib.rs`: `pub use spicerack_ir as ir;` and `pub mod dialect; pub mod passes;`.
- The no-default-features build (`cargo build --no-default-features`, wasm32) still builds: the
  new modules have no I/O. `nix build .#spice2rack` and the wheel still build (the flake's source
  must include `crates/`; no vendored dependency changes, so no hash moves).

## 2. The contract (`src/dialect/mod.rs`)

```rust
/// Text of one dialect → the IR.
pub trait Read: Sync {
    /// The files this text names (includes, library calls, loads). No I/O.
    fn references(&self, text: &str) -> Vec<Reference>;
    /// Every statement of every file of `src` becomes IR items, or an `Opaque` item with a coded
    /// diagnostic. Total: never panics on input, never drops a statement, never learns a target.
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags);
}

/// The IR → text of one dialect.
pub trait Write: Sync {
    /// The IR features this dialect spells.
    fn features(&self) -> &'static ir::FeatureSet;
    /// Spells one file of a design that is legal for `features()` (asserted on entry). Sees
    /// only the IR. Total over legal IR: every item is spelled, or copied when it is this
    /// family's `Opaque`.
    fn write(&self, design: &ir::Design, file: ir::FileId, out: &mut String, diags: &mut ir::Diags);
    /// Appends a refusal for everything in `design`, legal by `features()`, that this dialect
    /// cannot spell for a reason no feature states (a model parameter its simulator's module
    /// does not have, a name it cannot quote). Never for something a feature could state.
    /// Default: nothing (AMEND-A12 S1, R21).
    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {}
}

/// One language. CDL and Verilog have no reader.
pub struct DialectRow {
    pub name: &'static str,
    /// The dialect whose `Opaque` text this one's writer may copy (itself, or its parent:
    /// ESPice copies ngspice's).
    pub text_family: &'static str,
    pub read: Option<&'static dyn Read>,
    pub write: Option<&'static dyn Write>,
}

pub static BUILTIN: &[DialectRow];              // the IR's own text first; dialect slices append
pub fn by_name(name: &str) -> Option<&'static DialectRow>;
```

`Sources` is the caller-owned set of files a read works on: `(path, text)` pairs, the entries,
and whether the entries are decks or libraries. `Sources::one(path, text)`; `add`, `has`;
`Sources::load(entries, form, reader, &mut loader)` is the convenience that walks `references`
breadth first (depth 16, 4,096 files, the confinement rule of today's `resolve::Kit::load`). A
reader never touches a file system.

## 3. The spines (`src/dialect/mod.rs`)

```rust
pub fn read(src: &Sources, from: &DialectRow, policy: Policy, design: &mut ir::Design, diags: &mut ir::Diags);
pub fn legalize(design: &mut ir::Design, to: &DialectRow, diags: &mut ir::Diags);
pub fn write(design: &ir::Design, file: ir::FileId, to: &DialectRow, diags: &mut ir::Diags) -> String;
pub fn transpile(src: &Sources, from: &DialectRow, to: &DialectRow, policy: Policy) -> Result<Written, ir::Diags>;
```

- `read` = the dialect's `read`, then `ir::check`. A violation is a reader bug: an `X` row, and a
  debug assertion.
- `legalize` = once per (design, target), in place (the caller clones first to keep the neutral
  design or to write a second target): every `Opaque` of another text family is dropped with a
  note when its class is `Inert` (it moves no number) and refused otherwise (a `U` row at the
  item); then `passes::legalize` with the target's features, whose `ir::check_legal` reports
  what no pass removed; last, always, the writer's `admits`, so the caller sees every refusal at
  once. The whole design is legal for the target, or it is refused: a file the caller loaded is
  a file the caller converts.
- `write` = the dialect's `write` of one file of a design `legalize` accepted for that target.
  Handing it a design that is not legal is the caller's bug: a debug assertion
  (`features_used ⊆ features`, no foreign `Opaque`), never a second legalization. A kit is
  `read`, one `legalize`, one `write` per file.
- `transpile` = `read`, `legalize`, `write` of the entry file, and the verdict: any
  `Kind::Refusal` row refuses the deck, and a step is not run after a refusal. `Policy::Strict`
  turns an `Opaque` of class `Unread` into a refusal; `Lenient` keeps it (a writer of the same
  text family copies it).
- Every branch on which work happens is in these four functions.

## 4. Legalization (`src/passes/mod.rs`)

```rust
pub struct Pass {
    pub name: &'static str,
    /// The feature it removes.
    pub removes: ir::Feature,
    /// What the target must have for the pass's output to be legal.
    pub requires: ir::FeatureSet,
    pub run: fn(&mut ir::Design, &mut ir::Diags),
}
pub static PASSES: &[Pass];                     // empty in this slice
pub fn legalize(design: &mut ir::Design, target: &ir::FeatureSet, diags: &mut ir::Diags);
```

`legalize` loops: the features the design uses and the target lacks; for each, the first pass
that removes it and whose `requires` the target has; run it; `ir::check`; repeat until nothing
changes (bounded by `PASSES.len() + 1` rounds, asserted). What is left is refused: one `U` row
per item that carries the feature. A pass imports `crate::ir` only. One pass per file, tested on
its own (IR text in, IR text out, and the reference evaluator or a simulator that runs both
forms).

## 5. The seam: the old entry points route (`src/dialect/facade.rs`, two call sites)

`syntax::render::render_text` and `render_kit` keep their signatures and types. First thing, each
asks `dialect::route(from, to)`:

- `ROUTES`: a constant table of (source name, target name) pairs that go to the new pipeline.
  Empty in this slice. A pair may be listed only when both dialects are in `BUILTIN` with the
  side it needs.
- `SPICERACK_PIPELINE=old|new` overrides the table for the harness (`new` only where the new
  pipeline has the pair; otherwise the old one runs and says so in a note on stderr once).
- A routed call runs `transpile` and converts the result to today's types: `render::Written {
  text, notes }`, `render::Diags { diags, messages }` with message lines `<name>:<line>:<col>:
  <CODE> <title>` computed from the spans, and each new code mapped to today's code through one
  table (`facade::OLD_CODE`: `(new code, feature, target) → syntax::diag::Code`). The table is
  filled by the dialect slices as they flip pairs; a code with no row maps to itself where the
  letter and number exist in the old catalogue, else to a designated row that fails loudly in
  the tests that pin codes (so a missing mapping is visible, never silent).
- The Python binding, the CLI, `golden` and every test keep calling the old entry points. This
  is the whole strangler.

## 6. The IR's own text as a dialect (`src/dialect/irtext.rs`)

`Read` = `ir::parse` per file; `Write` = `ir::dump`; `features` = everything. Name `ir`, file
suffix `.ir`. It makes every stage runnable from the command line and is the first dialect the
laws run on.

## 7. CLI (`src/bin/spice2rack.rs`)

`--pipeline old|new` (default: the route table); `ir` accepted by `--dialect` and `--to`;
`--dump-ir` writes the IR text after `read` instead of a deck, `--dump-ir=legal` after
`legalize` for the `--to` target. Exit codes, stdout and file suffixes unchanged.

## 8. Tests this slice adds

- `tests/dialect_laws.rs`: for every row of `BUILTIN`, the laws of REWRITE-PLAN §4.2 that apply
  to its sides (L1–L8; L6 over a corpus only under `--features corpus`; L9 belongs to each
  dialect's own simulator tests). Generated designs come from `ir::gen::generate(seed,
  row.write.features(), size)`; seeds are fixed and printed on failure. L4 reads: `write` is
  total over a design `legalize` accepted; a generated design `legalize` refuses (`admits`) is
  skipped and counted, and the law fails when no admitted design holds an item kind the
  writer's features claim. In this slice the only
  row is `ir`.
- `tests/layering.rs`: reads every `use crate::…` line under `src/` and fails on a forbidden
  edge: a file under `src/dialect/<d>/` importing another dialect (except a declared parent),
  `syntax::render`, or `passes`; a file under `src/passes/` importing anything but `ir`; a file
  under `src/dialect/spice/` or `src/dialect/named/` naming a dialect. The rule table is data at
  the top of the test.
- `tests/test_spine.rs`: `transpile` over the `ir` dialect on hand-written IR text (a deck with
  a subcircuit, an element, an analysis); a refusal when `PASSES` cannot remove a feature a stub
  writer lacks; `Policy::Strict` refusing an `Unread`; the facade's message format; the route
  override.

## 9. Gate to leave the slice

The whole existing gate is green and `golden diff` against the current baseline is byte-identical
(nothing is routed yet); the law suite passes on `ir`; `layering` passes; `spice2rack --dialect ir
--to ir` is a fixpoint on the worked example of `crates/ir/src/text.rs`.
