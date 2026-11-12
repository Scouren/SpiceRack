# Slice S3: the ngspice dialect (reader and writer)

The first real dialect and the tracer of the whole design: when it lands, real decks go text →
IR → text and run on ngspice with the numbers of the original. Read `REWRITE-PLAN.md` §2–§4 and
§7, `rewrite/SPINE.md`, `rewrite/IR-KERNEL.md`, `IR-CIRCUIT.md`, `IR-RUN.md`, `RULINGS.md` first.
Everything here is about ngspice-45 (the pinned simulator); a rule is a fact of ngspice with its
evidence (a row of `rewrite/INV-semantics.md` or `INV-constructs.md`, an ngspice manual clause, a
run), never a memory of what the old writers did.

## 1. What the slice delivers

`src/dialect/ngspice/` implementing `Read` and `Write`, one row in `BUILTIN` (`name: "ngspice"`,
`text_family: "ngspice"`), its own diagnostic table, its feature set, its fixtures, and the
`ngspice → ngspice` route flipped when the gate of §7 passes. ESPice and RSpice (flavours) are a
later slice and must cost no edit here beyond the flavour struct of §2.

## 2. Files and sub-slices

One owner per file; the two spine files are written once in S3.0 and name every area function,
so the later sub-slices never edit them.

| Sub-slice | Files | Holds |
|---|---|---|
| S3.0 scaffold | `mod.rs` (the `Read`/`Write` impls, `FEATURES`, the `Flavour` struct), `read.rs` (spine: sources → CST → declare → lower, the statement dispatch), `write.rs` (spine: one exhaustive `match` over `ItemKind`), `number.rs`, `name.rs`, `expr_read.rs`, `expr_write.rs`, `read_structure.rs`, `write_structure.rs`, `codes.rs`, `tables.rs` | numbers, names, expressions, and the structure items: title, comments, `.include`, `.lib` calls and sections, `.subckt` (nested too), `.param`, `.func`, `.if` chains, `.global`, `.end`; everything else an `Opaque` of class `Unread` through the area stubs |
| S3.1 elements | `read_elements.rs`, `write_elements.rs`, `read_sources.rs`, `write_sources.rs` | every element card and source waveform ngspice has |
| S3.2 models | `read_models.rs`, `write_models.rs`, `families.rs` | `.model` cards, (type word, level) ↔ `ModelFamily`, bins |
| S3.3 run | `read_run.rs`, `write_run.rs`, `read_outputs.rs`, `write_outputs.rs` | analyses, `.options`, `.temp`, `.ic`, `.nodeset`, `.save`, `.print`, `.probe`, `.meas`, `.four` |
| S3.4 control | `read_control.rs`, `write_control.rs` | `.control` blocks: commands that are run items, loops and alters as `Loop`/`Alter`, display commands as `Inert`, data-dependent flow as `Script`; and the writer's script form of a run plan |
| S3.5 gate | tests and lists only | corpus fixpoint and census, the simulator round trip, the differential against the old pipeline, the route flip |

S3.0 lands first; S3.1–S3.4 run in parallel; S3.5 last. Shared helpers for other SPICE-card
dialects are **not** designed here: the Xyce slice moves a function to `src/dialect/spice/` when
it needs the same one and the function takes no dialect parameter (REWRITE-PLAN §4.3).

## 3. The reader

`read(src, out, diags)`:

1. **CST per file.** `syntax::tokenize` + `syntax::parse` with ngspice's `Lexicon` (the existing
   front end, used as a private concrete syntax tree: `ast::Tag`, `Ast::fields`, `Ast::header`,
   `Ast::body`, `token::Tokens`). Nothing of `syntax::render`, `syntax::def` or `Behaviours`.
2. **Declare.** One pass over every file: subcircuits (name, ports, parameter names), models
   (name, type word, level), parameters, functions, sections, with their scopes. The kit walk of
   `syntax::resolve` may be reused for the cross-file part; its answers are read, never its
   behaviours.
3. **Lower.** One walk in source order (an explicit stack for nesting), one dispatch on the
   statement's tag to an area function `fn(&mut Lower, Node)`. An area function pushes IR rows
   through `ir::Builder`, or pushes one `Opaque` with a coded diagnostic. It never returns early
   without accounting for its statement (law L1 counts statements in and items out).
4. **Bind.** References are bound by ngspice's rules as they are lowered: names fold case (one
   spelling per name reaches the builder: the first seen); a model is visible in its subcircuit
   and the ones inside it, nearest declaration first; a parameter the same; `gnd` is node `0`
   (INV-semantics category 11). The neutral `ir::link` is never called.

Rules the reader must get right, each with a fixture (the list is `INV-semantics.md` filtered on
`ng`; the ones that shape code):

