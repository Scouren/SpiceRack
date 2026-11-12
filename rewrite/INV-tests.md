# INV-tests: test and oracle assets that can gate the IR rewrite

Read-only inventory of `SpiceRack-merge` (branch `batch-22`, HEAD `1059821`; the last full gate ran
on `5c8c64f`). Nothing was built or run. Counts come from `grep`/`wc` over the tree and from the
gate's own logs in `/tmp/claude-1000/spicerack-golden/` (`batch-22*.log`, `batch-19.out`).
Where a fact is not written anywhere I say `not stated`. Where I infer, I say so.

Verdict words: **KEEP** (runs unchanged against a new implementation that keeps the facade of §1.3),
**ADAPTER** (one small helper changes, the test bodies stay), **REWRITE** (the oracle idea is
reusable, the assertions are on today's internals), **DROP**.

Three qualifiers used in the tables:
- *format*: asserts exact written bytes or whole lines, so a writer that spells the same deck
  differently fails it.
- *codes*: asserts diagnostic codes by number.
- *sameness*: the expected value is the previous implementation's output, not an independent oracle.

---

## 1. Entry points tests use

### 1.1 Rust entry points, by test binary

There are 53 integration test binaries (48 top-level `tests/*.rs` + 5 directory crates). A binary is
counted when it reaches the item directly or through a helper it includes (`tests/common/oracle.rs`
is included by 23 binaries, `tests/common/mod.rs` by 2, `tests/common/acme.rs` by 3).

| Entry point | Binaries | Notes |
|---|---|---|
| `spicerack::syntax::render::render_text(text, from: impl Into<Lang>, name, to: impl Into<Lang>) -> Result<Written, Diags>` | 43 | 81 call sites; the one function behind `oracle::{write, written, refusal, spec_example, spec_refusal, sim_pair, closed_form}` |
| `spicerack::syntax::Dialect` (enum, `ALL`, `name()`) | 48 | 1,464 uses |
| `spicerack::syntax::diag::Code::<number>` | 39 | 281 references, 82 distinct codes (§6) |
| `spicerack::syntax::Lang` constants (`NGSPICE` 63, `NGSPICE_GIVEN` 20, `SPECTRE` 12, `XYCE` 7, `HSPICE` 7, ...) | 14 name one directly | the 23 `oracle.rs` users accept one through `impl Into<Lang>`; `test_steps` passes `NGSPICE_GIVEN` that way |
| `spicerack::result::{RawData, VarInfo, VarType, MeasureResult}` | 36 | outside `src/syntax` |
| `spicerack::backend::Dialect::{run, run_all}` (`backend::Dialect` re-exports `syntax::Dialect`) | 33 | runs a simulator process; 23 of them only through `oracle.rs`'s LTspice path |
| `spicerack::rawfile::{parse_raw, parse_raw_all, write_raw}` | 30 | 24 of them via the harness `sim.rs` |
| harness `sim::run` (`tests/test_cross_sim/sim.rs`: its own process launch, not `backend`) | 24 | via `oracle.rs` |
| `spicerack::unit::*` (`parse_spice_number`, ...) | 27 | via `cmp.rs`, `sections.rs` |
| `spicerack::syntax::diag::{info, Kind}` | 13 | `info(c).kind`; titles in 1 binary, `cite` in 1 |
| `spicerack::syntax::resolve::{FileId, Form, Role, Kit}` | 9 | kit handles |
| `spicerack::syntax::{tokenize, parse}` called directly | 8 | `test_dialect_kit`, `test_dialect_plugin`, `test_ngspice_analyses`, `test_rust_api`, `test_syntax`, `test_verilog_a`, `test_spectre_golden`, `test_vacask_runs` |
| `spicerack::syntax::def::{DialectDef, register}` | 6 | `dialect_defs`, `test_acme_oracles`, `test_dialect_full`, `test_dialect_kit`, `test_dialect_plugin`, `test_rust_api` |
| `spicerack::syntax::render::render_kit(kit, FileId, name, to)` | 6 | `dialect_defs`, `test_dialect_kit`, `test_spectre_import`, `test_verilog_a`, `test_xyce_write`, `test_cross_sim` |
| `spicerack::import::kit(entries, Form, lang, root) -> Kit` | 6 | `dialect_defs`, `test_rust_api`, `test_sections_pdk`, `test_spectre_import`, `test_verilog_a`, `test_cross_sim` |
| `spicerack::simulation::{run, run_all, run_each}` | 6 | outside `src/syntax` |
| backend registry (`Sim`, `SimDef`, `select`, `register_simulator`, `feature`) | 7 | outside `src/syntax` |
| `spicerack::import::decode(&[u8]) -> String` | 5 | |
| `spicerack::syntax::render::render(&Source{ast, tokens, lang, name, kit, loads, numbers}, to)` | 4 | `test_ngspice_analyses` (1 test), `test_rust_api`, `test_spectre_golden`, `test_vacask_runs` |
| `spicerack::measure_parse::{parse_measures, parse_fourier}` | 4 | |
| CLI `spice2rack` (`CARGO_BIN_EXE_spice2rack`) | 3 | `test_spice2rack` (5 tests), `test_cross_sim/kit.rs` (1), `test_vacask_runs/structure.rs` (1) |
| `spicerack::syntax::resolve::Kit::load(entries, form, lang, &mut loader)` (in-memory) | 2 | `test_dialect_kit`, `test_xyce_write` |
| `render::{render_pending, splice}` | 1 | `test_dialect_plugin` |
| `render::relink(text, base, moved)` | 1 | `test_cross_sim/kit.rs` (also the CLI) |
| `render::writes`, `import::dump` | 1 | `test_rust_api` |
| `spicerack::lint::lint_netlist` | 1 | `test_monte_carlo` |
| none of the reader/writer | 4 | `test_sim_command`, `test_sim_registry`, `test_unit`, `test_zero` |

CLI forms the tests invoke:
- `spice2rack -o . <deck>` (every backend dialect).
- `spice2rack --to <dialect> -o <dir> <deck>`.
- `spice2rack --to <dialect> --kit -o <dir> <deck>`.
- `spice2rack --dialect <src> --to <tgt> --kit -o <dir> <deck>`.
- 8 usage-error rows (exit 2).
- The gate and CI also run `nix build .#spice2rack && spice2rack --to espice --kit -o <dir> tests/fixtures/pdk_sections/models.lib`.

Gate tools that call the library:
- `src/bin/golden/*.rs`: `import::{decode, parse, kit}`, `render::{render_kit, references, Source, Numbers, Reference, Diags}`, `resolve::{Kit, FileId, Form}`, `Lang::lexicon()`, `Dialect::ALL`.
- `benches/perfbench.rs`: `tokenize`, `parse`, `render`, `token::Tag`; it also includes `src/bin/golden/inputs.rs` by `#[path]`.

### 1.2 Python entry points (30 collected `tests/test_*.py`; grep counts)

| Call | Files | Uses |
|---|---|---|
| `Subcircuit(...)` | 23 | 244 |
| `Testbench(...)` | 18 | 122 |
| `tb.add_<analysis>(...)` | 14 | 196 |
| `.netlist(<dialect>)` | 13 | 161 (ngspice 60, spectre 27, ltspice 11, vacask 10, xyce 7, espice 4, cdl 3, verilog 2) |
| `Circuit(...)` / `str(circuit)` | 11 / 4 | 174 / 114 |
| `.run(...)` | 10 | 29 |
| `Testbench.from_source(text, dialect)` | 6 | 24 |
| `register_simulator` / `simulators()` / `load_simulator` | 4 | 45 |
| `read_raw` | 4 | 8 |
| `spicerack.Error`, `.diagnostics[i].{code, kind, line, column, statement, element, title, cite, message, file}` | 4 | 14 + 21 |
| `netlist_with_notes(dialect)` | 3 | 7 |
| `spicerack.export`, `spicerack.diagnostics.{netlist, run}` | 3 / 1 | 15 / 11 |
| `spicerack.def_` / `spicerack.dialects` (dialect-definition API) | 4 | 20 |
| `spicerack.testbenches.*`, `remeasure`, `unit`, `run_corners` | 8 / 2 / 4 / 2 | |

Frozen names: `tests/fixtures/api_snapshot.txt` has 17 lines, 2 of them `spicerack.def_.*`.

### 1.3 The facade a new implementation must keep for the KEEP set

Rust:
1. `syntax::render::render_text` and `render_kit`, with the signatures above.
2. `Written { text: String, notes: Diags }` and `Diags { diags: Vec<Diag>, messages: String }`.
   Message lines are `<name>:<line>:<col>: <CODE> <title>`. `test_cross_sim/kit.rs::codes` parses
   codes out of that text, `test_spice2rack` greps `" W0012 "`, and Python asserts whole lines.
3. `syntax::diag::{Code, Kind::{Note, Refusal}, info(Code) -> Info{title, cite, explain, kind}, Diag{code, ..}}`.
   `Code`'s `Debug` text is the code itself (`W0052`).
4. `syntax::Dialect`: 9 variants, `ALL`, `name()`, `FromStr`.
5. `syntax::Lang`: built-in consts including `NGSPICE_GIVEN`, `From<Dialect>`, `name()`.
6. `import::{decode, kit}`.
7. `syntax::resolve::{Kit, FileId, Form, Role}`:
   - `Kit::{files(), file(FileId), lang()}`, with `path`, `role`, `text`, `diags` per file.
   - `FileId(u32)` is the index into `files()`; the entry file is 0.
   - `Form::{Deck, Library}`; `Role::{Deck, Library, VerilogA, Unread(_)}`.
8. `render::relink`.
9. The CLI contract:
   - exit codes 0, 1, 2;
   - stdout is the written path under `--to`;
   - suffixes `.ngspice.sp .hspice.sp .ltspice.cir .spectre.scs .vacask.sim .espice.sp .xyce.cir .cdl .v`;
   - the `--kit` layout with `_root/`.
10. Writer-to-backend marker lines: `render::NGBEHAVIOR` (`*spicerack ngbehavior=`) and
    `render::LTSPICE_EACH`. They are read by `backend::ngspice` and `simulation::run_each`.
11. Untouched by the rewrite but used by the KEEP set: `rawfile`, `result`, `measure_parse`, `unit`,
    `simulation`, `backend`, `lint`.

Python: the calls of §1.2. `Diagnostic.line/column/statement` are positions in the ngspice-form deck
the builder makes (`test_python_errors.py` asserts `rc:5:1: W0023 ...`, statement text `.save v(vout)`).
An IR built without that text must still produce them.

For the ADAPTER set, two more things are needed:
- A way to hand the writer `loads: &[String]` and `Numbers::Given` for text read as `Lang::NGSPICE_GIVEN`.
  Today that is `render(&Source{..})`.
- `Kit::load` with an in-memory loader.

### 1.4 Test files that import `spicerack::syntax::*` beyond a render/import call

Tier A is handles only (facade). Tier B is the pipeline's stages. Tier C is the data structures.
Tier D is the dialect-definition tables.

| File | Tier | Internals |
|---|---|---|
| `tests/test_syntax.rs` | C | `ast::{self, NONE, Node}`, `token::Tag`, `resolve::{Binding, DefKind, FileId, Kit, LoadError, Role}`, `{parse, tokenize}`, `Tokens::{tags, text, lexicon_at, end, end_relexed}`, `Ast::{check, len, tag, header, main_token}`, `Kit::{binding, definition, node_count}`; `#[path]` includes `src/bin/golden/inputs.rs` |
| `tests/test_verilog_a.rs` | C | `ast::{Node, Tag}`, `resolve::{Binding, DefKind, FileId, Form, Role}`, `{parse, tokenize}`, `Kit::{binding, definition}` |
| `tests/test_rust_api.rs` | B/C/D | `render::{Numbers, Source, render, writes}`, `{parse, tokenize}`, `token::Tag`, `Tokens::{tags, value}`, `def::{DialectDef, register}`, the text format of `import::dump`, `resolve::{FileId, Form}`, `diag::Code` |
| `tests/dialect_defs.rs` | D | compiles all 11 `src/syntax/dialects/*.rs` into the test crate by `#[path]` behind a `pub use spicerack::syntax::*` shim; `def::{DialectDef, register}`, `DialectDef::renamed`, `Lang::{dump, behaviours().text_family, BUILTIN_COUNT}`, `resolve::{FileId, Form, Kit}`, `render::{Diags, Written}` |
| `tests/test_dialect_plugin.rs` | B/D | `behaviour::DotRule`, `def::{Answer, DialectDef, RegisterError, RunsOn, register}`, `lexicon::Scale`, `render::capability::Construct`, `render::{Diags, Numbers, Source, render_pending, splice}`, `token::Tag`, `{parse, tokenize}`, `diag::{Code, Kind}` |
| `tests/common/acme.rs` | D | `behaviour::NumberReading`, `def::{Answer, Card, DialectDef}`, `lexicon::{Arithmetic, Scale}`, `render::capability::{Alt, Construct}`, `render::spelling::FuncForm`, `token::Tag`, `diag::Kind` |
| `tests/test_acme_oracles.rs` | D (via `acme.rs`) | `def::register`, `Lang` |
| `tests/test_dialect_kit.rs` | B/D | `def::register`, `resolve::{FileId, Form, Kit}` (`Kit::load`), `tokenize`, `Tokens::lexicon_at`, `Lang::lexicon()` |
| `tests/test_dialect_full.rs` | D | `def::{DialectDef, RegisterError, register}`, `lang::LANGS_MAX`, `Lang::{BUILTIN_COUNT, from_name}` |
| `tests/test_spectre_golden/main.rs` | B (one helper) | `render::{Numbers, Source, render}`, `{parse, tokenize}`, `Lang::NGSPICE_GIVEN.lexicon()`, `diag::Code` |
| `tests/test_vacask_runs/main.rs` | B (one helper) | `render::{Numbers, Source, render}`, `{lexicon, parse, tokenize}`, `diag::Code` |
| `tests/test_ngspice_analyses.rs` | B (one test) | `render::{Numbers, Source, render}`, `{parse, tokenize}` in `measure_survives_alongside_an_osdi_control_block`; `diag::Code`, `Lang` |
| `tests/test_xyce_write.rs` | A + `Kit::load` | `resolve::{FileId, Form, Kit, LoadError}` in one test; `diag::Code` |
| `tests/test_cross_sim/{main,kit}.rs` | A | `diag::Code` (`W0052`), `resolve::{FileId, Kit, Role, Form}`, `render::{relink, render_kit}` |
| `tests/test_sections_pdk.rs` | A | `diag::{self, Code}`, `render::Written`, `resolve::{Form, Role}` |
| `tests/test_spectre_import.rs` | A | `diag::{Code, Kind, info}`, `resolve::{Form, FileId}` |
| `tests/test_sim_command.rs` | A | `diag::{Code, Kind, info}` with 6 catalogue titles as strings; `Lang` |
| `tests/common/oracle.rs` | A | `diag::Code`, `render::Written`, `{Dialect, Lang}` |
| `test_control`, `test_epochs`, `test_read`, `test_render`, `test_vacask_import`, `test_model_kinds` | A | `diag::{Code, Kind, info}` |
| `test_ltspice_dialect` | A | `diag::Code`, `diag::info(..).cite`, `render::Diags` |
| `test_conditionals`, `test_dialects`, `test_harmonic_balance`, `test_hspice_write`, `test_laplace_ngspice`, `test_ltspice_source`, `test_monte_carlo`, `test_options`, `test_sections`, `test_spice_exprs`, `test_spice_write`, `test_steps`, `test_waveforms`, `test_xyce_refusals`, `test_xyce_source`, `test_vacask_modules`, `test_sources/main.rs` | A | `diag::Code` (plus `render::{Diags, Written}` in some) |
| `benches/perfbench.rs` | B/C | `token::Tag`, `{parse, tokenize}`, `render` |

By binary: 8 are welded to tiers B-D (`test_syntax`, `test_verilog_a`, `test_rust_api`, `dialect_defs`,
`test_dialect_plugin`, `test_acme_oracles`, `test_dialect_kit`, `test_dialect_full`). 4 touch tier B
at one site. The other 41 need tier A only.

---

## 2. Oracle assets

Sizes: `n` is the nextest count of the batch-22 gate (default features) and `t` the sum of that
binary's test times in the same log. The gate box has 32 threads and other sessions were running.

### 2.1 Rust integration tests

| Asset | What it proves | Oracle type | Simulators | Coupling | Size | Verdict |
|---|---|---|---|---|---|---|
| `tests/test_cross_sim/` + `tests/fixtures/cross_sim/` | A deck written for another simulator computes the original's numbers, every shared vector of every plot, per (corpus, target) cell. Also self-tests of the comparer and lists, and proofs re-derived from CMC reference tables | cross-simulator numeric compare; reference data; closed form | ngspice, Xyce, VACASK, `espice`, `espice-hspice`; LTspice for `p1_019` and the nightly cell | PUBLIC-ONLY (tier A) + CLI; *codes* (`W0052`, message format). Knows 3 writer idioms (§4.5) | n=24 (4 sample sweeps of 48/48/38/38 decks + 20); +5 full sweeps under `corpus`; sample lane 47-51 s through batch 21, 170-178 s in batch 22; nightly 31 min on 12 workers, 5.1 CPU-h | **KEEP**. Lists are data: re-bless with `CROSS_SIM_BLESS` and review |
| `tests/test_import_suite/` (`corpus`) | spice-bench decks rewritten as ngspice reproduce every plot and `.meas` on ngspice; analysis cards survive; text fixpoint per dialect over 5 corpora | real simulator run (ngspice on original and rewritten); round trip (text) | ngspice for the round trip; none for fixpoints | PUBLIC-ONLY (`render_text`, `import::decode`) | 8 tests; 6,050 decks, 469 known failures, 28 slow; round trip 2,586 CPU-s (5 min 25 s on 20 threads, 20 min on 4-vCPU CI); fixpoints 64 s, 2.1 GB | **KEEP** |
| `test_ngspice_analyses.rs` | every analysis the library exposes on ngspice gives a closed-form number; `.pss` to a van der Pol limit cycle; RF-port refusals | real simulator + closed form; manual example | ngspice | PUBLIC-ONLY except 1 test building `Source` for `loads`; *codes*, *format* (a few lines) | n=31, t=12.9 s | **ADAPTER** (1 test) |
| `test_espice_analyses.rs` | the shared analysis suite on ESPice | real simulator + closed form | espice | PUBLIC-ONLY | n=9, t=0.5 s | **KEEP** |
| `test_laplace_ngspice.rs` | Laplace sources give closed-form responses on ngspice | real simulator + closed form | ngspice | PUBLIC-ONLY; *codes* | n=21, t=1.0 s | **KEEP** |
| `test_sources/` | PWL `r=`/`td=`, TRNOISE/TRRANDOM, ports, `.sp`; imported decks equal to 1e-12; Spectre PWL semantics | real simulator + closed form; statistical; same-simulator differential | ngspice (and ESPice) | PUBLIC-ONLY; *codes* | n=20, t=1.4 s | **KEEP** |
| `test_waveforms.rs` | ledger Phase 3 source rows, source simulator against target simulator | simulator pair; closed form from a manual clause; recorded LTspice run | ngspice, Xyce, VACASK, ESPice, LTspice | PUBLIC-ONLY via `oracle.rs` | n=6, t=173 s (LTspice queue) | **KEEP** |
| `test_xyce_write.rs` | ngspice to Xyce: same operating point or closed form; refusals against Xyce RG clauses | simulator pair; closed form; manual clause | Xyce, ngspice, VACASK | PUBLIC-ONLY except 1 test using `Kit::load` + `LoadError`; *codes* | n=24, t=8.3 s | **ADAPTER** (1 test) |
| `test_xyce_models.rs` | an ngspice model card written for Xyce computes ngspice's numbers | closed form on both simulators | Xyce, ngspice | PUBLIC-ONLY | n=2, t=0.7 s | **KEEP** |
| `test_xyce_source.rs` | Xyce to ngspice pair rules | simulator pair; closed form; manual clause for refusals | Xyce, ngspice | PUBLIC-ONLY; *codes* | n=9, t=6.3 s | **KEEP** |
| `test_xyce_refusals.rs` | what real Xyce stops on is refused with the right code | manual clause (recorded Xyce behaviour) | none in-run | PUBLIC-ONLY; *codes* | n=3, t=0.3 s | **KEEP** |
| `test_hspice_write.rs` | HSPICE writer lines against the HSPICE command reference; written deck on ESPice's HSPICE reader | manual spec example; closed form | `espice-hspice`, ngspice | PUBLIC-ONLY; *codes*, *format* | n=7, t=19.9 s | **KEEP** |
| `test_harmonic_balance.rs` | one-tone `.hb` written for each target meets an RC phasor | closed form; recorded Xyce run; manual clause (Spectre) | VACASK, ESPice, `espice-hspice` | PUBLIC-ONLY; *codes*, *format* | n=5, t=0.4 s | **KEEP** |
| `test_expr_rules.rs` | power grouping, function domains, number reading across dialects | simulator pair; closed form | ngspice, Xyce | PUBLIC-ONLY | n=7, t=2.7 s | **KEEP** |
| `test_options.rs` | options and temperatures across dialects | simulator pair; closed form; manual clause | ngspice, Xyce, VACASK, ESPice | PUBLIC-ONLY; *codes*, *format* | n=11, t=23.4 s | **KEEP** |
| `test_steps.rs` | `.step` runs the same points in the same order on every target | closed form at recorded source points; manual sentence (Spectre) | ngspice, VACASK, LTspice | PUBLIC-ONLY; *codes*, *format* | n=8, t=372 s (LTspice queue) | **KEEP** |
| `test_epochs.rs` | decks that alter the circuit between analyses | closed form per state; simulator pair | VACASK, ngspice, Xyce, ESPice | PUBLIC-ONLY; *codes* | n=8, t=1.7 s | **KEEP** |
| `test_control.rs` | ngspice `.control` loops unrolled | real ngspice on the source script; closed form | ngspice, Xyce | PUBLIC-ONLY; *codes* | n=3, t=1.6 s | **KEEP** |
| `test_spice_exprs.rs` | SPICE-family expression and value spellings | real simulator; source simulator's value | ngspice, Xyce, ESPice | PUBLIC-ONLY; *codes* | n=20, t=1.9 s | **KEEP** |
| `test_spice_write.rs` | SPICE-family writer conformance gaps | real ngspice to closed form; manual clause | ngspice, ESPice, `espice-hspice` | PUBLIC-ONLY; *codes* (20 distinct), *format* | n=26, t=1.7 s | **KEEP** |
| `test_read.rs` | what a deck's statements become, observed through writers | mixed: 4 characterization tests against golden `fx14` (*sameness*); 11 ledger-row tests on manual clauses and simulators | per test | PUBLIC-ONLY via `oracle.rs`; *codes*, *format* | n=16, t=1.3 s | **KEEP**; 4 of its 15 own tests are sameness |
| `test_vacask_import.rs` | VACASK decks written for ngspice | simulator pair; VACASK docs clause for refusals | vacask, ngspice | PUBLIC-ONLY; *codes* | n=24, t=1.7 s | **KEEP** |
| `test_vacask_modules.rs` | a `.model` LEVEL that VACASK ships as a module | simulator pair (1e-3) | Xyce, ESPice, VACASK | PUBLIC-ONLY; *codes* | n=2, t=0.4 s | **KEEP** |
| `test_vacask_runs/` | ngspice-given text written for VACASK runs to closed forms | real simulator + closed form | vacask | one helper (`emit_from`, about 15 lines) uses `tokenize`/`parse`/`Source`/`render`; 1 test passes a lexicon | n=67, t=8.0 s | **ADAPTER** |
| `test_spectre_golden/` | the Spectre writer emits the statement forms the Spectre Reference documents | manual spec example (exact strings); no execution | none | one helper (`spectre_with`, about 20 lines) uses `tokenize`/`parse`/`Source`/`render`; *format*, *codes* | n=34, t=1.5 s | **ADAPTER** |
| `test_ltspice_dialect.rs` | emitted LTspice text against the LTspice help | manual spec example | none | PUBLIC-ONLY; *format*, *codes* | n=8, t=0.4 s | **KEEP** |
| `test_ltspice_source.rs` | LTspice-only source constructs after translation | closed form that LTspice 26.1.1 met (recorded) | ngspice and others; no LTspice | PUBLIC-ONLY; *codes* | n=4, t=0.9 s | **KEEP** |
| `test_dialects.rs` | one circuit in every dialect, read back by ESPice | real simulator (`--tokenizer=<d>`) | espice, `espice-hspice` | PUBLIC-ONLY; *codes*; marker `ESPICE_REFUSES` | n=8, t=0.4 s | **KEEP** |
| `test_conditionals.rs` | `.if`, Spectre `if`, VACASK `@if`: hand counts, fixpoints, ngspice bit-identical | hand-read fixture; round trip; real simulator; closed form | ngspice (also `hsa`) | PUBLIC-ONLY; *codes* | n=3, t=0.4 s | **KEEP** |
| `test_sections.rs` | library sections: hand counts, fixpoints in 6 dialects, SPICE and Spectre compared by meaning, corners bit-identical | hand-read fixture; round trip; meaning via `common/sections.rs`; real simulator | ngspice, ESPice | PUBLIC-ONLY; *codes* | n=12, t=0.7 s | **KEEP** |
| `test_sections_pdk.rs` | real PDK files (gf180mcu, sky130, VACASK `cmos180n.lib`) | line scan; round trip; real simulator bit-identical | ngspice, VACASK, `espice-hspice`; `$PDK_ROOT`, `$VACASK_CORPUS` | PUBLIC-ONLY (tier A); *codes* | n=6, t=75.7 s | **KEEP** |
| `test_monte_carlo.rs` | statistical calls and Spectre `statistics`: T-G1..T-G10 hand-worked text; T-R1..T-R5 round trips; lint | hand-derived text; round trip; independent count | none; reads `$PDK_ROOT` | PUBLIC-ONLY; *format*, *codes* | n=17, t=103 s (T-R1 is 95 s) | **KEEP** |
| `test_monte_carlo_ngspice.rs` | T-N0/T-N1: draws through Spectre and back match closed forms, N=2000 | real simulator + statistical closed form | ngspice | PUBLIC-ONLY | n=4, t=104 s | **KEEP** |
| `test_model_kinds.rs` | r/c models and level-3 diode for Spectre and VACASK | closed form; simulator pair; byte pins of `tests/spectre/t_m*.scs` | ngspice, vacask | PUBLIC-ONLY; *format* (4 pinned decks), *codes* | n=7, t=0.9 s | **KEEP** |
| `test_pending_real_run.rs` + `tests/spectre/p_*.scs` (11) + `tests/ltspice/p_*.cir` (11) + `pending.json` ×2 | one deck per construct row of the Spectre and LTspice writers, byte-pinned, with the numbers a real run must give | closed form met by ngspice; pinned remote run (10 pass and 1 fail on Spectre 25.1, 2026-11-07); LTspice: 8 pending, 3 refused | ngspice | PUBLIC-ONLY; *format* (22 decks byte for byte) | n=3, t=0.6 s | **KEEP**. The Spectre evidence holds only while the bytes hold (§7.9) |
| `test_structural.rs` | CDL passes netgen LVS against the source; Verilog elaborates in Icarus | external tool | netgen 1.5, iverilog 13 | PUBLIC-ONLY; marker `VERILOG_FAILS` | n=2, t=0.3 s | **KEEP** |
| `test_spectre_import.rs` | Spectre Verilog-A instances and `sweep` written for ngspice, VACASK, Xyce | closed form; real simulator; manual clause | ngspice, openvaf, vacask, Xyce | PUBLIC-ONLY (tier A); *codes* | n=5, t=2.0 s | **KEEP** |
| `test_render.rs` + `fixtures/transpiler/pinned/` (74 files) | `statistics` tables, Laplace sources and drawing functions on every target | **byte golden against the old pipeline** | none | PUBLIC-ONLY; *format*, *codes*, *sameness* | n=7, t=0.7 s | **KEEP** as characterization only; re-bless with review |
| `test_spice2rack.rs` | the CLI contract | API contract; metamorphic | none | PUBLIC-ONLY (binary); *codes* (`W0012` in stderr), *format* (4 include cards) | n=5, t=0.3 s | **KEEP** |
| `test_oracle.rs` | the test templates' arithmetic; the recorded-pin readers; proofs quote their pins | by-hand numbers; pinned remote run | ngspice, espice, `espice-hspice` | PUBLIC-ONLY | n=15, t=0.5 s | **KEEP**; 1 test depends on an FNV-keyed pin of written bytes (§7.9) |
| `test_backends.rs` | raw-file reader and writer; LTspice run; Xyce Verilog-A plugin toolchain | spec example; real simulator | LTspice, Xyce, cmake | PUBLIC-ONLY (outside `src/syntax`) | n=14, t=98.7 s | **KEEP** |
| `test_sim_command.rs`, `test_sim_registry.rs` | simulator registry and command runner | spec example (coreutils stand-ins) | none | PUBLIC-ONLY; *codes* + 6 catalogue titles | n=3 + 2 | **KEEP** |
| `test_unit.rs` | the unit table | spec example | none | unrelated | n=2 | **KEEP** |
| `test_zero.rs` | the form of `fixtures/zero/untranslated.txt` | spec example of a list format | none | PUBLIC-ONLY (`Dialect::ALL`) | n=1 | **KEEP** while the Z1 list is kept |
| `test_syntax.rs` (`corpus`) | every golden input: lexicon regions, kit read order, every instance bound or `R0001`, token ends, `Ast::check` | hand-read fixture; self-consistency | none; all corpora + `$IHP_PDK` | INTERNAL (tier C) | 4 tests over 14,970 inputs; runtime not stated | **REWRITE** as IR invariants over the corpus |
| `test_verilog_a.rs` | a Spectre kit's Verilog-A modules bind | hand-read fixture | none | INTERNAL (tier C) | n=1 | **REWRITE** |
| `test_rust_api.rs` | documented behaviour of `render`, `writes`, `Numbers`, `import::dump`, `import::kit` confinement | API contract; round trip; file system | none | INTERNAL (tiers B/C/D) | n=5 | **REWRITE**. Keep the kit-confinement test: a path out of the root is refused `R0007`, a trust boundary |
| `test_dialect_plugin.rs`, `test_dialect_kit.rs`, `test_dialect_full.rs`, `test_acme_oracles.rs`, `dialect_defs.rs`, `common/acme.rs`, `fixtures/dialects/*` | the dialect-definition API: acme declared from public items, round trips, clone equals built-in, registry bound, pinned dumps | round trip; metamorphic; pinned dump; real ngspice (acme) | ngspice (acme); `$SPICE_BENCH` (200-deck sample) | INTERNAL (tier D) | n=17+2+1+2+3; `dialect_defs` t=115.6 s | **REWRITE** with the plugin API, or **DROP** with it |

### 2.2 Python tests (30 collected files, 657 `def test_`; the gate ran 814 passed + 4 xfailed + 3 heavy = 821 items)

| Asset | What it proves | Oracle type | Simulators | Coupling | Size | Verdict |
|---|---|---|---|---|---|---|
| `test_bench_*.py` (5 files), `test_characterize.py`, `test_run_corners.py`, `test_remeasure.py` | bench extractors against behavioural DUTs | real simulator + closed form | ngspice | PUBLIC-ONLY | 91 + 10 + 4 + 13 tests | **KEEP** |
| `test_python_circuit.py`, `test_python_netlist.py`, `test_python_simulator.py`, `test_python_subcircuit.py` | every builder call writes the card the manual documents | manual spec example (hand-written lines) | mostly none | PUBLIC-ONLY; *format* (substring/line), few *codes* | 41 + 129 + 86 + 97 | **KEEP** |
| `test_python_conformance.py` | 112 features × 9 targets: written, or refused with one exact code; written decks run where a simulator exists | API contract reviewed per manual; closed form | ngspice, ESPice, VACASK | PUBLIC-ONLY; *codes* (1,008 cells, 19 distinct) | 7 functions | **KEEP**. This is the densest refusal gate |
| `test_python_errors.py`, `test_python_diagnostics.py` | failures as data: `spicerack.Error`, `.diagnostics`, `<variant>.diag.json` | API contract; schema; hand counts | ngspice, VACASK | PUBLIC-ONLY; *codes*, catalogue titles verbatim, **line/column/statement of the intermediate ngspice text** | 6 + 4 | **KEEP**, but the IR must carry spans and a statement text |
| `test_espice_backend.py`, `test_vacask_backend.py`, `test_ltspice_backend.py` | a Testbench's deck and its numbers on the real simulator | real simulator | espice / vacask / ltspice | PUBLIC-ONLY | 16 + 9 + 8 | **KEEP** |
| `test_simulators.py`, `test_simulator_oracles.py` + `fixtures/sims/fake/` | custom simulators; registry parity bit for bit | fake simulator as a real process (closed form of node name); metamorphic | ngspice, ESPice, VACASK | PUBLIC-ONLY; marker `PARITY_SIM_GAP` | 9 + 3 | **KEEP** |
| `test_docs.py`, `test_python_examples.py` | every docs block and example runs and prints the numbers claimed; sky130/gf180 goldens at 1e-3 | docs execute; golden with provenance; real simulator | ngspice, ESPice; `$PDK_ROOT` | PUBLIC-ONLY | 5 + 22; three tests about 86-88 s each | **KEEP** |
| `test_python_api_snapshot.py` + `fixtures/api_snapshot.txt` | public names and signatures frozen | API contract (snapshot) | none | PUBLIC-ONLY; 2 of 17 lines are `def_` | 2 | **KEEP** |
| `test_python_export.py`, `test_python_units.py`, `test_python_lint.py`, `test_design_testbenches.py`, `test_bump_version.py` | export round trip; units; lint; design testbenches; version bump | round trip; external reader (ngspice `load`); hand lists | ngspice (some) | PUBLIC-ONLY | 6 + 26 + 32 + 13 + 3 | **KEEP** |
| `test_dialect_def.py` + `builtin_dialects.py` (1,687 lines) | the Python dialect-definition API; every built-in declared again from Python dumps the Rust row | pinned dump; parity; ngspice | ngspice | INTERNAL (tier D mirrored in Python) | 15 | **REWRITE** or **DROP** with the plugin API |
| `tests/api_parity.py` | build against a hand-written text twin on every target (111 rows × 9); **store round trip** `from_source(build.netlist(d), d)` equals the build; title survives; determinism | hand-written twin (differential); round trip | none | PUBLIC-ONLY | 6 functions; markers `PARITY_GAP` (13), `STORE_HIDDEN` (4), `STORE_DETOUR` (31) | **KEEP and wire in**: it is not collected today (§7.1) |
| `tests/schema_check.py`, `fixtures/schemas/*.json` | the suite's own JSON-schema checker | spec | none | none | helper | **KEEP** |
| `tests/spectre/run.py`, `tests/ltspice/run.py`, `tests/spectre/expected.json` | check a real Spectre or LTspice run of the pinned decks | pinned remote run / pending real run | Spectre, LTspice (licensed machine) | file-only | manual scripts, not collected | **KEEP** |
| `tests/espice/sky130_hspice_drain_current.sh` | repro of ESPice's HSPICE reader giving +5.1 % drain current | real simulators | ngspice, `espice-hspice` | CLI + `cargo test` | manual | **KEEP** |

### 2.3 In-crate unit tests (`#[cfg(test)]`): 347 lib tests (t=132.6 s) + 8 in `src/bin/golden`

| Asset | What it proves | Oracle type | Coupling | Size | Verdict |
|---|---|---|---|---|---|
| `src/syntax/tests/{spice_read, spice_read_sources}.rs`, `render/named/lift.rs`, `render/spice/verilog.rs` | constructs read from other dialects' forms, written by every target or refused; Spectre lifting; structural Verilog | manual clause + hand text; decks real Spectre 21.1 ran, byte for byte | only `render_text` + `Dialect` + `Code`; *format*, *codes* | 19 + 20 + 4 + 4 = 47 | **ADAPTER**: move to `tests/` or keep as is; the bodies need no internals |
| `src/syntax/tests/{mod, heads, spicerack}.rs`, `lexicon/*.rs`, `parse.rs`, `token.rs`, `ast.rs`, `statement.rs`, `number.rs` | the parser per node tag and header shape; tokenizer rules; 10⁴-deep nesting | manual clause with the tree derived by hand | INTERNAL: asserts tree text `Tag"head"[children]`, tokens | 32 + 6 + 5 + 30 + 6 + 2 + 2 + 2 + 1 | **REWRITE**. Each case's clause and input deck are reusable as reader-to-IR cases |
| `src/syntax/resolve/{tests, verilog_a}.rs` | resolve, one test per answer, from an in-memory loader | manual clause + hand-derived answer | INTERNAL (`Kit`, `Binding`) | 21 + 1 | **REWRITE** |
| `src/syntax/tests/{build, check}.rs` | a built AST is row for row the parse of its text; `Ast::check` | differential against the parser; hand-broken trees | INTERNAL | 5 + 2 | **REWRITE**. This is the nearest thing today to `read(write(x)) == x` on the tree |
| `src/syntax/tests/render.rs`, `render/spelling.rs`, `device.rs` | numbers with every suffix read back as the same value in every target; 10⁴-deep decks written; device letters invert | round trip (value level); hand text | INTERNAL helpers | 8 + 5 + 3 | **REWRITE** (keep the properties) |
| `render/spice/{ternary, powers, table, globals, pair, parasitics, laplace, ngspice_models}.rs`, `render/named/vacask_models.rs`, `render/{capability, models, draws, out, relink}.rs`, `render/read/*.rs` | private writer and reader helpers on strings | recorded simulator runs cited per case; hand text | INTERNAL (private function names) | about 60 | **REWRITE** or **DROP** per helper; the input-to-expected pairs are reusable data |
| `src/syntax/diag/mod.rs` | every code has a row; ranges and order; 8 titles | catalogue self-check | INTERNAL to the catalogue | 3 | **KEEP** if the catalogue stays |
| outside `src/syntax`: `backend` 24, `normalize` 23, `python` 22, `measure_parse` 19, `rawfile` 7, `veriloga` 6, `unit` 3, `import` 2, `lint` 2, `simulation` 1 | result readers, backends, Python glue | spec examples | not in the rewrite's scope, except `src/python` (deck, store, analysis: `syntax::build::Build`, `Tokens::from_rows`, `unlike_parse`, `syntax::ast`) | 109 | **KEEP**; the `src/python` 22 go with whatever replaces `Build` |

### 2.4 Gate tools, lists and fixtures

| Asset | What it proves | Oracle type | Coupling | Size | Verdict |
|---|---|---|---|---|---|
| `src/bin/golden` (`record`, `diff`, `check-determinism`, `zero`) + `migration.xfail` + baseline tree | output bytes for every input × target are the baseline's, or the difference is a reviewed line; 1 job equals N jobs | **byte golden against the previous implementation**; determinism | ADAPTER: `inputs.rs::references` builds a `render::Source` from `import::parse`; `main.rs` calls `render_kit`, `Kit`, `Lang::lexicon()`; `diff.rs` and `zero.rs` are file-only | 14,970 inputs × 9 targets; 203,147 output files (2.7 GB); xfail 4,103 entries; record 67 s, diff 30 s, determinism 92 s | **ADAPTER**, as a sameness gate only (§5) |
| `tests/fixtures/zero/untranslated.txt` | gate Z1: the outputs carrying `W0001` or `W0032`, strict both ways, only shrinks | strict list from the golden record | file-only; *codes* (the list is defined by two code numbers) | 5,318 names | **KEEP** if those two codes keep their meaning |
| `tests/fixtures/cross_sim/*.txt` (13 lists) + `proof/` (31 files) | every deck that does not pass, with class, rule and proof | strict name lists | file-only; free-text comments hold observed numbers | 7,881 lines | **KEEP** (re-bless) |
| `tests/fixtures/spice_bench_known_failures.txt`, `spice_bench_slow.txt` | decks ngspice cannot run or finish | strict name lists | file-only | 469 + 28 entries | **KEEP** |
| `tests/fixtures/conformance_noise.txt` | inputs left out of the conformance number | named list | read by `src/syntax/render/read/rejects.rs` | 195 entries | **KEEP** (the reader moves) |
| `tests/hspice/recorded/*.pin` (17), `tests/spectre/recorded/*.pin` (5) | numbers real HSPICE Y-2026.03 and Spectre 25.1 gave on iccad4, 2026-11-07, keyed by run name | pinned remote run | file-only | 22 pins | **KEEP**; cannot be re-made (§7.9) |
| `tests/spectre/pins/ed0648fe1d0bda7b.pin` | Spectre's number for one written deck, keyed by the FNV-1a of the written text | pinned remote run | welded to the Spectre writer's exact bytes for that deck | 1 pin | **KEEP**; lost if one byte moves |
| `tests/spectre/t_s*.scs` (6), `t_m*.scs` (4), `expected.json` | T-S: decks real Spectre 21.1 ran; T-M: closed forms and ngspice references, `t_m4` run on Spectre 25.1, `t_m3` unrun since a writer change (§7.9) | pinned remote run; closed form | *format* (byte for byte via `lift.rs` and `test_model_kinds`) | 10 decks | **KEEP** |
| `tests/fixtures/transpiler/` (15 inputs, `pinned/` 74 files, `refusal-map.txt` 2,644 lines) | made-up known-bug inputs; the old writer's bytes; the old messages beside the codes they became | byte golden against the old pipeline | file-only | see left | pins: sameness; `refusal-map.txt`: history, **DROP** from gating |
| `tests/fixtures/{pdk_sections, conditionals, verilog_a}` | hand-written kits read by hand | hand-read fixture | file-only | 5 + 2 + 2 files | **KEEP** |
| `tests/fixtures/raw/` (27 files, README with provenance) | raw files from real simulators (Xyce 7.10, LTspice 26.1.1, ngspice-45) and hand-built Spectre nutascii/nutbin | spec example / recorded run | read by `src/rawfile.rs`, `src/backend/ltspice.rs`, `test_backends.rs` | 27 files | **KEEP** (out of scope) |
| `tests/fixtures/dialects/{acme, my-hspice}.dump`, `spice-bench-200.txt`, `acme-bench-refused.txt` | pinned rows of two plugin definitions; a pinned 200-deck sample | pinned dump | tier D; also read by `src/syntax/behaviour.rs` and `lexicon/mod.rs` | 112 + 107 lines; 200 decks | goes with the plugin API. The 200-deck sample list is reusable |
| `benches/perfbench.rs`, `perf.py`, `pybench.py`, `perf.pins` | instructions and allocations of tokenize/parse/render and the raw reader against pins (+3 % / +2 % fails) | pinned counts | `tokenize`, `parse`, `token::Tag`, `render` | 25 pins; pre-commit < 10 s, merge about 2 min, nightly about 5 min | **REWRITE** the syntax workloads and re-pin; `raw:*` and `py:*` pins survive |
| `.github/scripts/check_style.py`, `bump_version.py`; `.github/workflows/*.yaml` | house style (≤ 500 code lines, header, order); version bump; CI tiers | checker with `--self-test` | none | n/a | **KEEP** |

### 2.5 Verdict totals

| Group | KEEP | ADAPTER | REWRITE / DROP |
|---|---|---|---|
| Rust integration binaries (53) | 41 | 4 | 8 |
| Rust integration tests, default features (546) | 359 | 156 | 31 |
| ... `corpus`-only | `test_import_suite` 8, cross-sim full sweeps 5 | | `test_syntax` 4 |
| Python collected files (30; 657 functions) | 29 files, 642 functions | | 1 file, 15 functions (`test_dialect_def.py`) |
| In-crate `src/syntax` tests (238) | 3 (`diag`) | 47 | 188 |
| In-crate tests elsewhere (109) + `bin/golden` (8) | 117 | | |
| Gate tools | `check_style`, strict lists | `golden` | `perfbench` workloads + pins |

Of the 359 KEEP tests, 11 are sameness tests: 7 in `test_render` and 4 in `test_read`.

These verdicts assume the facade of §1.3. Nothing was run to confirm them.

---

## 3. Corpora and pins (`flake.nix`, `flake.lock`)

Shells:
- `devShells.default`: toolchain, maturin, cargo-nextest, Python 3.12 + numpy + pytest + pytest-xdist; no simulators.
- `devShells.testing`: adds the simulators and corpora below.
- `devShells.testing-ltspice`: `.#testing` + LTspice.
- `devShells.docs`: mkdocs.
- Packages: `default` (the wheel) and `spice2rack` (no default features, `doCheck = false`).

`flake.lock` sha256 is `ab2eea91...` today; fx18's provenance records `8b1e41e0...`.

### 3.1 Simulators and tools

| Name | Pin | Exposed as | Read by |
|---|---|---|---|
| ngspice-45, built `--enable-pss` | nixpkgs `9e92285f` (lock) | `ngspice` on PATH | most oracle tests; cross-sim (`Sim::Ngspice`, `NgspiceHsa` = `-D ngbehavior=hsa`) |
| Xyce 7.10 with plugin support, shared libs, ADMS | same nixpkgs, overridden | `Xyce` | cross-sim, `test_xyce_*`, `test_expr_rules`, `test_backends::p1_039` |
| VACASK (upstream) | codeberg `arpadbuermen/VACASK` `36be560b`, `sha256-dzC795...` | `vacask` | `test_vacask_*`, cross-sim, others |
| OpenVAF for ngspice OSDI | `arpadbuermen/OpenVAF` `2e066436`, `sha256-AXtp8q...` | `openvaf-r`, alias `openvaf` | `src/veriloga.rs`, `test_spectre_import` |
| OpenVAF for VACASK (`mb-experimental`) | `96b3322c`, `sha256-pFgK7H...` | `SIM_OPENVAF=<store>/bin/openvaf-r` | set by the shell; no reader in `src/`, `tests/`, `python/` by grep (VACASK reading it: not stated in the tree) |
| ESPice (current) | `OpenFAT/ESPice` `5f0b8fda` (lock) | `espice` | `test_espice_*`, `test_dialects`, cross-sim `Sim::Espice` (`--tokenizer=ngspice`) |
| ESPice with its HSPICE reader | same repo, rev `5f0b8fda` fixed in the URL (0.3.0) | `espice-hspice` | every test that runs HSPICE text (`--tokenizer=hspice`), cross-sim `EspiceHspice` / `EspicePinned` |
| netgen-vlsi 1.5, Icarus Verilog 13, cmake | nixpkgs | PATH | `test_structural`, `test_backends` |
| LTspice 26.1.1 | vendor MSI, `requireFile` sha256 `f22163f1...eebc357`; under Wine in headless Xvfb, one run at a time behind a `flock` | `ltspice` (`.#testing-ltspice` only) | `test_backends` (`p1_018`, `p1_007`), `test_steps`, `test_waveforms`, cross-sim `p1_019` and `ltspice_corpus`, `test_ltspice_backend.py`. CI has no MSI: marker `LTSPICE_CI_PENDING_MIRROR` |
| Spectre 25.1.0, HSPICE Y-2026.03-SP2-1 | not in the flake: remote host iccad4, runs recorded 2026-11-07 (`research/REMOTE-ORACLE.md`, `research/pins/pin.sh`) | pins under `tests/{spectre, hspice}/` | `oracle::{pinned, recorded}`, `tests/spectre/run.py`. "iccad4 access lost, ZERO D43" (`tests/fixtures/raw/README`) |

### 3.2 Corpora and PDKs

On-disk sizes are `du` of the store paths in the gate's saved shell profile; they are not in `flake.nix`.

| Name | Pin | Decks (TESTING.md; verified by `find`) | Size | Env var | Read by |
|---|---|---|---|---|---|
| spice-bench | `OpenFAT/VAMS-Conformance-Suite` `a763eeed` (lock) `external/spice-bench/fetch.sh`, fixed-output hash `sha256-g3Gw1U9a...` | 6,050 `*.sp` | 584 MB, 21,323 files | `SPICE_BENCH`, and `NGSPICE_CORPUS` (same path) | golden (`spice-bench`), `test_import_suite`, cross-sim, `dialect_defs`, `test_acme_oracles`, `proofs.rs`, `kit.rs` |
| HSPICE corpus | the same conformance flake, `tests/netlist`, paths containing `hspice` and not `invalid` | 75 of 951 `*.sp` | 25 MB | `HSPICE_CORPUS` | golden, import suite, cross-sim |
| Xyce_Regression | `Xyce/Xyce_Regression` `bbde4027`, sparse `Netlists`, `sha256-D+eOS9...` | 4,068 `*.cir` | 65 MB | `XYCE_CORPUS` | golden, import suite, cross-sim |
| VACASK corpus | `robtaylor/VACASK` `bcd48e2d`, `sha256-/x6yJ+...` (the fork's tree, kept apart from the simulator) | 151 `*.sim`; golden reads 148 (3 `gnucap` decks excluded) | 14 MB | `VACASK_CORPUS` | golden (`vacask`, `pdk-vacask`), import suite, cross-sim, `test_sections_pdk` |
| spice-datasets (LTspice) | `symbench/spice-datasets` `a3a1c4fc`, `sha256-sf1d3j...` | 4,342 `*.net`; 34 self-contained decks form the cross-sim cell | 24 MB | `LTSPICE_CORPUS` | golden, import suite, cross-sim (nightly, `.#testing-ltspice`) |
| sky130 + gf180mcu PDKs | ciel-releases `sky130-8afc8346`, `gf180mcu-1e355470`; 4 archives each with a hash | golden: 194 files (sky130A), 3 (gf180mcuD) | 307 MB | `PDK_ROOT` | golden, `test_sections_pdk`, `test_monte_carlo`, `test_python_examples` 20/21, `docs/models.md`, `src/veriloga.rs` |
| IHP-Open-PDK | **not pinned**; a checkout named by `$IHP_PDK` (baselines used `5e6d592`; the gate sets `/home/omare/pdks/IHP-Open-PDK`, at that commit today) | 36 files from 7 `corner*.lib` entries | not stated | `IHP_PDK` | golden, `test_syntax`, `perfbench` |
| Spectre corpus | none pinned | 0 | n/a | `SPECTRE_CORPUS` (unset) | `spectre_corpus_fixpoint` is `#[should_panic]` |
| test fixtures | the tree | 53 golden inputs today (TESTING.md's table says 18): 33 Spectre, 12 LTspice, 6 ngspice, 2 Xyce | n/a | n/a | golden (`fixtures`), `dialect_defs` |

Golden inputs today: 14,970 (`MANIFEST` of the batch-22 record and of fx18). TESTING.md's table sums
to 14,938 and is stale on `fixtures` (18 → 53) and `vacask` (151 → 148).

Test knobs:
- `CROSS_SIM_{BLESS, JOBS, ONLY, TUBE, TMAX, KEEP}`.
- `SPICE_BENCH_TIMEOUT` (default 120 CPU-s).
- `SPICERACK_BLESS` (rewrites pins in `oracle::golden`, `test_render`, `test_pending_real_run`, `test_model_kinds`, `test_dialect_plugin`, `test_acme_oracles`).
- `SPICERACK_KEEP_HSPICE`.
- `GOLDEN_JOBS`.
- `ESPICE`.
- `LTSPICE_CI_PENDING_MIRROR`.

---

## 4. Cross-simulator harness (`tests/test_cross_sim/`)

### 4.1 End to end, one deck in one cell

1. **Pick decks.** `sweep` walks `$<CORPUS>` for `*.<ext>`, filters, and sorts by path. The merge tier
   takes every `ceil(len/48)`-th deck (`SAMPLE = 48`: 48 Xyce, 48 spice-bench, 38 VACASK, 38 HSPICE).
   `--features corpus` takes all. It asserts 30 GiB free under the temp dir (`DISK_MIN`) and every
   simulator on PATH.
2. **Read.** `spicerack::import::decode(bytes)`, then
   `spicerack::import::kit(&[deck], Form::Deck, corpus.dialect, root)`: the deck with the files it includes.
3. **Write** (`kit.rs::write`). `render_kit(kit, FileId(0), "t", target)` for the deck, and
   `render_kit(kit, FileId(i), ...)` for every `Role::Deck | Role::Library` file, cached per
   (path, target). A file beside the deck keeps its name; others go under `.cross_sim_inc/<i>-<name>`.
   Paths are rewritten with `render::relink`. A refusal anywhere refuses the deck: its codes are
   parsed out of `Diags.messages`. The deck's note codes are kept (only `W0052` is used).
4. **Run the original** on its own simulator (`sim.rs::run`): a fresh temp dir with a symlink to every
   file beside the deck, under `timeout <10×secs> prlimit --cpu=<secs> --fsize=2GiB`, with
   `OMP_NUM_THREADS=1`.
   - ngspice: `-b -r raw` with `.option seed=1` prepended; a deck whose `.control` runs analyses
     gets a `write` loop inserted before the last `.endc`.
   - Xyce: `-r raw`, plus `<deck>_noise.dat` read as a noise plot.
   - VACASK: `-se -sp`, one raw file per `analysis <name>`.
   - ESPice: `--rawfile --format=binary --tokenizer={ngspice|hspice}`.
   - LTspice: `-b`, with `plotwinsize=0` and `numdgt=15` prepended.

   Raw files are read with `spicerack::rawfile::parse_raw_all` (product code inside the oracle, but
   outside `src/syntax`). Budgets: 60 CPU-s for an original, 120 for a written deck, 240 for a refined run.
5. **Run the written deck** on the target's simulator, with the written includes in place.
6. **Compare** (`cmp.rs::compare`). Plots pair by kind (op, dc, ac, tran, noise, total, read off the
   scale and plot name), then by the swept grid, then by values. Vectors pair by a normalized name
   (`v(x)`, `X`, `x`; `i(v1)`, `V1#branch`, `v1:flow(br)`).
7. **Classify a failure.**
   - If the cell has a `control` simulator (the target's simulator in a mode that reads the source:
     ESPice for the two ESPice cells) and it gives the target's numbers on the original, the class
     is `simulator-differs:source`.
   - Otherwise a transient mismatch that goes away when both decks are refined to TMAX =
     (TSTOP − TSTART)/2000 or /20000 is `simulator-differs:refinement`.
   - Otherwise it is `mismatch`.
   - A written deck that does not run is `target-fails`, or `simulator-fails` when the control also
     fails on the original.
8. **Check against the lists**, strict both ways (`against`): an unlisted failure fails; a listed deck
   that passes, is refused, or lands in another class fails; in a full run a listed deck missing from
   the corpus fails. `CROSS_SIM_BLESS=<dir>` writes the observed lists.

A refused deck is counted per code set on stderr and never listed. An unlisted deck that goes from
pass to refused is therefore **not** a failure of this harness (§7.5).

### 4.2 Tolerances (`cmp.rs`; mismatches.md §4.1)

- RELTOL, VNTOL, ABSTOL: the larger of the defaults (1e-3, 1e-6 V, 1e-12 A) and any `.option(s)`
  value in either deck. GMIN is 1e-12 S.
- op, dc, ac: `|a − b| <= 2·(RELTOL·swing + floor)` per point. `swing` is the reference vector's
  largest magnitude. `floor` is VNTOL for a voltage and `ABSTOL + GMIN·Vmax` for a current. ac is a
  complex difference.
- Sweep grid: same point count, each point within `max(1e-9·span, 1e-6·|x|)`. A nested `.dc` on
  VACASK matches on any of the first four vectors.
- tran: `1e-2·(reltol/1e-3)·swing + 2·floor`, through a time tube of ±1 % of the stop time
  (`CROSS_SIM_TUBE`), over the common time range. Stop times must agree to 1e-9. In the last tube
  width the tube is tried the other way round (`end_holds`).
- Autonomous decks (no source with a waveform): mean period within 1 % and peak-to-peak within the
  tolerance over the second half; a current by its mean.
- Not compared: `@dev[...]` vectors and a missing op plot when the writer noted `W0052`;
  `inoise*` when a source's AC magnitude is not 1; an extra op plot on the target; integrated noise
  where one side has none. "No vector in common" fails.
- A Xyce `UIC`/`NOOP` or VACASK `icmode="uic"` transient has its t = 0 row dropped.
- LTspice cell: same tolerances (its defaults are the comparer's; TRTOL 2.0).
- The import suite's ngspice-against-ngspice comparer (`test_import_suite/compare.rs`) is tighter:
  `1e-9 + 1e-3·swing`, same tube, `.meas` within `1e-9 + 1e-3·|x|`, plots in the same order by name.

### 4.3 Classes and list line format

Line: `<deck relpath> <class>[ reason=<RULE-ID> proof=<file|harness>] # <first difference observed>`.
`#` starts a comment, which the gate ignores, except that `proofs.rs` compares some comments character
for character.

| Class | Meaning | Who decides |
|---|---|---|
| `mismatch` | a wrong number in a written deck: the writer's | harness |
| `target-fails` | the written deck does not run on the target | harness |
| `simulator-differs:source` | the target's simulator gives the same number on the original | harness control (`reason=S-TARGET-ON-SOURCE proof=harness`), or a proof file (`:by-hand`) |
| `simulator-differs:refinement` | agrees once both runs are refined | harness (`reason=S-NUMERIC-CONVERGES proof=harness`), or a proof file when the step is finer than the harness tries |
| `simulator-differs:{third-simulator, reference-data, closed-form, self-inconsistency}` | settled by a file under `proof/`; still observed as `mismatch` | the list + `lists.rs::admits`; `proofs.rs` re-derives 5 proofs |
| `simulator-fails` | the control also fails on the original | harness; with `reason=` and `proof=` it is observed as `target-fails` (`PROVEN_FAILS`) |
| `slow` | past the CPU budget or a raw file past 2 GiB: quarantined by name, not simulated | list |
| `source-fails` / `slow` in `<corpus>-source.txt` | the original does not run on its own simulator; no target is judged | harness |
| (refused) | the writer refused: counted per code set, never listed | n/a |

A line with a `simulator-differs` kind outside the 6 kinds, or without `reason=` and `proof=`, fails
the gate; so does a missing proof file. 24 rule ids and 25 proof names are in use.

### 4.4 List sizes today (lines = entries)

| List | Lines | Classes |
|---|---|---|
| `xyce-source.txt` | 526 | 514 source-fails, 12 slow |
| `xyce-ngspice.txt` | 480 | 148 target-fails, 115 mismatch, 87 refinement, 58 third-simulator, 48 self-inconsistency, 18 slow, 6 closed-form |
| `spice-bench-source.txt` | 1,989 | 1,957 source-fails, 32 slow |
| `ngspice-xyce.txt` | 167 | 102 reference-data, 25 target-fails, 22 mismatch, 9 slow, 7 refinement, 2 third-simulator |
| `ngspice-espice.txt` | 1,989 | 1,425 source, 560 simulator-fails, 4 slow |
| `ngspice-hspice-espice.txt` | 2,557 | 1,802 simulator-fails, 745 source, 5 slow, 2 target-fails, 2 closed-form, 1 mismatch |
| `ngspice-hspice-hsa.txt` | 97 | 80 simulator-fails, 10 refinement, 5 source, 1 slow, 1 mismatch |
| `ngspice-vacask.txt` | 49 | 34 target-fails, 7 refinement, 6 mismatch, 2 third-simulator |
| `vacask-source.txt` | 10 | 6 slow, 4 source-fails |
| `vacask-ngspice.txt` | 8 | 5 mismatch, 3 refinement |
| `hspice-source.txt` | 1 (blank) | 0 |
| `hspice-ngspice.txt` | 0 | 0 |
| `ltspice-ngspice.txt` | 8 | 4 mismatch, 4 target-fails |
| **total** | **7,881** | 154 `mismatch` + 213 `target-fails` are the open writer-side lines |
| `tests/fixtures/zero/untranslated.txt` | 5,318 | by target: spectre 819, hspice 803, vacask 765, espice 735, ngspice 731, ltspice 697, cdl 442, xyce 326. By set: xyce 3,269, ltspice 900, spice-bench 442, vacask 362, hspice 224, pdk-ihp 78, fixtures 37, pdk-vacask 6 |

Current pass and refused counts per cell: not stated (printed to stderr only). The last table on
file is FINDINGS-oracles round 1 (2026-11-06), which predates most fixes.

### 4.5 Dependence on the public surface

Tier A only (§1.4): `import::{decode, kit}`, `render::{render_kit, render_text, relink}`,
`resolve::{Kit, FileId, Role, Form}`, `diag::Code::W0052`, the message line format, and the CLI in
one test. It also uses `rawfile::parse_raw_all`, `result::*` and `unit::parse_spice_number`.

Three places know how today's writer spells things. A new writer that spells them otherwise
mis-pairs plots without failing loudly:
- `sim.rs::dc_loop` / `join_dc_passes`: an ngspice `.control` line `dc <src> $<var> ...` is the
  writer's `foreach` form of a decade/octave/list `.dc`; its passes are joined into one DC plot.
- `cmp.rs::swept`: the VACASK deck names its inner sweep vector `vsweep` (`render/named/vacask_bench.rs`).
- `test_import_suite::analyses`: counts analysis cards by text in the source and in the rewritten
  ngspice deck, including commands inside `.control`.

Two red cross-sim lanes seen on the day, neither a writer bug:
- batch-22 (`5c8c64f`): `vacask_corpus_sample` failed with
  `c6288 ... listed mismatch, now simulator-differs:refinement`. The batch's harness change (VACASK
  refinement by `maxstep`) moved the deck's class; the list line was updated in `1059821` and the
  rerun passed. That sweep now takes 170 to 178 s, where the whole lane took 47 to 51 s before.
- batch-20b: all 4 sweeps failed in 0.013 s because 19 GB was free under the temp dir (30 needed).

TESTING.md separately warns that a deck near the CPU budget can flap between `slow` and another
class; such decks are listed `slow` and not simulated.

---

## 5. The golden gate (`src/bin/golden`)

**What it records.** For every input (§3.2: 14,970) and every target in `Dialect::ALL` (9), under
`<out>/<set>/<relpath>/`:
- `<target>.txt`: the exact text `render_kit` writes.
- `<target>.refused`: `codes: <sorted codes>` on line 1, then the messages.
- `<target>.notes`: the same form, for a written deck that has notes.
- `parse.refused`: no reader for the input's dialect.
- `<name>.panicked`: a panic's message.
- `MANIFEST`: sorted lines for sets, kit entries and read order, each input's FNV-1a 64 content hash,
  each output's hash.

Each corpus deck is read in its own `Kit` (itself + includes); a PDK kit is read once from its entries.
The batch-22 record holds 116,368 `.txt`, 18,362 `.refused` and 68,417 `.notes` files, 2.7 GB.

**What it diffs.** `golden diff <baseline> <new> --xfail migration.xfail --refusal-status-only <report> --skip verilog`:
- `.txt`: byte for byte.
- `.refused`: by the code line only; with `--refusal-status-only`, refused-on-both is equal whatever
  the text, and each differing pair goes to the report.
- Written against refused always differs.
- `.notes` files are **not compared**.
- The `verilog` target is **skipped** by the gate script.
- `MANIFEST` is compared without its `output` lines; any other difference means different inputs,
  and no xfail line excuses that, except a `<key> manifest <reason>` line for an input re-read in
  another dialect.

**Baseline.** `/tmp/claude-1000/spicerack-golden/baseline-ccc7930-fx18` (2.7 GB, 14,970 inputs,
203,008 outputs), recorded on 2026-11-11 by the gate from batch-11b `ccc7930`. Its `.PROVENANCE`
file beside it names the commit, the IHP commit, the `flake.lock` hash and the tool versions.
It is not in the repository (§7.10).

**`migration.xfail`.** One line per key: `<set>/<relpath> <target> <reason>`. The target is a dialect
name, `parse` or `manifest`. The list is strict: an unlisted difference fails, and a listed key that
is now identical fails (`XPASS`). Lines sit under headers naming the ledger row and its oracle.

Today it has 4,103 entries (4,235 lines):
- by target: vacask 2,442, ltspice 374, spectre 337, cdl 312, xyce 249, ngspice 247, hspice 77, espice 65;
- by set: spice-bench 2,097, xyce 1,866, ltspice 105, vacask 21, hspice 14.

The batch-22 gate printed `IDENTICAL except 3687 listed xfail(s)`. The other 416 listed keys are,
by my reading of `diff.rs`, refused on both sides with different text, which the status-only mode
neither counts nor flags (inferred, not run).

When the list grows large, the baseline is re-recorded and the list restarts empty: fx15, then fx16
(17,172 lines absorbed), fx17, fx18 (19,770 lines absorbed).

**`golden zero`** is gate Z1 (§2.4). **`check-determinism <tree>`** renders everything again in
another process with another `GOLDEN_JOBS` and the queue reversed. It compares every byte, including
`.notes` and messages, and the whole MANIFEST.

**Correctness or sameness?** Sameness only. TESTING.md says so in its oracle matrix legend:
"`G`: byte-identical to the old writer only (the golden gate), no oracle of correctness yet". Three
facts make it concrete:
1. The original oracle was a record of the old PEG reader and old writers (`9d1d6bb`, later
   `baseline-daa0ad0`). A pipeline that was golden-identical at the time still held known-wrong
   behaviour: ZERO-PLAN (2026-11-07, line 51) counts about 50 WRONG and 45 PARTIAL refusal cells of
   about 210 audited, beside a list of wrong numbers. `tests/fixtures/transpiler/README.md` calls
   its inputs "known-bug inputs" whose baseline "records exactly that".
2. Since fx15 the baseline is a record of the **new** pipeline at an earlier commit. Tens of
   thousands of intended differences were folded in at each re-record. Each was argued by its own
   ledger row and oracle, but the tree itself cannot tell a right byte from a wrong one.
3. In the oracle matrix, 36 of the 54 source × target cells are `G`: every pair that is not
   ngspice-centred or a same-dialect fixpoint. §7.4 has the full split.

What the golden gate does prove, and nothing else does at this scale:
- No unintended change over 134,730 (input, target) keys.
- The written/refused status and the refusal code set per key.
- No panic.
- Determinism across job counts.

For a rewrite that aims at byte-identical output it is the right migration gate, with a baseline
re-recorded at the branch point so the list starts empty. For a rewrite that changes spelling, only
its status and code-set census transfers.

---

## 6. Diagnostic-code coupling

**Catalogue.** `src/syntax/diag/`: `mod.rs` (`Code`, `Kind`, `Info`, `Diag`, `info`, ranges),
`rows_lp.rs`, `rows_r.rs`, `rows_w_spice.rs`, `rows_w_named.rs`, `rows_s1.rs` .. `rows_s4.rs`,
`rows_w_def.rs`, `table.rs`, `messages.rs`.

There are 115 codes: P 18, R 13, W 84 (61 refusals, 23 notes), L none. The stated rule is "Codes are
never renumbered or reused". `W0201`/`W0202` carry a `detail` naming a plugin's own code.

**Rust integration tests.**
- 281 `Code::<number>` references in 44 files (39 of 53 binaries), 82 distinct codes.
- Counting strings and comments: about 460 mentions, 86 distinct.
- About 190 of the 532 `#[test]` blocks mention a code (block-level grep; slightly high).
- Most asserted: `W0001` 24, `W0039` 15, `W0023` 12, `W0104` 11, `W0027` 10, `W0016` 10.
- Heaviest files: `test_spice_write` (20 distinct), `test_render` (18), `test_read` (12).
- Many assert the **exact code list in source order** (`spec_refusal`, `assert_eq!(codes, [..])`),
  not just membership.

**Python.**
- 388 mentions in 12 collected files, 30 distinct.
- `test_python_conformance.py::EXPECTED` is 112 features × 9 targets = 1,008 cells, 19 distinct
  codes; a refused cell is one exact code or a `+`-joined set.
- 39 of 657 test functions mention a code.

**All tests together:** 93 of the 115 codes are named at least once. Not named anywhere in `tests/`:
`P0005-7`, `P0011`, `P0020-26`, `R0002`, `R0004-5`, `R0008-10`, `W0037`, `W0107`, `W0110`, `W0115`,
`W0137`.

**More than the number is pinned.**
- Titles as strings: 6 in `test_sim_command.rs`, 8 in `src/syntax/diag/mod.rs`, several in
  `test_python_errors.py` and `test_simulators.py`.
- The message line format `<name>:<line>:<col>: <CODE> <title>`: `test_python_errors.py`,
  `test_python_diagnostics.py`, `test_dialect_plugin.rs`, `test_cross_sim/kit.rs`, `test_spice2rack.rs`.
- `Kind` per code (`info(c).kind == Kind::Refusal`): 9 files.
- One `cite` string (`test_ltspice_dialect.rs`).

**Data files that carry codes.**
- Every golden `.refused` and `.notes` first line.
- `fixtures/zero/untranslated.txt`, defined by `W0001`/`W0032`.
- `migration.xfail` reasons (19 distinct, 1,165 mentions).
- `fixtures/transpiler/pinned/*.refused`.
- `fixtures/dialects/acme-bench-refused.txt`.
- `tests/ltspice/pending.json` (`refused W0013`, `W0008`).
- `fixtures/schemas/spicerack.diagnostics-1.schema.json`.
- The cross-sim harness's one semantic use, `W0052`.

Renumbering would break all of the above. Keeping the `Code` enum's names and each code's `Kind` is
cheaper than touching about 700 references.

---

## 7. Gaps

### 7.1 Is there any test asserting `read(write(ir)) == ir`?

Not at the public level, and not on a dialect-neutral structure. What exists:

| Nearest thing | Where | What it actually asserts |
|---|---|---|
| Text fixpoint | `tests/test_import_suite/fixpoint.rs` (5 corpora + a `should_panic` Spectre marker), `spice2rack --check`, `test_sections`, `test_conditionals`, `test_dialect_plugin`, `test_acme_oracles` | `write(read(write(read(x)))) == write(read(x))` in one dialect. It proves the second pass is stable, not that the first write kept the meaning. A loss that is the same on both passes is invisible |
| Built tree equals parsed tree | `src/syntax/tests/build.rs` (5 tests) | a deck built by `syntax::build::Build` is row for row the parse of the text it wrote: today's AST, ngspice-given text only |
| Python store round trip | `tests/api_parity.py::test_store_round_trip` | `from_source(build.netlist(d), d)` netlists like the build, for ngspice and espice only |
| Value round trip | `src/syntax/tests/render.rs`, `render/spelling.rs`, `device.rs` | a number with each suffix reads back as the same f64 in every target; device letters invert |
| Meaning compare | `tests/common/sections.rs` (used by `test_sections*`) | SPICE and Spectre sections compared through a 150-line hand parser of the ngspice text |

**`tests/api_parity.py` is not collected by the gate.** This is inferred, not run:
- The file name matches neither of pytest's default patterns (`test_*.py`, `*_test.py`).
- `pyproject.toml` sets only `testpaths`; `conftest.py` has no collection hook; nothing imports the module.
- The gate reports 814 passed + 4 xfailed + 3, and exactly 4 xfail markers exist in the collected
  files (1 in `test_python_conformance.py`, 3 in `test_python_simulator.py`). `api_parity.py` alone
  would add about 1,000 parity cases and 48 marked xfails.

So the tree's only store round trip and its only "builder against hand-written text twin"
differential do not run in CI or in the batch gate.

### 7.2 Is there a per-dialect reader test independent of writers?

Only inside the crate, and only on today's AST shape: `src/syntax/tests/{mod, heads, spicerack}.rs`
(43 tests), the lexicon tests (30) and `resolve/tests.rs` (21). Each takes a manual clause and
asserts a printed tree. No test, in-crate or integration, reads a dialect and asserts a
dialect-neutral meaning ("this HSPICE line is a resistor between `a` and `b` of 1e3 Ω").

Every integration test observes a reader through some writer's text. `test_read.rs` says so in its
own header: "what a deck's statements become", checked on the written deck. Reader and writer bugs
are confounded, and a pair of matching bugs cancels.

### 7.3 Is there a writer test independent of readers?

No. Every writer test feeds text through a reader (`render_text(text, from, name, to)`).

- The Python builders reach the writers through the ngspice-form deck and the same pipeline.
- The only reader-free writer tests are private string helpers (`render/spice/{ternary, powers, table, globals}.rs`,
  about 25 tests).
- No test constructs the writer's input directly and asserts one target's text.

### 7.4 Correctness is covered only around ngspice

TESTING.md's matrix has 6 sources × 9 targets = 54 cells:

| Cells | Count | Which |
|---|---|---|
| with a correctness oracle | 11 | ngspice → {ngspice, ESPice, HSPICE, Xyce, VACASK} by simulators, → CDL by netgen, → Verilog by iverilog; {HSPICE, Xyce, VACASK} → ngspice by simulators; Spectre → ngspice by closed forms (a few constructs) |
| `G`: sameness only | 36 | every other off-diagonal pair |
| text fixpoint only | 5 | HSPICE, Xyce, LTspice, Spectre, VACASK onto themselves |
| pending a licensed run | 2 | ngspice → LTspice, ngspice → Spectre |

So 43 of 54 cells have no correctness oracle today.

The matrix lists LTspice → ngspice as `G`, while the Oracles table describes a 34-deck LTspice →
ngspice cell (nightly, `.#testing-ltspice`). The matrix looks stale there.

With an IR, the 36 cells factor into "reader A to IR" and "IR to writer B", and each half can be
tested once.

### 7.5 Refusal and note regressions are thinly gated

- The cross-sim harness does not fail when an unlisted deck goes from pass to refused (§4.1).
- The import suite counts other dialects' refusals and does not fail on them.
- `golden diff` ignores `.notes`. A deck that keeps its bytes but loses a note ("left out, noted")
  passes. Only `check-determinism` reads notes, and it compares a build with itself.
- The gate skips the `verilog` target, 14,970 outputs. The Verilog writer has no corpus-level gate
  beyond determinism, `test_structural` (2 tests) and a few Python assertions.
- So the gates on "refuses more than before" are: the golden status + code line (sameness), the
  Python matrix (1,008 cells, Python-built decks only), and Z1 (two codes only).

An IR makes a conservation property testable: every IR node is written, noted, or refused, by
construction.

### 7.6 No structural equivalence check without a simulator

Netlist-graph equality (same devices, same nets) is checked only for CDL, by netgen LVS, on 2 tests.
With an IR, `ir(read_B(write_B(ir)))` isomorphic to `ir` is cheap for every pair and needs no
licence. That matters most for Spectre and HSPICE, which have no local simulator.

### 7.7 No generated inputs

There is no property-based or fuzz testing:
- dev-dependencies are `pretty_assertions` and `tempfile`;
- the Python env has `numpy`, `pytest`, `pytest-xdist`.

Inputs are hand-written decks and fixed corpora. The 10⁴-deep nesting tests are the only stress cases.

### 7.8 Spectre and LTspice

- No Spectre corpus (`spectre_corpus_fixpoint` is a `should_panic` marker); 33 Spectre fixture inputs.
- No Spectre binary. HSPICE text runs on ESPice's reader, which itself has a named known gap
  (+5.1 % drain current on sky130, `tests/espice/*.sh`) and 1,297 `ESPICE-HSPICE-READER` list lines.
- LTspice runs only in `.#testing-ltspice`; CI carries a marker instead.
- The LTspice writer's 8 pending decks have never run.

### 7.9 Pinned evidence that a rewrite can silently void

21 Spectre decks (`tests/spectre/*.scs`) are byte-pinned. For 18 of them real Spectre ran exactly
those bytes:

| Decks | Real run | Pinned by |
|---|---|---|
| `t_s1`, `t_s2_*` (3), `t_s3_*` (2): 6 decks | Spectre 21.1 | `src/syntax/render/named/lift.rs::the_decks_spectre_ran_are_written_byte_for_byte` |
| `p_*.scs`: 11 decks | Spectre 25.1, iccad4, 2026-11-07: 10 pass, 1 fail | `test_pending_real_run.rs`, which stores each deck's FNV-1a as it ran (`SPECTRE_RUNS`) |
| `t_m4_dependent.scs` | Spectre 25.1 (`expected.json`) | `test_model_kinds.rs` |
| `t_m3_diodes.scs` | **already unrun**: Spectre ran an earlier text; the writer changed it (P3-089) and `expected.json` now says `UNRUN_SPECTRE_HSPICE` | `test_model_kinds.rs` |
| `t_m1`, `t_m2` | not stated | `test_model_kinds.rs` |

Also:
- One pin is keyed by the FNV-1a of the written text (`tests/spectre/pins/ed0648fe1d0bda7b.pin`;
  its provenance line cites the `p_dc_sweep` run). `oracle.rs` says of it: "a writer change that
  moves one byte loses the pin: no run can replace it (ZERO D43)".
- The remote host is gone ("iccad4 access lost, ZERO D43").

A new Spectre writer either reproduces these bytes or drops the only real-Spectre evidence the
project has. `t_m3` is the precedent.

For the 11 `p_*` decks the loss is at least recorded: a re-bless writes `UNRUN_SPECTRE_HSPICE`
into `pending.json`. For the 6 T-S decks nothing records it: updating the pinned file keeps the
test green.

### 7.10 Oracle assets that live outside the repository

- The golden baseline (`baseline-ccc7930-fx18`, 2.7 GB) is under `/tmp/claude-1000/spicerack-golden/`.
- So is the evidence tree, `research/impl/evidence/` (207 directories). Tests, `src/` and TESTING.md
  cite `research/...` paths from 108 files, 95 distinct evidence directories. `research/` has 0
  tracked files in the repository.
- That `/tmp` is ext4 on the same disk as the repo, not tmpfs (BUILD-TEST-SPEED §header).
- Whether any of it is backed up: not stated.
- `IHP_PDK` is not pinned by the flake.

### 7.11 Smaller holes

- `tests/fixtures/golden/` (the directory `oracle::golden` reads) does not exist. The "golden with
  provenance" template has no fixture; only `test_oracle.rs` exercises its guards.
- Zero gates: only Z1's list is checked in. Z2's `tests/fixtures/zero/citations.toml` (ZERO-PLAN
  line 22) is not in the tree.
- TESTING.md is stale on the fixture count (18 against 53), the VACASK count (151 against 148) and
  the LTspice → ngspice matrix cell.
- The cross-sim comparer shares `rawfile::parse_raw_all` with the product. mismatches.md records one
  real bug found that way (`H-RAW-PLOTNAME`).
- Diagnostic positions in the Python API are positions in a deck the user never sees (§1.3).

---

## 8. Gate runtime budget

### 8.1 Tiers as documented (AGENTS.md, TESTING.md)

| Tier | Budget | Contents |
|---|---|---|
| pre-commit | < 10 s | `cargo fmt --check`, clippy `-D warnings`, `check_style.py src tests`; `perf.py pre-commit` < 10 s |
| merge | < 15 min | both suites in `.#testing` (CI: `cargo test`, then `maturin develop --target-dir target/py` + `pytest -v`), `nix build .#spice2rack` + one kit |
| nightly | 180 min CI timeout | `test_import_suite` and `test_cross_sim` with `--features corpus --release` (about 20 min of a 4-vCPU runner for the import suite; cross-sim 31 min on 12 workers, 5.1 CPU-h); `GOLDEN_JOBS=1 golden check-determinism` by hand (447 + 18 CPU-s) |
| golden | local, before each rewrite phase | record, diff, zero, determinism; `test_syntax` with `--features corpus` |
| perf | local | `perf.py pre-commit` < 10 s, `merge` about 2 min, `nightly` about 5 min |

CI does not run the golden gate or `test_syntax` (they need `$IHP_PDK`). CI runs without LTspice.

### 8.2 What the local batch gate costs today (`batch-gate2.sh`; `batch-19.out` GREEN, `batch-22.out`)

Wall time is about 5.3 to 6.5 min (318 s, 319 s and 390 s for batches 20b, 21 and 22, from log timestamps).

| Stage | Step | batch-19 | batch-22 |
|---|---|---|---|
| 0 (serial; red stops here) | shell profile | 5 s | 5 s |
| | `cargo fmt --check` | 2 s | 1 s |
| | `check_style.py src tests` | 0 s | 0 s |
| | clippy `--all-targets --all-features -D warnings` | 6 s | 10 s |
| 1 (serial) | `cargo nextest run --no-run` | 6 s | 14 s |
| | `maturin develop --target-dir target/py` | 6 s | 9 s |
| | `cargo build --release --no-default-features --bin golden` | 13 s | 13 s |
| serial | xsim: the 4 cross-sim sample sweeps alone (they need the 30 GB) | 48 s | 178 s (170 s on rerun; 51 s in batch 21, before the VACASK refinement) |
| 2 (lanes) | nextest (everything but xsim) | 118 s, 841 tests | 114 s, 897 tests |
| | `cargo test --doc` | 1 s | 3 s (0 doctests) |
| | pytest `-n 8`, minus 3 tests | 86 s, 803 + 4 xfailed | 104 s, 814 + 4 xfailed |
| | 3 sky130 tests, each in its own process | 86-90 s | 86-88 s |
| | `golden record` (`GOLDEN_JOBS=16`) | 69 s | 67 s |
| | `golden diff` + `golden zero` | 17 s | 30 s |
| | `golden check-determinism` (`GOLDEN_JOBS=7`) | 105 s | 92 s |
| | `nix build .#spice2rack` + kit | 28 s | 33 s |

Where the time goes in nextest (batch-22 sums):
- `test_steps` 372 s and `test_waveforms` 173 s are the LTspice queue;
- lib unit tests 133 s (one 85 s 10⁴-deep test);
- `dialect_defs` 116 s;
- `test_monte_carlo*` 207 s;
- `test_backends` 99 s;
- `test_sections_pdk` 76 s.

The remaining 43 integration binaries sum to about 2.4 minutes of test time. Three tests account
for 50 s of that (`p3_036` 20 s, `p3_086` 19 s, the `.pss` oscillator 11 s).

Before the speed work (BUILD-TEST-SPEED §1, batches 9 to 14b) a gate was 429 to 690 s, and 14 of 18
full gates that day were red: `golden-diff` in 9 and `zero` in 2 (ten gates, one with both), nextest
in 5, pytest in 2, xsim in 1.

### 8.3 Machine constraints stated in the docs and scripts

- Gate box: i9-14900HX, 32 threads, 31 GB RAM, 31 GB swap, shared with other agent sessions.
- AGENTS.md:
  - one cargo process per agent;
  - no build while `free -g` shows under 6 GB available or `df -h /` under 30 GB free;
  - flags in `.cargo/config.toml` (no such file exists in the tree).
- Gate script: `CARGO_BUILD_JOBS=12`, `GOLDEN_JOBS=16` for the record and 7 for determinism, pytest `-n 8`.
- `golden record`: 16 jobs is the measured optimum (32 is slower). Peak RSS is 6.0 to 6.3 GB.
  Inputs of 32 MB or more render at most 3 targets at once (9.4 GB unbounded). Determinism peaks at 4.6 GB.
  Three records at once pushed the box into swap (BUILD-TEST-SPEED F2).
- Cross-sim:
  - 30 GiB free under the temp dir or the sweeps fail at once;
  - 2 GiB per output file;
  - 60 / 120 / 240 CPU-s per run;
  - one worker per CPU unless `CROSS_SIM_JOBS` is set.
- nextest: `fail-fast = false`, `slow-timeout = 120 s` (it marks tests SLOW; no kill is configured).
- LTspice: one run at a time per user (a `flock` in the wrapper); a queued launch costs about 5 s for
  2 s of LTspice. Other sessions' runs share the queue.
- pytest: three sky130 `tt`-corner tests take 34 to 55 s each on ngspice (TESTING.md) and 86 to 88 s
  in the gate.
- Corpus tier: fixpoints peak at 2.1 GB RSS; the spice-bench round trip is 2,586 CPU-s.
- Golden disk: 2.7 GB per record tree, plus the 2.7 GB baseline.

---

## Appendix: what to carry into the plan

1. The 41 KEEP + 4 ADAPTER binaries and 29 Python files run on a new implementation if it keeps the
   facade of §1.3. The single choke point is `render_text` (43 binaries) with
   `Written` / `Diags` / `Code`.
2. KEEP does not mean "passes unchanged". Many of those tests assert exact lines. A writer that
   re-spells output turns them into review items, and for the 18 Spectre decks of §7.9 a review
   cannot restore the evidence.
3. The strict lists (cross-sim 7,881 lines, known failures, Z1 5,318 names) are strict both ways. A
   rewrite that fixes a listed deck also goes red until the line is removed.
4. `tests/api_parity.py` should be collected before the rewrite starts: it is the only round-trip
   and build-against-text differential on the Python side.
5. The golden baseline and the evidence tree need a home outside `/tmp` before they are relied on
   as the rewrite's gate.
