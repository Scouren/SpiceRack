# Slice S4: the VACASK dialect (reader and writer) and the pairs ngspice ↔ VACASK

The second dialect, from a second syntax family: it tests whether the contract (`Read`, `Write`,
`FEATURES`, passes that know no dialect) holds outside SPICE cards, and whether pair rules really
become passes. Read `REWRITE-PLAN.md` §2–§5, §7, `rewrite/SPINE.md`, `IR-KERNEL.md` (A1–A10),
`IR-CIRCUIT.md`, `IR-RUN.md`, `RULINGS.md` (R1–R22), `AMEND-A12.md` (what this plan asked of the
IR; accepted with rulings R16–R22) and `DIALECT-NGSPICE.md` first. Everything
here is about VACASK 36be560 (`/nix/store/dhxz6l3iac3r2b8n91kcz04a0j3hs4s6-vacask-unstable-2026`,
`nix develop .#testing`). A rule is a fact of that binary with its evidence, never a memory of
what the old writers did.

Evidence keys: `S n.nn` a row of `INV-semantics.md`; `C X99` a row of `INV-constructs.md`; `VD
<file>` VACASK's `share/doc/vacask/docs/<file>.md`; `VS Vnn` a row of the previous team's
`research/vacask-spectre-source.md`; `RUN <deck>` a run made for this plan (Appendix A: each deck,
what it does, what VACASK printed; the decks become `tests/fixtures/dialect/vacask/settle/` in the
Definition phase). Where the docs and a run disagree the run wins (R5).

## 1. What the slice delivers

- `src/dialect/vacask/` implementing `Read` and `Write`; one row in `BUILTIN` (`name: "vacask"`,
  `text_family: "vacask"`); its code table (§7); `FEATURES`; fixtures; the routes `vacask →
  vacask`, `ngspice → vacask`, `vacask → ngspice` flipped at the gates of §9.
- The passes of §6 under `src/passes/`, one file each, registered in `PASSES` (the passes owner's
  directory; they are written in this slice because it is the first that needs them, §8.1).
- The kernel, circuit and run amendments of §5 that §10 Q1–Q3 accept, requested from the IR
  owners before the Definition phase (they own `crates/ir`; this slice never edits it).
- Not here: Spectre. Helpers shared with the Spectre reader move to `src/dialect/named/` in the
  Spectre slice, only if they take no dialect parameter (REWRITE-PLAN §4.3).

The two cross pairs need S3's ngspice reader and writer at their own gate; `vacask → vacask`
does not.

## 2. Files and sub-slices

One owner per file. S4.0 writes the two spines and every area file as a stub exporting its area
functions, its `FEATURES` part (a `const FeatureSet`, unioned in `mod.rs`) and its `CODES` rows,
so later sub-slices never edit a spine.

| Sub-slice | Files | Holds |
|---|---|---|
| S4.0 scaffold | `mod.rs` (the `Read`/`Write` impls, `FEATURES` = union of the area parts), `read.rs` (spine: sources → CST → declare → lower; dispatch on CST tag and head word), `write.rs` (one exhaustive `match` over `ItemKind`, no `_` arm; hoists loads, then the circuit, then one `control` block), `number.rs` (literal and its VACASK type), `name.rs` (exact names, `'…'` quoting, ground set), `expr_read.rs`, `expr_write.rs` (VACASK precedence, the typed-division spelling of §4), `read_structure.rs`, `write_structure.rs` (title, comments, `include`/`section=`, `section`, `load`, `embed`, `subckt` (nested), `parameters` (formal or local, §3.4), `global`, `ground`, `@if` chains), `codes.rs`, `tables.rs` (module ↔ (`ElemKind`, `ModelFamily`, polarity rule, library file), instance/model parameter renames) | numbers, names, expressions, structure; everything else an `Opaque` of class `Unread` through the stubs |
| S4.1 devices | `read_elements.rs`, `write_elements.rs`, `read_models.rs`, `write_models.rs`, `families.rs` | `model` lines, module → family, every instance of a model or subckt, `$mfactor`, behavioural sources |
| S4.2 sources | `read_sources.rs`, `write_sources.rs` | `vsource`/`isource` (`type=` dc sine pulse exp pwl am fm), `mag`/`phase`, controlled sources, `mutual`, `tline_ideal` |
| S4.3 run | `read_run.rs`, `write_run.rs`, `read_outputs.rs`, `write_outputs.rs` | the `control` block: `options`, `var`, `sweep`, `analysis`, `alter`, `elaborate`, `clear`, `mc … endmc`, `save`, `print`/`abort`/`postprocess`; `ic=`/`nodeset=` lists |
| S4.4 passes | `src/passes/<name>.rs`, one per row of §6 | IR → IR, each with its own test (§8) |
| S4.5 gate | tests and lists only | corpus fixpoint and census, simulator round trip, cross-sim, differential, route flips |

S4.0 first; S4.1–S4.4 in parallel; S4.5 last.

## 3. The reader

### 3.1 Pipeline

1. **CST per file**: `syntax::tokenize` + `syntax::parse` with VACASK's `Lexicon`
   (`syntax/dialects/vacask.rs`, `named(true)`), as a private tree (`ast::Tag`, `Ast::fields`,
   `token::Tokens`); nothing of `syntax::render`, `def` or `Behaviours`. It serves: every
   statement is a node (`Subckt`, `Model`, `Param`, `Include`, `Section`, `If`/`Else`, `Control`,
   `Global`, `Save`, `Analysis`, `Options`, `Instance`, and `Card` for any other head,
   `syntax/named.rs:306`). What it loses, each recovered by the reader from token bytes, never by
   editing the front end: (a) comments live in token gaps: `Comment` items come from the gaps;
   (b) a number token's value is the lexicon's arithmetic: the reader re-lexes its bytes into a
   `Literal` and a VACASK type (`2.` and `2` differ only in bytes, §3.3); (c) hex is not decoded
   (S 1.13); (d) `var alter elaborate clear print abort postprocess mc endmc` have no keyword:
   they arrive as `Card` and the reader dispatches on the head word; `mc … endmc` is not a CST
   block, so lower keeps an explicit stack; (e) `statement::Analysis` knows 8 kinds: the reader
   reads the kind word itself; (f) a behavioural source's second parameter list after `,`.
   Definition-phase probe: dump the CST of all 184 corpus files and check that every byte is in a
   token or a gap (L1 depends on it).
2. **Declare**: one pass over every file: loads (file, `map=`), models (name, module word),
   subckts (name, ports, formals), parameters, `var`s, globals, ground names, sections, scopes.
3. **Lower**: one walk in source order, dispatch to an area function `fn(&mut Lower, Node)` that
   pushes rows through `ir::Builder` or one `Opaque` with a coded diagnostic (law L1).
4. **Bind**, by VACASK's rules (never `ir::link`, R3):

