# S4 areas: the VACASK dialect's files, their functions and their contracts

The list `rewrite/DIALECT-VACASK.md` §2 asks for: every file of `src/dialect/vacask/`, its
owner sub-slice, the functions it exports to the spines, and the contract of each (also each
function's doc comment, which wins where the two differ). S4.0 wrote the two spines and every
file; a stub's contract is what its sub-slice must make true.

Shared types: `read::Lower` (the builder, the diagnostics, the file's CST, the current scope,
the ground names, the files loads and includes name, the parameter reads to bind) and
`write::Writer` (the design, the text, the depth).

## Rules every area function keeps

| # | Rule | Checked by |
|---|---|---|
| A1 | A reader area function accounts for its statement: rows through `l.b`, or exactly one `Opaque` (`read::opaque`, `read::unread`) with a coded diagnostic. It never returns early otherwise and never panics on input. | L1; reader fixtures |
| A2 | An expression is checked before any row is pushed (`expr_read::expr` returns `Err` with the statement's class and code), so a refused statement leaves no orphan rows. | L2 (`ir::check`) |
| A3 | Names are exact bytes (VACASK keeps case); a node named in `l.grounds` is `0`. | reader fixtures |
| A4 | A parameter read is recorded in `l.reads` with the scope it is read from; `read::bind` binds it by VACASK's rule (own subcircuit, then the top; never an enclosing subcircuit). | `scope` fixture |
| A5 | An area adds to its `FEATURES` exactly what its rows can use (the reader's image), and to its `CODES` a row per code it pushes, in VACASK's range (`P2xxx`, `R2xxx`, `W2xxx`). | L5, L7; `codes::tests` |
| A6 | A writer area function writes its statement's first line at `w.depth`; the spine writes bodies and closers. It writes nothing for a row its file's `FEATURES` lacks (it is never called for one). | L4, L5; writer fixtures |
| A7 | Every area function names, in its doc, the VD clause or the Appendix A run behind each rule. | review |

## S4.0 (written)

| File | Exports | Contract |
|---|---|---|
| `mod.rs` | `Vacask` (`Read`, `Write`), `VACASK`, `FEATURES`, `info` | the row of `dialect::BUILTIN`; `FEATURES` = union of the area parts |
| `read.rs` | `Cst`, `Lower` (`span`, `text`, `body`, `comments_until`), `references`, `read`, `opaque`, `unread`, `statement_text` | sources → CST (a library behind one empty line) → declare (ground names, loaded and included paths) → lower in source order with comments from the gaps → bind |
| `write.rs` | `Writer` (`line`), `write` | title line, items in order, bodies indented two spaces per level with an explicit stack, one exhaustive `match` over `ItemKind` |
| `number.rs` | `Number`, `NotANumber`, `lex`, `spell` | token bytes → `Literal` and VACASK type, no float arithmetic; `spell` reads back through `lex` |
| `name.rs` | `GROUND`, `Place`, `unquote`, `spell` | `'…'` parts and `''`; bare only when VACASK reads it bare |
| `expr_read.rs` | `Context`, `Refusal`, `FEATURES`, `CODES`, `expr` | §3.3 of the plan; `/` is `DivTyped`, integer literals `Int`, `int` `ToInt`, `sgn` `SgnZeroPos` (A12 K1) |
| `expr_write.rs` | `write_expr` | VACASK precedence, fewest parentheses, the reader's tree back |
| `read_structure.rs` | `FEATURES`, `CODES`, `title`, `include`, `section`, `subckt`, `if_chain`, `param`, `global`, `ground`, `load`, `embed` | §3.2 rows title … `@if`; formals are the constant `parameters` entries (RUN c1) |
| `write_structure.rs` | `CODES`, `title`, `comment`, `include`, `section`, `section_end`, `load`, `subckt`, `subckt_end`, `param`, `global`, `branch`, `if_end`, `opaque` | the inverse of `read_structure` |
| `codes.rs` | `CODES`, `UNREAD`, `PRIVATE`, `info` | the spine's rows and the lookup over every area's table |
| `tables.rs` | `CONSTANTS`, `LIBRARY_FILES`, `is_library_file` | VD expr-constants; the shipped `lib/vacask/mod` files |

## S4.1 devices and models (stubs)

| File | Exports | Contract |
|---|---|---|
| `read_elements.rs` | `FEATURES`, `CODES`, `instance` | `name (nodes) master params` → `Element` of its master's kind (model, subckt, user module; `Unresolved` when the kit lacks it); `$mfactor` → `mult` on a device, a parameter on a subckt instance (RUN n2); the behavioural source (no master) → `Behav` with `Context::Behavioural`; R, C, L parameters → `Passive` fields (plan §3.2) |
| `write_elements.rs` | `element` | the inverse; a name no quoting spells (whitespace, VD input-identifiers) is `Write::admits`' refusal (R21) |
| `read_models.rs` | `FEATURES`, `CODES`, `model` | `model name module params` → `Model{family, polarity, params, module}` (`families.rs`); `model_<p>`/`instance_<p>` renames; builtins → the families of plan §10 Q15 |
| `write_models.rs` | `model` | the inverse; the module word from the family |
| `families.rs` | `FAMILIES` | `(module, family, library file)` rows, plan §3.2 (model) |

## S4.2 sources (stubs)

| File | Exports | Contract |
|---|---|---|
| `read_sources.rs` | `FEATURES`, `CODES`, `source` | `vsource`/`isource` with `type=` and VACASK's defaults filled (pulse `rise=1n fall=0 width=0 period=0`, `PulseLaw::Vacask`; exp `td2_from: Td1`; sine/am/fm `pre: HoldDelayValue`; `DcRule::WaveAtZero` on a waveform); controlled sources; `mutual`; `tline_ideal` |
| `write_sources.rs` | `source` | the inverse |

## S4.3 the control block (stubs)

| File | Exports | Contract |
|---|---|---|
| `read_run.rs` | `FEATURES`, `CODES`, `control` | plan §3.4: run items in order; `sweep` + `op` is a `Dc`, around any other analysis a `Loop{Sweep, Restore}` (RUN s1); `options` split into typed keys, one `Hint` opaque, `Private` accuracy targets; every state item (`Option`, `Env`, `NodeSet`) `applies: FromHere` (A14, R24); `mc … endmc` a `Loop{MonteCarlo}` (an explicit stack) |
| `write_run.rs` | `analysis` | one `control` block; a loop's `sweep` lines repeated before each analysis of its body; generated analysis names `<kind><n>` (the backend reads them until S12) |
| `read_outputs.rs` | `FEATURES`, `CODES`, `save` | `save` directives (read from tokens: the CST drops a probe's parentheses) → `Save`; `ic=`/`nodeset=` lists → `NodeSet` (`next_only`, differential pairs) |
| `write_outputs.rs` | `save` | the inverse |

## Known gaps in the shared harness (for the spine owner)

- `tests/dialect_laws.rs` writes every file of a generated design and reads the concatenation as
  one source: right for the IR text (its `file` lines), wrong for a dialect whose files are
  separate texts. A row with `files > 1` needs the written files as `Sources` under their paths.
- VACASK reads line 1 of an entry deck as its title whatever it holds, so the writer spells an
  absent title and an empty one alike (an empty line). L5 needs `Gen` to give an entry deck a
  non-empty `Title` as its first item, or none in a library.
- `Write::admits` (R21) needs name pools per family for `Gen` (parameter names a module has).

## Evidence: how to check a VACASK fact

A rule cites a VD clause or a run; when the two disagree the run wins (R5). Everything below is in
`nix develop .#testing`.

| What | Where |
|---|---|
| the binary (36be560) | `vacask` on PATH; `/nix/store/dhxz6l3iac3r2b8n91kcz04a0j3hs4s6-vacask-unstable-2026/bin/vacask` |
| its docs (`VD <file>`) | `…-vacask-unstable-2026/share/doc/vacask/docs/<file>.md` (`dev-builtin-src.md`, `cmd-sweep.md`, `cir-instance.md`, `expr-functions.md`, …) |
| its demo and test decks | `…-vacask-unstable-2026/share/doc/vacask/{demo,test}/`; the corpus the tests walk is `$VACASK_CORPUS` (184 netlists, gnucap decks out of set) |
| its device modules | `…-vacask-unstable-2026/lib/vacask/mod/` (`spice/*.osdi`, `spice/{sn,full}/`, the CMC modules): what `tables.rs::is_library_file` lists |
| the settle runs | `tests/fixtures/dialect/vacask/settle/*.sim`, asserted by `tests/dialect_vacask/settle.rs`; DIALECT-VACASK Appendix A records each |
| ngspice-45, for pair facts | `ngspice -b deck.cir` in the same shell (the `ngspice` outside it is 44.2: do not use it) |

How the settle runs read a number (copy the pattern for a new fact, then add the deck and its
test in the same commit as the rule that cites it):

- **A parameter's value, typed**: `control` … `print ("label", expr, …)` … `endc` prints
  `label value …` on one line (VD cmd-print). Dividing by an integer shows the type: `x/2` is
  1 for an integer 3 and 1.5 for a real (runs i1, f2–f4).
- **What an instance received**: `print instance("r1", "x1:r1")` dumps each instance's
  parameters (`resistance = 11 (real)`): a resistor of `r=expr+10` reads `expr` back.
- **A waveform or a sweep**: `options rawfile="ascii"` and an analysis; read the raw file with
  `spicerack::rawfile::parse_raw` (the settle tests' `raw`/`at` helpers).
- **A refusal**: VACASK prints the message and a location line (`9:10 (0xad) in …`); assert the
  message text (`settle::says`).
- Every builtin needs a `model` line (`model vsource vsource`); every SPICE device a `load
  "spice/<m>.osdi"` and a `model` (run n2).
