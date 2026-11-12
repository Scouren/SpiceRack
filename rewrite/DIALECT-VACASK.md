# Slice S4: the VACASK dialect (reader and writer) and the pairs ngspice ↔ VACASK

The second dialect, from a second syntax family: it tests whether the contract (`Read`, `Write`,
`FEATURES`, passes that know no dialect) holds outside SPICE cards, and whether pair rules really
become passes. Read `REWRITE-PLAN.md` §2–§5, §7, `rewrite/SPINE.md`, `IR-KERNEL.md` (A1–A10),
`IR-CIRCUIT.md`, `IR-RUN.md`, `RULINGS.md` (R1–R15) and `DIALECT-NGSPICE.md` first. Everything
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
| load | top level only; `.osdi` or `.va` (compiled by OpenVAF-r); `map=` renames modules; parameter and terminal names lowercased (VD cir-loading) | a shipped library file (`spice/*.osdi`, `spice/{sn,full}/*.osdi`, VACASK's own `resistor capacitor inductor diode bsim3v3 bsim4v8 psp103v4 vbic_1p3* tline_ideal …` under `lib/vacask/mod`) → `Load{Library, family}` (§5 G2); any other → `Load{Osdi\|VerilogA, path}`; `map=` other than the default → `Opaque` `Private` |
| embed | `embed "f" <<<MARK … >>>MARK` writes a file (VD input-embed) | read by a `load` → `Opaque` `Private` (the module text has no IR home; VS V27); read by nothing → `Inert` (C A20) |
| subckt | `subckt n (ports) … ends`; may nest; `global`, `ground`, `load`, `embed`, `control` forbidden inside (VD cir-subckt) | `Subckt`; nested → `Extra::NestedSubckt` |
| parameters | a value that folds to a constant at parse time (literals, builtin constants, functions of them) is a formal an instance may override; one that reads a parameter or variable is dependent and cannot be overridden (VD cir-subckt; RUN c1: `2*3`, `M_PI`, `sqrt(4)` overridable, `q*2` "Parameter 'x' not found"; RUN t2) | in a subckt: constant → `Subckt.params`; dependent → a local `Param` item; at the top → `Param` |
| `var` | a circuit variable, global, below parameters in lookup (VD expr-cirvars, cmd-var: right-hand sides use the values before the statement) | before the first analysis or elaborate and not shadowed by a top `Param` → top `Param`; later → `Alter{Param}`; shadowed → `Private` |
| global | top only; ground nodes are global (VD cir-nodes) | `Global` |
| `@if/@elseif/@else/@end` | elaboration-time; bodies hold instances, models, nested `@if` only; nonzero number or non-empty string is true (VD cir-conditional) | `If` chain; a string condition → `Private` |
| model | `model n module params`; the module is a builtin or loaded (VD cir-masters) | `Model{family, polarity, params, module}` by `tables.rs`: `sp_resistor` Resistor, `sp_capacitor` Capacitor, `sp_inductor` Inductor, `sp_diode` Diode (`level=3` DiodeLevel3), `sp_bjt` GummelPoon, `sp_mos1/2/3/6/9` Mos1…Mos9, `sp_bsim3v3` Bsim3 (3.3.0), `sp_bsim4v8` Bsim4 (4.8.x only, S 7.04), `sp_jfet1` JfetShichman, `sp_jfet2` JfetParkerSkellern, `sp_mes1` Mesfet, `sp_vdmos` Vdmos, `vbic13`/`vbic13_5t` Vbic (1.3), `psp103va` Psp103, `bsimbulk` BsimBulk (C D47, F5; VD dev-spice); `type=±1` → `Polarity` (S 7.21); `model_<p>`/`instance_<p>` → the family's `<p>` (C F15); any other module → `ModelFamily::Module` + `Model.module` |
| instance | `name (nodes) master params`; OSDI and subckt terminals may be left unconnected (VD cir-instance); an unknown parameter is a hard error (S 7.11) | `Element` with the kind of its master; `Instance` for a subckt, `ModuleModel` for a user module |
| `$mfactor` | parallel copies; current branches ×m, a voltage branch's voltage unchanged and its reported current per copy (VD cir-mfactor); on a subckt instance only when the subckt declares `$mfactor` (RUN n2: "Parameter '$mfactor' not found") | on a device → `Element.mult` (`CircuitExtra::Mult*`); on a subckt instance → an ordinary parameter |
| R, C, L | `sp_resistor` takes `r temp dtemp l w tc tc2 tce bv_max scale noisy` (RUN t1 dump); `sp_capacitor` has no `ic` (`render/named/vacask_ic.rs` header, run); no `ac=` (S 6.17) | `Passive{value, temp_abs, dtemp, tc1, tc2, tce, scale, noisy}` |
| sources | `dc` is ignored on a waveform `type`: every analysis reads the waveform, the op its value at t = 0 (S 5.13, VS V5, V33 run); `mag` default 0, `phase` AC only (VD dev-builtin-src; VS V32 run) | `Source{dc: WaveAtZero (§5 G7), ac_mag, ac_phase_deg, wave}`; `type="dc"` → `dc` given, no wave |
| sine | `sinedc ampl freq delay theta tdphase`; before `delay` the value at `delay`; phase at `delay` (S 5.06, run) | `Sin{…, pre: HoldDelayValue}` |
| pulse | `val0 val1 delay rise fall width period`; defaults `rise=1n fall=0 width=0 period=0`; `rise` must be > 0 (RUN p2: "must be grater than 0"); `fall ≤ 0` is **no falling edge**: val1 to the end of the period, val0 again at its start (RUN p1: 1.0 at 3.5 µs, 0.497 at 4.5 µs); `period ≤ 0` one pulse (VD) | `Pulse{…, law: Vacask}` (§5 G3); every field filled with VACASK's default |
| exp | `val0 val1 delay tau1 td2 tau2`, `td2` counted from `delay` (S 5.08 run) | `Exp{td1: delay, td2, td2_from: Td1}` (§5 G4) |
| pwl | `wave=[t, v, …]`, holds first and last values; `period` wraps from 0 (binary extrapolates when x1 > 0, S 5.11, 14.10); `scale offset stretch tdphase break slope*` (VD) | `Pwl{points, delay, repeat: Period}`; `file=` → `Pwl.file`; any of `scale offset stretch tdphase` not at its default, or `break`/`slope*` given → `Private` (§5 G5) |
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
| literals | digits, `.`, exponent `e`/`E`, one scale `a f p n u m k K M x X meg G T mil`, then letters/`_` ignored after a scale only; `1k5`, `1e3k`, `1d3` are no numbers; `0x…` hex; `inf`, `nan` (S 1.01–1.07, 1.13; VD input-numbers; RUN f1: `1M`=1e6, `2x`=2e6, `1mil`=2.54e-5) | `Literal` as written (R1); hex → its value, `Plain`; `inf`/`nan` → `InvalidInSource` (the NaN gate, C9) |
| integer type | digits only (or hex) is an integer literal; a point, exponent or scale makes a real (RUN f1: `2./3` 0.667; RUN i1: `1/3` 0, `1k/3` 333.3) | `NumForm::Int` for an integer literal (§5 G1) |
| `/` | truncating when both operands are integers **at evaluation**: `-7/2` −3, `a/b` with `a=1 b=3` 0 (RUN i1); the type follows the actual value: `x/2` is 1 for `x=3`, 1.5 for `x=3.0` (RUN t3) | `BinOp::DivTyped` (§5 G1), always: the reader does not type (§6 does) |
| result types | int: `int()`, `sgn`, comparisons, `+ - *` and unary `-` of ints, `min`/`max` of ints, a ternary's chosen branch; real: `**`, `integer()`, `floor`, `ceil`, `round`, `abs` (RUN f2) | the typing table of `DivTyped`'s definition (§5 G1) |
| operators | `? :` lowest, right; `\|\| && \| ^ & == != < <= > >= << >> + - * /` left; `**` right; unary `- ! ~` bind tighter than `**`: `-2**2` = 4; `2**3**2` = 512; `^` bitwise XOR (`7^3` = 4) (VD expr-operators; RUN e1) | tree as grouped; `Pow`; `BitXor`, `BitAnd`, `BitOr`, `Shl`, `Shr`, `BitNot`; `Not`; no `%` |
| ternary | C grouping: `1?0?4:8:16` = 8 (RUN e1) | `Ternary` |
| functions | `log`=`ln` natural, `log10`, `sqrt`, `exp`, `pow`=`**` (signed: `(-2)**3` = −8, RUN e1), `abs`, `floor`, `ceil`, `round` half away (2.5→3, −2.5→−3), `integer` truncates, `int` truncates to an int, `fmod` dividend's sign (−1), trig, hyperbolic, `atan2`, `hypot`, 2-argument `min`/`max` (VD expr-functions; RUN e1, f2) | `Ln`, `Log10`, `Sqrt`, `Exp`, `Pow`, `Abs`, `Floor`, `Ceil`, `RoundHalfAway`, `Trunc`, `ToInt` (§5 G1), `Fmod`, …, `Atan2`, `Hypot`, `Min`, `Max` |
| `sgn`, `sign` | `sgn(0)` = 1 and `sgn(-0.0)` = 1: zero counts as positive (VD dev-builtin-behavioral; RUN e1); `sign(x, y)` = \|x\| with `sgn(y)`'s sign (`sign(3,-0.0)` = 3, RUN f1) | sugar (R13): `sgn(x)` → `Ternary(Ge(x, 0), 1, -1)` (Int literals); `sign(x, y)` → `Mul(Abs(x), Ternary(Ge(y,0),1,-1))` |
| constants | `M_*`, `P_*` (C C28; VD expr-constants) | the literal of the value VACASK documents (S 2.22), `Plain` |
| special | `$temp`, `$tnom`, `$scale` in every expression; `$abstime` in behavioural sources only (VD expr-special, dev-builtin-behavioral) | `Temp`, `Tnom`, `Time`; `$scale` → `Private` statement (§5 G15) |
| vectors | `[a, b]`, `{…}` lists, `x[i]`, `vector() range() readfile() randunif()` (VD expr-vectors) | the statement → `Private` (§5 G13) |
| draws | `agauss(nom, avar, σ)`, `gauss(nom, rvar, σ)`, `aunif`, `unif`; the nominal outside an `mc` loop; one sample per call site (VD expr-functions; RUN g2: `r=agauss(5,1,1)` is 5; S 12.01, 12.03) | `Draw(dist, Sampling, CallSite, args)` |
| context | `fmod`, `round` and the vector functions are refused in a behavioural source (VD dev-builtin-behavioral "Functions"; S 2.17 run) | `FuncDyn` features absent from `FEATURES` |

### 3.4 The control block (run items, in order: VD cmd-overview, S 8.04)

| Statement | VACASK's meaning (evidence) | IR |
|---|---|---|
| `options k=v` | accumulate, applied before the next analysis (VD cmd-overview) | `reltol abstol vntol chgtol gmin gshunt scale` → `OptionRow` portable keys; `temp`/`tnom` → `Env`; `tran_method` trap/euler/gear2/bdf2/am/gear + `tran_maxord` → `MethodTrap\|MethodEuler\|MethodGear` + `MaxOrd`; `op_itl` Itl1, `tran_itl` Itl4, `homotopy_gminsteps` GminSteps, `homotopy_srcsteps` SrcSteps, `op_skipinitial=1` NoOpIter (C I5; `syntax/dialects/vacask.rs:181-237`); `rawfile` and output-only keys → `Inert`; every other (`tran_lteratio`, `tran_rmax`, `tolscale`, `nr_*`, `homotopy_*`, `smsig_*`, `solver`, `omp`, …) → `OptionKey::Private` (§10 Q4) |
| `analysis n op` | (VD cmd-analysis-op) | `Analysis{Op, name}` |
| `sweep` + `analysis n op` | one raw file over the grid (VD cmd-sweep) | `Dc{axes}`: the last `sweep` line is the innermost axis (VD "Nested sweeps"); targets `instance=… parameter="dc"` SourceDc, principal parameter ElementValue, other DeviceParam, `model=` ModelParam, `option="temp"` Temp (S 4.04), other option Option, `variable=`/`__topinst__` Param |
| `sweep` + any other analysis | re-runs that one analysis only; the swept value is **restored** afterwards (RUN s1: the op after sweeps over 2 and 3 reads 1; VD says "left at its last value": the run wins); `points` counts intervals (VD; S 8.06) | `Loop{Sweep, after: Restore}` whose body is the analysis; `lin points=n` → `n+1` values (spelling); bounds may read an outer swept variable (S 8.22) |
| `continuation=` | 1 is the default and only seeds Newton (VD; VS V13) | 1: nothing; 0: `Private` (COVERAGE G-CONTINUATION) |
| `ac` | `from to mode points` or `values=[…]` (VD cmd-analysis-ac) | `Ac{sweep}` (`List` for `values`) |
| `tran` | `stop`, `step` = **initial** step, `start` = start of recording, `maxstep`; `icmode="op"\|"uic"`; `ic=`/`nodeset=` lists or a slot name (VD cmd-analysis-tran); with no `maxstep` the step is capped at (stop − start)/50 (RUN tr1: 2e-4 for 10 ms, 8e-5 with start 6 ms) | `Tran{stop, record_start, initial_step, max_step, cap: SpanOver50, uic, print_step: NONE}` |
| `trannoise` | tran with device noise (VD cmd-analysis-trannoise; S 8.20) | `Tran` + `noise_fmax/fmin/seed` (the reader fills `fmin = fmax/1e3`) |
| `noise` | `out=["n"]` or a pair, `in="src"` (VD; VS V21; S 15.04 V²/Hz) | `Noise{output, source, sweep}` |
| `dcinc`, `dcxf`, `acxf`, `acstb`, `acsp`, `hb`, `pss`, `pac`, `pnoise` | VD cmd-analysis-*; `acstb` gives +T (S 8.18); `hb` `immax` 0 is box in the binary (S 8.15, 14.10) | `DcInc`, `DcXf`, `Xf`, `Stb`, `Sp{ports}`, `Hb{tones, harmonics, intermod, truncate}`, `Pss`, `Pac`, `Pnoise` |
| `hbac`, `hbnoise` | (VD) | `Private` (COVERAGE-run) |
| `ic=`, `nodeset=` | lists of node, value; two nodes then a value is a difference (VD cmd-analysis-op; C G63); hierarchical nodes quoted `'x1:n'`; on that analysis only | `NodeSet{kind, node, reference, value, next_only: true}`; a slot name, `store=`, `opsolve=0`, `writeop` → `Private` (G-SLOTS) |
| `save` | accumulate until `clear saves`; `default`, `full`, `v()`, `i(inst)`, `p(inst, var)` (VD cmd-save) | `Save` in order: `default` AllV + AllSourceI, `full` AllV, `V`, `I`, `Param` (quantity as spelled, G-OPQUANT); `yin zin yout zout opdefault opfull` → `Private` (G-PORTOUT) |
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
- spells `Load{Library, family}` as the library file of the family (`tables.rs`), and each
  `Model` with its module word; never invents a load or a model (§6 does);
- spells `Dc{axes}` as `sweep` lines (innermost last) + `analysis … op`; a `Loop{Sweep}` as its
  `sweep` lines repeated before every analysis of its body (a sweep binds one analysis, VD
  cmd-sweep); `Loop{MonteCarlo}` as `mc … endmc`; `NodeSet` lists on the analyses they apply to;
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
`tests/fixtures/dialect/vacask/citations.toml`.

## 5. IR gaps

R8: a gap gets its typed row in the slice of the second dialect that has the meaning. G1–G4 and
G6–G11 are typed here because ngspice reads or writes the meaning through a §6 pass in this
slice (Q3 asks the lead to confirm that reading of R8).

| Gap | VACASK meaning (evidence) | Proposal | Owner | Now? |
|---|---|---|---|---|
| G1 typed arithmetic | integer literals, truncating `/` on integer values, result types per operator and function (§3.3; RUN i1, t3, f2) | `NumForm::Int`; `BinOp::DivTyped` (C's division on the operands' types); `Func::ToInt` (trunc, integer-typed); the reference evaluator carries an int/real tag per value; the typing table in `DivTyped`'s doc | kernel | now (Q1) |
| G2 device library | a device of a family is the simulator's own build of it, named by a `load` of a shipped file and a `model` (VD cir-loading, dev-spice; RUN n2) | `LoadKind::Library` with `family: ModelFamily` and `path` (`NONE` = the target's default file for that family; `spice/sn/…` kept as written); a design feature "a model of family F with no `Library` load of F" and "an element of a module-only kind with no model" for the passes of §6 | kernel (`Load`), circuit (the features) | now (Q2) |
| G3 pulse law | `fall ≤ 0` no falling edge, `period ≤ 0` one pulse, `rise > 0` (RUN p1, p2) | `Pulse.law: PulseLaw::{Spice, Vacask}`; features `CircuitExtra::PulseSpice`, `PulseVacask` | circuit | now |
| G4 EXP `td2` | counted from `delay` (S 5.08). IR-CIRCUIT §3 says "its reader adds": that puts `delay` in two fields, which R13(b) forbids | `Exp.td2_from: TdFrom::{Zero, Td1}` + features | circuit | now (amends IR-CIRCUIT §3) |
| G5 PWL shaping | `scale offset stretch tdphase` (VD; S 5.33 says fold at read: that repeats an operand in every point, R13(b)) | `Pwl.{scale, offset, stretch, phase_deg}`; Spectre has the first three (S 5.33) | circuit | Spectre slice; `Private` now (0 `type="pwl"` decks in the corpus) |
| G6 SFFM pre-delay | VACASK holds the value at `delay` before it; ngspice 0 (S 5.02; VD dev-builtin-src `delay`) | `Sffm.pre: PreDelay`, as `Sin`/`Am`; `SffmLaw::Ngspice` keeps only the clamp and the defaults | circuit | now |
| G7 DC from the waveform | a waveform source's DC is its value at t = 0 (S 5.13; VS V33); ngspice's rule for SIN/PULSE/EXP/PWL with no DC is the same (DIALECT-NGSPICE §3) | `Source.dc_rule: DcRule::{Given, WaveAtZero}` (`dc` `NONE` under `WaveAtZero`); a structural "is `dc` the wave at 0" test is fragile, and the VACASK writer must know it to avoid the `alter type="dc"` pair | circuit | now (Q12) |
| G8 elaborate | promoted subckts form the top for the run items after it (VD cir-elaboration; VS V1, V2) | COVERAGE-run G-ELABORATE: run item `Elaborate{circuits: RefList}` | run | now (49 corpus files) |
| G9 enclosing scope | ngspice binds a nested subckt's reference to an enclosing subckt's parameter or model; VACASK never does (RUN n1, m1) | `Extra::EnclosingRef` (a `Ref` bound in an enclosing, non-top subckt scope), set by `features_used` | kernel | now |
| G10 dependent formal | a formal default that reads a parameter (ngspice `params: y={x*2}`); VACASK cannot override it (RUN c1, t2) | `Extra::FormalReadsParam` | kernel | now |
| G11 loop over op | VACASK spells a `Dc` as `sweep` + `op`, so a `Loop{Sweep}` whose body is one `Op` has no other spelling | `RunExtra::LoopOverOp` | run | now |
| G12 solver hints | VACASK-only options that tune convergence or accuracy only (`tran_lteratio`, `tran_rmax`, `nr_*`, `homotopy_*`, `tolscale`, …): the old writer dropped them with W0024 (VS V14) | `OptionRow.hint: bool`, dropped with a note by `legalize` like `Inert` (R11) | run | Q4 |
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