| Rule | Evidence | IR |
|---|---|---|
| Names are exact bytes; `R1` and `r1` are two instances, `A` and `a` two parameters | S 3.02, 11.11; RUN n4 | one `NameId` per spelling; `Extra::CaseSensitiveNames` |
| Masters and parameters resolve in the own scope, then the top scope, then circuit variables, constants, special identifiers; **never an enclosing subckt** | VD cir-hier ("Scope"), cir-masters, cir-subckt-nested; RUN n1 (`p` of the outer subckt: "Variable or constant 'p' not defined"), RUN m1 (outer's model: "Master 'res' not found") | `Ref.def` bound by that order; a name found only in an enclosing subckt is unbound and gets R2003 (§7) |
| Ground: `0` and every name of a top-level `ground` directive; `gnd` is an ordinary node | VD cir-nodes; S 11.01 | `NameId::GROUND`; the `ground` statement is an `Opaque` `Inert` (R15) |
| Every instance names a master (a model, a subckt, or none for a behavioural source); a builtin device needs a `model` line too | VD cir-instance, dev-builtin-behavioral; RUN n2 ("Master 'vsource' not found"); S 6.28 | `Element.model` always bound or `Unresolved` |
| Forward references are legal | RUN n5 (`a=b*2` before `b=3` gives 6) | bind after declare |
| A parameter defined twice in one scope is refused | S 3.01; RUN n3 ("Parameter redefinition.") | `Opaque` `InvalidInSource`, R2001 |
| `elaborate circuit("tb")`: tb's models and subckts join the global scope | VD cir-elaboration | `Elaborate` (§5 G8) |

### 3.2 Statements → IR

| Area | VACASK's meaning (evidence) | IR |
|---|---|---|
| title | line 1 of a top-level file, whatever it holds; a library file has none (S 14.12) | `Title` in the entry file only |
| comments | `//` to end of line, `/* */` (S 14.02) | `Comment` |
| include | textual; `section=` makes it a library call; inside a subckt what it defines is local (C A9; VD input-include) | `Include{path, section}` in its scope |
| section | `section n … endsection` (C A14) | `Section` |
| load | top level only; `.osdi` or `.va` (compiled by OpenVAF-r); `map=` renames modules; parameter and terminal names lowercased (VD cir-loading) | a shipped library file (`spice/*.osdi`, `spice/{sn,full}/*.osdi`, VACASK's own `resistor capacitor inductor diode bsim3v3 bsim4v8 psp103v4 vbic_1p3* tline_ideal …` under `lib/vacask/mod`) → `DeviceLibrary{family, path}` (A12 C5, R17; `path` `NONE` for the family's default file, a variant path as written); any other → `Load{Osdi\|VerilogA, path}`; `map=` other than the default → `Opaque` `Private` |
| embed | `embed "f" <<<MARK … >>>MARK` writes a file (VD input-embed) | read by a `load` → `Opaque` `Private` (the module text has no IR home; VS V27); read by nothing → `Inert` (C A20) |
| subckt | `subckt n (ports) … ends`; may nest; `global`, `ground`, `load`, `embed`, `control` forbidden inside (VD cir-subckt) | `Subckt`; nested → `Extra::NestedSubckt` |
| parameters | a value that folds to a constant at parse time (literals, builtin constants, functions of them) is a formal an instance may override; one that reads a parameter or variable is dependent and cannot be overridden (VD cir-subckt; RUN c1: `2*3`, `M_PI`, `sqrt(4)` overridable, `q*2` "Parameter 'x' not found"; RUN t2) | in a subckt: constant → `Subckt.params`; dependent → a local `Param` item; at the top → `Param` |
| `var` | a circuit variable, global, below parameters in lookup (VD expr-cirvars, cmd-var: right-hand sides use the values before the statement) | before the first analysis or elaborate and not shadowed by a top `Param` → top `Param`; later → `Alter{Param}`; shadowed → `Private` |
| global | top only; ground nodes are global (VD cir-nodes) | `Global` |
| `@if/@elseif/@else/@end` | elaboration-time; bodies hold instances, models, nested `@if` only; nonzero number or non-empty string is true (VD cir-conditional) | `If` chain; a string condition → `Private` |
| model | `model n module params`; the module is a builtin or loaded (VD cir-masters) | `Model{family, polarity, params, module}` by `tables.rs`; a builtin (`vsource isource vcvs vccs ccvs cccs mutual`) → a parameterless `Model` of the builtin's family (naming: §10 Q15, with `ir-circuit`); `sp_resistor` Resistor, `sp_capacitor` Capacitor, `sp_inductor` Inductor, `sp_diode` Diode (`level=3` DiodeLevel3), `sp_bjt` GummelPoon, `sp_mos1/2/3/6/9` Mos1…Mos9, `sp_bsim3v3` Bsim3 (3.3.0), `sp_bsim4v8` Bsim4 (4.8.x only, S 7.04), `sp_jfet1` JfetShichman, `sp_jfet2` JfetParkerSkellern, `sp_mes1` Mesfet, `sp_vdmos` Vdmos, `vbic13`/`vbic13_5t` Vbic (1.3), `psp103va` Psp103, `bsimbulk` BsimBulk (C D47, F5; VD dev-spice); `type=±1` → `Polarity` (S 7.21); `model_<p>`/`instance_<p>` → the family's `<p>` (C F15); any other module → `ModelFamily::Module` + `Model.module` |
| instance | `name (nodes) master params`; OSDI and subckt terminals may be left unconnected (VD cir-instance); an unknown parameter is a hard error (S 7.11) | `Element` with the kind of its master; `Instance` for a subckt, `ModuleModel` for a user module |
| `$mfactor` | parallel copies; current branches ×m, a voltage branch's voltage unchanged and its reported current per copy (VD cir-mfactor); on a subckt instance only when the subckt declares `$mfactor` (RUN n2: "Parameter '$mfactor' not found") | on a device → `Element.mult` (`CircuitExtra::Mult*`); on a subckt instance → an ordinary parameter |
| R, C, L | `sp_resistor` takes `r temp dtemp l w tc tc2 tce bv_max scale noisy` (RUN t1 dump); `sp_capacitor` has no `ic` (`render/named/vacask_ic.rs` header, run); no `ac=` (S 6.17) | `Passive{value, temp_abs, dtemp, tc1, tc2, tce, scale, noisy}` |
| sources | `dc` is ignored on a waveform `type`: every analysis reads the waveform, the op its value at t = 0 (S 5.13, VS V5, V33 run); `mag` default 0, `phase` AC only (VD dev-builtin-src; VS V32 run) | `Source{dc: WaveAtZero (§5 G7), ac_mag, ac_phase_deg, wave}`; `type="dc"` → `dc` given, no wave. **Run** (vc-sources t2): the sine's phase is `tdphase`; `phase` is the AC phase only (VD dev-builtin-src's sine line `phase=0` is wrong: `tdphase=90` gives op 3 with `sinedc=1 ampl=2`, `phase=90` gives 0 at t = 0); a dc source with a wave's parameters and `alter … type=` alters is R42's fold (S4-AREAS "dc plus type alters") |
| sine | `sinedc ampl freq delay theta tdphase`; before `delay` the value at `delay`; phase at `delay` (S 5.06, run) | `Sin{…, pre: HoldDelayValue}` |
| pulse | `val0 val1 delay rise fall width period`; defaults `rise=1n fall=0 width=0 period=0`; `rise` must be > 0 (RUN p2: "must be grater than 0"); `fall ≤ 0` is **no falling edge**: val1 to the end of the period, val0 again at its start (RUN p1: 1.0 at 3.5 µs, 0.497 at 4.5 µs); `period ≤ 0` one pulse (VD) | `Pulse{…, law: Vacask}` (§5 G3); every field filled with VACASK's default |
| exp | `val0 val1 delay tau1 td2 tau2`, `td2` counted from `delay` (S 5.08 run) | `Exp{td1: delay, td2, td2_from: Td1}` (§5 G4) |
| pwl | `wave=[t, v, …]`, holds first and last values; `period` wraps from 0 (binary extrapolates when x1 > 0, S 5.11, 14.10); `scale offset stretch tdphase break slope*` (VD) | `Pwl{points, delay, repeat: Period}`; `file=` → `Pwl.file`; any of `scale offset stretch tdphase` not at its default, or `break`/`slope*` given → `Private` (§5 G5). **Run** (vc-sources t2): `break` defaults to `"all"`, not VD's `"auto"` |
| am | `sinedc ampl modindex modfreq freq delay modphase tdphase`, holds before `delay` (S 5.03, VS V32 runs) | `Am{…, pre: HoldDelayValue}` |
| fm | SPICE3's law and order (S 5.01, 5.02 runs: va −0.15854 with xy) | `Sffm{…, law: Spice3, pre: HoldDelayValue}` (§5 G6) |
| controlled | `vcvs gain`, `vccs gain`, `cccs`/`ccvs ctlinst= gain`; a `ctlnode` other than `flow(br)` (C D16–D18; VS §2) | `Controlled`; the other `ctlnode` → `Private` |
| mutual | `mutual ind1= ind2= k=` (C D10) | `Coupling` |
| tline_ideal | `z0 td` or `f nl`, default `nl=1` (S 6.24) | `LosslessLine`; the reader fills `nl=1` when `f` is given alone |
| behavioural | `b (p n) v=\|potential=\|i=\|flow=<expr>`; `v(n)`, `v(a,b)`, `i(inst)`; `$abstime`; `ddt` (0 at DC), `idt(x[, ic])`; only the listed functions translate; no `$mfactor` (VD dev-builtin-behavioral) | `Behav{Voltage\|Current, expr}`; `expr=`/`declarations=`/`evaluation=`/`discipline=`, `white_noise`, `flicker_noise` → `Private` (§5 G14) |
| charge | `i=ddt(q)` is a behavioural current (VD) | `Behav{Current, Ddt(q)}`, never `ChargeCapacitor` (transliteration) |

### 3.3 Numbers and expressions

| Area | VACASK's meaning (evidence) | IR |
|---|---|---|
| literals | digits, `.`, exponent `e`/`E`, one scale `a f p n u m k K M x X meg G T mil`, then letters/`_` ignored after a scale only; `1k5`, `1e3k`, `1d3` are no numbers; `0x…` hex; `inf`, `nan` (S 1.01–1.07, 1.13; VD input-numbers; RUN f1: `1M`=1e6, `2x`=2e6, `1mil`=2.54e-5) | `Literal` as written (R1); hex → its value, `Int` (RUN f1: `0x10/3` = 5); `inf`/`nan` → `InvalidInSource` (the NaN gate, C9). **Run** e8 (vc-run): the lowercase `g` is no scale (`to=1g`: "Parser syntax error … expecting ="); `G` is  Runs iw1, lr1: an integer is 32 bits and wraps (a literal past `i32` is `Private` P2009, written W2008); a real with more than 18 significant digits is a `Long` literal, rounded correctly  **The domain (R54):** VACASK's integer is 32 bits and wraps, literals and arithmetic alike (runs iw1: `2147483648` → −2147483648, `99999999999` → 1215752191, `2147483647+1` → −2147483648); the IR's integer stays exact, and what a deck computes outside VACASK's domain is not carried (R12): a literal past `i32` reads `Private` P2009 and is refused on write W2008; overflowing arithmetic gets no static check |
| integer type | digits only (or hex) is an integer literal; a point, exponent or scale makes a real (RUN f1: `2./3` 0.667; RUN i1: `1/3` 0, `1k/3` 333.3) | `NumForm::Int` for an integer literal, hex included (A12 K1) |
| `/` | truncating when both operands are integers **at evaluation**: `-7/2` −3, `a/b` with `a=1 b=3` 0 (RUN i1); the type follows the actual value: `x/2` is 1 for `x=3`, 1.5 for `x=3.0` (RUN t3) | `BinOp::DivTyped` (§5 G1), always: the reader does not type (§6 does) |
| result types | int: `int()`, `sgn`, comparisons, `&& \|\| !`, bit operators and shifts, `+ - *` and unary `-` of ints, `min`/`max` when every operand is int, a ternary's chosen branch; real: `**`, `pow`, `integer()`, `floor`, `ceil`, `round`, `abs`, `atan2`, `hypot`, `min(3, 5.0)` (RUN f2, f3) | A12 K1's tag table (it matches every row of f2 and f3) |
| operators | `? :` lowest, right; `\|\| && \| ^ & == != < <= > >= << >> + - * /` left; `**` right; unary `- ! ~` bind tighter than `**`: `-2**2` = 4; `2**3**2` = 512; `^` bitwise XOR (`7^3` = 4) (VD expr-operators; RUN e1) | tree as grouped; `Pow`; `BitXor`, `BitAnd`, `BitOr`, `Shl`, `Shr`, `BitNot`; `Not`; no `%` |
| ternary | C grouping: `1?0?4:8:16` = 8 (RUN e1) | `Ternary` |
| functions | `log`=`ln` natural, `log10`, `sqrt`, `exp`, `pow`=`**` (signed: `(-2)**3` = −8, RUN e1), `abs`, `floor`, `ceil`, `round` half away (2.5→3, −2.5→−3), `integer` truncates, `int` truncates to an int, `fmod` dividend's sign (−1), trig, hyperbolic, `atan2`, `hypot`, 2-argument `min`/`max` (VD expr-functions; RUN e1, f2) | `Ln`, `Log10`, `Sqrt`, `Exp`, `Pow`, `Abs`, `Floor`, `Ceil`, `RoundHalfAway`, `Trunc`, `ToInt` (§5 G1), `Fmod`, …, `Atan2`, `Hypot`, `Min`, `Max` |
| `sgn`, `sign` | `sgn(0)` = 1 and `sgn(-0.0)` = 1: zero counts as positive (VD dev-builtin-behavioral; RUN e1); `sgn`'s result has its operand's type (`sgn(3.0)/2` 0.5, `sgn(3)/2` 0: RUN f4); `sign(x, y)` = \|x\| with `sgn(y)`'s sign, always real (`sign(3,-0.0)` = 3, RUN f1; `sign(3,1)/2` 1.5, RUN f4) | `sgn(x)` → the kernel member `SgnNonNeg` asked for (no sugar types ±1 by `x` without repeating it, R13 b; the sugar `x >= 0 ? 1 : -1` stands in until K1); `sign(x, y)` → `Mul(Abs(x), Ternary(Ge(y,0),1,-1))` (Real by K1's table) |
| constants | `M_*`, `P_*` (C C28; VD expr-constants) | the literal of the value VACASK documents (S 2.22), `Plain` |
| special | `$temp`, `$tnom`, `$scale` in every expression; `$abstime` in behavioural sources only (VD expr-special, dev-builtin-behavioral) | `Temp`, `Tnom`, `Time`; `$scale` → `Private` statement (§5 G15) |
| vectors | `[a, b]`, `{…}` lists, `x[i]`, `vector() range() readfile() randunif()` (VD expr-vectors) | the statement → `Private` (§5 G13) |
| draws | `agauss(nom, avar, σ)`, `gauss(nom, rvar, σ)`, `aunif`, `unif`; the nominal outside an `mc` loop; one sample per call site (VD expr-functions; RUN g2: `r=agauss(5,1,1)` is 5; S 12.01, 12.03) | `Draw(dist, Sampling, CallSite, args)` |
| context | `fmod`, `round` and the vector functions are refused in a behavioural source (VD dev-builtin-behavioral "Functions"; S 2.17 run) | `FuncDyn` features absent from `FEATURES` |

