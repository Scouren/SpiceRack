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
| numbers longer than a literal (R1's evidence) | only the digits as written reproduce ngspice's double. Run (ngspice-45, `set numdgt=17`): numparam `{0.12345678901234567890123}` and `{0.12345678901234568}` both print 1.23456789012345691e-01 (sscanf/strtod); card values (INPevaluate, digit-by-digit in a double): `0.12345678901234567890123` → 1.23456789012345677e-01, `0.1234567890123456789` → …677e-01 (same), `0.123456789012345679` (18-digit rounding) → …705e-01, `0.12345678901234568` (shortest of strtod) → …691e-01; `12345678901234567890123` → 1.23456789012345677e+22 against `1.23456789012345679e22` → …698e+22 | A18 (R37): `NumForm::Long`, the digits as written (`Builder::long_literal`), spelled back verbatim; `Extra::LongLiteral` in FEATURES. A long `mil` number is no number (254 times 18 digits fits no literal). P1005 is gone |
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
| a model's type word beyond the SPICE words | `INPtypelook` (`inptyplk.c`, `inpdomod.c` default branch) looks it up among the built-in devices, then the build's code models, then OSDI modules a `pre_osdi` loaded; an unknown word is "Unknown model type - ignored" (run `.model dd foo`). Limit: a user code-model library loaded with the `codemodel` command is invisible to the reader, so its models read as `Module`; corpus count (spice-bench 21,331 files, the ngspice-45 tarball's examples and tests, the PDKs): 0 decks, the one `codemodel` line is `tests/xspice/digital/spinit.in`, an init template loading the build's own `digital.cm` | `CodeModel` when the word is one of the pinned build's code models (`devhelp`), else `Module`; a built-in device name `Unread`; the writer refuses a word that would read back as another kind (W1206) |
| SOI3 `M` card (`nsoi`/`psoi` level 60) | six terminals d, fg, s, bg, bulk, thermal (`soi3.c:30`); the node count is `get_number_terminals`'s (items before the first `k=v`/`off` from the fifth), so all six are read. With four or five nodes the pinned build aborts in `spbuild.c:267` (`spGetElement` assertion, exit 134; run) instead of an error | the card as written: the node count needs the model's family, known only at bind time (fixture `elements/mosfet_soi3`) |
| analyses in one deck | run by type rank (AC, DC, OP, TRAN, PZ, TF, DISTO, NOISE, SENS, PSS, SP), reverse card order only inside one type: the order of the results (corrected: `cktdojob.c:208-216` loops `analInfo` in `analysis.c:36-58` order over `task->jobs`, which `cktnewan.c:34` prepends; runs: `.tran .ac .dc .op` gives AC, DC, OP, Tran; two `.ac`, the later first; INV 8.04's "reverse card order" is wrong) | run items in card order (R25); the result order is a run fact (S12) |
| state cards | `.options`, `.temp`, `.ic`, `.nodeset` hold for the whole deck wherever they stand; a repeated key or node, the last wins; `.temp` beats `.options temp=` in either order (runs late.cir, icdup.cir, optdup2.cir, temp1/2/5.cir; `inp.c:1159-1186`) | `applies: WholeRun` (R24); the losing `.options temp=` is `Inert` |
| a run card in a `.subckt` body | the deck's (run sub.cir); a `.ic` there holds each instance's own node (run icsub.cir) | in its scope, `RunExtra::RunItemNested` (R26); `NodeSetInSubckt` |
| `.temp` list | ngspice runs 27 °C and says so (4.02); so does a value `strtod` does not read whole (`.temp 0.1k`: run temp4.cir, `inp.c:1173-1182`) | `Env` 27 with a note |
| `.control` | `alterparam` needs `reset`; a dot-card analysis runs after the script and at each `run` (13.01) | run items in run order; display commands `Inert` |
| save set | no `.save`, `.save all` and `.save allv` keep the same vectors: every node voltage at every depth and every branch current (V, E, L; no device current) (run: `a b c x1.n e1#branch l1#branch v1#branch`; `outitf.c` `cieq(name, "all") \|\| cieq(name, "allv")`); `.save v(a,b)` keeps `a` and `b` (`dotcards.c` `gettoks`) | `all` → `Save { AllV, AllBranchI }`; no `.save`: no item, the target's default (R29) |
| implied saves (batch) | with no `.save`, `ngspice -b` without `.control` saves only what `.print`, `.plot`, `.four`, `.meas` name (`dotcards.c` `ft_savedotargs`, `measure_extract_variables`); a measure's `vm( vdb( vp( vr(` saves nothing (`gettok_iv` takes the head as the vector: run "can't parse 'vr'", the analysis does not run) unless another card saves the node | open, with the default save set (R29): the run side decides; the writer spells a measure's real part `v(…)` |
| `.meas` | a measure's `v(x)` in ac is the real part, `vp` radians unless `set units=degrees` (`com_measure2.c` `get_value`, run: `ph` = −0.5610 at 100 Hz); no RISE/FALL/CROSS is the first crossing after the first interval (576–579); FROM= skips below as TD= does in tran and is copied TRIG↔TARG when the other has none (runs); DERIV and ERR are "not supported"; `td=<measure>` is fatal; `vr(x)` in a measure saves nothing in batch mode (run) | `Measure` rows; `skip_initial` (R30), `until`, `Stat::MinAt`/`MaxAt` (A15) |

### Options by class (R19, A16; `read_run/state.rs`)

From `cktsopt.c:246-330` and the frontend's option variables, key by key:

| Class | Keys |
|---|---|
| typed (`OptionKey`) | `reltol abstol vntol chgtol trtol pivtol pivrel maxord method=trap, method=gear, itl1 itl2 itl4 gminsteps srcsteps itl6 noopiter gmin rshunt gshunt cshunt rseries scale defl defw defad defas klu sparse seed autostop interp acct noacct list nomod nopage node opts warn maxwarns norefvalue keepopinfo noinit nfreqs fourgridsize`; `temp`, `tnom` are `Env` |
| `Hint` (tunes the solver, names no accuracy target) | `gminfactor bypass oldlimit nodedamping absdv reldv noopac copynodesets klu_memgrow_factor noopalter maxopalter maxevtiter convlimit convstep convabsstep autopartial` |
| `Private` (names an accuracy target, changes the equations, or not established) | every other key: `xmu minbreak epsmin ramptime defm badmos3 trytocompact indverbosity`, the SPICE2 keys (`numdgt cptime limtim limpts lvlcod lvltim`), the frontend's (`ngbehavior savecurrents` …), a flag given a value, a `method=` other than trap or gear |

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

### `admits` refusals, classed (R21; the lead rules on the candidates after the trunk)

What `Write::admits` still refuses on ngspice, at dialect-ngspice's trunk candidate. Class: **I**
invalid on every simulator (an IR invariant candidate), **F** valid elsewhere but not on ngspice
(a feature candidate), **N** ngspice's own rule (stays an `admits` refusal).

| Code | Refused | Class | Candidate, or why it stays | Owner |
|---|---|---|---|---|
| W1006 | a string or list in any expression but a model's | F | gone (round 9): `Extra::StringValue`, `ListValue` (a `.param` value, a formal default) are features `FEATURES` lacks, a `.if` condition or `.func` body is X0021, X0020 (A25 item 4) | ir-kernel |
| W1007 | an expression its evaluator cannot spell: a draw in a value that reads the temperature or the circuit | N | R52's approximation; stays until a deck needs the three-form split (`DistDyn`) | spine dialect |
| W1100 | an element ngspice has no card for though its features are in `FEATURES` | N | the card forms (manual §3) | ng-elements |
| W1150 | a source ngspice's positional card cannot spell | N | the V/I card's fixed order | ng-sources |
| W1202 | a version ngspice has no device for; a version on a module or code model; (interim) a versioned family with none | N | the device table; the missing-version arm goes with ir-circuit b6888c35's invariant | ng-models |
| W1203 | a model parameter the device's tables lack | N | the device tables | ng-models |
| W1204 | a bin ngspice does not bin | N | the binning families | ng-models |
| W1205 | a model value ngspice reads back as another value | N | numparam's card grammar | ng-models |
| W1300 | a `.four` count other than ngspice's default (only `nfreqs` sets it) | F | a feature (count by option), with ir-run's per-kind state split | ir-run |
| W1301 | an analysis field with no position on ngspice's card | F | a feature per field form | ir-run |
| W1302 | a dot-card value that reads the temperature | N | numparam evaluates a dot card once, before the run | ng-run |
| W1303 | a node set that holds from its place on | F | gone (round 9): `NodeSetFromHere` (ir-run 7f9aed10's split), which `FEATURES` lacks; ngspice has no script command for it | ir-run |
| W1350 | an output ngspice has no card or command for | N | the output grammar | ng-outputs |
| W1401 | a parameter alter whose `reset` would drop another alter or a from-here state | N | ngspice's `alterparam` applies only through `reset` | ng-control |
| W1402 | a loop that is no sweep over a list of numbers | N | `foreach` reads words | ng-control |
| W1403 | an alter value that is no number, or no one-token spelling | N | `alter` reads the frontend's grammar | ng-control |
| W1404 | a Verilog-A load | F | waits for a second dialect that spells it (ngspice loads OSDI only) | lead |
| W1405 | a load that names its modules | F | `LoadModules`; waits for a second dialect | lead |
| W1406 | a partial restore (a `Reset` of fewer than every class) | F | waits for a second dialect (ngspice's `reset` restores all) | lead |

No code is class I today.

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
