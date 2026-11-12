# Slice S2 notes: decisions where SPINE.md is silent, and open points

Each line: question → choice → why.

## Decisions

1. `src/passes/mod.rs` alone fails `check_style.py` (a folder holding one file) → `src/passes.rs`;
   it becomes `passes/mod.rs` with the first pass file → the house rule; `layering.rs` covers both forms.
2. `Sources` and `Reference` → `src/dialect/sources.rs` (spine file, re-exported from `dialect`) →
   keeps `mod.rs` one transform; `layering.rs` lists `mod`, `facade`, `sources` as spine, not dialects.
3. `Sources` entries → the first file `add`ed is the entry; `load` marks its `entries` (all leading);
   form is `ir::Role::{Deck, Library}` (no new enum in the API) → the caller code of §2.4 case 2 needs no
   extra call.
4. `Sources::load`'s loader → `&mut dyn FnMut(&str) -> Option<String>`; `None` leaves the path out (the
   reader sees a reference with no file) → no new error type; `Role::Missing` is the reader's to make.
5. `Reference` → `enum { File(String), VerilogA(String) }` with `path`, `resolve(from)`;
   `Sources::load` follows `File` only → Sources holds `(path, text)` pairs with no role, so a
   Verilog-A text would be read as netlist text. A dialect that needs modules adds a role then.
6. Path normalization → a local copy of `resolve::normalize` (it is `pub(super)` and `syntax/` is not mine
   to edit) → eight lines.
7. Verdict for codes no catalogue knows → `dialect::refuses` treats them as refusals → a missing
   catalogue row fails loudly. Dialect slices must make their `P`/`R`/`W` notes known here (a
   catalogue lookup per row, when the first dialect lands).
8. `Policy::Strict` → `dialect::read` pushes `STRICT_UNREAD` (`S9001`, the spine's own code) at each
   `Opaque` row of class `Unread` (scanning the `Opaque` table) → the IR catalogue has no refusal
   for it (request 1). `S0001` (the reader's note) stays a note.
9. Text family → the spine refuses another family's `Opaque` (`U0001`, `arg` =
   `Feature::Item(Opaque)`) before the writer runs; `ANY_FAMILY = "*"` marks the IR text writer,
   which spells every dialect's → one rule in the spine instead of one loop per writer. An `Opaque`
   counts for the file being written by its span's file (`Span::NONE` counts as that file).
10. `write` on any refusal (foreign opaque, features left after `legalize`) → the writer is not called,
    the text is empty → a writer only ever sees legal IR (rule 4). `transpile` does not write after
    a read that refused.
11. `legalize`'s leftovers → one `U0001` per feature at `Span::NONE` (not per item) → the IR has no
    per-item feature query (request 2). `ir::check_legal` then runs only when nothing is left and must
    add nothing (a debug assertion: a pass bug).
12. `legalize` stops when a round changes the missing set not at all (and refuses), bounded by
    `PASSES.len() + 1` rounds (asserted) → "until nothing changes" without spinning a no-op pass.
13. Facade policy → `Policy::Lenient` → today's `render_text` keeps foreign text as notes (`W0001`).
14. Facade anchors → a span's `file` indexes `Sources` in add order (a reader of a SPICE-family dialect
    creates its files in that order, stated on `Read::read`); a row with no span or a file out of range
    prints `name:0:0:`; every row is named `name` (today's messages name one deck). Old `Diag::token`
    holds the span's first byte (there are no tokens).
15. Messages → `name:line:col: CODE title` with the *old* code and the *old* catalogue's title → the
    line reads as today's does. Columns are bytes from 1, as `Tokens::location`.
16. `OLD_CODE` row → `{code, feature: Option, target: Option<&str>, old}` (`None` = any), first match
    wins; no row → the old code of the same letter and number; else `UNMAPPED = R0042` ("the simulator
    registry is full", which no transpile raises) → the old catalogue has no designated row and is not
    mine to edit (request 3).
17. `SPICERACK_PIPELINE` → read once per process (`OnceLock`); a value other than `old`/`new` is said
    on stderr and ignored; the "no such pair" note prints once per process. `route_with` is the pure
    form the tests use.
18. CLI `--pipeline` → sets `SPICERACK_PIPELINE` for the process before the first render (single
    thread at that point) → the old entry points keep routing themselves; no second code path.
19. CLI `ir` sides and `--dump-ir` → a separate `convert_new` (new pipeline only); with `--kit` or
    `--check` it is a usage error (exit 2). `--dump-ir` writes every file of the design as IR text to
    `<stem>.ir` and prints its path; `--dump-ir=legal` needs `--to` with a new-pipeline writer.
    A side the new pipeline lacks (e.g. `--dialect ngspice --dump-ir`) fails the deck (exit 1).
20. `IrText::features` → every feature, built once at run time (`OnceLock`) from
    `Feature::from_index` → `FeatureSet` has no `ALL` const (request 4) and its `with` is `todo!()`
    today, which a `const` would evaluate at compile time.
21. The IR reader reads each `Sources` text with `ir::parse`; the text's own `file` lines name the
    design's files (the `Sources` path is not used) → IR text is self-describing.
22. L1's "statement count in = out" → not asserted for `ir` (no statement counter the IR exposes);
    L1 asserts the read returns and L2 that `check` adds no `X` row. L6's corpus for `ir` is the
    worked example of `text.rs` (`--features corpus`). L8's "registry order" is vacuous with one row.

## Requests for the IR crate's owners (not added: `crates/ir/src/` is theirs)

1. A refusal code for a strict read of an `Unread` item (today the spine uses `S9001`).
2. Per-item features: `item_features(&Design, ItemRef, &mut FeatureSet)` (or `check_legal` reporting at
   items), so `legalize` can put one `U` row per item that carries a feature (SPINE §4).
3. (Old catalogue, `syntax/diag`) a designated row for an unmapped new code, instead of `R0042`.
4. `FeatureSet::ALL` (a `const`), for writers that spell everything.
5. A walk of every item of one file (all nested scopes), so the family check and diagnostics can
   attribute an item to its file without its span.

## Open

- The gate item "`spice2rack --dialect ir --to ir` is a fixpoint on the worked example" cannot run until
  the IR bodies land (the test `the_worked_example_is_a_fixpoint_of_the_ir_dialect` carries the marker).