### 3.4 The control block (run items, in order: VD cmd-overview, S 8.04)

| Statement | VACASK's meaning (evidence) | IR |
|---|---|---|
| `options k=v` | accumulate, applied before the next analysis (VD cmd-overview): every state item is `applies: FromHere` (A14) | `reltol abstol vntol chgtol gmin gshunt scale` → `OptionRow` portable keys; `temp`/`tnom` → `Env`; `tran_method` trap/euler/gear2/bdf2/am/gear + `tran_maxord` → `MethodTrap\|MethodEuler\|MethodGear` + `MaxOrd`; `op_itl` Itl1, `tran_itl` Itl4, `homotopy_gminsteps` GminSteps, `homotopy_srcsteps` SrcSteps, `op_skipinitial=1` NoOpIter (C I5; `syntax/dialects/vacask.rs:181-237`); `rawfile` and output-only keys → `Inert`; solver knobs (`tran_lteratio`, `tran_rmax`, `tran_fs`, `tran_minpts`, `nr_*`, `homotopy_*` other than the two above, `smsig_*`, `solver`, `omp`, …) → one `Opaque` of class `Hint` per `options` line (R19, A12 K3); accuracy targets VACASK alone has (`tolscale`, `tolmode`, `relref`) → `Private` (R19; the corpus has one deck with `tolscale`, `demo/sc/lpfilter.sim`, at its default 1, beside `tolmode="mixed"`). **Runs** (vc-run): an unknown key is "Parameter 'foo' not found." (e6); only `trap` and `euler` are typed methods, the others and `tran_lteratio` are `Hint` (R43) |
| `analysis n op` | (VD cmd-analysis-op) | `Analysis{Op, name}` |
| `sweep` + `analysis n op` | one raw file over the grid (VD cmd-sweep) | `Dc{axes}`: the last `sweep` line is the innermost axis (VD "Nested sweeps"); targets `instance=… parameter="dc"` SourceDc, principal parameter ElementValue, other DeviceParam, `model=` ModelParam, `option="temp"` Temp (S 4.04), other option Option, `variable=`/`__topinst__` Param |
| `sweep` + any other analysis | re-runs that one analysis only; the swept value is **restored** afterwards (RUN s1: the op after sweeps over 2 and 3 reads 1; VD says "left at its last value": the run wins); `points` counts intervals (VD; S 8.06) | `Loop{Sweep, after: Restore}` whose body is the analysis; `lin points=n` → `n+1` values (spelling); bounds may read an outer swept variable (S 8.22) |
| `continuation=` | 1 is the default and only seeds Newton (VD; VS V13) | 1: nothing; 0: `Private` (COVERAGE G-CONTINUATION) |
| `ac` | `from to mode points` or `values=[…]` (VD cmd-analysis-ac) | `Ac{sweep}` (`List` for `values`) |
| `tran` | `stop`, `step` = **initial** step, `start` = start of recording, `maxstep`; `icmode="op"\|"uic"`; `ic=`/`nodeset=` lists or a slot name (VD cmd-analysis-tran); with no `maxstep` the step is capped at (stop − start)/50 (RUN tr1: 2e-4 for 10 ms, 8e-5 with start 6 ms) | `Tran{stop, record_start, initial_step, max_step, cap: SpanOver50, uic, print_step: NONE}`. **Run** e7 (vc-run): a literal `maxstep=0` is no ceiling (identical raw files; R47: read as the default rule) |
| `trannoise` | tran with device noise (VD cmd-analysis-trannoise; S 8.20) | `Tran` + `noise_fmax/fmin/seed` (the reader fills `fmin = fmax/1e3`); `noisefmin` absent or 0 is `noisefmax/1e3`, stated as written |
| `noise` | `out=["n"]` or a pair, `in="src"` (VD; VS V21; S 15.04 V²/Hz) | `Noise{output, source, sweep}` |
| `dcinc`, `dcxf`, `acxf`, `acstb`, `acsp`, `hb`, `pss`, `pac`, `pnoise` | VD cmd-analysis-*; `acstb` gives +T (S 8.18); `hb` `immax` 0 is box in the binary (S 8.15, 14.10) | `DcInc`, `DcXf`, `Xf`, `Stb`, `Sp{ports}`, `Hb{tones, harmonics, intermod, truncate}`, `Pss`, `Pac`, `Pnoise`. R43: `pss` (by period), `pac`/`pnoise` (their own steady state), `hb` and `var` are `Private` under their own codes P2304–P2307 |
| `hbac`, `hbnoise` | (VD) | `Private` (COVERAGE-run) |
| `ic=`, `nodeset=` | lists of node, value; two nodes then a value is a difference (VD cmd-analysis-op; C G63); hierarchical nodes quoted `'x1:n'`; on that analysis only | `NodeSet{kind, node, reference, value, next_only: true}`; a slot name, `store=`, `opsolve=0`, `writeop` → `Private` (G-SLOTS) |
| `save` | accumulate until `clear saves`; `default`, `full`, `v()`, `i(inst)`, `p(inst, var)` (VD cmd-save) | `Save` in order: `default` AllV + AllSourceI, `full` AllV, `V`, `I`, `Param` (quantity as spelled, G-OPQUANT); `yin zin yout zout opdefault opfull` → `Private` (G-PORTOUT). **Run** e5 (vc-run): `default` is AllV + AllBranchI (`l1:flow(br)` is in it), not AllSourceI; `full` is every unknown, internal nodes included: `Private` P2308; `save v(b)` governs op, its sweeps and tran, not ac (A22 types the set: `Save.analyses`) |
| `alter` | `instance(…)`/`model(…)` p=v; persists, no reset (VD cmd-alter; S 13.03 run) | `Alter{DeviceParam\|ElementValue\|SourceDc\|ModelParam\|Param (`__topinst__`)}`; `type="…"` → `AlterString` |
| `elaborate` | `circuit` alone: the default; `circuit("a", …)`: a, … promoted; `changes`: automatic (VD cmd-elaboration) | `circuit` and `changes` → `Inert`; `circuit("…")` → `Elaborate{circuits}` (§5 G8); `topdef=`/`topinst=` → `Private` |
| `clear` | `saves` resets saves (no number moves), `options`/`variables` reset state (VD cmd-save, cmd-options-set) | `saves` → `Inert`; others → `Private` |
| `mc n samples=k … endmc` | repeats the enclosed statements, every draw call varies (VD cmd-analysis-mc; S 12.05) | `Loop{MonteCarlo}` with that body |
| `print`, `abort`, `postprocess` | no circuit meaning (C K10) | `Inert` |

A construct with no row is an `Opaque`: `Unread` is work to do, `Private` is VACASK's alone,
`Inert` moves no number, `InvalidInSource` is what VACASK itself rejects, `Script` is not used
(the control block has no data-dependent flow; `abort` is `Inert`).

## 4. The writer

`write(design, file, out, diags)`: one exhaustive `match` over `ItemKind`; loads are hoisted to
the top of the entry file (VD cir-loading: top only), then items in order, then one `control …
endc` holding the run items in order (one per file, VD cmd-overview). The writer:

- spells a `Literal` with its digits: a `Scale` with VACASK's letter (`meg` for 10⁶, never `M`),
  `Mil` as `mil`, `Exponent` as written; **a `Plain` whole number with a trailing point** (`2.`,
  real in VACASK, and read back as the same `Plain` literal), an `Int` with digits only;
- spells expressions with VACASK's precedence and the fewest parentheses that read back as the
  same tree; `Temp` `$temp`, `Tnom` `$tnom`, `Time` `$abstime`; no braces;
- writes names exactly, quoting with `'…'` any name that is no identifier or holds `:`
  (C B19; VD cmd-save), and `0` for ground;
- spells `DeviceLibrary{family, path}` as `load` of `path`, or of the family's default file
  (`tables.rs`), and each `Model` with its module word; never invents a load or a model (§6
  does);
