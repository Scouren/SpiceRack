//! The catalogue's W0001–W0100 rows: the SPICE-family writer's and the
//! rules both writers share. A block of `catalogue!`'s list (`mod.rs`
//! chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_w_spice`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_w_spice {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            // W0001–W0100: the SPICE-family writer and the rules both writers share.
            /// A statement kept as text from another dialect family: only its own family reads it.
            W0001 Refusal {
                title: "a statement kept as another dialect's text",
                cite: "no clause: the text is the source dialect's (Pandoc's RawBlock rule)",
                explain: "The reader kept this statement as its source dialect's text (P0001, or a construct the reader keeps raw). Only a target of the same family can copy it; translate the statement by hand, or write it for its own family.",
            }
            /// A `^` whose meaning differs between the source and the target (power in one, XOR in the other).
            W0002 Refusal {
                title: "a `^` that means power in one dialect and XOR in the other",
                cite: "ngspice manual §2.8.5 (`^` power); LTspice help (`^` XOR); Xyce RG §2.1.18 (`^` XOR); VACASK Verilog-A operators",
                explain: "The deck's dialect reads `^` as power and the target as XOR, or the reverse, so the same text would compute another value. Spell the power as `**` or `pwr()` in the source.",
            }
            /// A statistical function (`agauss`, `gauss`, `aunif`, `unif`, `limit`) the target has no function for.
            W0003 Refusal {
                title: "a statistical function the target lacks",
                cite: "ngspice manual §2.11.5; LTspice help (`gauss(x)` takes one argument)",
                explain: "The target has no function with this statistical call's meaning (LTspice's `gauss` takes one argument). Write the variation in the target's own Monte Carlo form.",
            }
            /// A Verilog-A load the target loads another way (an OSDI binary where it compiles the source, or a command-line plugin).
            W0004 Refusal {
                title: "a Verilog-A load the target cannot spell",
                cite: "ngspice manual §13 (`pre_osdi`); HSPICE `.hdl`; Xyce RG (`-plugin`); LTspice help (no Verilog-A or OSDI loader)",
                explain: "The target loads compiled models in a way a deck line cannot name (Xyce takes a `-plugin` on its command line), or cannot load one at all (LTspice). Load the model through the simulator's own mechanism.",
            }
            /// A `.lib file` with no section: LTspice reads the whole file; another target reads the card as a section definition.
            W0005 Refusal {
                title: "a `.lib` call with no section",
                cite: "ngspice manual §2.7; LTspice help (`.lib`)",
                explain: "A `.lib file` with no section is LTspice's include; other SPICE targets read `.lib name` as the start of a section. Spell it `.include file`.",
            }
            /// An R, C or L card naming a model the target's element line has no field for.
            W0006 Refusal {
                title: "an R/C/L model reference the target lacks",
                cite: "ngspice manual §3.3.1-§3.3.10; LTspice help (R, C, L)",
                explain: "The element names a `.model` for its value, which the target's R, C or L line has no field for. Write the value on the element.",
            }
            /// A source's RF port (`portnum n z0 Z`) the target has no form for.
            W0007 Refusal {
                title: "an RF port the target lacks",
                cite: "ngspice manual §4.1.11",
                explain: "The source is an ngspice RF port (`portnum`, `z0`), which the target has no source for. Model the port as a source with a series resistor.",
            }
            /// A PWL source's repeat or delay (`r=`, `td=`) the target does not read.
            W0008 Refusal {
                title: "a PWL repeat or delay the target lacks",
                cite: "ngspice manual §4.1.4",
                explain: "The PWL waveform repeats (`r=`) or starts late (`td=`), which the target's PWL does not read. Write out the repeated points.",
            }
            /// A TRNOISE or TRRANDOM waveform (ngspice only).
            W0009 Refusal {
                title: "a TRNOISE or TRRANDOM waveform the target lacks",
                cite: "ngspice manual §4.1.7, §4.1.8",
                explain: "Transient noise sources are ngspice's own. Use the target's noise analysis, or a PWL of sampled noise.",
            }
            /// A Laplace source whose expression has no transfer-function reading.
            W0010 Refusal {
                title: "a Laplace expression that does not read",
                cite: "ngspice manual §12.2.16 (`s_xfer`); HSPICE `LAPLACE`",
                explain: "The Laplace expression is not a proper rational function of `s` with numeric coefficients (a symbolic coefficient, an improper ratio, arithmetic around the call). Write the coefficients as numbers.",
            }
            /// A Laplace source the target has no form for (Xyce).
            W0011 Refusal {
                title: "a Laplace source the target lacks",
                cite: "Xyce RG §6.1.5",
                explain: "The target has no Laplace or transfer-function source. Realize the transfer function as a network of controlled sources.",
            }
            /// A library section on a target that reads `.lib name` as a file (LTspice).
            W0012 Refusal {
                title: "a library section the target reads as a file",
                cite: "LTspice help (`.lib`); ngspice manual §2.7",
                explain: "LTspice reads `.lib name` as a file to include, so it has no library sections. Split the sections into files and include the one wanted.",
            }
            /// A conditional block on a target with no `.if` (Xyce, LTspice).
            W0013 Refusal {
                title: "a conditional block the target lacks",
                cite: "ngspice manual §2.15; Xyce RG Table 6-1",
                explain: "The target has no `.if`. Keep the branch wanted, or select it with a library section.",
            }
            /// A `.param` or a load inside a conditional branch, which a SPICE `.if` may not hold.
            W0014 Refusal {
                title: "a `.param` or load inside a SPICE `.if` branch",
                cite: "ngspice manual §2.15",
                explain: "A SPICE `.if` branch may hold elements and models only. Move the `.param` or load out of the branch, or choose between branches with a ternary parameter.",
            }
            /// An analysis the target has no card for.
            W0015 Refusal {
                title: "an analysis the target lacks",
                cite: "ngspice manual ch.11; HSPICE UG; Xyce RG Table 6-1; LTspice help",
                explain: "The target has no analysis of this kind, or its `.dc` does not sweep what the card names: ngspice's sweeps a source, a resistor or `temp`; LTspice's an independent source or `temp` (a Xyce `.dc` over one parameter that is the deck's only analysis is written for LTspice as `.step param` beside `.op`). Run it on a simulator that has one, sweep a source, step a parameter the device reads, or drop it from the deck.",
            }
            /// A `.step` sweep the target has no card for (it would run
            /// once), or one ngspice's control script has no command for.
            W0016 Refusal {
                title: "a `.step` the target lacks",
                cite: "ngspice manual (no `.step`; `.dc` sweeps); HSPICE `.alter`; Xyce RG p.155",
                explain: "The target has no `.step`, so the deck would run once. Sweep with the target's own form (`.dc` over a parameter, `.alter`, a script loop). ngspice runs a `.step` over the temperature, a parameter the deck's top declares or a top-level R, C, L, V, I as a loop of its `.control` script; a step over anything else (a device or model parameter, a name an included file declares) is refused. LTspice, Spectre and VACASK step the temperature, a parameter and a top-level V or I source's DC value (LTspice and Spectre: a source with no waveform; Spectre also a top-level R, C or L's value); a step over another device or a device's parameter is refused: step a parameter the device reads.",
            }
            /// An option the target has no spelling for.
            W0017 Refusal {
                title: "an option the target lacks",
                cite: "Xyce RG pp.96-104 (`.options <pkg>`)",
                explain: "The target groups options by package and has none of this name. Look up the target's option for the same setting.",
            }
            /// A Spectre `statistics` block a SPICE or VACASK deck cannot hold.
            W0018 Refusal {
                title: "a statistics block the target cannot hold",
                cite: "Spectre Reference 19.1 pp.175-179",
                explain: "The `statistics` block uses a form the target's statistical functions cannot draw (a correlated device list, a truncation under 4 sigma, a percent lognormal, a block inside a subcircuit or branch). Rewrite the block without them.",
            }
            /// A Spectre `vary` of a parameter no `parameters` of its scope declares.
            W0019 Refusal {
                title: "a vary of an undeclared parameter",
                cite: "Spectre Reference 19.1 p.176 (vary names a parameter)",
                explain: "The `vary` names a parameter no `parameters` statement of its scope declares, so there is no nominal value to draw around. Declare the parameter.",
            }
            /// A Spectre `correlate` whose correlation matrix is not positive definite.
            W0020 Refusal {
                title: "a correlation matrix that is not positive definite",
                cite: "Spectre Reference 19.1 p.177 (correlate); the Cholesky factor of the correlation matrix",
                explain: "The correlations given have no Cholesky factor (a coefficient outside [-1, 1], or an inconsistent set), so no draw has them. Check the `cc=` values.",
            }
            /// An element CDL's layout writes as an `* error:` comment: its ngspice
            /// card has no form (the writer's refusal codes), or a Verilog-A load.
            W0021 Note {
                title: "an element written as an error comment",
                cite: "no clause: the xschem layout's own form (`render::spice::cdl`)",
                explain: "The CDL netlist is still written, with this element as an `* error:` comment naming why (the ngspice card's refusal codes, or a Verilog-A load CDL cannot hold). An LVS run sees no device there; fix the element for ngspice first.",
            }
            /// A Verilog-A or OSDI load not written: on LTspice (no OSDI loader),
            /// or inside a subcircuit (the writers load the top's alone).
            W0022 Note {
                title: "a Verilog-A load not written",
                cite: "ngspice manual §13 (`pre_osdi`, a deck-level command)",
                explain: "A load inside a subcircuit is not written on any target but ESPice. An instance of the model then names nothing the simulator knows; load it at the top.",
            }
            /// An output request (`.print`, `.save`, `.probe`) not written.
            W0023 Note {
                title: "an output request not written",
                cite: "LTspice help, Xyce RG §2.1.21, VACASK docs (every vector saved); Spectre Reference 5.0 p.886 (`save` takes nodes and terminal currents only); Xyce RG 7.10 §2.1.30 (`.PREPROCESS ADDRESISTORS` writes a second netlist file; its own run adds no resistor)",
                explain: "The target saves every vector anyway (LTspice, Xyce, VACASK), or has no form for the request: a `.print` output that is no node voltage or device current on Spectre (an expression, `format=`, a device parameter), or no node voltage or source current on VACASK; HSPICE's `.probe` under a noise run. Select the outputs in the target's post-processing. Xyce's `.PREPROCESS ADDRESISTORS` asks for a copy of the netlist with a resistor from each dangling node to ground and runs the deck as it stands, so the card is left out; add the resistors to the deck to have them.",
            }
            /// An option not written: another simulator's own, or one with no effect
            /// on the numbers.
            W0024 Note {
                title: "an option not written",
                cite: "Xyce RG pp.96-104 (`.options <pkg>`); VACASK docs/cmd-options.md",
                explain: "The option is another simulator's own (VACASK's on a SPICE target, Xyce's packaged options on Spectre or VACASK) or only cosmetic, so it is left out. Set the target's own option if one has the same effect.",
            }
            /// A `.control` line that changes no run: not written.
            W0025 Note {
                title: "a control-block line not written",
                cite: "ngspice manual ch.13 (the control language is a script, not a netlist)",
                explain: "Of an ngspice `.control` block the analyses and the top-level parameter changes (`alterparam` then `reset`) are read, each once a pass of the loops around it (a loop or an `if` over constants is unrolled). A line that shows, writes or post-processes results (`let`, `print`, `write`, `plot`, `set filetype=` …) changes no run and is left out, as is an `alterparam` no `reset` follows; it is noted once however many passes run it. (A line that changes the circuit is W0160; a VACASK control block's script lines are W0120.)",
            }
            /// A waveform's arguments past the count the target's form takes.
            W0026 Note {
                title: "a waveform argument past the target's count not written",
                cite: "ngspice manual §4.1.1 (PULSE's NP), §4.1.2 (SIN)",
                explain: "The source's waveform has more arguments than the form written takes (ngspice's PULSE pulse count NP), and the extra ones are left out. Check that the waveform does not depend on them.",
            }
            /// A statement kept as text, copied as written into a target of its own
            /// family (or CDL's layout).
            W0027 Note {
                title: "a statement kept as text, copied as written",
                cite: "no clause: the text is the source dialect's (Pandoc's RawBlock rule)",
                explain: "The reader kept this statement as its source's text (P0001, or a form the reader keeps raw), and the target reads that text, so it is copied unchecked: no capability or `^` check looks inside it.",
            }
            /// A statement the CDL layout (an LVS netlist) does not write.
            W0028 Note {
                title: "a statement the CDL layout does not write",
                cite: "Cadence CDL (no analyses); the xschem layout (`render::spice::cdl`)",
                explain: "CDL is a netlist for LVS: analyses, library sections, conditionals, includes, functions, a subcircuit's models and text kept as the source's (no rule read it) are not written. Netlist the deck for a simulator to keep them.",
            }
            /// What structural Verilog has no form for, left out: a primitive element or kept text (a comment in its place), a parameter that is no number, anything but subcircuits and their instances.
            W0029 Note {
                title: "a statement structural Verilog does not write",
                cite: "IEEE 1364-2005 §12.1 (modules), §12.2.2 (port connections); the xschem layout (`render::spice::verilog`)",
                explain: "Structural Verilog holds modules and their instances: a primitive element (R, C, a source) is a comment naming this code, and a parameter that is an expression, models and analyses are not written. Netlist the deck for a simulator to keep them.",
            }
            /// ngspice's compatibility mode on a target that has none.
            W0030 Note {
                title: "ngspice's compatibility mode not written",
                cite: "ngspice manual, the compatibility modes (`set ngbehavior` in `.spiceinit`, or `-D ngbehavior=`, read before the deck)",
                explain: "`.options ngbehavior=` selects how ngspice reads the deck (`hsa`, `ps`, `lt`, …); ngspice takes it on its command line, and no other target has such a mode, so it is left out. The target reads the deck in its own dialect.",
            }
            /// A sweep by total point count (Spectre's `log=N`) on a SPICE target, which sweeps by points per decade or octave, or linearly.
            W0031 Refusal {
                title: "a `log` sweep the target lacks",
                cite: "ngspice manual §11.3 (`.ac`, `.noise`, `.sp`, `.sens`: `dec|oct|lin`); HSPICE `.AC` (DEC, OCT, LIN, POI); Xyce RG `.AC` (DEC, OCT, LIN); LTspice help (`.ac oct|dec|lin`)",
                explain: "`log` is Spectre's sweep of a total number of points spaced logarithmically; the SPICE targets' `.ac`, `.noise`, `.sp` and `.sens … ac` take `dec` or `oct` (points per decade or octave) or `lin`, and ngspice stops on `log`. Spell the sweep as `dec` points per decade.",
            }
            /// Text the parser could not read (P0002, P0003, P0008), kept as written, on a target of another dialect.
            W0032 Refusal {
                title: "text that did not parse, for another dialect",
                cite: "the source dialect's grammar, at the parse note beside this code (P0002, P0003, P0008)",
                explain: "The parser kept this text as written: an expression that is no expression, an unclosed delimiter, a block the file ends inside. A target of the source's own dialect gets it back as written; another target would read text no rule checked. Fix the source at the parse note.",
            }
            /// A noise analysis's points per summary on a target whose `.noise` has no such field: not written.
            W0033 Note {
                title: "a noise summary interval not written",
                cite: "ngspice manual §15.3.4 (`.noise … pts_per_summary`: the per-device contributions); LTspice help `.NOISE` (no summary field)",
                explain: "The source asks for a noise summary (each device's contribution) every N points; the target's `.noise` card has no such field, so it is left out. The noise spectrum is the same; read the contributions from the target's own noise output.",
            }
            /// An ngspice `{}` value's ternary in a then-branch, written as ngspice's evaluator reads it (`a?b?c:d:e` as `a?c:e`).
            W0034 Note {
                title: "a nested ternary written as ngspice reads it",
                cite: "ngspice-45 src/frontend/numparam/xpressn.c (the `first-of-triple` operator stack); C's grammar (Xyce RG §2.1.18) reads `a?(b?c:d):e`",
                explain: "ngspice's `{}` evaluator reads `a?b?c:d:e` as `a?c:e`, dropping `b` and `d`; C's grammar, which the target follows, reads `a?(b?c:d):e`. The deck is written with ngspice's value (`a?c:e`). If the source meant C's grouping, parenthesize it there.",
            }
            /// An ngspice ternary in a then-branch that ngspice reads no one way: nested deeper, or in a function body.
            W0035 Refusal {
                title: "a nested ternary ngspice reads no one way",
                cite: "ngspice-45 src/frontend/numparam/xpressn.c (`{}` values) and its B-source parser (C's grammar), run by hand",
                explain: "ngspice reads a function body's `a?b?c:d:e` by C's grammar in a B source and as `a?c:e` in a `{}` value, and a deeper nest in a then-branch by neither rule, so the source has no single meaning to write. Parenthesize the then-branch in the source.",
            }
            /// An LTspice `.backanno`: an annotation request with no simulation meaning, not written.
            W0036 Note {
                title: "an annotation directive with no simulation meaning, not written",
                cite: "LTspice help (`.BACKANNO`: annotate the subcircuit pin names onto the port currents)",
                explain: "LTspice adds `.backanno` to a netlist it writes from a schematic, so that the waveform viewer names a subcircuit's port currents by its pins. It changes no number of the run, and no other simulator has it, so the reader leaves it out on every target.",
            }
            /// LTspice's `.tran … startup` over a source whose DC value no ramp is written for.
            W0037 Refusal {
                title: "a transient start-up ramp the target lacks",
                cite: "LTspice help (`.TRAN` modifier `startup`: the operating point with the independent sources off, each ramped on over the first 20 µs); ngspice manual, transient options (`RAMPTIME`, a ramp of given length whose shape LTspice does not state); HSPICE, Xyce RG `.TRAN`, Spectre, VACASK: no start-up ramp",
                explain: "The transient starts from an operating point with every independent source at zero and ramps them on over its first 20 µs. A target with no such start gets each top-level DC source as a PWL from zero to its value over 20 µs. This source's DC value is neither a number nor an expression, so no ramp is written for it; without one the run starts from the full-source operating point, another answer for a circuit that latches or soft-starts. Write the value as a number or a braced expression, or ramp the source in the deck (a PWL from zero) and drop `startup`.",
            }
            /// A `.dc` axis by decade, octave or list on a target whose `.dc` is linear only and that has no loop for it.
            W0038 Refusal {
                title: "a log or list DC sweep the target lacks",
                cite: "ngspice manual, `.dc` (`.dc srcnam vstart vstop vincr [src2 …]`: linear only, two sources), §13.6.4 (`foreach`); Xyce RG `.DC` (`DEC`, `OCT`, `LIST`); ESPice 0.3.0 (runs no `.control` command)",
                explain: "A `.dc` axis sweeps by decade, by octave or over a list of values, and the target's `.dc` steps linearly only. ngspice runs such an axis as a `foreach` loop of its control script around one `dc`; it is refused there when the script has no such run: a third axis, an axis over a parameter (ngspice's `dc` sweeps a source, a resistor or `temp`), a decade or octave grid that does not run up from a positive start to its stop, more than 4096 values, or a `.meas dc` in the deck (ngspice would evaluate it at every pass of the loop, not over the sweep). Sweep the values another way (a linear sweep covering them) or run the deck on a simulator with log sweeps.",
            }
            /// A statement only its own simulator has (Xyce's own analyses and `Y`
            /// devices, HSPICE's eye and vector cards, LTspice's `.fra` with its devices
            /// and its state files).
            W0039 Refusal {
                title: "a statement only its source simulator has",
                cite: "Xyce RG, analysis commands (`.MPDE`, `.MOR`, `.PCE`, `.EMBEDDEDSAMPLING`) and devices (`Y` devices: neuron, synapse, ROM, reaction network, general external, accelerometer, power grid, memristor §2.3.32, and the ADC, DAC, external, IBIS, anti-windup and neuron population devices Xyce 7.10 runs) and behavioural digital devices (`U`, RG 7.10 §2.3.28; `Y` types, §2.3.29); none in the ngspice manual, HSPICE, LTspice help, Spectre Reference, VACASK docs; Xyce RG 7.10 §2.3.6, a `K` line with a `CORE` model (a nonlinear magnetic core; ngspice manual §3.3.12 and LTspice help, K: a linear coupling only); HSPICE UG, `.STATEYE`, `.VEC`, `.CHECK`, `.POWER`, in no other simulator; LTspice help, `.FRA` (a time-domain frequency response analysis) with its `@` analyzer and `&` probe devices, in no other simulator; LTspice help, A. Special Functions (`A` devices: Analog Devices proprietary special function/mixed mode simulation devices), in no other simulator; LTspice help, `.SAVESTATE` and `.LOADSTATE` (a transient's state in a proprietary file), `.SAVEBIAS` (the operating point as a file for `.loadbias`); LTspice help, `.MACHINE` (an arbitrary state machine: `.mach`, `.state`, `.rule`, `.output`, `.endmach`), in no other simulator; LTspice 26.1.1 run, `.softstart` (the soft-start time of its own regulator models; not in its help, and ngspice-45 stops on it as an unimplemented dot command); Xyce 7.10 run, `.ROL` with `.ROL_DC` and `.ROL_OBJ` (its optimization analysis, which asks for `.ROL_OBJ` lines; not in its RG, and ngspice-45 stops on it as an unimplemented dot command)",
                explain: "The statement is an analysis, a device, a behavioural block (LTspice's `.machine`), a setting of its own models (LTspice's `.softstart`) or a request for a file of its own (LTspice's saved state) that only the simulator the deck was written for has, so no other target can run it. Run the deck on that simulator, or replace the device with a model the target has; a run that restarts from a saved state starts on the target from its own operating point (`.ic`, `.nodeset`).",
            }
            // W0040–W0044: the Python deck builder's (`python::deck`).
            /// An analysis argument from the Python API the target has no word for.
            W0040 Refusal {
                title: "an analysis argument the target has no word for",
                cite: "HSPICE `.FOUR` (nine harmonics, no count); Xyce RG `.FOUR` (no count); ngspice-45 `src/frontend/fourier.c` (one `nfreqs` per deck); Spectre Reference 19.1 p.272-274 (`pss`: no sciter/steadycoeff/uic), `spectre -h ac` (no `oct`)",
                explain: "The analysis was asked for with an argument (a Fourier harmonic count, ngspice's PSS shooting controls, an octave sweep) the target's statement has no field for. Leave the argument out, or netlist for a target that takes it.",
            }
            /// Xyce's time integral `sdt(x)` on a target with no integral function.
            W0045 Refusal {
                title: "a time integral the target lacks",
                cite: "Xyce RG §2.3 (`sdt`); ngspice manual §5.1 (B-source functions: `ddt`, no integral); ngspice-45 run by hand (\"no such function 'sdt'\")",
                explain: "`sdt(x)` integrates x over time in Xyce; ngspice's expressions have a derivative (`ddt`) but no integral, and ngspice stops on the call. Integrate with a capacitor (a current source of x into 1 F) instead.",
            }
            /// A `.include` or `.lib` inside a `.subckt` on a target that does not scope what the file defines to it.
            W0046 Refusal {
                title: "a file read inside a subcircuit the target does not scope",
                cite: "ngspice manual §2.7 (a `.lib` in a `.subckt` is local to it); Xyce 7.10 run by hand (a `.include` there defines globally, a `.lib` there is \"Invalid device type\")",
                explain: "ngspice scopes the subcircuits and models a file read inside a `.subckt` defines to that subcircuit; Xyce makes them global, where they replace the deck's own of the same name. Read the file at the top level, renaming what collides.",
            }
            /// A subcircuit instance's multiplicity `m` on a target that has none.
            W0047 Refusal {
                title: "a subcircuit multiplicity the target lacks",
                cite: "ngspice manual §2.4.3 (an `X` line's `m`, unless the subcircuit declares `m`); Xyce RG `X` (`M`, always); ESPice run by hand (`m=5` on an `X` line changes nothing)",
                explain: "The source multiplies the subcircuit instance by `m` (parallel copies); the target ignores `m` on an `X` line, so the instance would count once. Write the copies out, or scale the subcircuit's elements by a parameter.",
            }
            /// A `.measure` of an analysis, or with a qualifier, the target's `.meas` does not read.
            W0048 Refusal {
                title: "a measure the target cannot take",
                cite: "ngspice manual §15.4 (`.meas {DC|AC|TRAN|SP}`; ngspice-45: \"unrecognized analysis type 'noise'\"); Xyce RG `.MEASURE` (`RFC_LEVEL`, Xyce's own)",
                explain: "The measure reads an analysis the target's `.meas` does not (a noise run on ngspice) or a qualifier only the source has (Xyce's `RFC_LEVEL`). Measure the result in post-processing, or rewrite the measure with the target's own forms (`when`, `find … at=`).",
            }
            /// A device's `IC=` the source holds in its operating point, on a target that applies it only under UIC and cannot hold it by `.ic`.
            W0049 Refusal {
                title: "a device initial condition the target applies only with UIC",
                cite: "Xyce RG (C, L, Q, M `IC=`: applied in the DCOP); ngspice manual §3.3.5, §3.3.9 (\"only if the uic option is specified\"), §15.2.2 (`.ic` holds node voltages)",
                explain: "The source holds the device's initial condition in its operating point; the target ignores it unless the transient has UIC, and its `.ic` holds node voltages only, so a floating capacitor's, an inductor's or a transistor's IC has no spelling. Add UIC to the transient (the source's meaning then differs in the operating point), or replace the IC with an `.ic` on the nodes.",
            }
            // W0050–W0054: the Xyce target's (`render::spice::xyce_*`).
            /// An ngspice model parameter Xyce's device lacks, set to change a number, or an ngspice model kind no Xyce device computes.
            W0050 Refusal {
                title: "a model parameter or kind Xyce's devices lack",
                cite: "Xyce RG 7.10 `D` and `Q` model parameter tables (levels 1 and 234; Xyce 7.10 ignores another name with a warning, run), `M` (level 18 VDMOS, no `vdmos` type; no BSIM1, BSIM2, MOS9, BSIM3SOI FD/DD, HiSIM2, HiSIM_HV), `Q` (VBIC is levels 11 and 12), `Z` (level 1 alone) (Xyce 7.10 run: \"Model is required for device … and no valid model card found\"); ngspice-45 `devices/dio/dio.c`, `devices/bjt/bjt.c`, `devices/hicum2/hicum2.c`, `devices/vdmos`",
                explain: "The instance's model card sets a parameter ngspice's device reads that Xyce's has no parameter for (a `tlev`/`tlevc` temperature coefficient, a tunnelling current, the BJT substrate diode, an instance default on the card) to a value other than the one that turns it off, so Xyce would run without it and give another number; or the card is ngspice's `vdmos`, or at a LEVEL Xyce has no device for (MOSFET 4, 5, 9, 55, 56, 68, 73; BJT 4, 9; MESFET 2 to 4; `nhfet`/`phfet`), which no Xyce model type computes. The card itself is left out where it is declared (W0053). Set the parameter to its off value, or model the effect another way.",
            }
            /// An ngspice C or L model card giving the element's value, which Xyce reads as a multiplier.
            W0051 Refusal {
                title: "a model value Xyce reads as a multiplier",
                cite: "ngspice-45 `devices/cap/cap.c` (`cap`), `devices/ind/ind.c` (`ind`), `parser/inpgmod.c` (an instance parameter on the card is the instance's default); Xyce RG 7.10 C, L model parameters (`C`, `L`: multipliers of the instance value)",
                explain: "In ngspice the model's `c=` (`l=`) is the value of an element that gives none; Xyce multiplies the element's value by it. Give the value on each element and leave it off the model.",
            }
            /// An output the target writes no vector for: on Xyce an `.op` beside another analysis, the integrated noise, an ngspice `@dev[param]` vector; on HSPICE a saved ngspice `@dev[param]`.
            W0052 Note {
                title: "an output the target writes no vector for",
                cite: "Xyce RG 7.10 `.OP` (\"if specified with another analysis type, no additional operating point will be calculated\"), `.NOISE` (the total noise integrals are sent to stdout), `.PRINT` (node voltages, lead currents, power); HSPICE cmdref E-2010.12 `.PROBE` (p.255: \"voltage, current, or element template\"); ngspice manual §15.6 (`@device[param]`)",
                explain: "Xyce runs `.op` beside another analysis as a printout of the bias point, writes a noise sweep's integrals to its standard output only, and has no name for an ngspice device vector (`@q1[gm]`); HSPICE's `.probe` takes a voltage, a current or an element template, so a saved device vector is left out of its cards. The results lack that plot or vector. Run the operating point alone, integrate the noise spectrum, or compute the quantity from node voltages and lead currents.",
            }
            /// A model card Xyce's devices cannot compute, left out of the file written for Xyce.
            W0053 Note {
                title: "a model card Xyce cannot compute, left out",
                cite: "Xyce RG 7.10 `D`, `Q`, `C`, `L` model parameter tables and `M` (no `vdmos`); the instance's refusal W0050, W0051",
                explain: "The card sets what Xyce's device has no equation for (W0050) or reads otherwise (W0051), so it is not written: a deck whose instance uses it is refused there, and Xyce stops on an instance naming the missing card (\"no valid model card found\") instead of running another circuit. Decks that use the file's other models are unaffected.",
            }
        }
    };
}