| Area | ngspice's meaning (evidence) | IR |
|---|---|---|
| numbers | digits, point, exponent `e`/`d`, one scale (`t g meg k m u n p f a`, `mil`), then an ignored unit tail (1.01–1.07) | `Literal` with the digits as written (R1); no floating arithmetic |
| `**`, `^`, `pow` | `\|x\|^y`, left associative; unary minus binds looser (2.02–2.04) | `PowAbs`, tree as grouped |
| `log`/`ln`/`sqrt`/`exp` in a B source | the real functions, an error outside the domain; `exp` exact (corrected, R12: ngspice-45 `ptfuncs.c:275-326` PTexp/PTlog/PTsqrt; runs `B V=ln(-v(c))` "out of range for ln", `exp(20*v(c))` = 4.85e8; INV 2.07 was manual-only; `ExpLin14` is `ngbehavior=ps`, a flavour) | `Ln`, `Sqrt`, `Exp` |
| which evaluator | an expression holding `v(`, `i(`, `time`, `temper`, `hertz` is the B-source evaluator's wherever it stands (2.06) | the member chosen by that test |
| ternary | numparam reads a bare ternary in a then-branch by its then, recursively: `a?b?c:d:e` is `a?c:e` (runs `{1?0?4:8:16}` = 4, `{1?1?1?2:3:4:5}` = 2, `{1?x?2:y?6:7:5}` = 2 for all x, y; corrected: INV 2.13's "deeper nests have no reading"); such a collapsed ternary as a bare else-branch has none (run `{1?2:0?1?3:4:5}` = 3, C and the rule give 2) | the collapsed tree; the else case `InvalidInSource` (`P1004`) |
| `nint`, `int`, `%`, `\` | half to even; truncate; C `fmod`; `\` is `trunc(\|a/b\|)` (corrected: `xpressn.c` operate `x = trunc(fabs(x / y))`; run `{(-7)\2}` = 3; INV 2.11's `-7\2` = −3 is `0-(7\2)`) | `RoundHalfEven`, `Trunc`, `Fmod`, sugar `Trunc(Abs(Div(a, b)))` (R13) |
| `limit`, `agauss`, `gauss`, `aunif`, `unif` | draws, a fresh sample at every reference even in a plain run (12.01–12.03) | `Draw` with `DrawWhen::Always`, `DrawPer::Reference` |
| the two grammars | numparam (`xpressn.c` fetchoperator): `^`/`**` < `* / % \` < `+ -` < every comparison at one level (`{0==1<2}` = 1) < `&&` < `\|\|` < `:` < `?`, all left-associative, `!` on the atom, a leading `-` is `0 - …`; B source (`inpptree-parser.y`): C levels, `^` above unary minus (`2*-2**2` = 8 in braces, −8 in a B source) | one tree per evaluator's parse |
| `-` after a binary operator (numparam) | the sign of the next number only; before anything else ngspice errors (`{2*-q}`: "wrongly determined negation") or negates a later number (`{2*-q+1}` = 5) | a negative literal; else `InvalidInSource`, `P1011` |
| numbers | numparam `sscanf` + `parseunit`: no `d` exponent, `mil` is milli + tail (`{2mil}` = 2e-3); card values and B sources `INPevaluate` (`2mil` = 5.08e-5, `1d3` = 1000) | `number.rs`: two grammars |
| B `u`, `pwr`, `u2`; numparam `sqr`, `pwr`, `pow` | `u` is 0.5 at 0 (PTustep); B `pwr` is sgn·\|x\|^y (PTpwr), B `pow` \|x\|^y; numparam `pwr` is `pow(fabs(x), y)`, `pow` C's `pow` (xpressn.c mathfunction); `u2` clamps to [0, 1] (PTustep2) | `StepHalf`, `PowSigned` (B), `PowAbs` (numparam `pwr`, B `pow`), `Pow` (numparam `pow`), `Sqr` (A11); `u2` → `min(max(x, 0), 1)` (R13) |
| duplicate `.param` | the last definition wins (3.01) | one `Param`: the winner |
| PULSE/EXP given zeros | a given zero rise, fall or tau is TSTEP; width and period TSTOP (5.05) | the `TStep`/`TStop` leaf in the field |
| SFFM, AM | ngspice's argument order and law; 0 until the delay, at the operating point too (5.01–5.03) | named fields, `SffmLaw::Ngspice`, `PreDelay::Zero` |
| a source with no DC | the waveform's value at t = 0; 0 for SFFM, AM, TRNOISE, TRRANDOM (5.13) | `dc` always present |
| device `ic=` | applied only under UIC (8.21) | `IcMode::UicOnly` |
| `m` on an X line | the multiplicity unless the subcircuit declares `m`; it overrides an inner `m` (6.01) | `mult` on the instance, the effective `mult` on inner elements |
| R/C/L whose value reads the circuit | ngspice runs it as a B source: keeps `tc1`, `tc2`, `m`, drops `dtemp` and thermal noise (6.14) | the kind and flags that state exactly that |
| model levels | ngspice's own table; above 99 it silently runs level 1 (7.01, 7.02) | `ModelFamily` by (type word, level); a level ngspice would not run as written is `InvalidInSource` |
| SOI3 `M` card (`nsoi`/`psoi` level 60) | six terminals d, fg, s, bg, bulk, thermal (`soi3.c:30`); the node count is `get_number_terminals`'s (items before the first `k=v`/`off` from the fifth), so all six are read. With four or five nodes the pinned build aborts in `spbuild.c:267` (`spGetElement` assertion, exit 134; run) instead of an error | the card as written: the node count needs the model's family, known only at bind time (fixture `elements/mosfet_soi3`) |
| analyses in one deck | run in reverse card order in batch mode (8.04) | run items in the order ngspice runs them |
| `.temp` list | ngspice runs 27 °C and says so (4.02) | `Env` 27 with a note |
| `.control` | `alterparam` needs `reset`; a dot-card analysis runs after the script and at each `run` (13.01) | run items in run order; display commands `Inert` |

A construct with no row in the IR is an `Opaque`, classed honestly: `Unread` is work to do,
`Private` is ngspice's alone (XSPICE code models beyond the typed ones, `external` sources),
`Inert` moves no number, `InvalidInSource` is what ngspice itself rejects, `Script` is
data-dependent control flow.

## 4. The writer

`write(design, file, out, diags)`: one exhaustive `match` over `ItemKind` (no `_` arm) in item
order; each arm calls its area function. The writer:

- spells a `Literal` with its digits and form (the scale letter ngspice has for that power, else
  `e<exp>`); a number built from a double is already an engineering-form literal;
- spells an expression with ngspice's precedence and the fewest parentheses that read back as
  the same tree, in `{}` where ngspice needs them;
- writes an element's name as it is: the IR name is the whole name, letter included (R23,
  AMEND-A12 §A13; a name not beginning with its kind's letter is
  `CircuitExtra::NameWithoutLetter`, which `FEATURES` lacks and a pass renames); two IR names that
  fold to one ngspice name are not its problem: the legalizer's rename pass ran, or `check_legal`
  stopped the write;
- writes a run plan with loops or alters as one `.control` script (ngspice's native form of
  them), and plain analyses as dot cards in the order that makes ngspice run them in IR order;
- copies an `Opaque` whose dialect's text family is `ngspice`; any other reached it only through
  a bug (`check_legal` refuses foreign `Opaque` before the writer runs);
- never emits a card for a row its `FEATURES` lacks (asserted).

`FEATURES` is what ngspice-45 spells natively, as a `const`: the reader's image (every node the
reader can produce must be writable: law L5) and nothing else. Each feature it lacks has a pass
or a citation row (`tests/fixtures/dialect/ngspice/citations.toml`: the manual clause or the run
that shows ngspice has no form for it).

## 5. Diagnostics

`codes.rs`: this dialect's own table in its own range (`P1xxx` reading, `R1xxx` binding, `W1xxx`
writing; the ranges are allotted in `src/dialect/mod.rs`), each row with title, cite and
explanation. Tests pin codes, never messages. The facade's old-code map (`facade::OLD_CODE`)
gets a row for each code the routed tests assert.

## 6. Tests (each names its oracle)

- **Reader fixtures** `tests/fixtures/dialect/ngspice/read/<clause>.sp` + `<clause>.ir`: the
  expected IR text written by hand from the ngspice manual clause or the INV-semantics row named
  in the fixture's header, with a refusal twin where the clause has an invalid form. One per row
  of §3's table at least.
- **Writer fixtures** `…/write/<name>.ir` + `<name>.sp` (expected text, by hand), and for every
  node with a number in it, a deck built in IR text, written, run on ngspice, compared with a
  closed form (the house `closed_form` template).
- **Laws** L1–L8 from `tests/dialect_laws.rs` (the row is in `BUILTIN`, so they run unasked):
  round trip on generated designs within `FEATURES`, determinism, totality.
- **Corpus** (S3.5): L6 over spice-bench (6,050 decks) and the ngspice PDK sets; the census of
  `Opaque` by class, strict both ways, in `tests/fixtures/dialect/ngspice/census.txt`.
- **Simulator round trip** (S3.5): `ngspice(write(read(d)))` reproduces every plot and `.meas`
  of `ngspice(d)`: today's `test_import_suite`, entry point routed; and the PDK corner tests of
  `test_sections_pdk` bit-identical.
- **Differential** (S3.5): for every golden input read as ngspice and written as ngspice, old
  text and new text compared by meaning (`read(old) eq_meaning read(new)`), every difference a
  line of `REWRITE.md` with its evidence.

## 7. Gate to flip `ngspice → ngspice`

Compared by name, never by count: laws green; every existing test that names the pair green
through the facade (codes mapped); the import suite's known-failure and slow lists unchanged or
shrunk; the differential with no unledgered difference; `Unread` on spice-bench no more than the
old pipeline's W0001 + W0032 there; `tests/layering.rs` green. Then one row in `ROUTES`.
