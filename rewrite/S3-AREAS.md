# S3 areas: the stubs S3.0 leaves for S3.1–S3.4

S3.0 wrote both spines (`src/dialect/ngspice/read.rs`, `write.rs`) once; they name every area
function below, so S3.1–S3.4 fill the stub files and never edit a file S3.0 owns. Each stub's
`//!` header repeats its contract. Read `rewrite/DIALECT-NGSPICE.md` §3–§5 first.

## The stubs

| Sub-slice | File | Functions (the spine calls them) | Codes |
|---|---|---|---|
| S3.1 | `read_elements.rs` | `pub(super) fn element(lw: &mut Lower<'_, '_>, st: Stmt)`: every `Tag::Instance` whose letter is not `V`/`I` | `P1100–P1199`, `R1100–R1199` |
| S3.1 | `read_sources.rs` | `pub(super) fn source(lw: &mut Lower<'_, '_>, st: Stmt)`: a `V` or `I` card | `P1100–P1199`, `R1100–R1199` |
| S3.1 | `write_elements.rs` | `pub(super) fn element(w: &mut Out<'_>, item: ir::ItemRef)`: every `Element`; today it forwards `VSource`/`ISource` to `write_sources::source` | `W1100–W1199` |
| S3.1 | `write_sources.rs` | `pub(super) fn source(w: &mut Out<'_>, item: ir::ItemRef)` (called by `write_elements::element`) | `W1100–W1199` |
| S3.2 | `read_models.rs` | `pub(super) fn model(lw: &mut Lower<'_, '_>, st: Stmt)`: `Tag::Model` | `P1200–P1299`, `R1200–R1299` |
| S3.2 | `write_models.rs` | `pub(super) fn model(w: &mut Out<'_>, item: ir::ItemRef)` | `W1200–W1299` |
| S3.2 | `families.rs` | `pub(super) fn family(word: &str, level: Option<&str>) -> Option<ir::ModelFamily>`, `pub(super) fn card(family: ir::ModelFamily) -> Option<(&'static str, Option<u32>)>` (inverse on every family `card` spells) | — |
| S3.3 | `read_run.rs` | `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt)`: `Tag::Analysis`, `Options`, `Temp`, `Ic`, `Nodeset`, `Step`, and every `Tag::Card` dot card `read_structure::card` does not claim (it claims the stray closers and `.title`) | `P1300–P1399`, `R1300–R1399` |
| S3.3 | `read_outputs.rs` | `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt)`: `Tag::Save` (`.save`, `.probe`), `Print`, `Measure` | `P1300–P1399`, `R1300–R1399` |
| S3.3 | `write_run.rs` | `pub(super) fn analysis`, `option`, `env`, `node_set`, `statistics` (each `(w: &mut Out<'_>, item: ir::ItemRef)`) | `W1300–W1399` |
| S3.3 | `write_outputs.rs` | `pub(super) fn save`, `print`, `measure` | `W1300–W1399` |
| S3.4 | `read_control.rs` | `pub(super) fn control(lw: &mut Lower<'_, '_>, st: Stmt)`: a `.control` … `.endc` block (today one `Unread` for the whole block) | `P1400–P1499`, `R1400–R1499` |
| S3.4 | `write_control.rs` | `pub(super) fn load`, `loop_`, `alter` (each `(w: &mut Out<'_>, item: ir::ItemRef)`); `Load` is here (an OSDI load is `pre_osdi` in a `.control` block) | `W1400–W1499` |

Each area file also exports, and the stub declares empty:

- `pub(super) const FEATURES: ir::FeatureSet`: what the area adds; `mod.rs`'s `FEATURES` is the
  union of every area's set (`write_structure::FEATURES`, `expr_write::FEATURES` and the twelve
  stubs'). A reader area's set is the image of what it produces; its writer must spell exactly
  that (law L5). Build it with `tables::feature_set(&[…])`.
- `pub(super) const CODES: &[codes::Row]`: its rows (`Row = (ir::Code, ir::Info)`), each code in
  the area's hundred; `codes::RANGES` chains them and `codes.rs`'s test checks the ranges and
  that no code repeats. `dialect::info` reaches every row, so a new code is known to
  `dialect::refuses` the moment it is in a table.

## What an area function gets

