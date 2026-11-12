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
- R19. "Analysis kind or none" fields (IR-RUN §6) → `Save { analysis, every_analysis: bool }`; `Print` always names its kind (`.print` requires one).
- R20. Item count of `Extra` → the run owner adds 118 members; with the kernel's 8 the `u8` repr leaves the circuit owner 130. If the two owners' regions together pass 256, `Extra` needs `#[repr(u16)]` (kernel).

## Addendum A6 (one detail door)

- The merge of the two halves did not compile (E0592: both owners defined `Design::detail` and `Editor::set_detail`) → one kernel trait `DetailRow { rows(&Design), rows_mut(&mut Design) }` in `design.rs` beside `ItemRow`, and one `Design::detail`, `Builder::detail`, `Editor::set_detail`, defined once in `design.rs`, `build.rs` and `edit.rs`. `design.rs` stays under 500 code lines, so there is no `detail.rs`.
- Circuit: the circuit-local `DetailRow` trait and its `Design::detail`/`Editor::set_detail`/`Builder::detail` are deleted; `element.rs`'s `detail_rows!` now implements the kernel trait through `design.circuit.<field>`; `wave.rs`'s `WaveRow: DetailRow` names the kernel trait. `DetailKind` and every typed accessor are unchanged.
- Run: `Detail` became `RunDetail: DetailRow { type Kind; const KIND }` (the name says it is the run owner's sub-trait), with `table`/`table_mut` removed; `analysis.rs`'s `details!` implements both traits through `design.run.<field>`; `build_run.rs`'s three typed pushes bound on `RunDetail<Kind = …>`. Its duplicate `Design::detail`/`Editor::set_detail` are deleted.
- `lib.rs` re-exports `DetailRow` from `design` (kernel group) instead of `element`, and `RunDetail` instead of `Detail`; its header names every run re-export.

## Addendum A7 (requests from the run owner)

- `Feature::Option(OptionKey)` comes after `DrawPer` in index order and counts `OptionKey::COUNT` (49); `OptionKey::Private` is one feature, and whether a private option belongs to the writer's own dialect is the writer's check.
- `DefKind::Measure` is appended last (no earlier `DefKind` moves). `Builder::resolve` accepts a `Param`-tag expression bound to a `Measure`; `link` tries `Param`, then `Measure`, for `Param`-tag expressions reached from a `Measure` item's handles; `check`'s `RefTarget` reports a `Param`-tag expression bound to a `Measure` anywhere outside a `Measure` item.
- **Note for the run owner (Implementation phase):** `Builder::measure` (`build_run.rs`) is a defining push: it must call `define(DefKind::Measure, name, scope, item)` for the measure it pushes. Not edited by the kernel.
- `Extra` is `#[repr(u8)]` with 202 members (8 + 76 + 118); its doc says passing 256 needs `#[repr(u16)]` (`Feature::index` is already `u16`).
