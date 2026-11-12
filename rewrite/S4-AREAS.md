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

## Shared between S4.2 and S4.3: dc plus `type` alters (R42; `fold.rs`, the integrator's)

`DcRule::Given` with a wave is VACASK's `type="dc"` source carrying the wave's parameters, plus
`type` alters between the analyses. Proof: settle `dcw.sim` against ngspice-45's `dcw.cir`
(Appendix A run dcw).

**Reader.** `fold::folds` walks every top-level `control` block before lowering; `Lower.folds`
holds the verdicts.
- `l.folds.of(name, top)` for a source (its whole name; `top`: it stands at a file's top):
  - `Some(Fold::Given(word))`: `read_sources` reads `DcRule::Given`, `dc` from `dc=`, and the
    wave `word` names (`pulse`, `sine`, `exp`, `pwl`, `am`) from the line's own parameters.
  - `Some(Fold::Other)`: the source line is `Private` (code: `read_sources`' own).
  - `None`: no `type` alter reaches it; a parameter its `type` does not read is dropped with
    `read_sources`' note (P2203).
- `l.folds.is_framing(l.cst, n)` for an `alter`: `true` means a `Given` fold consumed it; `read_run`
  pushes no item for it (it is accounted for as framing, like a closer). Any other `alter` that
  sets `type` (`Fold::Other` for a source it names) is `Private` in `read_run`, under the same
  code as its source line; ask me for a shared code if you want one.
- A fold holds exactly when:
  - the source line is at a file's top and has `type="dc"` or no `type`;
  - every `type` alter naming it stands directly in the block (not in `mc` or a loop), names
    that one source and sets `type` alone;
  - each alter is followed by an analysis before the next alter for the source, and that
    analysis needs the switch: `type="<wave>"` before a transient, `type="dc"` before a DC-kind
    analysis (op, dcinc, dcxf, ac, acxf, noise, acstb, acsp);
  - every analysis sees its side: a DC-kind one sees dc, a `tran` the wave (the line starts at
    dc), and the block has no other analysis kind;
  - one wave word throughout, at least one transient, and no alter of it in another block.

**Writer.** `fold::admits` refuses (W2007) a `Given` source with a wave unless:
- it stands at the entry file's top;
- every analysis stands at the entry file's top and is DC-kind (Op, Dc, Ac, Noise, DcXf, Xf,
  Stb, Sp, DcInc) or `Tran`;
- at least one analysis is a `Tran`;
- the wave has a VACASK word (`fold::wave_word`).

Text, exactly:
- `write_sources`: `<name> (<p> <n>) <master> type="dc" dc=<dc> <the wave's parameters as for
  type="<word>">` (`mag=`/`phase=` as usual).
- `write_run`: `fold::given(design, top)` lists the sources in item order with their words.
  Before each analysis whose side differs from a source's current state (the state starts at
  dc), write one line per such source, in that order, after the previous analysis's line and
  before this analysis's `sweep` lines: `alter instance("<name>") type="<word>"` before a
  `tran`, `alter instance("<name>") type="dc"` before a DC-kind analysis. Nothing after the last
  analysis.

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

## Known failures (strict both ways: a line goes the day its cause is fixed)

| Test | Fails because | Goes when |
|---|---|---|
| `dialect_laws` l5 (`vacask`) | seed 1 Untyped: a ccvs, vcvs or isource whose model names no definition reads back Unresolved, and an unresolved element's mult reads back as a `$mfactor` parameter (the row in `tests/dialect_laws.rs`; the invented-name cause, W2301, is gone with R59's naming rows) | vc-sources and vc-devices' fix |
| `dialect_laws` l7 (`vacask`) | no ngspice↔VACASK passes until S4.4 | S4.4's passes |
| `dialect_laws` reach (`vacask`) | Item(Save) (W2303), Elem(Capacitor/Inductor/Mosfet) (W2104, W2105, W2108), Elem(Coupling) (never generated), Elem(Diode) and Elem(ModuleModel) (`interim`: fixture fixpoint), Wave(Am) (W2201): each names its covering property in KNOWN_REACH | each row with its cause; the two `interim` rows by S4.5 |

A scale letter VACASK lacks (`c`, `peta`) is already respelled exactly as `e<exp10>` with the same
digits (R28): it reads back equal by value (`eq_meaning`), not field by field.
