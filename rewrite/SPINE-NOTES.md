# Slice S2 notes: decisions where SPINE.md is silent, and open points

Each line: question → choice → why.

## Decisions

1. `passes` was `src/passes.rs` while it held no pass (check_style.py refuses a one-file folder);
   with `deck_title` it is `src/passes/mod.rs` + one file per pass, as SPINE §4 says.
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
    (zero bytes, R46; "\n" is each dialect's own fixture) reads as one file with no item and no
    row. L6's corpus for `ir` is a kernel-kinds text
    and the worked example of `text.rs` (`--features corpus`). L8's "registry order" is vacuous
    with one row.
23. `Write::admits` (AMEND-A12 S1) → default empty; `dialect::legalize` calls it last; `write`'s debug
    assertions run it into a scratch list that must stay empty. L4 skips a generated design
    `legalize` refuses (any refusal) and fails when no admitted design holds an item kind the
    writer claims (checked over `Feature::Item` only; kinds outside the kernel's join when the
    generator builds them).
24. `OpaqueClass::Hint` (R19, kernel b32ee486) → another family's hint is dropped from its scope with
    the spine's `HINT_DROPPED` note (`S0103`: "a solver hint of another dialect was dropped"; the
    numbers may move within the target solver's tolerance), distinct from `INERT_DROPPED`; its own
    family keeps it.
25. The `Read` contract (lead's ruling on surprise 4) → a reader creates a file for every text of
    `src`, the entry first, even when empty. `IrText` meets it by opening an empty file at the
    `Sources` path when a text opens no `file` block (empty, comments only, or refused lines): an
    empty `.ir` is then one empty deck, not a panic in `Design::entry` (the lead's exception, "a text
    with no `file` line is a `P9001`", does not hold: an empty or comment-only text gives no row).
    `transpile` asserts an entry after a read with no refusal.

26. Each file its own text (law harness) → every row, `ir` included: `write_all` puts each file of a
    design in one `Sources` at the design's path, the entry first (its role setting the form,
    `Sources::set_form`), and the row reads them back. IR text needs no exception: each dumped
    file is a standalone `file` block, so reading the files one text each rebuilds the same
    design. `Read::names_its_files` (a9b9f257) is therefore removed again: the smallest way to
    express the exception is none. The harness is the ngspice scaffold's (460dd86f/3e97620b) with
    mine on top (decision 28).
27. `deck_title` (the first pass) → `src/passes/deck_title.rs`, `pub fn deck_title`: an empty
    `Title` first in each `Deck` whose top scope has none, built with `Builder::title` and moved to
    the front of the list; no diagnostic. Tested as SPINE §4 says (IR text in, IR text out,
    nothing else moves, it gains only `Item(Title)` and loses exactly `Extra::DeckWithoutTitle`),
    and through `passes::legalize` for a target that lacks the feature. Registered in `PASSES`
    (`removes: Extra(DeckWithoutTitle)`, `requires: EMPTY`) since kernel 4ff01bbf.

28. `tests/dialect_laws.rs` reconciled → the ngspice scaffold's file (3e97620b) as the base: per-row
    laws checked against a strict `KNOWN` list of (law, row, why), L3/L5/L8 on legalized designs
    read back from one `Sources`, no generated `Opaque` for a row that is not lossless. On top:
    the full feature set (no kernel restriction), `admits` and the item-kind anti-vacuity check
    in L4, the empty-text L1 case (now a per-row reader law, `l1-empty`, through the same
    `KNOWN` check), the worked example in the L6 corpus, the entry first in `write_all`. `KNOWN`
    is empty here: this branch has only the `ir` row, and an entry for a row it lacks would fail
    as an unexpected pass. Each dialect branch keeps its own entries when it merges this.
29. Golden input set → `tests/fixtures/dialect/` is excluded from the fixtures walk
    (`inputs.rs`, `FIXTURES_EXCLUDED`, unit test `a_dialect_fixture_is_not_an_input_…`): those
    are the new pipeline's per-dialect fixtures, read by each dialect's own tests; the old
    pipeline never had them, so they are no golden input.

30. Pass seam for four pass agents (SPINE §4) → `passes::GROUPS` = structure (U0300–0399),
    expr (U0100–0199), waves (U0200–0299), run (U0400–0499), tried in that order; a group is
    `<group>.rs` while empty and `<group>/mod.rs` + one file per pass after (check_style.py
    refuses a one-file folder, so `structure.rs` + `structure/deck_title.rs` is not an option;
    `structure/` holds `mod.rs` and `deck_title.rs`). `legalize` now walks passes in that order
    (it walked features in index order before, which made the order of passes meaningless).
    `Pass` gains `notes` (the codes it may leave at an item; law P2 reads them). `passes::info`
    is chained into `dialect::info`. Codes letter `U`: a pass's note says why an item stays
    unsupported.
31. `deck_title.requires` → `Item(Title)`, not `EMPTY` as first ruled: law P3 found it (the pass
    adds a title; seed 1 shows it). A target with no `Title` cannot take its output.
32. `tests/pass_laws.rs`: P1–P5 per registered pass over 96 seeds of `generate(ALL)` (designs
    with the feature) and `generate(ALL − removes)` (without), plus the fixture runner over
    `tests/fixtures/passes/`; strict `KNOWN` by pass name; a law with no design to run on fails.
    "Items carrying the feature" are `check_legal(ALL − removes)`'s `U0001` rows for it; a note
    counts when it is one of `Pass::notes` at the same span. Planted bugs: a no-op `deck_title`
    turned P2 and the fixtures red; `requires: EMPTY` turned P3 red.

33. Pass fixtures state their notes (A26) → a sidecar `<case>.notes` beside `<case>.out.ir`: the
    codes the pass appended, one a line, in order (`U0101`), `#` comments. Absent means "the
    pass appends nothing"; a pass that appends a row with no sidecar fails, and so does a
    sidecar listing what it did not append. A sidecar over a header line: `.out.ir` stays the
    exact canonical text `dump` writes.
34. L1/L2 on card-shaped text → `tests/dialect_mutants.rs` (its own binary: `dialect_laws.rs` is at
    the 500-line cap), 32 seeded token mutants per fixture deck (`tests/fixtures/dialect/<row>/
    read/**`, the corpus under `--features corpus`): a `k=v` duplicated, a key renamed to another
    key of the deck, a token dropped, two tokens swapped, a line duplicated or moved. Each mutant
    is read by the row's reader and checked apart (not through `dialect::read`, whose debug
    assertion would hide the `X` code); a panic is caught; each distinct failure is reported once
    with its deck, seed and the mutant shrunk line by line. The `ir` row's decks are new:
    `tests/fixtures/dialect/ir/read/{worked,kernel}.ir`. Its first run found three IR-text
    reader holes (KNOWN): `ir::parse` pushes an element row `check` refuses instead of a `P9001`
    for a second element of one name (X0016), a key of another kind (`dc=` on a resistor,
    X0015) and a terminal count out of range (X0006).

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

## The IR at b32ee486

- Surprise 1 resolved (b8035c65): `parse` names each span's file block; a routed IR-text diagnostic
  prints `name:3:…` (`a_routed_ir_text_diagnostic_names_its_line` lost its marker).
- Surprise 4 resolved: `Design::entry()` is total (`FileId::NONE`); `transpile`'s assertion is now the
  reader-bug check it was meant to be.
- Surprise 6 resolved (4ff01bbf): `entry()` is file 0; `a_kit_whose_entry_is_a_library_writes_the_library`
  lost its marker. No marker is left in the spine's tests.
- A flaky test fixed (bf132d1b merge): two `test_spine` tests bumped one static counting writer in
  parallel. Each test now makes its own stub and row (`counting`, `picky`, leaked to `'static`);
  50 runs of the binary at `--test-threads 16`: 0 failures.
7. The facade maps `span.file` to the `Sources` entry of that index. For a reader that names its
   files (IR text) one `Sources` text holds several design files, so a row in the second `file`
   block of one text prints `name:0:0:` (its offset is into the first text, its file id 1). Today
   only `ir` is such a reader and the facade never routes it; a fix needs either `span.file` →
   the source text's index from the reader, or the facade asking `names_its_files` and using
   text 0.

## Open

- Done at cf98e0d4: `spice2rack --dialect ir --to ir` (and `--dump-ir`) is a byte fixpoint on the
  worked example of `text.rs`; `the_worked_example_is_a_fixpoint_of_the_ir_dialect` passes.
- Waiting for the kernel: `span.file` set by `parse` (then a routed IR-text diagnostic prints its file
  and line; test `a_routed_ir_text_diagnostic_names_its_line` carries the marker), a total
  `Design::entry()`, `OpaqueClass::Hint` (decision 24).