Reader (`read.rs`): `Lower` holds `b: &mut ir::Builder` (the append door), `diags`, `names:
&mut Folder` (case folding across the read: `fold` for names, `node` for nets: `gnd` is `0`),
`ast`/`toks` (the CST of the file), `src`, `path`, `file`, `role`. `Stmt { node, scope,
container }`: the statement's CST node, the IR scope its items go into, the CST block holding it.
Helpers: `lw.opaque(st, class, code)`, `lw.unread(st)` (`Unread` + `P1001`), `lw.note(st,
code)`, `lw.framing()` (a statement with no item: `.end`), `lw.span(n)`, `lw.raw(n)` (the
statement's text, a block's body included), `lw.text(n)` (a field's tokens, one blank per gap),
`lw.rest(n)` (the head line after its first token), `lw.keyword(n)`, `lw.expr(st, &tree, scope)`
(appends a parsed expression and queues its parameter reads and user calls for the bind step),
`lw.number(lit, &fit)` (appends a card number from `number::Scan`'s `literal` and `fit`: a
`Fit::Long` one as its digits, A18; never `lw.b.literal` on a scanned number),
`lw.wins(st, def, name, k)` (the declare step's verdict), `lw.include_path(st, path)`, and
`lw.open(scope, body, container, closer)` to schedule a block's body. Free functions:
`expr_read::parse(text) -> Result<Tree, Fail>` (picks numparam or the B-source parser by the
text; `Tree::dynamic()` says which), `number::card(token)` (a card value, `INPevaluate` with
gobble), `number::scan(text, Grammar)` (a `Scan { literal, len, fit }`: `fit` is `Exact`,
or `Long` past 18 significant digits, A18; negate both with `Fit::negate`), `cst::{body, next, closer, line, extent, joined,
unquote}`.

Rules the spine asserts or relies on: each statement pushes at least one item into `st.scope`
or calls `lw.framing()` (L1, asserted per statement); validate the whole statement before the
first push (a refusal leaves no row: emit expressions only after every one parsed); never call
`ir::link` (R3).

Writer (`write.rs`): `Out` holds `design: &ir::Design`, `out: &mut String`, `diags`. Helpers:
`w.line(text)`, `w.open(scope, tail)` (writes a scope's items next, then `tail`). Free functions:
`expr_write::value(design, e, out)` (bare number or name, else `{…}`), `expr_write::expr`
(the expression alone, in the evaluator its tree implies), `expr_write::expr_in(design, e, eval, out)`
(in a given evaluator: a B source's field is always the B-source parser's),
`expr_write::spells_in(design, e, eval) -> bool` (every node spellable there; for `admits`), `number::spell(design, lit, Grammar, out)`
(a `Long` literal as its digits); read a design's literal with `design.literal_value` and
compare with `design.literal_eq` (`Literal::value`/`eq_value` panic on a `Long`),
`name::letter(kind)` (the letter of a kind whose letter does not depend on its detail).

## Open points the areas inherit (reported to the lead with the S3.0 report)

- A row `Ref` to a definition pushed later (an instance before its `.subckt`, a model in a later
  file) is bound in `read.rs`'s bind step, the one place that binds. An area pushes the row with
  its `Ref` unresolved and queues it: `lw.refer(item: ir::ItemRef, k: u32, scope: ir::ScopeId,
  kind: ir::DefKind, name: ir::NameId)`, `k` the slot in `RowFacts::map_refs` order, `name` as
  pushed (folded). After every file is lowered, `bind_rows` looks up the `kind` definition of
  `name` nearest `scope` outward (`Design::lookup`) and calls `Builder::bind_ref(item, k, def)`;
  no definition leaves it unresolved (R4). Each slot expects one kind (`map_refs` states it);
  queue that kind.
- More of `Lower` for the areas: `lw.subckt_formals(st, name) -> Option<Vec<String>>` (the folded head parameters of the
  `.subckt name` visible from `st`'s block, else a top-level one of an earlier file);
  `lw.measure_reads(mark)` with `mark = lw.pending()` taken before the `lw.expr` calls of a
  `.meas … param=` (those reads bind to a measure of the name first, then to a parameter).
- Element names (R23, AMEND-A12 §A13): the IR name is the whole folded name, letter included;
  `i(vdd)` is `I("vdd")` and every `i(<element>)` reads. The writer writes names as they are;
  `CircuitExtra::NameWithoutLetter` stays out of `FEATURES`.
- The evaluator class is `ir::expr_class` (A11, R14: transitive, `Temp` counts); the writer
  decides numparam or B source by it.

## Evidence: how S3.0 checked ngspice-45

- **Source.** The pinned simulator's source tarball is a nix store path:
  `nix-store -r /nix/store/515ksnwkf7z112xz6pifn1ixgr9p4bys-ngspice-45.tar.gz.drv` realises
  `/nix/store/b9hi97vj3s8y32h41a9gs6kzicvvg8yf-ngspice-45.tar.gz` (the `.drv` is the one
  `ls /nix/store | grep ngspice-45.tar.gz` lists); unpack it with `tar xzf … -C <scratch>`. The
  files S3.0 read: `src/frontend/numparam/xpressn.c` (`formula`, `fetchoperator`, `operate`,
  `fetchnumber`, `parseunit`, `mathfunction`), `src/spicelib/parser/inpptree-parser.y` (the
  B-source grammar), `inpptree.c` (`PTlex`, `PT_mksnode`, `funcs[]`, `constants[]`),
  `ptfuncs.c` (each B function), `inpeval.c` (`INPevaluate`), `src/frontend/inpcom.c`
  (`inp_fix_gnd_name`, `inp_fix_temper_in_param`, `inp_sort_params`).
- **Runs.** `nix develop .#testing` has `ngspice` (ngspice-45). A numparam value:
  `printf 'probe\n.param p={%s}\nV1 a 0 {p}\nR1 a 0 1\n.control\nop\nprint v(a)\n.endc\n.end\n' "$e" > d.cir; ngspice -b d.cir`;
  a B-source value with nothing folded: `Vc c 0 1` and `B1 b 0 V=<expr with v(c)>`, then
  `print v(b)`. Read the printed `v(a) = …` line; an error line (`Undefined parameter`,
  `out of range for ln`, `wrongly determined negation`) is the run's answer too. Every value
  pinned in `expr_tests.rs` and `tests/dialect_ngspice.rs` came from such a run.
