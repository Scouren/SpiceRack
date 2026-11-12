# IR decisions (kernel, Definition phase)

Where `IR-KERNEL.md` is silent or two statements conflict: the question, the choice, why.

- Table fields "private" (C13) vs owners adding methods in their own files? → `pub(crate)` fields on `Design`, `Builder`, `Editor`; private outside the crate. Sibling modules cannot see private fields, and owners must not edit kernel files.
- Where do the circuit and run detail tables live? → `CircuitDetails` (`element.rs`) and `RunDetails` (`analysis.rs`), held by `Design` as two fields; plain `Vec`s without spans (a detail row's span is its element's). Owners then add tables without editing `design.rs`.
- "One typed accessor per kernel item kind"? → one generic door: trait `ItemRow` (implemented in `design.rs` for all 23 rows) gives `Design::row::<R>`, `Builder::push`, `Editor::set`. 23 kinds × 3 near-identical methods otherwise; the compiler still lists every kind.
- An expression row `{tag, a, b}` cannot hold an operator and two children → a fourth column `sub: u8` (the `Func`, `BinOp`, `UnOp` or `Dist`), a `Ternary`'s then/else as a pair in the expression pool, and one side table for `UserCall` (a `Ref` and a list). Encoding table in `expr.rs`'s header.
- `**` vs `pow()`, `%` vs `fmod()`? → one `Func` node each (`Pow`/`PowAbs`/`PowRe`, `Fmod`); `BinOp` has no power or modulo. One node per meaning (§2).
- The item row `Func` and the function enum `Func` share a name (both requested) → both kept in their modules; the crate root re-exports `expr::Func`, and the row is `item::Func`.
- Builder `param` was requested both as an expression constructor and as an item push → the item push is `param`, the expression is `param_ref`.
- `Feature` lists no expression tags, yet the A1 leaves (`TStep`, `TStop`, `Tnom`), `UserCall`, `Str` and `List` need a key for passes and refusals → added `Feature::Expr(ExprTag)` ("every kind is a feature", §8).
- `Include`/`Load` target: `FileId` or path? → the path (`NameId`) of the design file it reads; `Design::find_file` finds the target. This keeps IR text lossless without forward file ids, and "missing" is simply no file with that path.
- Who defines? → pushes of defining items (`subckt` with its head params, `section`, `param`, `func`, the owners' `model`) define automatically; `Builder::define` is for modules and anything else. A reader keeps one winner per duplicate before it pushes (§3.3, the duplicate `.param` row), so `define` asserts uniqueness.
- How does a function body read its formal arguments? → as `Param` expressions bound to the function's own `DefId`; there is no separate formal-argument definition.
- `If` branches must be contiguous in the branch pool → `Builder::if_` takes every condition at once (readers hold the whole CST chain) and returns the `BranchList`; `NONE` as the last condition is `else`.
- Name lookup map → `HashMap<u64 hash, NameId>` plus a per-name collision chain (`next`): 4 bytes per name, no second copy of the bytes. Definitions → `HashMap<(ScopeId, DefKind, NameId), DefId>`. Both are lookup only, never iterated.
- `ScopeKind` → `File, Subckt, Section, Branch, Loop` (`Loop` because the run's `Loop` opens a scope, §5).
- `Role` → `Deck, Library, VerilogA, Missing` (INV A2's "Unread" renamed so it does not collide with `OpaqueClass::Unread`).
- Owners appending to `Extra` would both edit the length of `Extra::ALL` → `ALL` is a slice (`&[Extra]`), so `COUNT = ALL.len()`; regions are marked in both the enum and `ALL`.
- IR text syntax errors fit none of `X`/`U`/`S` → `diag::TEXT_SYNTAX` = `P9001`, with `P9001..=P9099` reserved for the IR text reader.
- Resolved references in IR text → bare `name` (bound by `Design::lookup` from the item's scope), `name?` unresolved, `name#k` an explicit definition (only when lookup would bind elsewhere). This is lossless and still writable by hand.
- `Dist` → `AbsGauss, RelGauss, AbsUnif, RelUnif, Limit`. There is no log-normal: its argument meaning is not established for the SPICE draw idiom; the run owner's `Vary` may need one (see the report).
- `Min`/`Max` take exactly 2 arguments (a reader nests); unary plus is no node; bitwise operators require integer-valued operands (else a domain error).
- `ElemKind`, `WaveKind`, `AnalysisKind` → one `Placeholder` variant each; owner rows that the kernel does not read are empty structs (size 0).
- `clippy::ptr_arg` on `text::dump(&mut String)` is a definition-phase false positive (the body is `todo!()`) → allowed on that function only, with a comment.
- `todo!()` cannot satisfy an `impl Iterator` return → `Design::files` and `FeatureSet::iter` have their one-line real bodies; `Code::new` also has a body, because `UNSUPPORTED` is a `const`.
- `gen` is a reserved word in edition 2024 → `pub mod r#gen;` (file `gen.rs`), placed last in `lib.rs` because `check_style.py` does not parse the raw identifier as a module.
- Capacities (C10) → `NAMES_MAX` 2²⁶, `EXPRS_MAX` 2²⁸, `ROWS_MAX` 2²⁷ per table, `SCOPES_MAX` 2²⁴, `DEFS_MAX` 2²⁶, `POOL_MAX` 2²⁸, `BYTES_MAX` 2³¹, `FILES_MAX` 4096. That is about 10× the largest kit (10⁶–10⁷ statements, §3.2).

## Addendum A2 (row facts, draws)

- 23 `todo!()` impls of a 6-method trait in the definition phase → one crate-internal macro, `todo_facts!(Row);` (in `facts.rs`, `#[macro_use]`, declared first in `lib.rs`), one line per row in the row's own file. The implementation phase replaces each line with the row's `impl RowFacts`. `item.rs` stays under 500 lines, so there is no `item_facts.rs`.
- Every item row must have its facts → a compile-time check in `design.rs`'s `item_rows!` (`has_facts::<Row>`), so a kind without `RowFacts` fails to build.
- `Handle` adds `Branches(BranchList)` (an `If` holds one).
- Row references in IR text cannot be bound after the whole text: `RowFacts` has no way to rewrite a row's `Ref` → a bare `name` binds by `Design::lookup` among the definitions pushed so far, and `dump` writes `name#k` for a reference to a later definition. Expression references (parameter reads, user calls) are still bound after the whole text, through `Builder::resolve`.
- `Words` is a cursor over one statement line: `next`, `peek`, `at_end`, `expect`, `key` (for an optional `key=`), `name`, `names`, `number`, `string` (`Cow`: borrowed unless escaped), `reference`, `value` (atom or parenthesized expression), `expr`, `exprs`, `params`, `error`, `span`. A failure reports `TEXT_SYNTAX` and returns its failure value. `next` allows `clippy::should_implement_trait` (it is not an `Iterator`).
- `Gen` holds the PRNG state, the `Size`, items left and a fresh-name counter. Methods: `below`, `chance`, `number`, `fresh_name`, `expr` (with `Reads::{Constant, Circuit}`, the field's context), `pick_def`, `budget`.
- `Draw`'s `sub` byte: bits 0–3 `Dist`, bit 4 `DrawWhen`, bits 5–6 `DrawPer` (const-asserted to fit). `Bindings::draw` also receives `when` and `per`.
- `Feature::DrawWhen` and `Feature::DrawPer` come after `Dist` in index order.
- `clippy::ptr_arg` is allowed on the placeholder `RowFacts::dump` impls (the definition-phase false positive, as for `text::dump`).

## Addendum A3 (decimal literals)

- Packing → `Num`'s `a` is the `NumId`, `b` is 0. Literals live in `Exprs::nums`, one per `Num` row, so the literal table shares `EXPRS_MAX`. `Literal` is 16 bytes (`i64`, `u8`, `i16`, `u8` form; size-asserted).
- Which `exp10` a `Scale` may have → a power some scale letter states: −18, −15, −12, −9, −6, −3, −2, 3, 6, 9, 12, 15 (Spectre `%`/`c` is −2, `P` is 15). `Mil` has `exp10 == 0`; `frac <= 18`. Checked by `Literal::is_valid`, `Builder::literal` (assert) and `check` (`Violation::LiteralShape`, new, the 14th). `NonFinite` now means a literal whose value overflows.
- IR text scale letters, one spelling each → `a f p n u m c k meg g t peta` and `mil`. A negative literal is `-1.5`; the `Neg` of a literal is `-(1.5)` (two trees, so two spellings).
- Added `Literal::ZERO`, `Literal::eq_value` (exact decimal comparison, which `Design::eq_meaning` uses) and `Literal::is_valid`. `Gen::number -> f64` became `Gen::literal -> Literal`, so the generator exercises every form; `Words::number` returns `Option<Literal>`.

## Addendum A4 (the linker)

- Supersedes the A2 line on row references in IR text: `link` (`link.rs`) runs after the whole text, so bare forward references work; `Words::reference` is now `(b, diags) -> Ref` with no kind and no scope (the linker knows both through `map_refs`), and `Words` lost its `pending` list.
- `name?` must survive a `link` that binds every unresolved `Ref` → a crate-private second sentinel, `DefId::HELD` (`u32::MAX − 1`). `Words` writes it, `link` skips it, and `text::parse` turns it into `NONE` before it returns, so it never reaches a finished design (`check` reports it as `HandleRange`).
- `link` reports each reference it leaves unresolved as a note, `diag::LINK_UNBOUND` = `R9001` (range `R9001..=R9099` for the linker). Unresolved stays legal.
- `map_refs` gives one `DefKind` per `Ref`, but an instance's master may be a `Subckt`, `Model` or `Module` → for now the circuit owner passes the kind its row states; reported to the lead (see the report).
- Follow-up from the lead's A4 brief, which supersedes the line above on `LINK_UNBOUND`: `link` appends no diagnostic (unresolved is legal), so `R9001` is withdrawn before any use; `link` keeps `&mut Diags` for the spine's uniform signature.
- `Editor` gains `map_refs(item, f)` (the exhaustive dispatch to `RowFacts::map_refs` that also stores the row back); `link` and renumbering passes use it, and use `Builder::resolve` for expression references.
- The pinned-unresolved mark for `name?` is the `DefId::HELD` sentinel (the line above), not a set passed to `link`: it travels inside the row, so `link` needs no side input.


## Circuit

- Kinds the kernel list lacks → added `FluxInductor` (lt `Flux=`, 6.16), `FreqTable` (5.28), `Delay` (5.30), `Igbt` (D103), and two module kinds: `ModuleModel` (ng `N`, va: master a `Model`) and `Module` (sp, hs: master a loaded module). One `DefKind` per `Ref` (A4) then holds for every kind; 37 kinds.
- `master_kind` for a kind with no master and for `Unresolved` → `Option<DefKind>` (`None`), a function return, not a row field (C2 is about rows). `map_refs` skips a `NONE` name and every `None` kind.
- Constant facts (`terminals`, `detail`, `needs_model`, `master_kind`) and `map_refs` → real bodies in the definition phase: the tables are the definition.
- Detail tables → a `DetailKind` per element kind and one generic door, trait `DetailRow` (`Design::detail`, `Builder::detail`, `Editor::set_detail`), as the kernel's `ItemRow`; typed accessors (`design.passive(&e)`) assert the kind on top. 25 row types otherwise need 75 near-identical methods.
- Cold optional data → side rows indexed by `u32` (`NO_ROW` = none): `Parasitics`, `PortSpec`, `Distof`. A port is a `Source` with a `PortSpec`; the slots hold the EMF.
- `Source.mult` under 5.16's "fold at read" vs IR-CIRCUIT's "`mult` on every kind" → IR-CIRCUIT: `mult` stays; folding is a pass. Xyce's reader leaves it `NONE` (Xyce ignores `M` on V/I).
- POLY (5.26 says expand at read) vs IR-CIRCUIT ("stays `Poly`") → IR-CIRCUIT; the coefficient order is the SPICE polynomial's definition, which sp `pvcvs` shares, not one dialect's order.
- The `given` bit of 3.09/5.05 → not added: a given zero is the literal 0, ngspice's default is the `TStep`/`TStop` leaf (A1), and every other default is written by the reader.
- A capacitor or inductor whose value reads the circuit → the value is the incremental c(v) / L(i) law; sp `ctype=1` is `OutKind::CapacitanceCharge`, q(v) is `ChargeCapacitor`. One field never carries two laws.
- `IcMode` → `None | UicOnly | InOp`, `None` exactly when there is no `ic`, so no field means "mode of nothing".
- PWL repeat (spec: `repeat_from`, `repeat_join`) → `PwlRepeat::{Never, From, Period}` + `repeat_at` + `repeat_count` + `PwlJoin::{Jump, Ramp, RampWidth, Extrapolate}` + `join_width`: va and sp repeat by period, not from a point, and lt counts passes; `repeat_from` alone cannot state them.
- PWL from a file (5.12 wants inline points) → `Pwl.file`: readers do no I/O; inlining is a pass.
- AM (5.03): sp's modulation time is shifted by the carrier phase → `AmModTime`, so the sp reader does not rewrite the phase.
- Laplace → `LaplaceForm::{Polynomial, PoleZero}` (hs `POLE`, sp poles/zeros) and an `input` expression (lt `B … Laplace=`); sp's DC-gain normalisation is undone by its reader, as IR-CIRCUIT §2 says.
- Switch law → the model family (`SwitchHyst`, `SwitchHystSmooth` (lt negative vh), `SwitchLogCubic`, `Relay`, current twins), never a row field; xy `CONTROL=` → `Switch.control`.
- `Semi` exists for every semiconductor kind (fields may be `NONE`), so `has_detail` is constant per kind; the thermal node is the terminal count, not a flag.
- `Model` adds `version` (7.04: identity), `module: Ref` (7.16: a qualified module, never a bare word) and `word: NameId` (the code model's name; the `Other` type word, since a `#[repr(u8)]` family cannot carry it).
- `Table` row → named `TableSource` (the kernel's `table::Table` is re-exported at the root).
- Look-alike families split (7.18): `Diode`/`DiodeLevel3`/`DiodePspice`/`DiodePspiceTemp`/`DiodeLtspice`/`DiodeIdeal`/`DiodeSpectre`, `Mos0`, `VdmosRpi`, `Gaas`, `JfetShockley`, `JfetHspice2`, `*Multiplier` R/C/L (xy). 70 families; xy levels the inventory does not name are `Other` (COVERAGE gap G12).
- Multiplicity features → one `Extra` per kind class (`MultResistor` … `MultOther`), since targets drop `m` per kind (ESPice on X, C, L; Xyce on V/I).

## Addendum A5 (requests from the circuit owner)

- `Violation::FieldDomain` is appended last (`X0015`), so no earlier `X` code moves; `arg` is the row's index in its kind's table.
- `ProbeContext` keeps its name; its doc now reads "a circuit read inside a `Handle::Expr` field". `Gen`'s `Reads::Constant`/`Reads::Circuit` are documented as `Handle::Expr`/`Handle::ExprDyn`.


## Run

Run owner, Definition phase. Question → choice, why. `R<n>` ids are cited from `rewrite/COVERAGE-run.md`.

- R1. IR-RUN lists `MonteCarlo` as an `AnalysisKind` that "opens a scope", but only a `Loop` owns a `ScopeKind::Loop` scope (kernel) → Monte Carlo is `LoopKind::MonteCarlo` (detail `MonteCarlo { runs, seed, samples, variations, sampler, nominal }`); `AnalysisKind` has no Monte Carlo member and so no kind opens a scope: `AnalysisKind::opens_scope()` is not defined (it would be `false` for all 24). A Monte Carlo runs its body once per sample, exactly as a sweep does; no kernel change.
- R2. `Loop`'s detail by kind → `Loop { kind: LoopKind, after: LoopEnd, name, detail, body }` with three detail tables (`SweepLoop`, `TableLoop`, `MonteCarlo`), as `Element`/`Analysis` do; no `Loop` field that means "really another kind" (C3).
- R3. What a swept target holds after the loop differs (Spectre and VACASK restore, ngspice `foreach` keeps the last value, 13.03) → `LoopEnd::{Restore, KeepLast}` on the loop.
- R4. One detail door for 31 detail row types (22 analysis, 3 loop, 6 measure) → trait `Detail { type Kind; const KIND; table; table_mut }` implemented in `analysis.rs` by one macro, with `Design::detail::<D>`, `Editor::set_detail::<D>`, `Builder::analysis::<D>`, `Builder::loop_::<D>`, `Builder::measure::<D>`; the same call the kernel made for `ItemRow`.
- R5. A periodic small-signal analysis needs "its" PSS/HB, and `Handle` has no item variant → order: `Pac`, `Pnoise`, `Pxf`, `Pstb` linearise around the nearest `Hb`/`Pss` before them in run order (Spectre's and HSPICE's own rule), checked; no handle field.
- R6. Spectre `xf` and VACASK `acxf` are one meaning (AC transfer from every source) → one kind `Xf`; IR-RUN's `AcXf` is that kind. `DcXf` (every source, DC) and `Tf` (one source, with Rin/Rout) stay two kinds.
- R7. HSPICE `.trannoise`, VACASK `tran noisefmax=` and Spectre's tran noise are a transient with device noise → `Tran::{noise_fmax, noise_fmin, noise_seed}`, not a kind.
- R8. The default step ceiling differs (ngspice `min(tstep, span/50)`, Spectre/VACASK `span/50`, Xyce `span/10`) and IR-RUN says the reader "fills the cap its simulator would apply" → a law field `Tran::cap: StepCap` used when `max_step` is `NONE`, not an expression the reader builds (an expression would not write back as the original text; the law reads back to itself). LTspice's and HSPICE's default ceilings are not established: their readers use `StepCap::None` until cited.
- R9. `Target` as `{ kind, a: NameId, b: NameId }` cannot reach an element inside an instance, a private option, or be linked → `Target { kind, option: OptionKey, a: Ref, b: NameId, path: NameList }` (24 bytes): `a` is a `Ref` the linker binds when `TargetKind::def_kind()` names a kind, `path` the instance path (C14), `option` the key. Added `TargetKind::SubcktParam` (ngspice `alterparam sub p=`).
- R10. A table of states → `TableLoop { columns: TargetList, values: ExprList }` (row-major) with typed targets as columns, not parameter names only, so HSPICE `.alter` blocks that set temperature or options fit.
- R11. `SweepSpec` → adds `SweepKind::Log` (points in all: Spectre `log=`, the Python API) beside `Dec`/`Oct` (per decade/octave), and `bounds: SweepBounds::{StartStop, CenterSpan}` for Spectre's `center=`/`span=`; `end: SweepEnd::{Grid, OnStop}` applies to linear ranges too (LTspice `.step 1k 10k 4k`). Embedded by value (28 bytes) in every row that sweeps; no sweep pool.
- R12. "All" means another set on every simulator (9.03–9.05) → no `ProbeKind::All`; the save set is categories: `AllV`, `AllVTop` (LTspice's default), `AllSourceI`, `AllI`, `AllPinI`, and each reader expands its own `all` and default. ngspice `savecurrents` and HSPICE `post`/`probe` are read into `Save` rows, not options.
- R13. A probe of one terminal → `Probe::terminal: u8`, the index in the element kind's terminal order (HSPICE `I3(M1)` is native; terminal names are the circuit owner's per-kind facts). ngspice's `vp` is radians, HSPICE's and Xyce's degrees → `AcPart::{PhaseDeg, PhaseRad}`.
- R14. Measures typed without a word list → one `Event { kind: Cross | At | Bound, edge, last, signal, value, count, delay }` and six detail rows (`TrigTarg`, `When`, `Find` (find-when and find-at), `Deriv`, `Stat` with `Stat::{Avg, Min, Max, Pp, Rms, Integ}`, `Param`); windows are two events (`Event::BOUND` for the analysis's ends).
- R15. `OptionKey` membership → a key is one quantity with a stated meaning; ngspice-only options whose meaning is not established, and every option of one simulator not listed, are `Private` (49 keys). SPICE `reltol` is not Xyce's split tolerance; HSPICE `gmin`/`gmindc` are `GminTran`/`GminDc` beside ngspice's `Gmin`; `rshunt`/`gshunt` are two keys (reciprocal: a pass); the integration method is three flag keys (`MethodTrap`, `MethodGear`, `MethodEuler`) plus `MaxOrd`, so no option value is a method word. `OptionKey::class()` gives 10.09's tag.
- R16. Where a `.ic` holds differs (ngspice: the transient's OP only; Xyce: every OP, 8.21) and VACASK's `ic=` belongs to one analysis → `NodeSet { applies: IcScope, next_only: bool, reference: NameList }`; an empty `node` on a `Nodeset` is every node (ngspice `.nodeset all=`).
- R17. HSPICE `.noise` takes the deck's AC frequencies; Xyce refers input noise to the source's AC magnitude → `Noise::ac_sweep: bool`, `Noise::input_ref: NoiseRef::{Unit, SourceAc}`.
- R18. A Spectre statistics `correlate param=[a b c]` → its pairs (`Correlation { a, b, cc }`), which write back as pairs (the law holds on the IR). `Vary::mean` (`NONE` = the parameter's value) is added for table forms that state a centre (Xyce `means=`).
- R19. "Analysis kind or none" fields (IR-RUN §6) → `Save { analysis, every_analysis: bool }` (A22: `Save { analyses: AnalysisKinds }`); `Print` always names its kind (`.print` requires one).
- R20. Item count of `Extra` → the run owner adds 119 members; with the kernel's 8 the `u8` repr leaves the circuit owner 129. If the two owners' regions together pass 256, `Extra` needs `#[repr(u16)]` (kernel).
- R21. A device's initial state stated outside the device (VACASK's transient `ic` entry, Xyce `.INITCOND`; circuit cross-request D6) → `NodeSetKind::Device`, `node` the element path; an instance's own `IC=` stays the circuit owner's field. The HB drive of a source (5.20) and Spectre `xfmag` stay on the source side (gaps G-HBDRIVE, G-XFMAG, for the circuit owner's ruling); `Hb` and `Xf` refer to no source field.

## Addendum A6 (one detail door)

- The merge of the two halves did not compile (E0592: both owners defined `Design::detail` and `Editor::set_detail`) → one kernel trait `DetailRow { rows(&Design), rows_mut(&mut Design) }` in `design.rs` beside `ItemRow`, and one `Design::detail`, `Builder::detail`, `Editor::set_detail`, defined once in `design.rs`, `build.rs` and `edit.rs`. `design.rs` stays under 500 code lines, so there is no `detail.rs`.
- Circuit: the circuit-local `DetailRow` trait and its `Design::detail`/`Editor::set_detail`/`Builder::detail` are deleted; `element.rs`'s `detail_rows!` now implements the kernel trait through `design.circuit.<field>`; `wave.rs`'s `WaveRow: DetailRow` names the kernel trait. `DetailKind` and every typed accessor are unchanged.
- Run: `Detail` became `RunDetail: DetailRow { type Kind; const KIND }` (the name says it is the run owner's sub-trait), with `table`/`table_mut` removed; `analysis.rs`'s `details!` implements both traits through `design.run.<field>`; `build_run.rs`'s three typed pushes bound on `RunDetail<Kind = …>`. Its duplicate `Design::detail`/`Editor::set_detail` are deleted.
- `lib.rs` re-exports `DetailRow` from `design` (kernel group) instead of `element`, and `RunDetail` instead of `Detail`; its header names every run re-export.

## Addendum A7 (requests from the run owner)

- `Feature::Option(OptionKey)` comes after `DrawPer` in index order and counts `OptionKey::COUNT` (49); `OptionKey::Private` is one feature, and whether a private option belongs to the writer's own dialect is the writer's check.
- `DefKind::Measure` is appended last (no earlier `DefKind` moves). `Builder::resolve` accepts a `Param`-tag expression bound to a `Measure`; `link` tries `Param`, then `Measure`, for `Param`-tag expressions reached from a `Measure` item's handles; `check`'s `RefTarget` reports a `Param`-tag expression bound to a `Measure` anywhere outside a `Measure` item.
- **Note for the run owner (Implementation phase):** `Builder::measure` (`build_run.rs`) is a defining push: it must call `define(DefKind::Measure, name, scope, item)` for the measure it pushes. Not edited by the kernel.
- Instead of the width note (lead's A7 revision): `Extra` is split by owner, for good. Three `#[repr(u8)]` enums with `ALL`/`COUNT`: the kernel's `Extra` (8 members) stays in `feature.rs`; `CircuitExtra` (76) is in the new circuit-owned `circuit_extra.rs`; `RunExtra` (119, after the run follow-up's `IcDevice`) is in the new run-owned `run_extra.rs`. Every member, name and doc moved unchanged. `Feature` is `{ …, DrawPer, Extra(Extra), Circuit(CircuitExtra), Run(RunExtra), Option(OptionKey) }`, with index offsets in that order. The owner regions and their markers are gone from `feature.rs`.
- File map (IR-KERNEL §10) as amended by A7: circuit owner += `circuit_extra.rs`; run owner += `run_extra.rs`; `feature.rs` holds only kernel members. An owner adds a feature without touching a kernel file.
- Merge of the run follow-up (6c74d081): the only conflict was here, R20 vs this file's later sections; I kept the run owner's R20/R21 and the kernel's A6/A7 sections.

## Addendum A8 (= the A7 revision above)

- A8 (merge the run owner's follow-up 6c74d081, then split `Extra` into `Extra` / `CircuitExtra` / `RunExtra`) is what the A7 bullets above record: merged in dc870504 (the only conflict was this file; both sides kept), split in d0e0c80. The file map has `circuit_extra.rs` (circuit) and `run_extra.rs` (run). The `Extra` doc's 256 note is gone; each enum is `#[repr(u8)]`, and the largest, `RunExtra`, has 119 members.

## Implementation, kernel M1 (storage core)

- `build.rs` passed 500 code lines → the expression constructors moved to a new kernel file, `build_expr.rs` (one `impl Builder` block, split along its transform). `expr_row` and `assert_expr` are crate-visible helpers there.
- `Builder::push` asserts the scope and the table's capacity, not the row's handles. Checking handles would call the owners' `RowFacts::handles`, which are placeholders until they land, and `check` covers it. The doc now says so; nothing in a signature changed.
- `Builder::string("")` is legal: a `Str` value may be empty, while a name may not. `Names::intern_any` (crate) interns without the non-empty assertion; `Names::intern` keeps it.
- `Ref::is_resolved` is false for the crate-private `DefId::HELD` as well as for `NONE`.
- `Literal::value` formats `m e k` and parses it with Rust's decimal parser, which rounds correctly (half to even): the reference rule, with no hand-written float arithmetic. `Literal::from_f64` takes the shortest round-trip digits from `{:e}` and places them on the engineering exponent (the multiple of 3 at or below the decimal exponent).
- One exhaustive dispatch from `ItemKind` to its row type, `for_kind!(kind, R => body)`, lives in `facts.rs` (crate-internal macro, declared first). `span_of`, `Editor::map_refs` and the row comparison of `eq_ignoring_spans` use it; M2–M4 use it too.

## Implementation, kernel M2 (eval, features, check, diag, link)

- The 11 kernel rows' `RowFacts` live in a new kernel file, `item_facts.rs` (`item.rs` would pass 500 lines). In M2 they implement `handles`, `map_refs` (none hold a `Ref`), `features` (`IncludeSection`, `SectionLibrary`) and `check` (`FieldDomain`, `OpaqueEmpty`); `dump`, `parse` and `generate` land in M3 and M4.
- Handle kinds of the kernel rows: an `If`'s conditions and a `Param`'s value are `Handle::Expr` (constant in time); a `Func` body is `Handle::ExprDyn` (a function may be called from a circuit-reading expression); a `Subckt`'s head defaults come through `Handle::Params`, read as constant.
- Reachability is read from the scope table (each non-file scope's `owner`), not from rows: `Design::reachable` (crate) walks files in id order and each item's bodies right after it, entering each scope once, so it is total and needs no `RowFacts`. Every expression walk (`ExprWalk`, crate) visits a row once per walk and descends only to lower existing ids, so it terminates on any table contents. `Design::children` reads the raw columns and skips an out-of-pool list.
- `check` runs four passes (files and scopes, definitions, expression rows, reachable items). Classification where the variant docs overlap: a list that overruns its pool, a `sub` byte outside its enum and a `Num` without its literal are `HandleRange`; a ternary pair that overruns the pool and a count outside the arity are `ExprArity`. `ScopeOwner` is reported once per scope, from the scope's side (owner kind, owner listed in the parent, owner's handles naming the scope). Rows the expression pass flags are not decoded by the item pass.
- A formal-argument read is bound to its function's definition, whose name is the function's, not the argument's: `RefTarget` accepts it when the function row's arguments hold the read's name. A `Param` or `Measure` target must carry the read's name.
- Placement `Extra`s (`NestedSubckt`, `LocalModel`, `LocalFunc`, `ParamInBranch`, `LoadInBranch`) look at every enclosing scope, not only the nearest. `CaseSensitiveNames` is design-level, over every interned name (paths and strings included); `check_legal` reports it with `Span::NONE`.
- `link` binds row references by `Design::lookup` from the item's scope; for expressions in a row, also from the item's scope. Consequence: a subcircuit head default that reads a sibling head parameter is looked up from the subcircuit's enclosing scope and stays unresolved (legal). Recorded as a limit, not changed.
- `eval`: `Ddt`/`Idt` are `Unbound` before their arguments are evaluated; a user call whose argument count differs from its function's is `Unbound`; the bitwise operators accept integer values below 9.2e18 in magnitude.

## Implementation, kernel M3 (IR text)

- Files, split along the transforms (each under 500): `text.rs` (the two spines `dump` and `parse`, the keyword table), `text_words.rs` (`Words`, re-exported as `text::Words`, so A2's path holds), `text_expr.rs` (the printer and the public spelling helpers), `text_parse.rs` (the expression reader, shunting-yard with explicit stacks), `text_lex.rs` (literal, name, quoted-run and mark lexers), `item_text.rs` (the kernel rows' text; their `RowFacts` delegate to it).
- The kernel writes an item's keyword, its indentation and its bodies; a row's `RowFacts::dump` writes only the fields after the keyword (no keyword, no indentation, no line break), and `RowFacts::parse` reads from after the keyword. An `If`'s `elif (cond)`/`else` lines are the kernel's (it reads the whole chain's conditions before any body); an `If` row's own text is its first condition.
- `parse(dump(d))` equals `d` only for a design built in a caller's natural order (expressions bottom-up and left to right, then the push with its `&str` names) and shaped as a tree (no expression row shared by two parents: text cannot say "the same row twice"). Every reader follows that order. `Words::name_text`, `Words::names_text` (names as text, interned by the push) and `Words::line_span` were added for that, and rows' `generate` must build in that order too.
- The reference mark, refined from A4: bare when a bare spelling would read back the same (the linker's lookup, or the formal binding inside a function, gives the reference's definition, or gives nothing for an unresolved reference); `name?` for an unresolved reference the lookup would bind; `name#k` otherwise. With no scope (`Ctx::detached`) only `?` and `#k` are written.
- Grammar facts the bodies fixed: every binary level is left-associative (`a == b == c` is canonical); a ternary's `?` follows a blank, a `?` glued to a name is its mark; scale letters are lower case only; a negative literal is `-1.5`, the `Neg` of a literal `-(1.5)`. A bad line gives one `TEXT_SYNTAX` row (the first) and its deeper lines are skipped; the reader refuses what the builder would assert on (a second definition of one name, a title outside a deck's top, ground among globals, repeated ports, parameters or arguments, a duplicate file path).
- Pending the ruling on `RowFacts::dump`'s missing context: `text::dump` writes the kernel rows through `item_text` with the item's `Ctx` (scope, item, function), and the owners' rows through their `RowFacts::dump`; the kernel rows' own trait `dump` writes the detached form.

## Implementation, kernel M4 (generator)

- `generate` fills each file depth first, each body right after its item (the order of `text::dump`), and builds every expression in post-order left to right from an explicit frame stack, so a generated design is in a caller's natural order and is a tree: `parse(dump(d))` equals it exactly. The kernel rows' generators are in a new `gen_rows.rs` (behind `gen`), and their `RowFacts::generate` delegate there.
- `Gen` gained two crate fields: `scope` (set by `generate` before each row; where `expr` finds definitions to read) and `formals` (the arguments of the function being generated, which its body may read). It also gained two crate methods: `fresh()`, a fresh name as text, not interned, so the push interns it in order; and `word()`, a string or text from a fixed lower-case list that holds the characters IR text escapes. `Gen::fresh_name` interns at once, so a row that uses it must call it where its parse would intern the name.
- Placement and context, decided by `generate`, not by the rows: `Title` only first in a deck's top; `Section` only in a file's top; scope-opening kinds only below `scope_depth`; the placement `Extra`s gate `Subckt`, `Model`, `Func`, `Param` and `Load` by their enclosing scopes; a kind whose fields need an expression only when `Expr(Num)` is in the set. A tree reads the circuit only under `Reads::Circuit`, and then holds at least one probe (its first leaf), so its members are exactly the `Dyn` features; `Ddt`/`Idt` appear only in such trees.
- Reads in generated expressions: a visible definition (resolved), a formal (bound by `Builder::func`), or a fresh name (unresolved, so the text spells it bare and it reads back unresolved). No name is defined twice; every name is lower case, so `CaseSensitiveNames` is never set.

## Implementation, kernel throughput (before M5)

- The throughput measurement (`examples/throughput.rs`, `required-features = ["gen"]`; numbers in `rewrite/IR-PERF.md`) found six super-linear paths, each fixed at its root: `Builder::func`'s formal binding (a visited vector sized by the whole design, now a set of the body's rows), `Gen::pick_def` (now a per-kind index and bounded random draws: "chosen at random", no longer uniform), `text::dump`'s per-load and per-function scans of every definition (now one map per dump and the `(scope, Func, name)` index), `check`'s owner test (now one listing map), literal validation through `value()` (now `Literal::finite` from digits and power), and SipHash on the interner and definition index (now `names::Fx`, a dependency-free FxHash with the final rotate).
- `generate` gives a file's top scope the whole item budget (it gave at most 12 before), so `Size::items` is the design's size. The generated test designs are larger as a result; every property still holds.
- There is no regression test for the quadratic class (a timing test would be flaky). The throughput example is the oracle: it prints the rate per phase at a chosen size, and a reviewer compares N and 4N.

## A9 and A10 applied (kernel)

- `RowFacts` has A9's final shape (see IR-KERNEL §7.2), with `dump(&self, design, ctx: &text::Ctx, out)`, `Ctx` being the scope, item and function context the writers need. The 11 kernel rows, the circuit owner's two `map_refs` bodies (Element, Model: same references, now read and stored through the design), the placeholder macro and every caller (`Editor::map_refs`, `link`, `check`, `features_used`, `text::dump`) follow it.
- `check` reads references through `map_refs`, which needs `&mut Design`, from one clone of the design per run. That is O(design) once, so check stays linear, but memory doubles while check runs. Proposed for later: a read-only twin, `refs(&self, design, at, out: &mut Vec<(Ref, DefKind)>)` (the api-design rule: every retained door has an immediate twin).
- `link` binds row references in two passes over the same `map_refs` order: record the references, look the unresolved ones up through `&Design`, then write the k-th answer into the k-th reference.
- The `design_features` hooks are empty in `element.rs` and `analysis.rs` (each owner fills its own). `features_used` and `check_legal` reach them through `feature::design_level`, which also holds the case clash, so the two agree by construction.
- `todo_facts!` carries `#[allow(unused_macros)]`: on the owners' branches every row is real and the macro has no user, while on `ir-rewrite` the owners' placeholder rows still use it. It is deleted at M5, after both merges.
- The writers live in `text_expr.rs` and are re-exported from `text` (`text::write_value`, …), together with `Ctx`. `write_names`, `write_exprs` and `write_params` were added as the mirrors of `Words::names`, `exprs` and `params`; `write_params` asserts every value is set (a head default without a value is the `Subckt` row's own spelling).
- A10: `link` and `item_text::dump_subckt` use the subcircuit's body as the lookup scope of its head defaults. Tests: `kernel_link::a_subckt_default_reads_its_sibling_head_parameter` (red with the rule reverted), `kernel_text::a_subckt_default_reading_a_sibling_needs_no_mark`.



## The spine's additions and the canonical empty list

- `FeatureSet::ALL` (every `Feature::COUNT` bit, the IR-text writer's set) and `Design::reachable` (public, its order part of the contract: files in id order, depth first, bodies right after their item) are in.
- An empty list is always `List::EMPTY` (start 0): every `Builder` list constructor (`names`, `name_ids`, `exprs`, `params`, and through them every row and expression list) returns it for an empty slice, and `check` reports any other empty range as `HandleRange`. Equality therefore no longer depends on how an empty list was made (the circuit owner's report; its `canonical()` in circuit_parse.rs can go).
- `check_legal` agrees with `features_used` by construction: both take the design-level features from `feature::design_level` (the case clash and both owners' `design_features`). A generated property checks it: for 40 designs × 20 random subsets, the set of reported `arg`s equals `features_used(d).minus(legal)`.

## A11 applied (kernel)

- The class lives in a new kernel file, `class.rs` (`ExprClass`, `expr_class`; crate `def_classes`, `node_class`). The per-definition table is cached in `Design::classes` (a `std::sync::OnceLock`, so `Design` stays `Sync`). `Design::builder()`/`editor()` and every mutating `Builder`/`Editor` method clear it, and it is not part of equality. `expr_class` costs O(its tree) once the table is built, and O(design) the first time after a mutation.
- A formal-argument read (a `Param` expression bound to its `Func`) is not a dependency: its class is the call's argument's, which the caller's own tree holds. A user call is a dependency on its `Func`.
- `ProbeContext` stays direct, as ruled. Making it transitive (a parameter whose value reads the circuit through another parameter) is open.
- The generator follows R14. It decides a tree's class up front: a dynamic tree starts from `Temp` (or a probe under `Reads::Circuit`); a constant tree holds neither and reads only definitions the generator noted as constant (`Gen::def_class`, set by each row's generator for the definitions it pushes). `Ddt`/`Idt` appear only in a dynamic tree under `Reads::Circuit`. The expression generator moved to `gen_expr.rs` (gen.rs passed 500 lines).
- `bind_ref` asserts the slot's kind, not the name (a reader may bind by its own case rule; `check`'s `RefTarget` then judges the name).
- kernel_store.rs passed 500 lines: its structure tests (`reachable`, the empty list, `bind_ref`) moved to `tests/kernel_structure.rs`.

## Implementation, run owner

- One crate-private trait `DetailFacts` (in `analysis_facts.rs`) carries a detail row's handles, features, field fault, text and generator; each header kind (`Analysis`, `Loop`, `Measure`) reaches its 22, 3 and 6 detail types through one exhaustive macro dispatch (`by_detail!`, `by_loop!`, `by_measure!`).
- Compound values (probe, target, sweep, event, variation) are parenthesized groups of their own fields in IR text; enum members are the snake case of their names; a `bool` is a bare word; a first-member enum is written only where its doc says so. The kernel's keyword for `NodeSet` is `nodeset`.
- The builder's pool appends (`probes`, `targets`, `dc_axes`, `varies`, `correlations`) do not check field domains, as `push` does not (`check`'s `FieldDomain`); the typed value constructors (`probe_v`, `target`, …) assert theirs. The empty list is `List::EMPTY` (a built and a parsed design compare equal).
- `map_refs` (A9) visits, in this fixed order: an analysis's DC axes' targets; a sweep loop's target or a table loop's columns; an alter's target; a statistics block's process then mismatch variations, then each correlation's two ends. An element target's `a` is a name, never visited.
- `design_features` sets `SeveralAnalyses`, `OptionMidRun` and `LoopScoped` (reported at `Span::NONE`); `NodeSetInSubckt` and `LoopNested` are set per row from its scope.
- Generators choose a kind whose detail they can build before interning anything (a failed attempt would leave a stray name), build in the parse's order (names and lists exactly where the text puts them), require `Expr(Num)` (`Gen::expr` asserts it), leave loop bodies to the kernel, and keep a non-string alter value free of strings.

## The spine's additions and the canonical empty list

- `FeatureSet::ALL` (every `Feature::COUNT` bit, the IR-text writer's set) and `Design::reachable` (public, its order part of the contract: files in id order, depth first, bodies right after their item) are in.
- An empty list is always `List::EMPTY` (start 0): every `Builder` list constructor (`names`, `name_ids`, `exprs`, `params`, and through them every row and expression list) returns it for an empty slice, and `check` reports any other empty range as `HandleRange`. Equality therefore no longer depends on how an empty list was made (the circuit owner's report; its `canonical()` in circuit_parse.rs can go).
- `check_legal` agrees with `features_used` by construction: both take the design-level features from `feature::design_level` (the case clash and both owners' `design_features`). A generated property checks it: for 40 designs × 20 random subsets, the set of reported `arg`s equals `features_used(d).minus(legal)`.


## The spine's two text requests

- `text::parse` gives every span (an item's, a `TEXT_SYNTAX` row's) the file of the block its line sits in (`FileId::NONE` for a `file` line itself), with byte offsets into the whole text handed to `parse`. `Words` carries the line's file in a crate field, so every reader's `line_span()`/`span()` is right without a change on the owners' side.
- `Design::entry` is total: `FileId::NONE` for a design with no file; a caller that needs a file asserts.

## A12 K1 applied (typed arithmetic)

- `Sgn` is Int-valued (Int iff its operand is), against AMEND-A12's table and with DIALECT-VACASK run f2 (`sgn(-3)/2` printed 0; R5, the run wins). Reported to the lead before building.
- The evaluator keeps every numeric rule on `f64` and adds the tag in `apply` from the operator and its operands' tags; `DivTyped` is the one operator whose number depends on the tags. `eval` is `eval_typed(...).map(|v| v.number)`.
- `Bindings::param_value` is a defaulted method (no existing `Bindings` impl breaks); a typed parameter (an instance passing `x=3` or `x=3.0`) comes through it.
- IR text: `3i` (the suffix `i`; no scale letter is `i`), `/:` at `*`'s precedence, `to_int`.
- The Int zero (`0i`) is a valid literal beside `Literal::ZERO`; `eq_value` separates Int from real literals of one value (also `Design::eq_meaning`).
- `Literal` and `NumForm` moved to `literal.rs` (expr.rs passed 500 lines), re-exported from `expr`.
- The generator makes Int literals only when the set has `Extra::IntLiteral` (`Gen::int_literals`, set by `expr`); `DivTyped` and `ToInt` follow the `Binary`/`Func` features as every member does.

## A12 K2 applied (two features of scope)

- `EnclosingRef` counts every resolved reference an item holds: its row `Ref`s and the `Param`/`UserCall` nodes of its expressions; an unresolved (`HELD` or `NONE`) one is not bound anywhere and does not count. The test is on the item's own scope chain (`Design::in_enclosing_subckt`, crate-private), so a subckt's head defaults (read in the subckt's parent scope) count against the parent's subcircuit.
- `FormalReadsParam` is a `Subckt` row fact (its `features`), not a design-level hook: it depends on the row's own defaults only.
- The generator: `Gen::enclosing_ok` filters `pick_def`'s draws; `Gen::param_reads` is cleared around a subckt's head defaults when the set lacks `FormalReadsParam`, and `leaf()` then drops `Param`.

## A12 K3 applied (`OpaqueClass::Hint`)

- Appended last (`S0006`), text word `hint`; no feature per class exists today, so `Feature::COUNT` does not move. The generator draws it with every other class (`OpaqueClass::ALL`).

## M5: the properties over every feature

- `kernel_gen::the_full_set_runs_every_property`: under `FeatureSet::ALL`, 400 seeds are valid, stay within the set and round-trip through IR text; up to 4,000 seeds between them use every feature but `Func(Ddt)` and `Func(Idt)`, named in the test. A `ddt`/`idt` makes its tree class `Circuit`, so it only ever counts as `FuncDyn`, and no valid design holds the non-dynamic pair. The pair stays in the enum: it is an index any writer's `FEATURES` may list, and nothing sets it.
- The generator makes `Extra::CaseSensitiveNames` (`Gen::case_ok`): when the set has it, one fresh name in 16 is the upper-case twin of the previous index (`G4` after `g4`). Before this, no generator made it.


## Amendment A14, run owner

- `NodeSet` already had a field `applies: IcScope` (which operating points hold an `Ic`), so A14's mode is named `holds: Applies` on all three state rows, the last field of each (IR text: a trailing `whole_run` word). `FromHere` is the default of every builder method and of IR text without the word: it is the meaning state items had before A14, so existing text and tests keep it; a reader of whole-run state (ngspice) pushes rows with `WholeRun`.
- Listed order for the mid-run features is one walk per deck file (items depth first, each body right after its item, an `Include` read in place once per walk); files no deck includes are walked alone. `OptionMidRun`, `EnvMidRun`, `NodeSetMidRun` mark a `FromHere` item after a run item, `WholeRunMidRun` (one member for the three kinds) a `WholeRun` one; all are set by `design_features` (reported at `Span::NONE`). `RunItemNested` is set per row (an analysis, option or `Env` in a subcircuit or section body), so it is reported at its item.
- The generator treats "a run item exists anywhere" as "after a run item" (generation follows listed order), and refuses a nested analysis, option or `Env` without `RunItemNested`.

## Amendment A15 and generator cost, run owner

- R30's pick rule is `Event::skip_initial: bool` beside `count` and `last`: a crossing is picked exactly one of the three ways (checked); `skip_initial` states ngspice's default with no `RISE`/`FALL`/`CROSS` (the first crossing after the first interval) and is never read or written as `CROSS=1`. `Event::until` is the search's upper limit (`NONE`: the analysis's end). `Event` grows to 44 bytes. Features `MeasSkipInitial`, `MeasUntil`, `MeasStatAt` (for `Stat::MinAt`/`MaxAt`), `ProbeAllBranchI`.
- The placement members (`NodeSetInSubckt`, `LoopNested`, `RunItemNested`) land under their old names; the kernel owner migrates them to `Feature::Placed` (A15 K1).
- The run generators are O(1) per row (measured: `throughput N all` builds ~420k items/s from 50k to 250k items and 373k at 10^6, against 170 s for 10^6 before): the "an analysis exists" guards read the table's length (generation appends, so no row is dead), "inside every loop" is `loop_depth(scope) == loops.len()`, a generated periodic small-signal analysis follows its `Hb`/`Pss` directly (`base_just_before`), and the probe, part and target kinds a set allows come from `const` tables rather than per-call feature sets. Enum words in parse compare against one reused buffer (parse 42 → 51 MB/s on the full set).

## A12 K1 addendum: `Sgn` Real, `Func::SgnZeroPos`

- Withdrawn: "`Sgn` is Int-valued" above. As ruled, the IR's `Sgn` (−1, 0, 1) is Real whatever its operand; VACASK's `sgn` is `SgnZeroPos`, appended last to `Func` (text `sgn_zero_pos`), tagged with its operand's type.
- −0.0 is tested as `Neg(0)`: a literal has no sign of zero, so IR text `-0` reads as the literal 0.
- `Func::ALL` and `ItemKind::ALL` are sized by their last member (`[Func; Func::SgnZeroPos as usize + 1]`), so a member appended moves no hand-written length.
- `kernel_gen`'s full-set reach carries one known gap as a marker: `Run(LoopOverOp)` (A12 R2) is not yet generated by the run owner.

- Reach under the kernel's full-set harness (4,000 seeds, `FeatureSet::ALL`) is part of the run owner's gate. A kind that needs a predecessor gets it from its own generator, not from the kernel happening to pick it next: an `Hb`/`Pss` generator follows itself, half the time, with one periodic small-signal analysis; a sweep loop with `LoopOverOp` in the set starts its body, half the time, with an `Op`; both count in the size budget. Measured over seeds 0..4000: every run item kind, analysis kind, `RunExtra` member and option key now appears in at least 48 seeds (before: `LoopOverOp` 1, the periodic kinds 6–11). `run_gen` checks reach again on seeds 50,000..54,000.


## A12 K2 addendum: `Extra::DeckWithoutTitle`

- A design-level feature (`feature::design_level`, beside the case clash): it is a fact of a file, not of an item. `check_legal` reports it at `Span::NONE`.
- The generator: with the feature absent, every deck gets its `Title` first, even past the item budget. A set holding neither the feature nor `Item(Title)` can hold no deck, so every generated file is then a `Library`.

## `Design::entry` is file 0 (ruled)

- Replaces "the first `Deck` file, else file 0": the entry is file 0 whatever its role (`NONE` with no file). A kit whose entry is a library reaching a deck writes the library. `kernel_store::files_scopes_and_entry` and `kernel_structure::entry_is_total` now pin file 0.


## Amendment A16, run owner

- `Pss::sc_iter` and `Pss::steady_coeff` are appended (`Pss` 28 → 36 bytes), written `sc_iter=` and `steady_coeff=` after the oscillator nodes. `Four::harmonics` may be `NONE`; `check`'s invariant for it: the design sets an `OptionKey::NFreqs` (O(options) per such `Four`). `OptionKey` gains `NFreqs` and `FourGridSize` (class `Output`, not flags), inserted before `Private`. `AnalysisNamed` is set per row; analysis names are generated only with it.
- The generator gives a count-less `Four` its predecessor itself: half the time, an `Analysis` generator that picked `Four` first pushes an `nfreqs` option (mode by the same mid-run rules), and `Four` drops its count when an `nfreqs` option is among the last 16. Reach over the harness's 4,000 seeds: every run feature in at least 41 (`FourHarmonicsOption` was in 3 without it).
- Follow-up (ng-run): one feature per `holds` member, `StateFromHere` and `StateWholeRun`, set per row wherever the item stands (the `*MidRun` members keep their meaning: after a run item). So a `FromHere` item before every run item is visible to a target that spells only `WholeRun`, and a pass relabels it exactly. The generator picks only a mode whose feature is in the set.

## A15 K2 applied

- Both are row facts (`Subckt::features`, `If::features`). `IfConditionDynamic` reads each condition's class through `class::tree_class`, a walk sized by the tree, not the design: `expr_class` (now built on it) allocated an expression-table stamp per call, quadratic when a row calls it per expression.
- The generator: `Gen::constant_only` keeps a tree non-dynamic around `If` conditions; a formal always gets a default unless the set has `FormalWithoutDefault`, and a set with neither literals nor that feature makes subcircuits without formals.

## A15 K3 applied

- The transitive `ProbeContext` is `check`'s last pass (`probe_classes`), run only when nothing else was reported: `class::compute` reads parameter rows and expression pools without range checks, and `check` must not panic on any input. A broken design reports its structural faults first and this one on the next check. `arg` is the field's root expression.
- The generator's reads go through `Gen::may_read`: a constant tree reads only constant definitions, a dynamic one under `Reads::Constant` none above `Temp`. Titles and comments draw from `Gen::line_word`.

- Reach under the kernel's full-set harness (4,000 seeds, `FeatureSet::ALL`) is part of the run owner's gate. A kind that needs a predecessor gets it from its own generator, not from the kernel happening to pick it next: an `Hb`/`Pss` generator follows itself, half the time, with one periodic small-signal analysis; a sweep loop with `LoopOverOp` in the set starts its body, half the time, with an `Op`; both count in the size budget. Measured over seeds 0..4000: every run item kind, analysis kind, `RunExtra` member and option key now appears in at least 48 seeds (before: `LoopOverOp` 1, the periodic kinds 6–11). `run_gen` checks reach again on seeds 50,000..54,000.

## A15 K1 applied (`Feature::Placed`)

- Dense group, appended last (after `Family`): index `(scope - 1) * ItemKind::COUNT + kind`, `File` asserted out. Every pair has an index, though `Placed(_, Title)` cannot occur (named in kernel_gen's full-set test beside `Func(Ddt)`/`Func(Idt)`); a sparse table would buy 4 bits and cost a lookup in `index`.
- Placement is set once, in `feature::item_features`, from `feature::chain_kinds` (which replaced `enclosing`). The run owner's flags and generator guards for its three members are removed, and so are `state_order::nested` and `run_words::in_scope_kind`, which had no other user. `Loop::generate`'s `LoopOverOp` seeding asks `gen::placeable` for the `Op` it pushes into its body.
- `Title::check` reports a title outside a deck's top (`FieldDomain`), as `ItemKind::Title`'s doc always said; before, only the builder's assert and the IR-text refusal held it.
- The generator no longer keeps sections in a file's top: `Placed(S, Section)` decides, and with it IR text's nested sections are exercised.

## A15: `FormalReadsParam` widened and renamed `FormalNotConstant`

- Ruled: every head default that is not a constant (a parameter read, or class `Temp`) sets it. The old name would mislead for `w=$temp`, so it is `Extra::FormalNotConstant`; its index is unchanged. `$tnom` and the other A1 leaves are constants of the run and do not set it. The generator keeps a default constant (`param_reads` off, `constant_only` on) unless the set has it.

## M5: the `refs` twin

- `RowFacts::refs(&self, design, at, out: &mut Vec<(Ref, DefKind)>)` is a required method (no default could be derived: a `Handle::Ref` carries no kind), written for every row of all three owners in one commit, as ruled. `Design::item_refs` is its dispatcher (`Design::refs` already named the reference pool's accessor). `plan::target_refs_into` is `target_refs`'s twin.
- `check` reads through it and no longer clones the design; `link` and `bind_ref` keep `map_refs`, since they write.
- The drift property runs 300 seeds of `FeatureSet::ALL` and asserts the kinds that hold references (Element, Model, Analysis, Loop, Alter, Statistics, Elaborate) were each seen with one, so it cannot pass vacuously.
- `probe_classes` (A15 K3) walks with one stamped `ExprWalk` rather than `tree_class`'s per-call set; `tree_class` stays for the row facts that call it once per row.


## Amendment A22, run owner (R43)

- `DcAxis::name` is appended (`DcAxis` 52 → 56 bytes) and written `name=` after the axis's sweep. `Loop::name` already existed. Each name has a feature, `DcAxisNamed` and `LoopNamed` (any loop kind), set per row, and names are generated only with them. Before this, loops were named with no feature.
- `Save { analysis, every_analysis }` becomes `Save { analyses: AnalysisKinds }`, a `u32` bitset with one bit per `AnalysisKind` (24 ≤ 32, asserted). Every analysis is `AnalysisKinds::ALL`; empty is a `FieldDomain` fault, and `Builder::save` asserts against it. The text is `analysis=<kind>` for one kind and `analysis=(<kind> …)` for several, in declaration order; it is omitted for every kind. One kind is `SavePerAnalysis`, a set with two or more kinds short of all is `SaveAnalysisSet`, and every kind sets neither.
- `SweepBodyNotOne`: a `LoopKind::Sweep` whose body, comments aside, is not exactly one analysis or one sweep loop. An empty body counts, and so does an opaque. The generator opens a sweep loop only with this feature, because the kernel fills the body (0–3 items) and the row cannot bound it.
- `RunInterleaved` (owned here, not by the kernel: it is an order fact about run kinds, and `Placed` states only enclosure): a file top lists an item that is no run item, comment or opaque between two run items. A run item here is any kind of the run section of `ItemKind`. `state_order::interleaved_features` makes one pass over the file tops. Without the feature, every run row's generator first asks `state_order::run_append_ok`, which reads back at most 32 items and refuses when that does not decide, so generation stays O(1) per row (10⁶ items: build 3.8 s, linear with 10⁵).
- `IcBeyondNext` is `IcNextOnly`'s twin (`next_only` false), so each member of the field has its feature. A node set is generated only when one of the two is in the set.
- Docs: `OptionKey::TrTol` is not VACASK's `tran_lteratio` (a hint, R19). `MethodGear` is variable order up to `MaxOrd`, not VACASK's fixed-order `gear2`/`bdf2`, and no member states a fixed order yet.
- Reach over the harness's 4,000 seeds of `FeatureSet::ALL`: `DcAxisNamed` 132, `LoopNamed` 1305, `SweepBodyNotOne` 1503, `RunInterleaved` 3997, `IcBeyondNext` 2614, `SaveAnalysisSet` 1673.

## A19 applied (an `ExprDyn` field's form)

- `feature::item_features` marks a root dynamic when the row lists it as `Handle::ExprDyn`, or when its class is at least `Temp`. A root listed under both kinds counts as dynamic.
- Generator: `operator()` takes the dynamic form under `Reads::Circuit` or `Gen::dyn_form`. The run owner's `gen_event` sets `dyn_form` around the event value, an `ExprDyn` field generated with constant reads unless the set has `MeasCircuitThreshold`. The merged tree's `random_run_subsets_restrict_the_generator` found it as `FuncDyn(Sqrt)` outside a set.
- The test's unary is `-(2 + 1)`: IR text reads `-1` as a negative literal, not a `Neg` node.

## A21 applied (`Func::ToReal`)

- Appended last to `Func` (text `to_real`); its value is `x`, and its tag Real is an explicit arm beside `ToInt`'s. Spec examples from VACASK run b1 in kernel_typed: `to_real(3i) /: 2i` is 1.5, `to_int(to_real(3i)) /: 2i` is 1, and `ToReal` of a Real is the identity.

## A23 applied (the passes' door for expressions)

- Shape: a fourth facts function, `RowFacts::map_exprs`, with a drift property against `handles`. One mutable visitor per row from which all four derive would have replaced every row's `handles`, `map_refs` and `refs` in all three owners' files mid-round. The default body (no root) stays: most rows hold no expression, and the drift property fails a row that lists a root and keeps it.
- Line counts: the trait method and `ExprUse`, 25 lines (facts.rs); the kernel rows, 39 (Subckt, If, Param, Func); `Editor::map_exprs`, 9; `Design::item_handles`, 8; `Builder::rebuild`, 40; `rewrite.rs`, 124 with its doctest (the amendment's pass, with `Design::exprs` where it wrote `args`); `tests/kernel_rewrite.rs`, 258.
- `rewrite_exprs` reads the roots through `handles` and stores them back through `map_exprs`: the drift property makes the two the same list, so every root is reached and stored.
- `item_facts.rs` passed 500 lines: `Param` and `Func` moved to `item_facts_defs.rs`; `field_domain` and `distinct` became crate-visible for it.


## Amendment A23, run owner (`map_exprs`)

- `run_exprs::Roots` holds the kernel's expression-list pool, the run pools and the pass's visitor. Its methods mirror the `*_handles` helpers one for one (`expr`, `exprs`, `sweep`, `probe`, `probes`, `event`, `dc_axes`, `varies`, `correlations`), so each row's `map_exprs` is one call per field, in `handles`' order. Pools are written in place. A probe's expression and an event's value are `Circuit`; every other root is `Constant`.
- `DetailFacts::map_exprs(&mut self, &mut Roots)` is required, so every detail kind writes its own roots. `exprs_of::<D>` reads the detail row, visits it, and stores it back. Analysis, Loop and Measure dispatch through it; Alter, OptionRow, Env, NodeSet, Save, Print and Statistics write their own fields. Elaborate keeps the default because it holds no expression.
- Oracle: the kernel's drift property (`kernel_rewrite`). It went red on a swapped event order (`count`/`delay`) before it was trusted green.

## Amendment A24, run owner (R47)

- `Tran { max_step, cap }` becomes `Tran { ceiling: StepCeiling }`, where `StepCeiling` is `Given(ExprId)` or `Rule(StepCap)` (8 bytes; `Tran` 44 → 48). `check` refuses a `Given` that is `NONE` or the literal 0 (any form), and a `record_start` that is the literal 0, as `FieldDomain`. The text writes the ceiling where `cap=` stood, after the segments: `max_step=v` or `cap=<rule>`, exactly one. A `tran` line with only `cap=` keeps its text; a `max_step=` line loses its `cap=`. The generator fills these two fields with a nonzero literal: the generator's own literal, a 0 replaced by 1.
- The sweep form class is 18 `RunExtra` members, `Sweep{Dc,Loop,Freq}{LinStep,LinPoints,Dec,Oct,Log,List}`. Each sweep sets its (use, form) member: a DC axis is `Dc`, a sweep loop `Loop`, every other analysis's sweep `Freq`. They replace `SweepLinPoints`, `SweepDec`, `SweepOct`, `SweepLog` and `SweepList`; lin by step had no member before. `SweepEndOnStop` and `SweepCenterSpan` stay as they are, orthogonal. `gen_sweep` takes the use and picks only forms in the set; an analysis or sweep loop whose sweep has no form there is not generated (`sweep_possible`).
- `PssOscDiff`: an autonomous `Pss` whose `osc_n` is not `NameId::GROUND`.
- Rename table for the dialect owners. `SweepLinPoints` → `SweepDcLinPoints` | `SweepLoopLinPoints` | `SweepFreqLinPoints`, by the sweep's use; the same for `Dec`, `Oct`, `Log` and `List`. New: `Sweep{Dc,Loop,Freq}LinStep`, which every lin-by-step sweep now sets. `Tran.max_step` and `Tran.cap` → `Tran.ceiling`.
- Reach over 4,000 full-set seeds: the least-reached run feature is in 45 seeds (`DcAxisParam`); `PssOscDiff` 47; every sweep member 67 or more.
- The generator's cost is no longer linear under `FeatureSet::ALL`: 10⁵ items 1.2 s, 3·10⁵ 9.9 s, 10⁶ 105 s. The kernel set alone is still linear (10⁶ in 1.8 s). The change came in with ir-circuit 67dae602: on the same machine, ir-run 163ee90d built 3·10⁵ in 1.2 s and 67dae602 in 9.5 s. This A24 part adds nothing measurable: the red commit 8f2334d1 built 3·10⁵ in 10.5 s, the bodies 9.9 s.

## A18 applied (long literals)

- The significands live in a dedicated pool (`Exprs::long`), not the text pool the proposal named: `eq_meaning` compares the text pool exactly, so long digits there would make two spellings of one decimal unequal. `eq_ignoring_spans` compares the long pool exactly; `eq_meaning` compares each literal pair by exact decimal value.
- The design-level twins take a `Literal`, not a `NumId` (a writer holds the literal): `Design::literal_value(Literal)`, `Design::literal_eq(Literal, Literal)`, `Design::long_digits(Literal) -> Option<(&str, NumForm)>`; `text::write_num(&Design, Literal, &mut String)` beside `write_literal`.
- A long `Mil` or `Int` literal is no `Long` (the written form is `Plain`, `Exponent` or `Scale`): an integer beyond 2^53 is a domain error anyway, and a long `mil` would need the decimal product by 25.4e-6 on strings. A reader that meets one keeps R37's interim rule.
- `Words::number` (no caller) reads short literals only; the expression parser reads both through `text_parse::push_number`.
- The owners' two generic literal reads moved to the design: `circuit_check`'s sign test (`literal_value`) and `run_words::is_one` (`literal_eq`).
- `check`'s long-literal arm is `literal::long_violation` (check.rs passed 500 lines).

## A26 applied (`Form`, `root_form`, `rewrite_exprs_in`)

- `features_used` now reads each root's use from the row's handles (`feature::roots_with_use`, `map_exprs`'s order) instead of testing membership in the `ExprDyn` list. A root id listed under both kinds of handle counts in each field's own form, where before one `ExprDyn` listing made every listing dynamic.
- `rewrite_exprs` and `rewrite_exprs_in` are one walk (`rewrite_where`). Each root's form is decided before the builder opens, while the class table is cached, and `map_exprs` stores back by position in the same order.

## A25 item 1 applied (`Diag.item`)

- `check`'s report takes the item instead of a span; non-item rows (expression rows, definitions' own faults at `Span::NONE`, the design-level features) pass `ItemRef::NONE`. The owners' row checks (`item_facts::field_domain`, the opaque check, `element::design_check`, `circuit_check`, `analysis_facts::report`) push at the item too; `Diags::push(code, span, arg)` stays for a reader's syntax rows and sets no item.

## A25 items 2, 3, 5 applied

- `FormalNotConstant` counts `$tnom` through the row's tree walk (`item_facts::reads_dependent`, which replaced `reads_param`), not through the class: `$tnom` is a constant of the run, and only a head default reading it is dependent (VACASK run c3).
- `SubcktConstantParam` is a `Param` row fact for a `Param` listed directly in a `Subckt` body. A `Param` in a branch inside a body is not one, since VACASK's `parameters` at that place is the branch's. The generator lists no `Param` in a subcircuit body when the set lacks it; dependent locals are dropped there too, a simplification that costs no reachable feature under `FeatureSet::ALL`.
- A generated `Load`'s path ends in `.va` for `VerilogA` and `.osdi` for `Osdi`. The `DeviceLibrary` path half is the circuit owner's generator, in its round-5 head.
- Item 4 (string or list values) waits for `ng-scaffold`'s and `vacask-plan`'s answers on the contexts each accepts.

## The generator's tracked class is the tree's

- `Gen::class` now adds a node's own class (a circuit-reading function, a user function's noted class) when the node is built, not when it is chosen. A node chosen and then abandoned for a leaf (a tree that still owed its probe) no longer raises it. `gen_expr::tests::the_tracked_class_is_the_trees` pins it against `expr_class`. Found by the circuit owner: a generator choice that read `g.class` built a design check refused. `Gen::new` is the constructor `generate` and the test share.

## The scaling gate

- `cargo run --release --features gen --example throughput -- scaling` measures build, dump, parse and check over `FeatureSet::ALL` at 1e5 and 4e5 items (best of two runs) and exits 1 when a ratio exceeds 8: a linear phase measures 4.4 to 4.9 here, a quadratic one about 16. It is a release binary, not a test: a debug-build test at sizes cheap enough for the suite cannot tell a quadratic term from cache effects, and a release-only test would skip in debug. It goes in the merge tier beside the gate's commands (IR-PERF.md, How it is measured). On 68c6be2d it is red on build (13.6), the circuit regression.

## The element-name index (R23)

- `Builder::element_named(scope, name) -> Option<ItemRef>`: the first element of that whole name listed in `scope`, from an index on `Design` (`element_names`, lookup only) built on first use in O(items). It is extended by `Builder::push` itself whenever the row is an `Element`, so IR text (which pushes through the low tier) and `Builder::element` both keep it. It is cleared where `editor()` hands out its door and in `Editor::items_mut` and `Editor::set`, the mutations that can move or rename an element, as the class cache is. It takes `&mut self` because the builder's design is `&mut`; it reads only.

## A28 applied: dead scopes, draws

- `check` walks the live scopes: a file's top, and the body of an item listed in a live scope (`check_scopes::live_scopes`, an explicit worklist). Range checks still run on every scope, since a pass reads them; the tree's shape, owners and listings are checked on live scopes only. `files_and_scopes` moved to `check_scopes.rs` (check.rs passed 500 lines).
- `Feature::DistDyn(Dist)`, the 21st group, appended last. A draw's distribution counts `DistDyn` in a dynamic root, `Dist` in a static one; `DrawWhen` and `DrawPer` stay one feature each (no simulator was found to differ by form there). The generator picks a distribution from the form's features.

## A28 applied: body forms (R51)

- The forms are a second cache beside the class table (`Design::forms`, an `FxMap` from a body's `ExprId` to bits), computed on first use by an explicit worklist: every listed root that is no body seeds the calls its tree makes with its own form, and each (definition, form) is expanded once. One helper, `Design::derived_changed`, clears both caches at every site that cleared the classes. `root_form` and the feature count look a body up by its expression id; a body id also used by another field would be read as a body (no builder makes one).
- The form logic (`Form`, `root_form`, `form_of`, `form_feature`, the forms computation) moved to `form.rs`, re-exported from `feature`; feature.rs passed 500 lines.
- `Func(Ddt)` and `Func(Idt)` are no longer unreachable: an uncalled body that holds one counts it static.
- The generator builds a body from operators the set allows in both forms (`Gen::both_forms`), since its callers, and so its forms, come later.
- Finding, reported to the lead: inlining can change a root's form when it drops an argument of class `Temp` or above (an extra argument, or one for a formal the body never reads): the caller was dynamic only through that argument. The inlining property excludes those inlinings and counts them; inlining that drops nothing adds no feature. Separately, the generator calls user functions with an argument count unequal to the formals, and `check` does not refuse it.

## A28: the generator setting

- `gen::Params { Untyped, TypedOnly }` and `generate_with(seed, features, size, params)`; `generate` is `generate_with(…, Params::Untyped)`, so no caller changes. The setting reaches the row generators as `Gen::params`. The kernel's rows draw no untyped parameter list (a subcircuit's head parameters are formals, typed by the row), so the setting's effect is the circuit owner's element and model generators; they honour it in their files.


## Amendments A27 and A28, run owner (R49)

- "From here X is v" has one form, the state item: a `FromHere` `OptionRow` for an option, and a `FromHere` `Env` for the temperature or the nominal temperature (a script's `option tnom=`). `check` refuses an `Alter` of `TargetKind::Temp` or `Option` (`FieldDomain`), and `Builder::alter` asserts the same. Both kinds stay for loop and DC-axis targets; the generator never picks them for an alter. `RunExtra::AlterTemp` and `AlterOption` are removed. Rename: an alter of the temperature is now `Env` (`StateFromHere`, `EnvMidRun`); an alter of an option is now `OptionRow` (`StateFromHere`, `OptionMidRun`).
- `Reset { restores: ResetSet }` is a new run item: `ItemKind::Reset`, appended after `Elaborate`, its kernel arms written by the run owner on the lead's go-ahead, as for `Elaborate`. Its row is in `reset.rs`. `ResetSet` is a `u8` set of `Restorable { Values, Options, Temp, Tnom, Saves }`, never empty (checked). Text: `reset <class> …` in declaration order. One feature per class (`ResetValues` … `ResetSaves`). The row states what "earlier" means: run order, includes in place, and inside a loop body every earlier pass's items too. There is no class for parameters: no simulator restores them (R18).
- `Save.holds: Applies` (`Save` 12 → 16 bytes). The builder's `save` takes it, `WholeRun` for a card. Text: `from_here` after `analysis=`. Features `SaveFromHere`, `SaveWholeRun`, and `SaveMidRun` for a `FromHere` save after a run item. A `WholeRun` save there sets `WholeRunMidRun`, as every `WholeRun` state item does, so a `.save` card after an analysis now sets it too.
- `Loop.name`'s doc: the loop's own name in its dialect (Spectre's or VACASK's statement name, ngspice's `foreach` variable).
- Oracle: the feature table with a neighbour for each new member, three new `check` rows, the IR text round trip, and builder asserts. The tests were not committed red before the bodies. Instead, with the bodies stubbed out (no reset features, no `from_here` text, a save's mode ignored by the listed-order walk), the feature table, the text test and the walk test each failed on exactly the stubbed members.
- Reach over 4,000 full-set seeds: every new member is in 1,868 seeds or more. The scaling gate (`throughput -- scaling`) passes: build ratio 4.20 at 4× items, bound 8.

## A28: the definition cycle

- Withdrawn: A11's "what the worklist cannot order is `Circuit`". A definition on or behind a cycle rises to the fixpoint of its direct class and its reads' classes (each class rises at most twice). The cycle itself is `DefinitionCycle`, reported before `ProbeContext` in `check`'s last pass, on an otherwise valid design, since the graph reads definitions through their rows.
- The cycle test is two worklists over the read graph: a definition that neither settles is on a cycle, or between two (reported too; no generated or corpus design has one). `class.rs`'s graph, `kahn` and `order` are shared by the class table and the cycle test.

## `CaseSensitiveNames` per namespace (ruled)

- Replaces the pool-wide `case_clash`. `case::case_clashes` returns the live items that bring a second spelling; `features_used` sets the feature when there is one, and `check_legal` reports it at each such item, so the two still agree. A net clash against a global net is reported at the item naming the local net (the global's `Global` item is the first spelling's).
- Nets are what the kernel can name without the owners' rows beyond `Element.terminals`: element terminals, subcircuit ports (in the body), global nets. A net named only in a probe or an analysis (`v(A)` beside a terminal `a`) is not yet in the namespace: no reader has produced one, and adding it means walking every probe.

## Round 7b doors 2 and 3

- `items_mut` stays public; its doc says what it does not maintain (the index), and `StaleDefinition` makes a bypass fail. `remove_item` removes only the item's own definitions from the index: definitions inside its dead body are unreachable by any lookup from a live scope, so `check` (live scopes only) leaves them. `remove_item` scans the definitions table, O(definitions) per call; a pass removing many items pays that per item (a per-item index of definitions, when one does).
- `move_item` re-parents the item's body scopes and asserts both scopes are in one file (a scope's `file` is not rewritten).
- `rename_def` leaves bound references' names to the pass (else `RefTarget`).


## Run features per item kind: state modes (ng-run's W1303)

- `StateFromHere`/`StateWholeRun` are now split into `{Option,Env,NodeSet}{FromHere,WholeRun}`, and `WholeRunMidRun` into `{Option,Env,NodeSet,Save}WholeRunMidRun`. A dialect then lists exactly the kinds it spells from a given place: ngspice spells a from-here option and temperature, but not a from-here node condition. Each kind's four mode features are one `option::Modes` constant (`OPTION_MODES`, `ENV_MODES`, `NODE_SET_MODES`, `SAVE_MODES`), read by the row features, the listed-order walk and `state_gen::gen_holds`.

## A32 applied (the element-name door)

- `map_element_uses` takes the item's listing scope (`(scope, item, f)`, changed from the first proposal before any body): finding it per item would scan every list.
- Three phases per item: the row's paths are copied (the row's walk holds `&mut Design`), the pass answers each segment with the design readable (`walk_path`), and a second walk stores the names in the same order. A segment after `NONE` is still offered to the pass with `scope: NONE`, so it can note the reference.
- The drift property A32 asks for ("every use listed resolves, every resolved reference is listed") needs `check` to resolve element references, which it does not today; until the owners' rows land, the tests pin the kernel's `Expr::I` part and the path walk by hand.

## The class of a call (ruled)

- `class::class_nodes` is the one walk that decides which rows a tree's class reads; `tree_class`, `tree_class_with` (the generator's table), the definition graph, the feature count, the forms and `check`'s last pass go through it. `formals_read` decides, per call, which arguments' formals the body reads (a walk of the body per call node; no cache: bodies are small and the classes are cached above it). `node_class` is gone (no caller left).
- The generator no longer tracks a class node by node: it computes the finished tree's class with `tree_class_with` over its own definition table, which is exact by construction and O(tree).
- The inlining property no longer excludes inlinings that drop a dynamic argument.

## Round 7b: check's scaling, the A33 hook, strings and lists

- The scaling gate failed at cdc69867. Check went from 0.115 s to 1.201 s at 4x items, a ratio of 10.49 against the bound of 8. The cause was `class::formals_read`: it built an `ExprWalk`, a table sized by the design, once per user call node. `element_uses`' `current_reads` built one per item. Both now walk with `class::tree_nodes`, whose seen set grows with the tree only. The rule: a walk that a row or a node may call builds no table sized by the design.
- `Gen::typed_params` is a flag on the existing leaf, not a new draw. The read keeps its class, and the wrapper adds none.
- Strings and lists by type are root tests, by the ruling's words ("a string as the value"). A ternary of two strings is not a `StringValue`. The typed-field invariant also covers `Handle::Exprs` roots. Both owners stated their vector fields are lists of scalar roots, so a list root there is as wrong as in an `Expr` field.
- Exceptions are a row method (`RowFacts::string_roots`), not a kernel table of field names. The kernel does not know an owner's fields, and a row's exception holds wherever it is checked. `Param`'s value is the kernel's own exemption, by kind.

## R62/A37: a default's two rules, and a probe in an unread argument

- "Outside its subcircuit" is a definition whose scope is not the subcircuit's body: a formal, or a local of the body, is inside. A sibling by name is an unbound read whose name is one of the head's formals.
- The two new members shifted `random_subsets_restrict_the_generator`'s coin flips and exposed a generator bug that predates them. A dynamic tree's one probe could land in a bound call's argument whose formal the body never reads. Since the ruling on a call's class, that argument adds nothing, so the tree was Constant while its operators had been drawn in the dynamic form (subset 2, seed 12: `@g43($temp != …) << ""` used `Binary(Shl)` outside its set). The fix: a frame records which operands the class skips (`class::formals_read`), and a leaf drawn under one never pays the tree's probe.

