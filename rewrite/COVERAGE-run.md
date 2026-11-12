# Coverage: the run half of the IR against the inventories

One line per row of `INV-constructs.md` §G, §H, §I, §J, §K and of `INV-semantics.md` categories 4,
8, 9, 10, 12, 13 (plus rows 5.13, 5.20, 5.21): `<row> → <field | variant | feature | Opaque
class>`, or `gap: <why>`. Types are in `crates/ir/src/{analysis, dc_ac, tran, periodic, sweep,
plan, option, option_key, probe, output, measure, stats, build_run}.rs`. "Private" means
`Opaque` of class `OpaqueClass::Private` naming its dialect; "reader rule" means the reader
states its simulator's meaning in the named fields and nothing new is needed; "RUN" means a
result-side fact (backend, harness), outside the netlist IR.

## INV-constructs §G (analyses)

- G1 → `AnalysisKind::Op`; HSPICE `.OP format time` / timed form → `AnalysisKind::TimedOp`; Xyce `.op` beside another analysis: reader rule (keeps what Xyce runs), `Extra::SeveralAnalyses`.
- G2 → `AnalysisKind::Dc` (a VACASK `sweep` + `op` is a `Loop` around an `Op`: `RunExtra::LoopOverOp`, R20), `dc_ac::Dc { axes }`, `DcAxis { target: TargetKind::SourceDc, sweep: SweepKind::Lin }`; HSPICE `START= STOP= STEP=`: spelling.
- G3 → `SweepKind::{Dec, Oct, List}` on a `DcAxis`; the sweep form class `RunExtra::SweepDc*` (A24); ngspice `foreach` lowering is a pass.
- G4 → `plan::Target` / `TargetKind::{Temp, Param, SourceDc, ElementValue, DeviceParam, ModelParam}`; Xyce's "bare name, parameter wins": reader rule; `Extra::DcAxis*`.
- G5 → `AnalysisKind::Ac`, `dc_ac::Ac { sweep }`; Spectre `lin=`/VACASK `points` count steps: reader adds one (`SweepSpec::points` = values); Spectre AC over `dev=/param=` at one `freq=` → `Loop` over the target around a one-point `Ac`.
- G6 → `tran::Tran { stop, record_start, print_step, initial_step, uic, startup, segments, ceiling: StepCeiling }`; Xyce `NOOP` → `uic`; Spectre `skipdc=yes` → `uic`; VACASK `ic=`/`nodeset=` on the line → `NodeSet { next_only: true }`.
- G7 → `OptionKey::MaxStep` (HSPICE `delmax`).
- G8 → `AnalysisKind::Noise`, `dc_ac::Noise { output: Probe, source, sweep, points_per_summary, ac_sweep, input_ref }`; Xyce integrated noise to stdout: RUN.
- G9 → `AnalysisKind::Tf`, `dc_ac::Tf`; VACASK `dcxf` → `AnalysisKind::DcXf`; Rin/Rout availability: RUN.
- G10 → `AnalysisKind::Sens`, `dc_ac::Sens`; Spectre `sens (…) to (…) for (…)`: Private (gap G-SENS below).
- G11 → `AnalysisKind::SensAc`, `dc_ac::SensAc`.
- G12 → Private (Xyce `.SENS`: objective functions over listed parameters; gap G-SENS).
- G13 → `AnalysisKind::Pz`, `dc_ac::Pz { input: PzInput, transfer, find, in_p, in_n, out_p, out_n, source }`; HSPICE/Spectre source form → `PzInput::Source`, `Extra::PzSourceInput`.
- G14 → `AnalysisKind::Four`, `tran::Four { fundamental, harmonics, periods, grid, outputs }`; LTspice's own `.fourier`: same row (phase convention: RUN, see H16).
- G15 → `AnalysisKind::Sp`, `dc_ac::Sp { sweep, ports, noise }`; Xyce/HSPICE `.LIN` → `Sp` (ports are the circuit owner's RF port elements).
- G16 → `AnalysisKind::Hb`, `periodic::Hb { tones, harmonics, intermod, truncate }`; Xyce `HBINT NUMFREQ` read into `harmonics`; `HBOSC`: Private.
- G17 → `LoopKind::MonteCarlo`, `plan::MonteCarlo { runs, seed, samples, variations, sampler, nominal }`; HSPICE `sweep monte=` → same; Xyce `.SAMPLING` (gap G-SAMPLING).
- G18 → typed: `pss pac pnoise pxf pstb stb xf sweep(Loop) montecarlo(Loop) sens(Private)`; Private: `acmatch dcmatch hbnoise qpnoise pdisto hbstb envlp hbsp hbac hbxf qpac qpsp qpss qpxf psp tdr` (one simulator, parameters not typed yet).
- G19 → `dcinc` → `AnalysisKind::DcInc`; `dcxf` → `DcXf`; `acxf` → `Xf`; `acstb` → `Stb`; `acsp` → `Sp`; `hb` → `Hb`; `pss` → `Pss`; `pac` → `Pac`; `pnoise` → `Pnoise`; `trannoise` → `Tran::noise_*`; `mc` → `LoopKind::MonteCarlo`; `hbac`, `hbnoise` → `Pac`/`Pnoise` after an `Hb` (by order).
- G20 → `.trannoise` → `Tran::noise_*`; `.op` timed → `TimedOp`; `.sn` → `Pss { Driven }`; `.snac` → `Pac`; `.lstb` → `Stb`; Private: `.snnoise .snxf .ptdnoise .sample` (HSPICE only, untyped).
- G21 → Private: `.mpde .mor .pce .embeddedsampling .sampling .result .rol .rol_dc .rol_obj` (Xyce only).
- G22 → Private: LTspice `.fra` (one simulator).
- G23 → ngspice `.pss` → `Pss { mode: Autonomous }`; `.disto` → `AnalysisKind::Disto`, `dc_ac::Disto`.
- G24 → `LoopKind::Sweep`, `plan::SweepLoop { target, sweep }`, nesting by scope (first `.step` innermost: reader rule); LTspice end/sort → `SweepEnd::OnStop` + reader rule; cap 4096 points: reader's refusal; `Extra::{LoopParam, LoopTemp, …}`.
- G25 → `SweepKind::List` on a `SweepLoop`; HSPICE `.temp t1 t2 …` → `Loop` over `Target::TEMP`.
- G26 → `plan::Alter` items in order + `Loop`s; `LoopKind::Table` for HSPICE `.alter` blocks of parameters, Xyce `.DATA`, LTspice `table()`; `Extra::{AlterParam, …, LoopTable}`.
- G27 → `Analysis::name` (`RunExtra::AnalysisNamed`, A16), `Loop::name`.
- G28 → the nodes become typed fields (`Noise::output`, `Pz::out_p`/`out_n`, `Pss::osc_p`/`osc_n`).
- G29 → `Loop` body scope holding the inner run items (any depth up to `LOOP_DEPTH_MAX`).
- G30 → item order in the scope; `Extra::SeveralAnalyses`.
- G31 → writer refusals (W0040, W0031) keyed by the `Extra::Sweep*` features.
- G32 → `AnalysisKind` (24 members); `SpectreSweep`/`SpectreMonteCarlo` → `LoopKind::{Sweep, MonteCarlo}`; `TranNoise` → `Tran::noise_*`; `Fourier` → `Four`.
- G33 → `periodic::Pss { fundamental, tstab, points, harmonics, osc_p, osc_n, uic, sc_iter, steady_coeff }` (A16).
- G34 → `Sp::noise` (`Extra::SpNoise`).
- G35 → `Disto`; HSPICE `.DISTO Rload`: Private (another quantity).
- G36 → `Env::temp` (expression allowed); a list → `Loop` over `Target::TEMP`; ngspice's one-temperature rule: reader rule (4.02).
- G37 → `AnalysisKind::Net`, `dc_ac::Net { output, source, rin, rout }`.
- G38 → `DcAxis::target` kinds `SourceDc`, `ElementValue`, `Temp`, `Param`; nested axes.
- G39 → `Tran::startup`, `Tran::uic`; LTspice `steady`, `nodiscard`, `step`: gap G-LTTRAN.
- G40 → `TargetKind::{Temp, SourceDc, ModelParam, Param}`; `.step param temp` → `Temp` (reader rule).
- G41 → `SweepKind::List` on `Ac`/`Noise` sweeps; HSPICE `.ac poi` → `List`.
- G42 → `AnalysisKind::TimedOp`, `dc_ac::TimedOp { times }`.
- G43 → `Sp` (`noisecalc=1` → `Sp::noise`); `gdcalc`: gap G-LTTRAN.
- G44 → `.hb` → `Hb`; `.hbac` → `Pac`; `.hbnoise` → `Pnoise`; `.hbxf` → `Pxf`; `.hbosc` → `Pss { Autonomous }`; `.hblin`: Private.
- G45 → `Loop` around the one analysis (`Extra::LoopScoped`); `POI` → `SweepKind::List`; `DATA=` → `LoopKind::Table`; `MONTE=` → `LoopKind::MonteCarlo`.
- G46 → `Alter` items (parameter, temp, option changes) or `LoopKind::Table`; re-stated element cards: gap G-ALTERCARD.
- G47 → gap G-FILEVALUES (sweep values from a file).
- G48 → `Four::harmonics`, `Four::periods` (`Extra::FourPeriods`).
- G49 → `Dc::axes` (three axes), `SweepKind::{Dec, Oct, List}`.
- G50 → gap G-LTTRAN (`convreport`, `savestate`/`loadstate`: LTspice only; Private until typed).
- G51 → `Hb::tones` (several), `Hb::intermod`, `Hb::truncate`; `selectharms`, `tahb`: Private Xyce options.
- G52 → gap G-SAMPLING (Xyce `.SAMPLING`, `.EMBEDDEDSAMPLING`, `.PCE`: Private).
- G53 → `LoopKind::Table` around the analysis.
- G54 → `Tran::ceiling` (`StepCeiling::Given`) = the schedule's smallest nonzero step (reader rule, 8.25); the schedule itself: gap G-SCHEDULE.
- G55 → `AnalysisKind::Tran` (spelling).
- G56 → `TargetKind::{DeviceParam, Temp}` with `Target::path`.
- G57 → Private (Xyce optimisation).
- G58 → Private (Xyce `*_CONT` modes, FFT measures).
- G59 → `Hb { tones, harmonics, truncate, intermod }`; `store=`/`nodeset="slot"`: gap G-SLOTS.
- G60 → `AnalysisKind::DcInc`; `writeop`: gap G-SLOTS; `nodeset=[…]` → `NodeSet { next_only }`.
- G61 → `Sp::ports`; `Stb::probe`.
- G62 → `TargetKind::{ModelParam, DeviceParam, Option, Param}`; `__topinst__` → `Param`; `continuation=1`: gap G-CONTINUATION.
- G63 → `NodeSet::reference` (`Extra::IcDifferential`).
- G64 → `LoopKind::MonteCarlo` (`Loop::name`).
- G65 → `DcAxis` with `TargetKind::{Param, ModelParam, DeviceParam}`, `SweepKind::{Lin, Dec, Log, List}`.
- G66 → `Loop` over the target around `Ac`; `SweepBounds::CenterSpan`; `values=` → `List`; tran without `step` → `print_step` `NONE`; `tran ic=` mode: gap G-SPECTREIC; `noise oprobe=`: gap G-OPROBE.
- G67 → `periodic::Hb` (ranges: tones/harmonics as expression lists).
- G68 → `periodic::Pss`.
- G69 → `periodic::{Pac, Pnoise}` with the large-signal analysis found by order (decision R5), `FreqAxis`, `sidebands`, `relharm`.
- G70 → `periodic::Stb`, `dc_ac::Xf`.
- G71 → `DcInc`, `DcXf` and `Sp` stay their own kinds (one node per meaning); the normalisations are passes.
- G72 → `Alter` items in scope order + `Loop` (nesting ≤ `LOOP_DEPTH_MAX` = 8); `AlterKind` → `TargetKind`; alter scope → `TargetKind::SubcktParam` / `Target::path`; normalisations N1–N5: passes.
- G73 → `LoopKind::MonteCarlo` with its inner analyses in the body scope (full rows, own names).

## INV-constructs §H (outputs)

- H1 → `output::Save { analyses: AnalysisKinds, probes }`; HSPICE `.probe kind …` → one kind; VACASK `save` → op, dc and tran (A22); `.option post` → the save set (reader rule); Xyce `.SAVE`: Private (state file).
- H2 → `output::Print { analysis, view, probes }`; Xyce-only kinds `homotopy es pce mpde* ac_ic tranadjoint hb_ic hb_startup`: Private; `PRECISION=`/formats: gap G-PRINTFMT.
- H3 → `probe::Probe` kinds `V, Vdiff, I, Param, Power, Expr, AllV, AllI, …`; `V(*)` → `AllV`; Xyce `n(…)` → `Param`; lead currents → `ITerminal`; Spectre `dev:p`/`inst:index` → `ITerminal`; VACASK `default`/`full` → categories (reader rule).
- H4 → `output::Measure` + `measure::{MeasureTrigTarg, MeasureWhen, MeasureFind, MeasureDeriv, MeasureStat, MeasureParam}`; Xyce FFT measures: Private.
- H5 → `Measure` items / `Four` (the script lines are the deck's cards: reader rule).
- H6 → gap G-FFT (Xyce/HSPICE `.fft`: Private).
- H7 → `Opaque` of class `Inert`.
- H8 → Private (LTspice state files).
- H9 → RUN (result naming).
- H10 → `PrintView::HbTime` (`Extra::PrintHbTime`).
- H11 → Private (HSPICE raw output words); `.net` → `AnalysisKind::Net`.
- H12 → `Save` with `ProbeKind::{I, ITerminal, Vdiff, Power, AllI}`; the 0 V sources are the writer's.
- H13 → `ProbeKind::Expr`.
- H14 (A15 R2: `MIN_AT`/`MAX_AT` → `Stat::{MinAt, MaxAt}`, `TO=` on a crossing → `Event::until`, the default crossing → `Event::skip_initial`) → `MeasureKind::{TrigTarg, When, Find, Stat, Param, Deriv}`, `measure::Stat::{Avg, Min, Max, Pp, Rms, Integ}`; one failing measure fails alone: RUN.
- H15 → `AnalysisKind::Net`.
- H16 → `AnalysisKind::Four`; LTspice's +90° phase report: RUN (result convention).
- H17 → gap G-FFT.
- H18 → Private (HSPICE `.power`); a pass may lower it to `Measure` rows with `Stat` over a `Power` probe.
- H19 → Private (HSPICE checks).
- H20 → gap G-FILEVALUES (`.load file=`: the reader inlines the cards it reads, as `NodeSet` rows).
- H21 → `Measure` expressions read earlier measures by name (needs kernel request K2: `DefKind::Measure`).
- H22 → gap G-PRINTFMT.
- H23 → `ProbeKind::{ITerminal, Power, Param}`; Y-device paths `{YTRANSLINE!ERIC:len}`: gap G-YNAMES.
- H24 → `.INITCOND` → `NodeSet { kind: Device }`; `.DCOP input=/output=`, `.SAVE TYPE=IC`: Private (state files).
- H25 → gap G-PORTOUT (VACASK `yin zin yout zout opdefault opfull`).
- H26 → `r1:1` → `ITerminal`; `r1:oppoint`/`r1:all`: gap G-PORTOUT-like, category of one device: `ProbeKind::Param` per quantity (reader expands only when its list is known; else Private).
- H27 → `probe::Probe { kind, part, terminal, a, b, expr }`; terminal index in the element kind's order; unspellable output dropped with a note: writer rule.
- H28 → `measure::Event { kind, edge, last, signal, value, count, delay }` and the six detail rows; ngspice semantics: the reference.
- H29 → `tran::Four { fundamental, outputs, harmonics, grid, periods }`.

## INV-constructs §I (options and state)

- I1 → `option::OptionRow` items (one per option); Spectre's statement name: dropped (spelling).
- I2 → `option_key::OptionKey` (49 members); `temp`/`tnom` → `Env`; `savecurrents` → `Save` of `AllI`; ngspice-only keys with no typed meaning → `OptionKey::Private` (listed in `option_key.rs`'s header).
- I3 → `DEVICE TEMP/TNOM` → `Env`; `DEVICE GMIN` → `Gmin`; `PARSER SCALE` → `Scale`; `TIMEINT METHOD` → `MethodTrap`/`MethodGear`; `TIMEINT MAXORD` → `MaxOrd`; `TIMEINT RELTOL` → Private (LTE only, 10.01); `HBINT NUMFREQ` → `Hb::harmonics`.
- I4 → `RelTol, VnTol, AbsTol, ChgTol, Itl1, Scale, Gmin, PivRel, PivTol` (each key's doc), `temp`/`tnom` → `Env`.
- I5 → each key's doc (`tran_method`, `tran_maxord`, `op_itl`, `tran_itl`, `homotopy_*`, `op_skipinitial`); `gshunt` → `Gshunt` (reciprocal of `Rshunt`: a pass); cosmetic keys → `OptionClass::Output` or Private; `rawfile`: Private.
- I6 → writer refusal per key: gap K1 (`Feature::Option(OptionKey)` requested).
- I7 → `Env::temp` (`holds: WholeRun` for a deck-global `.temp`, A14); script `set temp=`/`option temp=` between runs → `Alter` of `Target::TEMP`.
- I8 → `Env::tnom` (always present, reader fills its default).
- I9 → `NodeSet { kind: Ic, node, value }`; expression values allowed.
- I10 → `NodeSet { kind: Nodeset }`; VACASK per-analysis → `next_only`.
- I11 → `Tran::uic`.
- I12 → `OptionKey::Scale`.
- I13 → `OptionKey::{Rshunt, Gshunt, GminDc, Gmin}`.
- I14 → `OptionKey::{MethodTrap, MethodGear, MethodEuler, MaxOrd}`.
- I15 → `OptionRow { key: Private, dialect: "ngspice", private: "ngbehavior" }` (IR-RUN §3).
- I16 → `Save` of `ProbeKind::AllI` (reader rule).
- I17 → `OptionKey::Search` (class `Path`).
- I18 → `OptionKey::Private` with its dialect.
- I19 → `OptionKey::NFreqs` (A16; `Four::harmonics` absent reads it), `OptionKey::FourGridSize`.
- I20 → `NodeSet { kind: Nodeset, node: empty }` (`Extra::NodesetAll`).
- I21 → keys where shared (`AbsTol VnTol RelTol ChgTol TrTol Gmin MethodTrap/Gear Itl1 Itl2 Itl4 GminSteps SrcSteps MaxStep Cshunt`); the rest Private (`ptrantau solver plot* gfloat numdgt measdgt logopinfo …`); `gshunt` → `Gshunt`; `temp`/`tnom` → `Env`.
- I22 → `GminDc`, `GminTran`, `Gshunt`, `Search`, `Acct`; `POST`/`PROBE` → the save set (reader rule); the rest Private.
- I23 → gap G-KEEPNODE (output-only; becomes a `Save` of `V` probes when read, else Private).
- I24 → mapped keys per I3; every other package option Private (`LOCA`, `minres`, `mincap` change the solution: Private, 10.07).
- I25 → `NodeSet` in a `Subckt` scope (per instance), `NodeSet::node` path; `$G_` globals: the kernel's `Global`.

## INV-constructs §J (statistics)

- J1 → `stats::Statistics { process, mismatch, process_correlations, mismatch_correlations, truncate, process_truncate, mismatch_truncate }`.
- J2 → `stats::Vary { param, dist: VaryDist::{Gauss, Unif, LogNormal}, spread, percent, mean }`.
- J3 → `stats::Correlation { a, b, cc }` (n > 2: the pairs); `dev=` correlation: gap G-DEVCORR.
- J4 → the three `*truncate` fields.
- J5 → `Statistics::process`/`mismatch` + `MonteCarlo::variations`; VACASK per call site → the kernel's `DrawPer::CallSite`.
- J6 → the kernel's `Expr::Draw` with `DrawWhen`/`DrawPer` (A2).
- J7 → a pass (table ↔ call); the `rack_*` spelling is the Spectre writer's.
- J8 → a pass / the VACASK writer.
- J9 → a pass.
- J10 → the reader's refusal (W0019); `Vary::param` is a `Ref` the linker binds.
- J11 → gap G-VARIATION (HSPICE `.variation`: Private until typed; maps onto `Statistics`).
- J12 → Private (raw dot words), or `LoopKind::MonteCarlo` where typed.
- J13 → `MonteCarlo::seed`, `OptionKey::Seed`.
- J14 → writer refusal (`Feature::Dist`, `Feature::DrawWhen`).
- J15 → gap G-VARIATION.
- J16 → `DrawWhen::Sampling` (kernel).
- J17 → call form: `Expr::Draw`; table form: `Statistics`/`Vary`.
- J18 → `Vary { param, dist, mean, spread }`; correlation group → `Correlation` rows; Xyce gamma: refused.

## INV-constructs §K (control and scripting)

- K1 → read into rows: analyses → `Analysis`; `pre_osdi` → kernel `Load`; `alterparam`+`reset` → `Alter`; display lines → `Opaque` `Inert`; circuit-changing unread lines → refusal; the block itself is `Opaque` `Script` when unread.
- K2 → `Opaque` `Inert`.
- K3 → `Opaque` `Inert`.
- K4 → `alterparam` → `Alter` (`Param`, `SubcktParam`); `alter` → `Alter` (`DeviceParam`/`ElementValue`); `altermod` → `Alter` (`ModelParam`); `option` → `Alter` (`TargetKind::Option`) or `OptionRow` mid-run; `set temp=` → `Alter` of `Target::TEMP`; `let @dev[p]` → `Alter` `DeviceParam`; `source`: refusal.
- K5 → unrolled by the reader into items (bounded); `foreach` over values → `LoopKind::Sweep` with `LoopEnd::KeepLast`; data-dependent: `Opaque` `Script`.
- K6 → listed order kept (R25); ngspice's state cards are `holds: WholeRun` (R24).
- K7 → writer lowering of `Loop`/`Alter` (a pass or the ngspice writer).
- K8 → writer rule.
- K9 → `options` → `OptionRow`; `sweep` → `Loop`; `analysis` → `Analysis`; `save` → `Save`; `alter` → `Alter`; `elaborate` → `ItemKind::Elaborate` (A12 R1); `var`: `Param`; `abort`/`print`/`postprocess` → `Opaque` `Script`.
- K10 → `Opaque` `Script`.
- K11 → `Alter` (`Param` for `__topinst__`, `DeviceParam` for an instance, expression values allowed).
- K12 → `alter` → `Alter`; `altergroup`: gap G-ALTERCARD; `set` → `OptionRow`; `assert check checklimit info shell`: `Opaque` `Inert`; `paramset` → `LoopKind::Table`; `paramtest`: Private.
- K13 → `LoopKind::Table` (parameters) or `Alter`s followed by the repeated analyses; element re-statements: gap G-ALTERCARD.
- K14 → `LoopKind::Table`, `TableLoop { columns, values }`.
- K15 → `LoopKind::Table` (the LTspice writer spells it as `table()`; its reader reads it back as a table only when the idiom is exact).
- K16 → order in the scope is the plan (no epochs table); the invariants are `check`'s.
- K17 → `ItemKind::Elaborate`, `plan::Elaborate { circuits }` (A12 R1).
- K18 → `clear saves` → reader rule (union of saves, N3) with a note; `clear options` → `Alter`s back to the defaults (reader rule).
- K19 → `Alter` with `TargetKind::DeviceParam` and a string value (`Extra::AlterString`); a topology flip under `@if`: kernel `If` re-evaluated after the alter (11.08).
- K20 → as K12.

## INV-semantics category 4 (temperature)

- 4.01 → `Env { temp, tnom }`, both always present, reader fills its default.
- 4.02 → `Env::temp` + `Loop` over `Target::TEMP`; ngspice keeps 27 °C and notes (reader rule).
- 4.03 → `Env::temp` is an expression; Xyce `device temp=` → `Env`.
- 4.04 → `TargetKind::Temp` (`Extra::{LoopTemp, AlterTemp, DcAxisTemp}`).
- 4.05 → circuit owner (instance `temp`/`dtemp` fields); not run state.
- 4.06 → RUN (comparison tolerance).

## INV-semantics category 8 (analyses)

- 8.01 → `Tran` fields named, `NONE` ≠ 0; `StepCeiling::Rule(StepCap)` for the default ceiling; `segments` for HSPICE.
- 8.02 → `Tran::ceiling` (`StepCeiling`), `StepCap`, `OptionKey::MaxStep`; VACASK `tran_rmax`: Private.
- 8.03 → `Tran::print_step` `NONE` (`Extra::TranSimulatorStep`); a non-positive span: reader's refusal.
- 8.04 → item order is the source's listed order (A14 R25; the row's "reverse order" is wrong: ngspice runs by type rank); independent analyses' result order is a run fact (S12); `RunExtra::SeveralAnalyses`.
- 8.05 → `TargetKind` (typed, never a string).
- 8.06 → `SweepSpec { kind, points = values }`; Spectre "no step parameter" spacing: reader picks `Lin`/`Log`; downward Spectre sweep: writer rule.
- 8.07 → `SweepEnd::{Grid, OnStop}`; ngspice `dec` "rescales" and the HSPICE rule: gap G-LOGEND.
- 8.08 → `SweepKind::List`, `SweepBounds::CenterSpan`, `Loop` around a one-point `Ac`.
- 8.09 → `Dc::restart`; repeated-addition rounding: RUN.
- 8.10 → `Noise { output: Probe, source, sweep, points_per_summary, ac_sweep, input_ref }`.
- 8.11 → `AnalysisKind::TimedOp`.
- 8.12 → `.sens` → `Sens`/`SensAc` (ngspice, HSPICE); Xyce and Spectre forms Private (gap G-SENS); `.fourier` → `Four`; `.save` → 9.04.
- 8.13 → `Pz` (`PzInput`, `PzTransfer`, `PzFind`), `Tf { output, source }`; result units and signs: RUN.
- 8.14 → `Sp { sweep, ports, noise }`, `Net`; port elements: circuit owner.
- 8.15 → `Hb { tones, harmonics, intermod, truncate: HbTruncate::{Box, Diamond, Hybrid} }`.
- 8.16 → `Pss { mode: PssMode::{Driven, Autonomous}, fundamental, tstab (s), points, harmonics, osc_p, osc_n }`.
- 8.17 → `FreqAxis::{Absolute, Relative}`, `relharm`, `sidebands`.
- 8.18 → `Stb { probe, sweep }`; the result's sign: RUN.
- 8.19 → `Four::harmonics`; `Disto`; HSPICE `.DISTO Rload`: Private; FFT windows: gap G-FFT.
- 8.20 → `Tran::{noise_fmax, noise_fmin, noise_seed}`; `AnalysisKind::DcInc`.
- 8.21 → `NodeSet { kind, applies: IcScope::{Transient, EveryOp}, next_only, reference }`, `Tran::uic`; device `IC=`: circuit owner (`IcMode`); Spectre `ic=` selector: gap G-SPECTREIC; branch-current IC: gap G-BRANCHIC.
- 8.22 → `SweepSpec` values are expressions (may read outer loop targets).
- 8.23 → `Pss::tstab` in seconds (reader multiplies); the rest writer rules.
- 8.24 → every analysis value is an `ExprId`.
- 8.25 → `.TR` → `Tran`; schedule → `Tran::ceiling` (`StepCeiling::Given`) (gap G-SCHEDULE); `.LIN SPARCALC=0` → `Ac`; `.OP` beside another: reader rule.
- 8.26 → RUN (result kind from the analysis item, not the point count).

## INV-semantics category 9 (outputs)

- 9.01 → `Probe::a`/`b` and `NodeSet::node` are name paths; IC scope by the item's scope.
- 9.02 → `ProbeKind::{I, ITerminal}` with the sign pinned "positive into the device"; `Probe::terminal` index.
- 9.03 → categories `AllV`, `AllVTop`, `AllSourceI`, `AllBranchI`, `AllI`, `AllPinI`; the save set of a deck that names none: gap G-DEFAULTSAVE (R29, open until S12).
- 9.04 → `Save` vs `Opaque` Private for state checkpoints; categories are union members.
- 9.05 → `ProbeKind::{AllI, AllPinI}`.
- 9.06 → `ProbeKind::Param` named as the source spelled it: gap G-OPQUANT (canonical quantity ids).
- 9.07 → `Print { analysis, probes }` (separate probes); writer drops with a note.
- 9.08 → `Measure` rows; one result per loop pass (by scope); Xyce `.RESULT`: Private.
- 9.09 → writer rule (element names are names; the type letter is spelling).

## INV-semantics category 10 (options)

- 10.01 → `OptionKey::RelTol` = SPICE's joint knob; Xyce's split tolerances Private; `timeint abstol`: Private.
- 10.02 → `RelTol, AbsTol, VnTol, ChgTol` with every spelling in the key's doc; VACASK `tolscale`: Private (gap G-TOLSCALE: the proposal applies it at read, which is a conversion).
- 10.03 → `MethodTrap`, `MethodGear`, `MethodEuler`, `MaxOrd`; Spectre `errpreset`-dependent defaults: Private (gap G-ERRPRESET).
- 10.04 → `OptionKey::TrTol`; Spectre `lteratio`: Private.
- 10.05 → `Gmin`, `GminDc`, `GminTran`, `Gshunt`, `Rshunt`, `Cshunt`; LTspice `gfloat`: Private.
- 10.06 → `Itl1` (incl. Xyce `nonlin maxstep`), `Itl4`, `MaxStep` (s), `Tran::ceiling` (`StepCeiling::Given`).
- 10.07 → Private (solution-changing, one simulator).
- 10.08 → writer/comparison tables (RUN); the IR holds only what the source set.
- 10.09 → `OptionKey::class()` → `OptionClass::{Equations, Accuracy, Path, Output}`.

## INV-semantics category 12 (statistics)

- 12.01 → kernel `Dist` (draw idiom); `VaryDist` states its spread per member.
- 12.02 → kernel `DrawWhen::{Always, Sampling}`.
- 12.03 → kernel `DrawPer`; `Statistics::process`/`mismatch` draw scopes.
- 12.04 → `LoopKind::MonteCarlo` + `MonteCarlo { runs, samples, seed, sampler }`; inner analyses in its body.
- 12.05 → `MonteCarlo::variations`; `Statistics` scopes; left-out scope nominal.
- 12.06 → `MonteCarlo::seed`, `OptionKey::Seed` (not portable; stated in docs).
- 12.07 → `Correlation`, the `*truncate` fields, `VaryDist::LogNormal`, `Vary::percent`; LHS → `Sampler::LatinHypercube`; gamma, PCE: refused.

## INV-semantics category 13 (control)

- 13.01 → scripts lowered to items by the reader; order of items is the plan; only state changes enter (`Alter`, `OptionRow`, `Loop`).
- 13.02 → `Alter` + `Loop`; data-dependent logic: `Opaque` `Script` (refused at the source).
- 13.03 → `LoopEnd::{Restore, KeepLast}`; alters persist until re-altered (reader emits restores for ngspice `reset`); `altergroup`: gap G-ALTERCARD.
- 13.04 → the loop's body is the set of analyses it wraps; nesting by scope (outer → inner).
- 13.05 → `SweepEnd::OnStop`; LTspice sorting/dedup: reader rule into an explicit `List`.
- 13.06 → `TargetKind::{SourceDc, ElementValue, DeviceParam, Temp}`.
- 13.07 → `LoopKind::Table` (`TableLoop { columns, values }`); inner analyses keep their own rows and names.
- 13.08 → `Measure` in a loop body (one row per step); per-analysis source alters and control loops are writer lowerings.

## INV-semantics rows 5.13, 5.15, 5.20, 5.21, 6.06 and the circuit owner's cross-requests

- 5.13 → circuit owner (three source slots); the run half's part is which slot an analysis reads: `AnalysisKind` fixes it (DC kinds read `dc`, small-signal kinds `ac`, `Tran`/`Hb`/`Pss` the waveform), stated in the circuit owner's source doc.
- 5.20 → gap G-HBDRIVE (circuit owner's G8): the per-tone drive belongs on the source, referring to `Hb::tones` by index.
- 5.15 → `Tran::startup` (IR-RUN §2 chose a field over the reader's PWL lowering; lowering is a pass).
- 6.06 → `OptionKey::Scale` (the fields it multiplies are in its doc; folding is a pass).
- D6 (circuit cross-request) → VACASK's device entry of the tran `ic` list → `NodeSet { kind: NodeSetKind::Device }`.
- D88 → `NodeSet` in a `Subckt` scope, path relative to the body.
- D76 `step(…)` in an LTspice source (circuit G4) → `LoopKind::Table` column with `TargetKind::ElementValue`/`DeviceParam`, one row per step (reader rule).
- D129 Spectre `xfmag` (circuit G13) → gap G-XFMAG: `Xf` reads each source's transfer magnitude; the field is the source's (circuit owner), `Xf` names none.
- 5.21 → circuit owner (`distof1`/`distof2` on the source); `Disto` reads them.

## Gaps and proposals

- G-DEFAULTSAVE (9.03, R29): what a deck saves when it names nothing. Open until the run side (S12); the IR is silent and the target's default applies.

- G-SENS (G10, G12, 8.12): Spectre `sens … to … for …` and Xyce `.SENS` (objective, parameter list, analyses) are Private. Proposal: a `SensSpec` row `{ outputs: ProbeList, params: TargetList (empty = all), of: AnalysisKind }` replacing `Sens`/`SensAc` when a second dialect's form is typed.
- G-SAMPLING (G17, G52): Xyce `.SAMPLING`/`.EMBEDDEDSAMPLING`/`.PCE` are Private. Proposal: `.SAMPLING` reads into `Statistics` (`Vary` with `mean`) + `LoopKind::MonteCarlo`; uniform bounds need `VaryDist::UnifBounds { lo = mean, hi = spread }` or two fields; PCE stays Private.
- G-PSSKNOBS (G33): closed by A16 (`Pss::sc_iter`, `Pss::steady_coeff`).
- G-LTTRAN (G39, G50, G43): LTspice `.tran` `steady`, `nodiscard`, `step`, `convreport`, `savestate`; HSPICE `.lin gdcalc`. Proposal: `Tran` flags when a second target has a meaning for them; until then the statement is Private.
- G-ALTERCARD (G46, K12, K13, 13.03): HSPICE `.alter` blocks that re-state element or model cards, Spectre `altergroup` (defaults first). Proposal: an `Alter` variant whose target is an element or model and whose value is a replacement item (`ItemRef` to an element/model row in a side scope), with a `reset: bool` for altergroup's defaulting.
- G-ELABORATE (K9, K17): closed by A12 R1: `ItemKind::Elaborate { circuits }`, references of `DefKind::Subckt`, the top circuit for every run item after it.
- G-FILEVALUES (G47, H20): values read from a data file (LTspice `file=`, HSPICE `.load`). Proposal: the kit's `Role::Data` files (`INV-constructs.md` L27) and `SweepSpec::List` filled from them at read.
- G-SCHEDULE (G54, 8.25): Xyce's `schedule()` max step. Proposal: a kernel `Func::Schedule` (piecewise-constant in time) in `Tran::ceiling` (`StepCeiling::Given`), which may then read `Time`.
- G-SLOTS (G59, G60): VACASK `store=`/`nodeset="slot"`/`writeop` (named solution slots). Proposal: Private until a second simulator has slots.
- G-CONTINUATION (G62): VACASK `continuation=1` on a sweep. Proposal: `SweepLoop::continuation: bool` beside `Dc::restart` once its meaning is measured.
- G-SPECTREIC (G66, 8.21): Spectre `tran ic=dc|node|dev|all` (which ICs are honoured). Proposal: `Tran::ic_use: IcUse` enum; until then the reader applies precedence and notes.
- G-OPROBE (G66): Spectre `noise oprobe=<component>`. Proposal: `Noise::output` of `ProbeKind::I` on that component (needs the sign rule checked against Spectre).
- G-PRINTFMT (H2, H22): Xyce `FORMAT=`, `FILE=`, `WIDTH=`, `PRECISION=`, `FILTER=`. Proposal: output-only; the reader drops them with a note (they move no number).
- G-FFT (H6, H17, 8.19): `.fft` (HSPICE, Xyce) with windows. Proposal: an `Fft` analysis kind `{ output, start, stop, points, window: Window (named formula + periodic flag), format }` when typed; Private now.
- G-YNAMES (H23): Xyce `Y`-device output paths. Proposal: circuit owner's element naming first.
- G-PORTOUT (H25, H26): VACASK `yin zin yout zout opdefault opfull`, Spectre `r1:oppoint`/`r1:all`. Proposal: `ProbeKind::{PortY, PortZ}` per port and a `DeviceAll` category.
- G-OPQUANT (9.06): device quantities are named as spelled (`gm`, `id`, `i_d`). Proposal: a canonical quantity table per model family (circuit owner's `ModelFamily`), `Probe::b` then holding the canonical name.
- G-LOGEND (8.07): ngspice `dec` "rescales" and HSPICE's `.ac dec` end rule are not established. Proposal: a third `SweepEnd` member once the rule is read from the source.
- G-BRANCHIC (8.21): an initial condition on a branch current. Proposal: `NodeSetKind::BranchIc` with `node` the element path.
- G-DEVCORR (J3): Spectre `correlate dev=[…]` (instance mismatch correlation). Proposal: `Correlation` with element paths; refused today (W0018).
- G-VARIATION (J11, J15): HSPICE `.variation` blocks. Proposal: read into `Statistics` (global → `process`, local → `mismatch`); Private until its argument meanings are cited.
- G-KEEPNODE (I23): LTspice `.keepnode`. Proposal: read into a `Save` of `V` probes.
- G-TOLSCALE, G-ERRPRESET (10.02, 10.03): VACASK `tolscale`, Spectre `errpreset`. Proposal: Private (applying them at read is a conversion; a pass may expand them into the keys they scale).
- G-HBDRIVE (5.20, circuit G8): the per-tone HB drive of a source. Proposal: the circuit owner's `HbDrive { tone (index into `Hb::tones`), harmonic, mag, phase_deg }` side row on the source; `Hb` stays as is.
- G-XFMAG (circuit G13): Spectre `xfmag` on a source. Proposal: a source field (circuit owner); `Xf` unchanged.
- K1 (I6): the per-key writer legality needs `Feature::Option(OptionKey)` (kernel change request).
- K2 (H21, 9.08): measures read earlier measures by name; needs `DefKind::Measure` so the linker binds them (kernel change request).