- spells `Dc{axes}` as `sweep` lines (innermost last) + `analysis … op`, each sweep by its name
  (A22 `DcAxis.name`: the raw file's vector); a `Loop{Sweep}` as its `sweep` lines before its one
  analysis (R43: the repeat before each analysis of a body is withdrawn; a body other than one
  analysis or one sweep loop is `SweepBodyNotOne`, a feature VACASK lacks); `Loop{MonteCarlo}` as `mc … endmc`; `NodeSet` lists on the analyses they apply to;
- names an unnamed analysis `<kind><n>` (the spelling the old writer used, which the backend
  reads back off the text until S12: `src/backend/vacask.rs::analysis_names`);
- copies an `Opaque` of family `vacask`; never emits a statement for a row its `FEATURES` lacks
  (asserted).

`FEATURES` is the reader's image and nothing else (L5). It lacks, among others, `Binary(Div)`,
`Func(PowAbs)`, `Func(RoundHalfEven)`, `Func(Sgn)`, `Binary(IntDiv)`, `Analysis(Tf)`,
`Elem(ChargeCapacitor)`, `Elem(Poly)`, `Elem(Table)`, `Elem(Port)`, `CircuitExtra::IcUicOnly`,
`PulseCycles`, `SffmNgspice`, `MultInstance`, `DrawWhen(Always)`, `RunExtra::LoopKeepLast`,
`TranCapNgspice`, and every element with no model of a kind VACASK serves only through a module
(§5 G2). Each lacked feature has a pass (§6) or a citation row in
`tests/fixtures/dialect/vacask/citations.toml`. What no feature states (a model parameter the
module lacks, which VACASK rejects: S 7.11) is refused by `Write::admits` (R21, A12 S1) from the
parameter tables in `tables.rs`.

## 5. IR gaps

R8: a gap gets its typed row in the slice of the second dialect that has the meaning. G1–G4 and
G6–G11 are typed here because ngspice reads or writes the meaning through a §6 pass in this
slice (R18 confirms that reading of R8).

| Gap | VACASK meaning (evidence) | Proposal | Owner | Now? |
|---|---|---|---|---|
| G1 typed arithmetic | integer literals, truncating `/` on integer values, result types per operator and function (§3.3; RUN i1, t3, f2) | `NumForm::Int`; `BinOp::DivTyped` (C's division on the operands' types); `Func::ToInt` (trunc, integer-typed); the reference evaluator carries an int/real tag per value; the typing table in `DivTyped`'s doc | kernel | accepted: A12 K1 (R16) |
| G2 device library | a device of a family is the simulator's own build of it, named by a `load` of a shipped file and a `model` (VD cir-loading, dev-spice; RUN n2) | item `DeviceLibrary{family, path}` (`path` `NONE` = the target's default file; `spice/sn/…` kept as written); a design feature "a model of family F with no `DeviceLibrary` of F" and an item feature "an element with no model" | circuit | accepted: A12 C5 (R17) |
| G3 pulse law | `fall ≤ 0` no falling edge, `period ≤ 0` one pulse, `rise > 0` (RUN p1, p2) | `Pulse.law: PulseLaw::{Spice, Vacask}`; features `CircuitExtra::PulseSpice`, `PulseVacask` | circuit | now |
| G4 EXP `td2` | counted from `delay` (S 5.08). IR-CIRCUIT §3 says "its reader adds": that puts `delay` in two fields, which R13(b) forbids | `Exp.td2_from: TdFrom::{Zero, Td1}` + features | circuit | now (amends IR-CIRCUIT §3) |
| G5 PWL shaping | `scale offset stretch tdphase` (VD; S 5.33 says fold at read: that repeats an operand in every point, R13(b)) | `Pwl.{scale, offset, stretch, phase_deg}`; Spectre has the first three (S 5.33) | circuit | Spectre slice; `Private` now (0 `type="pwl"` decks in the corpus) |
| G6 SFFM pre-delay | VACASK holds the value at `delay` before it; ngspice 0 (S 5.02; VD dev-builtin-src `delay`) | `Sffm.pre: PreDelay`, as `Sin`/`Am`; `SffmLaw::Ngspice` keeps only the clamp and the defaults | circuit | now |
| G7 DC from the waveform | a waveform source's DC is its value at t = 0 (S 5.13; VS V33); ngspice's rule for SIN/PULSE/EXP/PWL with no DC is the same (DIALECT-NGSPICE §3) | `Source.dc_rule: DcRule::{Given, WaveAtZero}` (`dc` `NONE` under `WaveAtZero`); a structural "is `dc` the wave at 0" test is fragile, and the VACASK writer must know it to avoid the `alter type="dc"` pair | circuit | now (Q12) |
| G8 elaborate | promoted subckts form the top for the run items after it (VD cir-elaboration; VS V1, V2) | COVERAGE-run G-ELABORATE: run item `Elaborate{circuits: RefList}` | run | now (49 corpus files) |
| G9 enclosing scope | ngspice binds a nested subckt's reference to an enclosing subckt's parameter or model; VACASK never does (RUN n1, m1) | `Extra::EnclosingRef` (a `Ref` bound in an enclosing, non-top subckt scope), set by `features_used` | kernel | now |
| G10 dependent formal | a formal default that reads a parameter (ngspice `params: y={x*2}`); VACASK cannot override it (RUN c1, t2) | `Extra::FormalReadsParam` | kernel | now |
| G11 loop over op | VACASK spells a `Dc` as `sweep` + `op`, so a `Loop{Sweep}` whose body is one `Op` has no other spelling | `RunExtra::LoopOverOp` | run | now |
| G12 solver hints | VACASK-only options that tune convergence or accuracy only (`tran_lteratio`, `tran_rmax`, `nr_*`, `homotopy_*`, `tolscale`, …): the old writer dropped them with W0024 (VS V14) | `OpaqueClass::Hint`: another family drops it with its own note | kernel | accepted: A12 K3 (R19) |
| G13 vectors | vector and list values, indexing, vector sweeps (VD expr-vectors; S 3.08) | `Private` | — | later |
| G14 behavioural extras | `expr=`/`declarations=`/`evaluation=`/`discipline=`; `white_noise`, `flicker_noise` | R13 says a function is a `Func` member: `WhiteNoise`, `FlickerNoise` when a second dialect has them; `Private` statement now (0 in the corpus) | kernel | later (Q11) |
| G15 `$scale` | the `scale` option as an operand (VD expr-special) | kernel leaf `ScaleOpt` | kernel | later; 0 in the corpus |
| G16 slots, port outputs, `hbac`, `hbnoise`, `continuation=0`, `clear options` | as COVERAGE-run G-SLOTS, G-PORTOUT, G-CONTINUATION | `Private` | run | later |

## 6. The passes the first pairs need

What the ngspice reader produces is DIALECT-NGSPICE §3, IR-CIRCUIT, IR-RUN; what VACASK spells is
§3. Every feature in the difference gets a row. Proof keys: **eval** = the reference evaluator on
generated points plus the boundary set (±0, k+½, k+½±ulp, 2⁵²±1, operands holding no draw);
**sim(X)** = both forms of the same design run on simulator X, compared at the cross-sim
tolerance (§8); a pass that would duplicate an operand holding a `Draw` refuses that item.

### 6.1 ngspice → VACASK

Every expression rule is two rows (A26): `<rule>` for the `Func`/`Binary` form and `<rule>_dyn` for `FuncDyn`/`BinaryDyn`; the rows below name the rule once. The expression rows follow the rows registered on pass-expr 6b5b936f (its audit of this section).