| Feature | Pass | IR → IR (one sentence) | Requires | Proof | Old rule it replaces |
|---|---|---|---|---|---|
| `Binary(Div)` | `div_to_div_typed` | `Div(a,b)` → `DivTyped(a,b)`, with `a` wrapped as `Mul(1., a)` unless an operand is real by the typing table | `Binary(DivTyped)` | eval (typed) | none: `respell` copied `/`, so ngspice `{1/3}` was 0 on VACASK (RUN i1): ledger |
| `Binary(IntDiv)` | `intdiv_to_trunc` | `IntDiv(a,b)` → `Trunc(Div(a,b))` | `Func(Trunc)`, `Binary(Div)` | eval | none |
| `Func(PowAbs)` | `powabs_to_pow` | `PowAbs(x,y)` → `Pow(Abs(x), y)` | `Func(Pow)`, `Func(Abs)` | eval | `abs_power_out` |
| `Func(RoundHalfEven)` | `round_half_even_to_away` | `nint` → a ternary on `x − floor(x) == ½` and the parity of `floor(x)` around `RoundHalfAway` | `Floor`, `Fmod`, `RoundHalfAway`, `Ternary` | eval | none (old copied `nint`, which VACASK lacks) |
| `Func(Sgn)` | `sgn_to_ternary` | `Sgn(x)` → `x>0 ? 1 : (x<0 ? -1 : 0)` | `Ternary`, `Gt`, `Lt` | eval | none: `sgn(0)` 0 → 1 (RUN e1): ledger |
| `Func(Step)`, `Ramp`, `Pwl` | `step_to_ternary`, `pwl_to_ternary` | each by its definition as ternaries of its argument | `Ternary` | eval | none |
| `DrawWhen(Always)` | refused | a fresh sample at every reference in a plain run has no VACASK form (VACASK draws only inside `mc`) | — | — | `VacaskCalls` wrote the call, so VACASK ran the nominal: ledger (Q5) |
| `Expr(TStep)`, `Expr(TStop)` | `substitute_tran_leaves` | each leaf → the design's one transient's step or stop; two transients → refused | one `Tran` | sim(ngspice) | none: a given-zero `TF` was written `fall=0`, which VACASK holds (RUN p1): ledger |
| `PulseSpice` | `pulse_spice_to_vacask` | relabel to `Vacask` when `rise > 0`, `fall > 0` and `period` is 0 or > rise+fall+width; else refused | `PulseVacask` | sim(VACASK) after `substitute_tran_leaves` | `vacask_wave` Pulse (it wrote period 0 when period ≤ rise+fall+width: Q13) |
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
| `Elem(Port)` | `port_to_source_r` | EMF source + series `z0` resistor + an internal node | `Elem(VSource)`, `Elem(Resistor)` | sim(ngspice) | RfPort arm (the `acsp` port convention is to settle: Q10) |
| `Elem(Laplace)`, switches, lossy lines, code models | refused | VACASK has none (C D23, D29–D35) | — | — | W0011, W0105 |
| `IcUicOnly` | `device_ic_to_nodeset` | each `Passive.ic` → a differential `NodeSet{Ic}` on every transient with `uic`; dropped where no transient has `uic` (ngspice ignores it, S 8.21) | `IcDifferential`, `IcNextOnly` | sim(ngspice) | `vacask_ic::held_ics` |
| `PassiveAc`, `SemiInitial` | refused | `ac=` is "not found" (S 6.17); VACASK has no `off` (S 6.28) | — | — | refusal; `off` (Q11) |
| `MultInstance` | `instance_mult_to_param` | the subckt gets a formal `mult__` (default 1), each element in it `mult × mult__`, the instance passes `mult__ = m` | `MultSemi`, `MultResistor`, … | sim(ngspice) | — (VACASK refuses an undeclared `$mfactor`, RUN n2) |
| `EnclosingRef` | `hoist_nested_subckts` | a nested subckt that reads an enclosing scope's parameter or model → hoisted to the top, the read parameters as formals passed by each instance, the models moved with it, names that collide renamed | `NestedSubckt` off | sim(ngspice) | `read/blocks.rs` hoist |
| `FormalReadsParam` | `formal_defaults_to_instances` | every instance passes every such formal, its default rewritten over the instance's actuals; the formal's own default becomes a constant | — | sim(ngspice) | none |
| `Item(Func)`, `LocalFunc` | `inline_funcs` | each call → the body with its arguments substituted (ngspice's own textual rule, S 2.26) | — | eval | W0106 refusal (now translates) |
| element with no model (G2) | `explicit_family_models` | one parameterless `Model` per (scope, family) for model-less elements, named `<family>` (renamed on collision) | — | sim(ngspice) | `Need` in `named/vacask.rs` |
| model with no `Library` load (G2) | `library_loads` | one `Load{Library, family, path: NONE}` per family in use, at the top | `Item(Load)` | sim(VACASK) | implied loads in `named/vacask.rs` |
| `TranCapNgspice`, `print_step` | `tran_explicit_cap` | `max_step := min(print_step, (stop − record_start)/50)` when `NONE`; `initial_step := print_step`; `print_step := NONE` | `TranMaxStep`, `TranInitialStep` | sim(VACASK) | `vacask_analysis` Tran (`step=` tstep, `maxstep`) |
| `Analysis(Dc)` with a target VACASK sweeps | — (writer) | `sweep` + `op` | — | sim | Dc/DcAxes arms |
| `Analysis(Tf)` | `tf_to_dcxf` | `Tf{output, source}` → `DcXf{output}` (the source's entry is the backend's) | `Analysis(DcXf)` | sim(VACASK) | Tf arm (`dcxf out=`) |
| `Sens`, `SensAc`, `Pz`, `Disto`, `Four`, `Measure`, `TimedOp` | refused | VACASK has none (C G10; `.four`/`.meas` evaluated by the backend, D7) | — | — | W0015, W0109 |
| `LoopKeepLast` | `loop_keep_last_to_alter` | append `Alter{target, last value}` after the loop | `Item(Alter)` | sim(VACASK) | none (ngspice `foreach` keeps the last value, S 13.03) |
| `Option(Rshunt)` | `rshunt_to_gshunt` | `gshunt = 1/rshunt` | `Option(Gshunt)` | sim(both, S 10.05: 3/7 V) | `option_spell("rshunt")` |
| ngspice-private and unmapped options | refused, or dropped when `Inert`/hint (G12) | — | — | — | W0017, W0024 |
| `StatisticsDraws` (Spectre block) | not this pair | — | — | — | — |
| `TranStartup` | `startup_ramp` (planned) | — | — | — | StartupRamp |

### 6.2 VACASK → ngspice

| Feature | Pass | IR → IR | Requires | Proof | Old rule |
|---|---|---|---|---|---|
| `Binary(DivTyped)` | `div_typed_to_div` | by the typing table over the whole design (every instance's actuals): both real → `Div`, both int → `IntDiv` (in a dynamic expression `Trunc(Div)`), mixed across instances → refused | `Binary(Div)`, `Binary(IntDiv)` | eval (typed) | none (old copied `/`) |
| `Func(ToInt)` | `toint_to_trunc` | `Trunc` once no `DivTyped` reads its type | — | eval | none |
| `Func(Pow)` | `pow_to_powabs` | whole literal exponent: even `PowAbs`, odd `Mul(Sgn(x), PowAbs)`; other literal: `PowAbs` (equal on `Pow`'s domain, R12); other exponent with a base not provably ≥ 0: refused | `Func(PowAbs)`, `Func(Sgn)` | eval | `abs_power_in` |
| bit operators | `fold_constant_bitops` | an integer-constant subtree → its literal; else refused | — | eval | `caret_reread` W0002 |
| `Func(RoundHalfAway)` | `round_half_away_to_floor` | sign-aware `floor` + a `x − floor(x) ≥ ½` ternary (never `floor(x+½)`, wrong at ½−ulp) | `Floor`, `Ternary` | eval | none |
| `Func(Atan2)`, `Func(Hypot)` | `atan2_to_atan`, `hypot_to_sqrt` | quadrant ternary; `sqrt(x²+y²)` (≤ 2 ulp, measured, not exact) | `Atan`, `Sqrt` | eval | old rewrite (VS §2) |
| `FuncDyn(Idt)` | `idt_to_integrator` | a `Behav` current into a new node with a 1 F capacitor and an `Ic` of the initial value | `Elem(Capacitor)`, `IcEveryOp` | sim(VACASK) both forms (S 2.18: 9e-9 on ngspice) | none |
| `Expr(Tnom)` | `tnom_to_env` | → the design's one `Env.tnom` | one `Env` | eval | none |
| `DrawWhen(Sampling)` outside every `MonteCarlo` | `sampling_draws_to_nominal` | `Draw` → its nominal argument | — | sim(VACASK) (RUN g2) | `stat_nominal` analogue |
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
| `Sp` | — | ngspice `.sp` when the ports are every `portnum` in order, else refused (VS §2) | — | sim | — |
| `Load{Library}` | `drop_library_loads` | removed: ngspice has the family built in | — | sim(both) | `own_loads`, A19 |
| `Model.module` of a user OSDI | — (writer) | ngspice `pre_osdi` + `N` card (C A18, B24) | — | sim | OsdiLoad |
| `Option(Private)` of VACASK, hints (G12) | refused or dropped (Q4) | — | — | — | W0024 note |
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
| R2001 | refusal | a parameter defined twice in one scope | RUN n3 |
| R2002 | note | an instance whose master no read file defines (`Unresolved`) | VS V29 |
| R2003 | refusal | a name only an enclosing subckt defines | RUN n1, m1 |
| R2004 | refusal | an instance overrides a dependent parameter | RUN c1, t2 |
| R2005 | refusal | `$mfactor` on a subckt that does not declare it | RUN n2 |
| W2001 | note | an analysis given a generated name | §4 |

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

## 10. Open questions for the lead

1. **VACASK's arithmetic is typed, and the IR's is not** (RUN i1, t3, f2). `1/3` is 0, and
   whether `x/2` truncates depends on the type of the value an instance passes. No field of
   today's IR can state it; the reader cannot type statically (types follow instance actuals,
   possibly in other files). Proposal G1: `NumForm::Int`, `BinOp::DivTyped`, `Func::ToInt`, a
   typed reference evaluator; the passes do the typing over the whole design; the VACASK writer
   spells whole `Plain` literals as `2.`. The alternative (reader types, refuses what it cannot)
   loses parameterised subckts. Either way the old pipeline gives wrong numbers today for any
   ngspice deck with a division of whole numbers. Needs the kernel owner's yes.
2. **"FEATURES is the reader's image" and "passes know no dialect" do not cover VACASK's device
   library.** Every device needs `load "spice/x.osdi"` and `model m sp_x` that only name the
   simulator's own family; a writer that invents them breaks L5, a pass that adds them must know
   VACASK's file names. Proposal G2: `LoadKind::Library{family}` with path `NONE` meaning "the
   target's default file", so `explicit_family_models` and `library_loads` stay neutral and the
   reader and writer stay inverse. The `spice/sn/` and `spice/full/` variants keep their path
   (they change noise and output variables, VD dev-spice) and are refused off VACASK. Is this
   the shape you want?
3. **R8 read narrowly blocks this slice.** G3 (pulse law), G7 (DC from the waveform), G8
   (`elaborate`) are VACASK's own meanings with no second reader; kept `Opaque` they would refuse
   43 pulse decks and 49 elaborate decks the old pipeline writes to ngspice. I read R8 as
   satisfied when ngspice writes the meaning through a §6 pass. Confirm, or rule otherwise.
4. **Solver-only options.** The old writer dropped VACASK-only options with a note (W0024, VS
   V14); with `OptionKey::Private` they become refusals off VACASK (a regression on corpus decks
   with `tran_lteratio`, `nr_*`, `tolscale`). Proposal G12 (`hint` flag, dropped like `Inert`).
   `tolscale` scales tolerances: hint or `Private`?
5. **ngspice draws in a plain run.** ngspice samples at every reference; VACASK only inside `mc`.
   Refusing is right by the contract; the old pipeline wrote the call and VACASK ran the nominal.
   Refuse, or a note-carrying `draws_to_nominal`?
6. **Per-module parameter names.** VACASK hard-errors on a parameter its module lacks (S 7.11),
   so the writer must refuse it, which L4 forbids for `Gen(FEATURES)` unless `Gen` draws
   parameter names from each family's schema. A family schema table (S 7.11's proposal) is a
   circuit-owner table: now, or a writer refusal row (W2xxx) tolerated by L4?
7. **IR-CIRCUIT §3 contradicts R13**: "VACASK counts `td2` from `td1`: its reader adds" puts
   `delay` in two fields (R13 b). G4 replaces it with a mode field.
8. **The docs contradict the binary twice where it matters**: a sweep restores the swept value
   (RUN s1; VD cmd-sweep says "left at its last value"); `hb immax` and the PWL wrap (S 14.10).
   The run wins (R5); the settle fixtures pin them.
9. **One meaning, two nodes?** VACASK `sweep` + `op` writes one raw file over the grid, which is
   `Dc`; the same over another analysis is `Loop`. The reader reads the first as `Dc` and G11
   makes `Loop`-around-`Op` a feature VACASK lacks. The run owner should say whether `Dc{axes}`
   and `Loop{Sweep}{Op}` are one meaning (then one node, and ngspice's `.dc` vs `foreach` is
   spelling).
10. **Unsettled, each with the run that settles it**: the `acsp` port convention (`port_to_source_r`
    needs it: run `acsp` on a 50 Ω vsource-plus-resistor port against ngspice `.sp`); whether
    `tran_lteratio` 3.5 equals ngspice's `trtol` 7 (`cmp.rs` assumes VACASK shares TRTOL; run a
    stiff RC at both settings); the result type of `min`/`max` of an int and a real and of
    `atan2` (RUN f2 covered the rest).
11. **`off` and R13's "no Opaque in an expression"**: `Semi.initial` only seeds Newton; refuse it
    (as written here) or drop with a note? And `white_noise`/`$scale` (0 corpus uses) need kernel
    members under R13 or a statement-level `Private` exception.
12. **Coordination with S3**: G7 asks the ngspice reader to state "no DC given" as
    `DcRule::WaveAtZero` instead of an expression of the wave's fields.
13. **Old pulse clamp**: the old writer wrote `period=0` (one pulse) when period ≤
    rise+fall+width; SPICE repeats seamlessly at equality. Run `PULSE(0 1 0 1u 1u 2u 4u)` on
    ngspice against the old VACASK text to confirm it was a wrong number; here it is refused.
14. **The backend reads the written deck** (`src/backend/vacask.rs`: analysis names, outer sweeps,
    relative paths) until S12, which constrains the writer's analysis names and sweep spelling.

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
