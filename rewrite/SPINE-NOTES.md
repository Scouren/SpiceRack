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
7. Verdict → `dialect::info` looks a code up in the spine's catalogue (`S0101..=S0199`), then the
   IR crate's; `refuses` is "its row is a refusal, or no catalogue holds it" → a missing row fails
   loudly. Dialect slices add their `P`/`R`/`W` tables to `dialect::info` when they land.
8. `Policy::Strict` → `dialect::read` pushes `STRICT_UNREAD` (`S0101`, the spine's own table, ruling 1)
   at each `Opaque` row of class `Unread` (scanning the `Opaque` table). `S0001` (the reader's note)
   stays a note.
9. Text family (R11) → `dialect::legalize` (once per design and target, in place) walks
   `Design::reachable()`: another family's `Opaque` of class `Inert` is removed from its listing
   scope with an `INERT_DROPPED` note (`S0102`), any other class is refused (`U0001`, `arg` =
   `Feature::Item(Opaque)`) at the item; `ANY_FAMILY = "*"` (the IR text writer) skips the step.
10. `write` (R11) → calls the writer only; legality is the caller's contract (debug assertions:
    features inside the writer's set, no foreign `Opaque`). `transpile` = read → refused? → legalize →
    refused? → write of the entry. The facade's `render_kit` keeps today's one-file-per-call shape:
    one read and one legalize per call.
11. `legalize`'s leftovers (ruling 2) → passes run to the fixpoint, then `ir::check_legal(design,
    target)` appends one `U0001` per (item, feature) at the item: that is the leftover report. `write`
    calls the writer only when nothing is left.
12. `legalize` stops when a round changes the missing set not at all (and refuses), bounded by
    `PASSES.len() + 1` rounds (asserted) → "until nothing changes" without spinning a no-op pass.
13. Facade policy → `Policy::Lenient` → today's `render_text` keeps foreign text as notes (`W0001`).
14. Facade anchors → a span's `file` indexes `Sources` in add order (a reader of a SPICE-family dialect
    creates its files in that order, stated on `Read::read`); a row with no span or a file out of range
    prints `name:0:0:`; every row is named `name` (today's messages name one deck). Old `Diag::token`
    holds the span's first byte (there are no tokens).
15. Messages → `name:line:col: CODE title` with the *old* code and the *old* catalogue's title for an
    `OLD_CODE` row (the line reads as today's does); `W0203|W0204 <new code> <new title>` otherwise. Columns are bytes from 1, as `Tokens::location`.
16. `OLD_CODE` row → `{code, feature: Option, target: Option<&str>, old}` (`None` = any), first match
    wins; a row exists only where a routed test asserts an old code. No row → `W0203` (refusal) or
    `W0204` (note) by the new code's verdict, whose message is `W0203 <new code> <new title>` (ruling
    3; the two rows are in `syntax/diag/rows_w_def.rs`, beside W0201/W0202 of the same shape). No
    same-number fallback: codes are renumbered, so a match would be a wrong code with no crash.
17. `SPICERACK_PIPELINE` → read once per process (`OnceLock`); a value other than `old`/`new` is said
    on stderr and ignored; the "no such pair" note prints once per process. `route_with` is the pure
    form the tests use.
18. CLI `--pipeline` → sets `SPICERACK_PIPELINE` for the process before the first render (single
    thread at that point) → the old entry points keep routing themselves; no second code path.
19. CLI `ir` sides and `--dump-ir` → a separate `convert_new` (new pipeline only); with `--kit` or
    `--check` it is a usage error (exit 2). `--dump-ir` writes every file of the design as IR text to
    `<stem>.ir` and prints its path; `--dump-ir=legal` needs `--to` with a new-pipeline writer.
    A side the new pipeline lacks (e.g. `--dialect ngspice --dump-ir`) fails the deck (exit 1).
20. `IrText::features` → `&ir::FeatureSet::ALL`.
21. The IR reader reads each `Sources` text with `ir::parse`; the text's own `file` lines name the
    design's files (the `Sources` path is not used) → IR text is self-describing.
22. L1's "statement count in = out" → not asserted for `ir` (no statement counter the IR exposes);
    L1 asserts the read returns, L2 that `check` adds no `X` row, and the empty-text case that ""
    and "\n" read as one file with no item and no row. L6's corpus for `ir` is a kernel-kinds text
    and the worked example of `text.rs` (`--features corpus`). L8's "registry order" is vacuous
    with one row.
23. `Write::admits` (AMEND-A12 S1) → default empty; `dialect::legalize` calls it last; `write`'s debug
    assertions run it into a scratch list that must stay empty. L4 skips a generated design
    `legalize` refuses (any refusal) and fails when no admitted design holds an item kind the
    writer claims (checked over `Feature::Item` only; kinds outside the kernel's join when the
    generator builds them).
24. `OpaqueClass::Hint` (R19) is not in the kernel yet: `legalize`'s match over classes is exhaustive,
    so the member's arrival stops the build; then a spine note row (distinct from `INERT_DROPPED`:
    "the numbers may move within the solver's tolerance") and the drop.
25. The `Read` contract (lead's ruling on surprise 4) → a reader creates a file for every text of
    `src`, the entry first, even when empty. `IrText` meets it by opening an empty file at the
    `Sources` path when a text opens no `file` block (empty, comments only, or refused lines): an
    empty `.ir` is then one empty deck, not a panic in `Design::entry` (the lead's exception, "a text
    with no `file` line is a `P9001`", does not hold: an empty or comment-only text gives no row).
    `transpile` asserts an entry after a read with no refusal.

## Requests (ruled by the lead)

1. Strict code: the spine's own (`S0101`). Done.
2. Per-item `U` rows: `ir::check_legal` already reports per (item, feature). Done in `legalize`.
3. Unmapped codes: two old-catalogue rows, `W0203`/`W0204`. Done.
4. `FeatureSet::ALL`: asked of the kernel owner; swap the `OnceLock` when it lands.
5. `Design::reachable() -> Vec<(ItemRef, ScopeId)>`: asked of the kernel owner; swap the span rule of
   decision 9 when it lands.

## The IR bodies at 79af0b3b, against the spine's assumptions

Merged into `ir-spine`; the kernel's kinds behave as the spine assumed in signature and verdict
(`check_legal` reports one `U0001` per (item, feature) at the item's span, `arg` the feature
index; `ir::diag::info` answers the kernel's codes). Differences found:

1. `ir::parse` leaves every span's `file` at `NONE` (items and `P9001` rows alike); the offsets are
   into the text handed to `parse`, which may hold several `file` blocks. The facade then prints
   `name:0:0:` for every row read from IR text. Request: `parse` sets `span.file` to the file block
   the line is in (or the spine needs the text's index to map it).
2. `TEXT_SYNTAX` (`P9001`) is a `Refusal`: one malformed IR line refuses a lenient read. Its span is
   the word that failed (or an empty span at the end of the line), not the line.
3. `ir::parse` of an `opaque … unread` line pushes no `S0001` note: in IR text the class is data,
   not a reader finding. The spine's strict rule scans the `Opaque` table, so it is unaffected.
4. A text with no `file` line yields a design with no file: `Design::entry` then panics ("a design
   with a file has an entry"). `transpile` is safe only because the `P9001` refusal stops it before
   `entry`; a dialect reader must create a file even for an empty text.
5. A repeated port (`subckt s (a a)`) is a `P9001` with nothing pushed, as `item_text.rs` says; an
   unresolved name (`param p = q + 1`) is no row (legal, C8).

## The IR at cf98e0d4

6. `Design::entry()` is the first `Deck` file, else file 0, not "the first file". A kit read with a
   library as its entry (`facade::render_kit` on an included file) whose walk reaches a deck file
   would write that deck instead. Today's walk follows the entry's own references only, so a deck
   is reached only when a library includes one. Either `transpile` writes file 0 (the `Read`
   contract puts the entry first), or `entry()` states the case; the spine keeps `entry()` until
   ruled.

## Open

- Done at cf98e0d4: `spice2rack --dialect ir --to ir` (and `--dump-ir`) is a byte fixpoint on the
  worked example of `text.rs`; `the_worked_example_is_a_fixpoint_of_the_ir_dialect` passes.
- Waiting for the kernel: `span.file` set by `parse` (then a routed IR-text diagnostic prints its file
  and line; test `a_routed_ir_text_diagnostic_names_its_line` carries the marker), a total
  `Design::entry()`, `OpaqueClass::Hint` (decision 24).