| Feature | Pass | IR → IR (one sentence) | Requires | Proof | Old rule it replaces |
|---|---|---|---|---|---|
| `Binary(Div)` | `div_to_div_typed`, `_dyn` | `Div(a,b)` → `DivTyped(a,b)`, with `a` wrapped as `Mul(1., a)` unless an operand is surely Real by the tag table (pass-expr 6b5b936f) | `Binary(DivTyped)` | eval (typed) | none: `respell` copied `/`, so ngspice `{1/3}` was 0 on VACASK (RUN i1): ledger |
| `Func(PowAbs)` | `powabs_to_pow` | `PowAbs(x,y)` → `Pow(Abs(x), y)` | `Func(Pow)`, `Func(Abs)` | eval | `abs_power_out` |
| `Func(RoundHalfEven)` | `round_half_even_to_away`, `_dyn` | `x - floor(x) == 0.5 ? 2 * round_half_away(x * 0.5) : round_half_away(x)` (pass-expr 6b5b936f) | `Ternary`, `Eq`, `Sub`, `Mul`, `Floor`, `RoundHalfAway` (no `Fmod`) | eval | none (old copied `nint`, which VACASK lacks) |
| `Func(Sgn)` | `sgn_to_ternary`, `_dyn` | `Sgn(x)` → `x > 0 ? 1 : x < 0 ? -1 : 0`; U0105 when the operand may hold a draw (pass-expr 6b5b936f) | `Ternary`, `Gt`, `Lt` | eval | none: `sgn(0)` 0 → 1 (RUN e1): ledger |
| `Func(Step)` | `step_to_ternary`, `_dyn` | by its definition as a ternary of its argument (pass-expr 6b5b936f) | `Ternary` | eval | none |
| `Func(StepHalf)` | `step_half_to_ternary`, `_dyn` | ngspice's `u` (½ at 0) by its definition (pass-expr 6b5b936f) | `Ternary` | eval | none |
| `Func(Ramp)` | `ramp_to_ternary`, `_dyn` | by its definition; `Mul` for an operand that may be Int (pass-expr 6b5b936f) | `Ternary`, `Mul` | eval | none |
| `Func(Pwl)` | `pwl_to_ternary`, `_dyn` | a ternary chain, only on strictly increasing literal points, else U0106 (pass-expr 6b5b936f) | `Ternary` | eval | none |
| `Func(PowSigned)`, `Sqrt` of `Abs`, `Ln` of `Abs`, `ExpLin14`, `Sqr` | `pow_signed_to_ternary`, `sqrt_abs_to_sqrt`, `ln_abs_to_ln`, `explin14_to_ternary`, `sqr_to_mul` (each with `_dyn`) | being written on the lead's word: VACASK lacks these (pass-expr 6b5b936f) | — | eval | none |
| `DrawWhen(Always)` | `draws_always_to_sampling`, `_dyn` | relabel `Always` → `Sampling` with note U0109 (a plain run then uses the nominal) | `DrawWhen(Sampling)` | sim(VACASK) nominal; eval | `VacaskCalls` (the same text, now with its note) |
| `DrawPer(Reference)` | `draws_per_reference_to_call_site`, `_dyn` | `Reference` → `CallSite` only for `Sampling` draws in a design with no `MonteCarlo` loop; the case "each draw-holding parameter read once" is not implemented and is refused U0110 (R22; pass-expr 6b5b936f) | `DrawPer(CallSite)` | eval | — |
| `Expr(TStep)`, `Expr(TStop)` | `substitute_tstep`, `substitute_tstop` (each with `_dyn`) | each leaf → the design's one transient's `print_step` (TStep) or stop; literals only, else U0108; two transients → refused (pass-expr 6b5b936f) | one `Tran` | sim(ngspice) | none: a given-zero `TF` was written `fall=0`, which VACASK holds (RUN p1): ledger |
| `PulseSpice` | `pulse_spice_to_vacask` | relabel to `Vacask` when `rise > 0`, `fall > 0` and `period` is 0 or > rise+fall+width; else refused (at `period` = rise+fall+width ngspice repeats and VACASK refuses: RUN q13) | `PulseVacask` | sim(VACASK) after `substitute_tran_leaves` | `vacask_wave` Pulse, which wrote `period=0` there: one pulse where ngspice repeats (RUN q13): ledger |
| `PulseCycles` | `pulse_cycles_to_pwl` | the counted pulses → one PWL | `Wave(Pwl)` | sim(ngspice) | `pulse_as_pwl`, `vacask_wave` counted |
| `SffmNgspice` | `sffm_ngspice_to_spice3` | law `Spice3`, `pre: Zero` when `mdi ≤ fc/fm` (else `wave_to_behavioural`) | `SffmSpice3` | sim(ngspice) | `sffm_slots` |
| `AmPreZero`, `SinPreOffset`, `SffmPre(Zero)` with `delay > 0` | `wave_to_behavioural` | the source → a `Behav` voltage or current of `Time` with the pre-delay value stated | `Elem(Behav)`, `FuncDyn(Sin)`, `Expr(Time)` | sim(VACASK) both forms | W0108 refusal of a delayed AM with offset |
| `TdFrom::Zero` | `exp_td2_relative` | `td2` → `Sub(td2, td1)`, `td2_from: Td1`; `td2 ≤ td1` refused (VACASK: "td2 … must be greater than 0") | — | eval + sim(VACASK) | `vacask_wave` Exp |
| `PwlRepeatFrom` | `pwl_repeat_to_period` | `repeat_at = 0` and first value = last → `Period`; else refused | `PwlRepeatPeriod` | sim(ngspice) | `vacask_wave` Pwl, W0008 |
| `Wave(TrNoise)`, `TrRandom` | refused | VACASK has no transient noise source (S 5.18) | — | — | NoiseWave W0009 |
| `DcRule::Given` with a wave | — (writer) | the VACASK writer spells it as `alter instance(…) type="dc" dc=…` before DC analyses and `type=<wave>` before a transient | — | sim(VACASK) | `dc_alters`, `alter_lines` |
| `Elem(Poly)` | `poly_to_behav` | the polynomial in SPICE's term order → a `Behav` expression | `Elem(Behav)` | sim(ngspice) | `vacask_cards::poly` |
| `Elem(Table)` | `table_to_ternary` | `TableLaw::Exact` → a linear-segment ternary chain; `NgspiceRounded` refused | `Ternary` | eval | W0102 refusal (now `Exact` translates) |
| `Elem(ChargeCapacitor)` | `charge_to_behav` | `q` → `Behav{Current, Ddt(q)}` | `Elem(Behav)`, `FuncDyn(Ddt)` | sim(ngspice) (P4-017) | `Card::Charge` arm |
| `Elem(Port)` | `port_to_source_r` | EMF source + series `z0` resistor + an internal node; `Sp.ports` names the (source, resistor) pairs, VACASK's `acsp` port (VD cmd-analysis-acsp; RUN sp2: S11 = S21 = 0.5 on both, the closed form) | `Elem(VSource)`, `Elem(Resistor)` | sim(ngspice) | RfPort arm |
| `Elem(Laplace)`, switches, lossy lines, code models | refused | VACASK has none (C D23, D29–D35) | — | — | W0011, W0105 |
| `IcUicOnly` | `device_ic_to_nodeset` | each `Passive.ic` → a differential `NodeSet{Ic}` on every transient with `uic`; dropped where no transient has `uic` (ngspice ignores it, S 8.21) | `IcDifferential`, `IcNextOnly` | sim(ngspice) | `vacask_ic::held_ics` |
| `PassiveAc`, `SemiInitial` | refused | `ac=` is "not found" (S 6.17); VACASK has no `off` (S 6.28) | — | — | refusal; `off` (Q11) |
| `MultInstance` | `instance_mult_to_param` | the subckt gets a formal `mult__` (default 1), each element in it `mult × mult__`, the instance passes `mult__ = m` | `MultSemi`, `MultResistor`, … | sim(ngspice) | — (VACASK refuses an undeclared `$mfactor`, RUN n2) |
| `EnclosingRef` | `hoist_nested_subckts` | a nested subckt that reads an enclosing scope's parameter or model → hoisted to the top, the read parameters as formals passed by each instance, the models moved with it, names that collide renamed | `Placed(Subckt, Subckt)` off (R27: transitive along the scope chain) | sim(ngspice) | `read/blocks.rs` hoist |
| `FormalNotConstant` (R53, A29) | `formal_defaults_to_instances` | every instance passes every such formal, its default rewritten over the instance's actuals; the formal's own default becomes a constant | — | sim(ngspice) | none |
| `Item(Func)`, `Placed(Subckt, Func)` | `inline_funcs` (waits for A28: a body counts in its callers' form) | each call → the body with its arguments substituted (ngspice's own textual rule, S 2.26); removes `Expr(UserCall)`, requires nothing; a second row drops a `Func` nothing calls. `LocalFunc` is no feature (A15 `Placed`) (pass-expr 6b5b936f) | — | eval | W0106 refusal (now translates) |
| `Extra::LongLiteral` | `long_literal_to_shortest`, either direction | a `Long` literal → the correctly rounded shortest double, with a note; VACASK keeps the digits (run lr1), so only a target without it needs it (A18; pass-expr 6b5b936f) | — | eval | none |
| element with no model (G2), sources, couplings and lossless lines (TlineIdeal, A20) included | `explicit_family_models` | one parameterless `Model` per (file or section, family), at the file's top; inside a library section the model goes in the innermost section only when the design already places models there, else note U0301 (A32); a model in a subckt, branch or section body adds `Placed(kind, Model)` (law P3). Until spine's legalize change lands it also adds the libraries its models need (pass-structure, S4.4c) | — | sim(ngspice) | `Need` in `named/vacask.rs` |
| model with no `DeviceLibrary` of its family (G2) | `device_libraries` | one `DeviceLibrary{family, path: NONE}` per family in use that is not library-free (library-free: the builtins but tline_ideal, and `Module`, `CodeModel`, `Other`: R44), at the entry file's top after its title | `Item(DeviceLibrary)` (pass-structure, S4.4c) | sim(VACASK) | implied loads in `named/vacask.rs` |
| `TranCapNgspice`, `print_step` | `tran_explicit_cap` | `max_step := min(print_step, (stop − record_start)/50)` when `NONE`; `initial_step := print_step`; `print_step := NONE` | `TranMaxStep`, `TranInitialStep` | sim(VACASK) | `vacask_analysis` Tran (`step=` tstep, `maxstep`) |
| `Analysis(Dc)` with a target VACASK sweeps | — (writer) | `sweep` + `op` | — | sim | Dc/DcAxes arms |
| `Analysis(Tf)` | `tf_to_dcxf` | `Tf{output, source}` → `DcXf{output}` (the source's entry is the backend's) | `Analysis(DcXf)` | sim(VACASK) | Tf arm (`dcxf out=`) |
| `Sens`, `SensAc`, `Pz`, `Disto`, `Four`, `Measure`, `TimedOp` | refused | VACASK has none (C G10; `.four`/`.meas` evaluated by the backend, D7) | — | — | W0015, W0109 |
| a `WholeRun` state item (`Option`, `Env`, `NodeSet`) after a run item (R24, A14) | `whole_run_state_first` | move the item before the first run item of its file (VACASK's `options` apply from where they stand, `FromHere`); refused when the run items are in another file than the item (an include) | — | sim(ngspice): both forms | the old writer gathered every option into one `options` line |
| `RunExtra::RunItemNested` (R26) | `hoist_run_items` | a run item inside a subcircuit body → the deck's top scope, after the deck's other items (VACASK forbids `control` in a `subckt`, VD cmd-overview) | — | sim(ngspice) | — |
| `LoopKeepLast` | `loop_keep_last_to_alter` | append `Alter{target, last value}` after the loop | `Item(Alter)` | sim(VACASK) | none (ngspice `foreach` keeps the last value, S 13.03) |
| `Option(Rshunt)` | `rshunt_to_gshunt` | `gshunt = 1/rshunt` | `Option(Gshunt)` | sim(both, S 10.05: 3/7 V) | `option_spell("rshunt")` |
| ngspice-private and unmapped options | refused, or dropped when `Inert`/hint (G12) | — | — | — | W0017, W0024 |
| `StatisticsDraws` (Spectre block) | not this pair | — | — | — | — |
| `TranStartup` | `startup_ramp` (planned) | — | — | — | StartupRamp |

### 6.2 VACASK → ngspice

| Feature | Pass | IR → IR | Requires | Proof | Old rule |
|---|---|---|---|---|---|
| `Binary(DivTyped)` | `div_typed_to_div`, `_dyn` | by the tag table over the whole design (every instance's actuals): an operand real → `Div`; both int → `Trunc(Div(a, b))` (never `IntDiv`: ngspice's `\` is trunc(\|a/b\|), so `IntDiv` has no speaker); mixed across instances → note U0101, and every Int quotient waits | `Div` and `Trunc` in both forms (tags cross forms, so it rewrites every `DivTyped` once its own form is present; pass-expr 6b5b936f) | eval (typed) | none (old copied `/`) |
| `Func(ToInt)` | `toint_to_trunc`, `_dyn` | `Trunc`; U0103 while any `DivTyped` stands (pass-expr 6b5b936f) | — | eval | none |
| `Func(Pow)` | `pow_to_powabs`, `_dyn` | whole literal exponent: even → `PowAbs`, odd → `PowSigned` (not `Mul(Sgn(x), PowAbs)`: that gives +0 at x = −0 and reads x twice); other literal → `PowAbs` (equal on `Pow`'s domain, R12); a non-literal exponent → `PowAbs` only when the base is never negative or −0, else U0104 (pass-expr 6b5b936f) | `Func(PowAbs)`, `Func(PowSigned)` | eval | `abs_power_in` |
| `Func(SgnZeroPos)` | `sgnzeropos_to_ternary`, `_dyn` | VACASK's `sgn` → `x >= 0 ? 1 : -1` typed as `x` (pass-expr 6b5b936f: a row §6 lacked) | `Ternary`, `Ge` | eval | none |
| `Func(ToReal)` | `toreal_to_mul`, `_dyn` | `ToReal(x)` → `Mul(1., x)` (A21; pass-expr 6b5b936f) | `Mul` | eval | none |
| `Extra::LongLiteral` | `long_literal_to_shortest` | as in 6.1 | — | eval | none |
| bit operators | `fold_constant_bitops` (12 rows: bitand, bitor, bitxor, shl, shr, bitnot, each with `_dyn`) | literal operands only (nested bit ops fold over legalize's rounds) → a `Plain` literal; else U0107 (pass-expr 6b5b936f) | — | eval | `caret_reread` W0002 |
| `Func(RoundHalfAway)` | `round_half_away_to_floor`, `_dyn` | by sign, with `floor` or `ceil` (never `floor(x+½)`, wrong at ½−ulp) | `Ternary`, `Ge`, `Sub`, `Add`, `Floor`, `Ceil` (pass-expr 6b5b936f) | eval | none |
| `Func(Atan2)`, `Func(Hypot)` | refused (R50: not exact) | over 184041 points: at most 1 ulp where both operands are nonzero within 1e±150, but a domain error where the original has a value at 21656 points (atan2) and 79062 (hypot), and atan2 loses a zero's sign at 218 (pass-expr 6b5b936f) | — | — | old rewrite (VS §2) |
| `FuncDyn(Idt)` | `idt_to_integrator` (the waves group's, not pass-expr's) | a `Behav` current into a new node with a 1 F capacitor and an `Ic` of the initial value | `Elem(Capacitor)`, `IcEveryOp` | sim(VACASK) both forms (S 2.18: 9e-9 on ngspice) | none |
| `Expr(Tnom)` | `tnom_to_env`, `_dyn` | → the design's one whole-run `Env` with a literal `tnom`; no `Env` is refused (the default tnom is each simulator's own; pass-expr 6b5b936f) | one whole-run `Env` | eval | none |
| `DrawWhen(Sampling)` outside every `MonteCarlo` | `sampling_draws_to_nominal`, `_dyn` | `Draw` → its nominal argument (`1 * nominal` while a `DivTyped` stands and the nominal may be Int); U0110 when a `MonteCarlo` loop exists (pass-expr 6b5b936f) | `Mul` in both forms | sim(VACASK) (RUN g2) | `stat_nominal` analogue |
| `Draw(CallSite)` inside `mc` | refused | ngspice's draws are per reference; the two-layer idiom (DESIGN-mc-lowering) is a later pass | — | — | none |
| `PulseVacask` | `pulse_vacask_to_spice` | `fall ≤ 0` → width = period − rise and an ideal fall at the period's end (period ≤ 0: width = `TStop`), law `Spice` | `PulseSpice` | sim(VACASK) after form with fall>0 | none (old read the pulse as SPICE's) |
| `TdFrom::Td1` | `exp_td2_absolute` | `td2` → `Add(td1, td2)`, `td2_from: Zero` | — | eval | named_waves |
| `SffmPre(HoldDelayValue)`, `AmPreHold`, `SinPreHold` with `delay > 0` | `wave_to_behavioural` | as in 6.1 | `Elem(Behav)` | sim(VACASK) | `sffm_as_b`; VS V32 R |
| `SffmSpice3` | `sffm_spice3_to_ngspice` | law `Ngspice` when `mdi ≤ fc/fm` (ngspice clamps), else `wave_to_behavioural` | `SffmNgspice` | sim(ngspice) | `sffm_swap`, `sffm_as_b` |
| `DcRule::WaveAtZero` | — | ngspice's own rule for SIN/PULSE/EXP/PWL; for AM and SFFM (ngspice: 0) `dc_wave_at_zero` writes the wave's t=0 expression | — | eval | VS V33 |
| `Alter` of a source `type` around DC analyses | read rule | the canonical pair (VS V5) is read as `DcRule::Given`; any other `type` alter → `Private` | — | sim | VS V5 |
| `MultVSource`, `MultVcvs`, `MultCcvs` | `drop_voltage_mult` | drop `mult`; every `I(element)` read divided by `m` (VACASK reports per copy) | — | sim(VACASK) | VS V31 |
| `LoopRestore` | `loop_restore_to_alter` | append `Alter{target, value before}` after the loop | `Item(Alter)` | sim(VACASK) | none |
| `LoopOverOp`, `Dc` axes ngspice's `.dc` lacks (`DcAxisDeviceParam`, `ModelParam`, `Option`) | `dc_axes_to_loops` | the axis → a `Loop` around a `Dc` of the rest (or an `Op`) | `LoopDeviceParam`, … | sim(ngspice) | `.dc R1` lin only (VS V12) |
| `Tran` `print_step: NONE`, `cap: SpanOver50` | `tran_explicit_cap` | `max_step := (stop − record_start)/50` when `NONE`, `print_step := initial_step` (or `max_step`) | `TranMaxStep` | sim(VACASK) (RUN tr1) | reader `num()` |
| `IcNextOnly`, nodeset per analysis | `ic_per_analysis_to_deck` | equal on every transient → one deck-wide `NodeSet`; else refused | — | sim | — |
| `IcDifferential` | refused | ngspice `.ic` is single-node (VS V26) | — | — | — |
| `Elaborate` (one, before every run item) | `splice_elaborate` | the promoted bodies' items move into the top scope, their definitions to the top; names already unique (VACASK refuses a collision, VS V1) | — | sim(VACASK) both forms | `vacask_promoted`, VS V1 |
| `Elaborate` (several) | refused | ngspice cannot swap its top circuit (VS V2) | — | — | W0122 |
| `Analysis(DcInc)` | `dcinc_to_ac0` | → `Ac` at one point of 0 Hz; refused when a source has `phase ≠ 0` and the design has another AC analysis | `Analysis(Ac)` | sim(VACASK) (VS V23: 9.561172e-1 both) | misclassified W0015 |
| `Analysis(DcXf)` | `dcxf_to_tf` | one `Tf` per independent source | `Analysis(Tf)` | sim(ngspice) | VS V24 |
| `Xf`, `Stb`, `Hb`, `Pss`, `Pac`, `Pnoise`, `TranNoise` | refused | ngspice-45 pinned has none of them (S 14.10: no `.pss`) | — | — | W0015 |
| `Sp` over (source, resistor) pairs | `source_r_to_port` | each pair → one `Port{index, z0 = the resistor's value}`, in list order; fewer than two ports refused (ngspice: "we need at least two", RUN sp2) | `Elem(Port)` | sim(both): RUN sp2 | — |
| `CircuitExtra::NameWithoutLetter` (R23) | `name_with_letter` | an element whose name does not begin with its kind's SPICE letter (`amp1` an instance, `load` a resistor) → the letter + its name, renamed again on a collision in its scope; every by-name reference follows (`Expr::I`, couplings, sensing elements, sweep, alter and probe targets, hierarchical paths); the rename map travels as a note of the pass at each renamed element, with the old name as its argument (A32), not as a run fact in the IR | — | sim(VACASK): both forms give the same numbers under the map | the named reader's `strip`/letter map |
| `Item(DeviceLibrary)` | `drop_device_libraries` | removed whatever its path: a `DeviceLibrary` exists only for a proven file (R45) | `ModelWithoutLibrary` (pass-structure, S4.4c) | sim(both) | `own_loads`, A19 |
| `CircuitExtra::BuiltinMaster` (a parameterless `Model` of a source, controlled source or coupling family, or a lossless line naming tline_ideal) | `drop_builtin_models` | removed, each element's `model` set to `NONE` (ngspice has no model for a source); a source whose model binds no definition keeps the feature with note U0302 | `ElementWithoutModel` (pass-structure, S4.4c) | sim(both) | the reader's implied models |
| `Model.module` of a user OSDI | — (writer) | ngspice `pre_osdi` + `N` card (C A18, B24) | — | sim | OsdiLoad |
| `Option(Private)` of VACASK | refused (`tolscale`: one corpus deck) | — | — | — | W0024 note |
| `Opaque` `Hint` | `legalize` drops it with its note (R19) | — | — | — | W0024 note |
| `Inert` opaques (`print`, `clear saves`, `ground`, `elaborate changes`) | `legalize` drops with a note (R11) | — | — | — | W0120 |

Old rules with no pass: `foreign_text` (the spine's foreign-`Opaque` refusal), `brace_groups`,
`foreign_prints`, `spell_if` (ngspice writer spelling), `ngspice_ternaries` (the ngspice
reader's tree), `stat_nominal` (`sampling_draws_to_nominal`); the named writer's lowercasing of a
SPICE-origin deck (C A7) is dropped: IR names are already one spelling (ledger line). Every other
pair.rs field holds only for Xyce, HSPICE or LTspice.

## 7. Diagnostics

`codes.rs`: VACASK's own range, `P2xxx` reading, `R2xxx` binding, `W2xxx` writing (ngspice took
`1xxx`; the range row goes in `src/dialect/mod.rs`'s table). Each row: title, cite, explanation,
kind. Tests pin codes, never messages; `facade::OLD_CODE` gets a row only where a routed test
asserts an old code (R9).

| Code | Kind | Meaning | Cite |
|---|---|---|---|
| P2001 | refusal | not a VACASK number (`1k5`, `1e3k`, `1d3`) | S 1.05–1.07 |
| P2002 | refusal | `inf`/`nan` literal (NaN gate) | VD input-numbers; C9 |
| P2003 | note | a statement read as `Private` (one per construct class of §3) | §3 |
| P2004 | note | a statement of an area not written yet, kept `Unread` (S4.0's stubs) | §2 |
| P2005 | refusal | `parameters` inside an `@if` branch | VD cir-conditional |
| P2006 | refusal | `v()`, `i()` or `$abstime` outside a behavioural source | VD dev-builtin-behavioral |
| P2008 | refusal | `load`, `embed`, `global` or `ground` inside a subcircuit or branch | VD cir-subckt, cir-loading |
| R2001 | refusal | a parameter defined twice in one scope | RUN n3 |
| R2002 | note | an instance whose master no read file defines (`Unresolved`) | VS V29 |
| R2003 | refusal | a name only an enclosing subckt defines | RUN n1, m1 |
| R2004 | refusal | an instance overrides a dependent parameter | RUN c1, t2 |
| R2005 | refusal | `$mfactor` on a subckt that does not declare it | RUN n2 |
| W2001 | note | an analysis given a generated name | §4 |

### 7.1 The writer's refusals by class (R21; state of the trunk candidate)

`Write::admits` refuses only what no feature states, each code at its item (A25), in one of three
classes: invalid on every simulator known (a candidate IR invariant), valid elsewhere but not on
VACASK (a candidate feature), or VACASK's own limit.

| Code | Owner | What it refuses | Class |
|---|---|---|---|
| W2105 (subckt half) | S4.1 | an instance whose terminal count is not its subckt's ports | IR invariant (the lead's ruling) |
| W2203 | S4.2 | a source-like element with instance parameters | IR invariant, to be confirmed by S4.2 |
| W2006 | S4.0 | `to_real` other than over a parameter read in a behavioural source's expression (in a parameter expression of class Temp: gap (c), deferred by R52) | feature |
| W2106 | S4.1 | a behavioural source with `tc1`, `tc2`, `temp`, `dtemp` or `rpar` (ngspice's B has them) | feature |
| W2108 | S4.1 | an element whose kind is not the one its model's family gives | feature |
| W2201 | S4.2 | an AM whose modulation offset is not the literal 1 (goes with A31's `AmModOffset`) | feature |
| W2304 | S4.3 | an option row of a key only another simulator has | feature |
| W2305 | S4.3 | a reset of a class other than the saves | feature |
| W2002 | S4.0 | a device library whose path is no file of its family | VACASK's own |
| W2004 | S4.0 | a load whose extension does not state its kind (R48 d) | VACASK's own |
| W2007 | S4.0 | a source with a DC value and a wave that the alternation cannot state (R42) | VACASK's own |
| W2008 | S4.0 | an integer literal past 32 bits (R54) | VACASK's own |
| W2101 | S4.1 | a name with white space or a line break | VACASK's own |
| W2102 | S4.1 | a parameter the shipped module lacks; a user module's `version=` | VACASK's own |
| W2103 | S4.1 | two libraries of one family whose files hold different modules | VACASK's own |
| W2104 | S4.1 | a master no visible model or subckt statement names (VACASK takes the kind from the master) | VACASK's own |
| W2105 (module half) | S4.1 | a device against its shipped module's terminal count | VACASK's own |
| W2107 | S4.1 | a model of a family no run proved a VACASK module implements (R41 a) | VACASK's own |
| W2303 | S4.3 | a save shape VACASK has no directive for (R29, S12) | VACASK's own |

W2301 and W2302 are notes (a name the writer gave), not refusals. They go with ir-run's
`AnalysisUnnamed`, `DcAxisUnnamed` and `LoopUnnamed`, which `FEATURES` will lack.

## 8. Tests (each names its oracle)

- **Reader fixtures** `tests/fixtures/dialect/vacask/read/<clause>.sim` + `.ir`, the IR written
  by hand from the VD clause or run of each §3 row, with a refusal twin where VACASK rejects a
  form (RUN p2, n3, c1, n2). One per row at least.
- **Settle fixtures** `…/settle/<deck>.sim` with the numbers Appendix A records: VACASK is their
  oracle; they fail if a binary update changes a rule.
- **Writer fixtures** `…/write/<name>.ir` + `.sim` (expected text by hand), and for every node
  with a number, a deck built in IR text, written, run on VACASK, compared with a closed form.
- **Laws** L1–L8 (`tests/dialect_laws.rs` iterates `BUILTIN`): round trip on `Gen(FEATURES)`,
  determinism, totality.
- **Corpus** (S4.5): L6 over the 184 VACASK corpus files (`$VACASK_CORPUS`, gnucap decks out of
  set, VS V35) and `demo/cmos180n.lib` with each section; the census of `Opaque` by class, strict
  both ways, in `…/vacask/census.txt`.
- **Simulator round trip**: `vacask(write(read(d)))` against `vacask(d)` for every corpus deck,
  **bit-identical** (same digits, same statement order but hoisted loads; VACASK is
  deterministic); every exception a ledger line. `test_sections_pdk`'s cmos180n corner test
  routed.
- **Pass tests**: each §6 row's proof, red on a stub first: eval rows on generated expressions and
  the boundary set; sim rows run both forms of each fixture on the named simulator.
- **Cross-sim** (the pairs): the existing harness (`tests/test_cross_sim/`, cells
  `ngspice-vacask` and `vacask-ngspice`) at its tolerance, which is each simulator's own Newton
  accuracy summed: op/dc/ac within 2·(RELTOL·swing + floor), RELTOL 1e-3, floor VNTOL 1e-6 V or
  ABSTOL + GMIN·Vmax; a transient within 1e-2 of the swing + 2·VNTOL in a 1 % time tube
  (`cmp.rs:9-25`). Its lists stay strict both ways: a line the rewrite fixes is removed.
- **Differential** against the old pipeline (REWRITE-PLAN §7.3) for `vacask → vacask`, `ngspice →
  vacask`, `vacask → ngspice`, compared by meaning; the expected differences are named now
  (integer division, given-zero `fall`, `sgn`, plain-run draws, name case, written `save`s): each
  a `REWRITE.md` line with its run.

## 9. Gates (by name, never by count)

- **`vacask → vacask`**: laws green for `vacask`; every test that names VACASK green through the
  facade (`test_vacask_import`, `test_vacask_runs`, `test_vacask_modules`, `test_sections_pdk`
  cmos180n, `test_vacask_backend.py`); the corpus census with no `Unread` beyond the old W0001 +
  W0032 lines of `research/in-ours-vacask.txt`; the simulator round trip green; the differential
  with no unledgered difference; `tests/layering.rs` green.
- **`ngspice → vacask`**: the above plus S3's gate; every §6.1 pass test green; the cell
  `ngspice-vacask`'s failures a subset of `tests/fixtures/cross_sim/ngspice-vacask.txt`; every
  `#[test]` that writes ngspice text to VACASK green (`test_spice_exprs`, `test_waveforms`,
  `test_steps`, `test_epochs`, `test_options`, `test_monte_carlo`, `test_conditionals`).
- **`vacask → ngspice`**: the above for §6.2 and the cell `vacask-ngspice`
  (`vacask-ngspice.txt`, `vacask-source.txt` unchanged or shrunk).

Then one row per pair in `ROUTES`.

## 10. Questions: the rulings, and what is still open

Answered by R16–R22 and AMEND-A12 (ir-rewrite ee2fbca0), applied above:

| Q | Ruling | Where it lands here |
|---|---|---|
| 1 typed arithmetic | yes (R16, A12 K1); hex is `Int`; int/int is `Trunc(Div)`, never `IntDiv` | §3.3, §5 G1, §6 |
| 2 device library | yes, as the circuit item `DeviceLibrary{family, path}` (R17, A12 C5) | §3.2, §4, §5 G2, §6 |
| 3 R8 | typed when a second dialect **or a pass** reads or writes it (R18) | §5 |
| 4 solver options | `OpaqueClass::Hint` (R19, A12 K3); `tolscale` `Private` (one deck) | §3.4, §6 |
| 5 plain-run draws | relabelled with a note; per-call-site only where the distribution is unchanged (R22) | §6.1 |
| 6 parameter names | `Write::admits` (R21, A12 S1) fed by `tables.rs`; L5 needs name pools per family (the hook's shape goes to the spine owner) | §4 |
| 7 EXP `td2` | `td2_from` (A12 C2) | §5 G4 |
| 8 docs against binary | the run wins; settle fixtures pin both | §8 |
| 9 `Dc` against `Loop{Op}` | two meanings (R20); G11 stands; results' shape is S12's | §5 G11 |
| 10 `acsp` port, `tran_lteratio` | settled by runs sp2, lte (Appendix A); `tran_lteratio` is a `Hint` | §6 |
| 11 `off`, noise functions, `$scale` | `off` refused; the others `Private` statements | §3, §6 |
| 12 ngspice DC rule | `DcRule` (A12 C4); the lead tells S3 | §5 G7 |
| 13 pulse at period = rise+fall+width | settled by run q13: ngspice repeats, VACASK refuses, the old `period=0` was a wrong number; refused | §6.1 |
| 14 backend reads the deck | the generated names stand until S12 | §4 |

Still open:

15. **Families of the builtin devices** (A12 C5, to agree with `ir-circuit`): VACASK needs a
    `model` for `vsource`, `isource`, `vcvs`, `vccs`, `ccvs`, `cccs` and `mutual` (RUN n2), and no
    `ModelFamily` names them. Proposal: one member per builtin, `SourceV`, `SourceI`, `Vcvs`,
    `Vccs`, `Ccvs`, `Cccs`, `Mutual`, each "the simulator's built-in device of that element kind,
    no parameters", so that one model serves one element kind and `drop_builtin_models` and
    `explicit_family_models` stay neutral.
16. **`tran_lteratio` against `trtol`**: run lte shows no difference on an RC (the span/50 cap
    binds); the estimators' equality stays unproven, which is why it is a `Hint`.

## Appendix A. Runs made for this plan (VACASK 36be560)

| Deck | What it does | VACASK printed |
|---|---|---|
| e1 | `parameters` and `print` of `sgn(0)`, `int(-2.5)`, `integer(-2.5)`, `round(±2.5)`, `(-2)**3`, `-2**2`, `2**3**2`, `fmod(-7,3)`, `7^3`, `1?0?4:8:16`, `sgn(-0.0)`, `int(2.7)`, `floor(-2.5)` as resistances | 1, −2, −2, 3/−3, −8, 4, 512, −1, 4, 8, 1, 2, −3 (same in `print` and in parameters) |
| i1 | `r=(1/3)*30+10`, `(a/b)*30+10` with `a=1 b=3`, `c=1.0`, `-7/2`, `1/3.0`, `2**-1`, `1k/3`, `1e3/3` | 10, 10, 20, 7, 20, 25, 343.3, 343.3 |
| t3 | subckt `x=1`, `r=x/2+10`; instances `x=3` and `x=3.0` | 11 and 11.5 |
| f2 | `print` of `integer(3.0)/2 int(3.0)/2 floor ceil round abs(3) min(3,5) min(3.0,5) sgn(-3) (1?3:4.0) -3 3**1 3*1 (3>2)`, each `/2` | 1.5 1 1.5 1.5 1.5 1.5 1 1.5 0 1 −1 1.5 1 0 |
| f1 | `sign(3,0) sign(3,-0.0) sign(-3,-1) 2./3 0x10/3 1e3/7 1M 2x 1mil 100u` | 3 3 −3 0.667 5 142.857 1e6 2e6 2.54e-5 1e-4 |
| n1 | nested subckt reads its enclosing subckt's parameter | "Variable or constant 'p' not defined." |
| m1 | nested subckt uses its enclosing subckt's model | "Master 'res' not found." |
| n2 | instance without `model vsource vsource`; then `$mfactor=2` on an X whose subckt does not declare it | "Master 'vsource' not found."; "Parameter '$mfactor' not found." |
| n3 | `parameters a=1` then `parameters a=2` | "Parameter redefinition." |
| n4 | `parameters A=1 a=2`, instances `R1 … r=A`, `r1 … r=a` | two instances, 1 and 2 |
| n5 | `parameters a=b*2` before `b=3` | 6 |
| c1 | subckt formal `x=` `2*3`, `q*2` (q top-level), `M_PI`, `sqrt(4)`; instance `x=7` | 7, "Parameter 'x' not found.", 7, 7 |
| t2 | override of a dependent `y=x*2` | "Parameter 'y' not found." |
| t1 | real given where an integer is expected (`noisy=0.`, `type=1.`, `points=2.`) | accepted: `noisy = 0 (integer)`, `type = 1 (integer)`, raw written |
| p1 | pulse `rise=1u fall=0 width=1u period=4u` beside `fall=1u` | at 2.5/3.5/4.5 µs: 1.0/1.0/0.497 against 0.5/0.0/0.497 |
| p2 | pulse `rise=0` | "Rise time of pulse transient must be grater than 0." |
| s1 | `sweep` of `v1 dc` over 2, 3 before `op`, then another `op` | 2, 3, then 1 |
| g2 | `r=agauss(5,1,1)` outside `mc` | 5 |
| tr1 | `tran stop=10m step=1u`, and with `start=6m`, no `maxstep` | largest step 2e-4 and 8e-5: (stop − start)/50 |
| f3 | `print` of `min(3,5.0) min(3.0,5) max(3,1.0) max(3.0,1) atan2(3,1) hypot(3,4) pow(3,1) (-3) (!0) (3&&5) (7&3) (3<<1) sinh(0) (3+0.0) (5-2) (0?3:4.0)`, each `/2` (`3<<1` over 4) | 1.5 1.5 1.5 1.5 0.6245 2.5 1.5 −1 0 0 1 1 0 1.5 1 2: `min`/`max` int only when every operand is |
| q13 | pulse 0→1, rise 1u, fall 1u, width 2u, period 4u (= rise+fall+width) on ngspice-45 and on VACASK | ngspice at 0.5/3.5/4.5/5.5 µs: 0.5/0.5/0.5/1.0 (repeats); VACASK: "Period of pulse transient must be greater than rise+fall+width." |
| sp2 | two ports, z0 50 Ω, 100 Ω in series: ngspice `V … portnum n z0 50` + `.sp`; VACASK `vsource` + 50 Ω series `resistor` pairs + `acsp ports=[…]` | S11 = S21 = 0.5 on both (closed form 2·50/(2·50+100)); a one-port ngspice deck: "Only one RF Port is found, we need at least two!" |
| lte | RC 1k·1u charged by a 1 ns step, `tran stop=5m step=1u` at `tran_lteratio` 3.5 and 7; ngspice `.tran 1u 5m` | VACASK 77 points, worst error to 1 − e^(−t/τ) 2.8e-4 at both ratios; ngspice 5022 points, 5.0e-7 |
| f4 | `print` of `sgn(3.0)/2 sgn(-3.0)/2 sgn(3)/2 sgn(0.0)/2 sign(3.0,1)/2 sign(3,1)/2 sign(3,1.0)/2` | 0.5 −0.5 0 0.5 1.5 1.5 1.5: `sgn` takes its operand's type, `sign` is real |
| c2, c3 | subckt formal `x=$temp+1` (c2), `x=$tnom+1` (c3); instance `x=7` and one without | "Parameter 'x' not found.": a default reading `$temp` or `$tnom` makes the formal dependent, not overridable |
| pl/sec, pl/br, pl/brs, pl/sub | a library `section` holding `load`, `global`, `model`, `parameters`, `subckt`, `include`; an `@if` branch including a file of `parameters` (br) and holding a `subckt` (brs); a `subckt` holding an `include` (sub) | sec and sub accepted; br and brs rejected: the placements of `read_structure::PLACEMENTS` |
| bp1, bp2, bp3 | behavioural sources `v=v(a)*2`, `v(a, b)+1`, `i(v1)*1k`, `idt(v(a))`, `ddt(v(a))` at an op (a = 2, b = 1 by a divider); `parameters p=ddt(1)`; `v=idt(1, 3)` | 4, 2, −1 (`i` is the branch flow into the first terminal), 0, 0; "Function 'ddt' not found."; 3 (the two-argument `idt` starts from its second argument: A28 `Idt(x, ic)`, a literal-0 `ic` read as the absent one) |
| dcw | ngspice-45 `V1 a 0 DC -6 PULSE(0 1 1u 1u 1u 2u 10u)` into 1k: `op`, `tran 10n 5u`, `op`; VACASK on `type="dc" dc=-6` with the pulse's parameters, `alter instance("v1") type="pulse"` before the `tran` and `type="dc"` after it (R42) | both: op −6, transient 0, 0.5, 1, 0.5 at 0.5, 1.5, 3, 4.5 µs, op −6 again |
| iw1 | `r=` of `2147483647`, `2147483648`, `4294967297`, `2147483647+1`, `65536*65536`, `99999999999` (a 21-digit integer: "Integer literal out of range.") | 2147483647, −2147483648, 1, −2147483648, 0, 1215752191: VACASK's integers are 32 bits and wrap, literals and arithmetic alike |
| lr1 | `vsource dc=` of 1 + 2⁻⁵³ (a tie), a hair above it, 1 + 3·2⁻⁵³ (a tie), and 30 digits `e-29` | the correctly rounded doubles, ties to even: VACASK keeps a long literal's digits (A18 `LongLiteral`) |
| dp1 | `subckt s(a a)` | "Terminal '1' is not unique.": a subckt naming a port twice is `InvalidInSource` R2001 (found by the mutation law) |
| ip1–ip5 | behavioural sources over `parameters pi=3 pr=2.7 pw=3.0 pn=-2.7`: `$intparam(pi)/2`, `$intparam(pr)`, `$intparam(pw)/2`, `$intparam(pn)`, `$realparam(pi)/2`, `pi/2`, `sin(pi)`, `$intparam(pr)/2`; `$intparam` of 3e9, 2147483647.5, 1e20; `v=ps` with `ps="x"`; `$intparam` in a parameter; `$intparam(pa+1)` | 1, 2, 1, −2, 1.5, 1.5, sin 3, 1: `$intparam` is `to_int`, `$realparam` and a bare name `to_real` (R56); −2147483648, 2147483647, −2147483648 (R54's domain); "Cannot convert string into real."; "Function '$intparam' not found."; "$intparam() requires a single identifier argument." (P2010) |
| fx | SPICE text in a VACASK file: `.model dn d is=1e-14`, `save v(p0)` at the top, `+ is=1`, `.subckt s a b` | "Syntax error, unexpected string." (and "Parser syntax error … expecting (" for `save`): every statement no reader rule reads is one VACASK rejects: today it stays `Unread` P2004 (all of them in the corpus' 37 ngspice-format files); `InvalidInSource` waits for `write_run::control_span` to tell a top-level `save` from a control block's |
| ca1, ca2 | `parameters p=sin(1, 2)`, `p=atan2(1)` | "Function 'sin' accepts at most 1 argument(s).", "Function 'atan2' requires at least 2 argument(s)).": a call of another argument count is `InvalidInSource` P2011 (the IR's CallArity X0019) |
