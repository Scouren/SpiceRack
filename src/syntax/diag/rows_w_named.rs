//! The catalogue's W0101–W0139 rows: the named writer's (Spectre, VACASK),
//! its reader's and the codes the ledger assigned one by one. A block of
//! `catalogue!`'s list (`mod.rs` chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_w_named`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_w_named {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            // W0101–W0200: the named writer's (Spectre, VACASK).
            /// A flag (a parameter with no value) a target taking only `name=value` cannot spell.
            W0101 Refusal {
                title: "a flag the target has no spelling for",
                cite: "Spectre Reference 19.1 p.506 (`name=value`); VACASK docs/input-instances.md",
                explain: "The element carries a bare flag (`off`, `ic`) and the target takes only `name=value`. Write the flag's parameter form, if the device has one.",
            }
            /// A behavioural source whose SPICE expression the target's grammar cannot be trusted to read.
            W0102 Refusal {
                title: "a behavioural source the target cannot spell",
                cite: "Spectre Reference 5.0 pp.856-860 (bsource), pp.867-870 (operators, functions); VACASK 36be560 docs/dev-builtin-behavioral.md (`v=`/`i=`, its function table); ESPice 0.3.0 (its B source compiles 64 operations, 32 constants, stack depth 16, 8 probed nets and currents: the error's own text; no `pwl()` or `table()` in it)",
                explain: "The expression holds a form the target's behavioural source (Spectre's bsource, VACASK's `v=`/`i=` line) has no spelling for with the same meaning (`hertz`, a function such as `u()` or a statistical call, another dialect's `**`/`pow` whose negative-base rule is not checked, on VACASK the current of a device that is no voltage source). Rewrite the expression with the functions both define, or model the element as Verilog-A. On ESPice a table source (Xyce's E/G `TABLE`) is written as a chain of ternaries over its segments, which the B source compiles for one probe and up to five segments (64 operations); a larger table, or one over an expression, is refused: write it with fewer points, or as two sources summed.",
            }
            /// A charge expression the target's form cannot spell (Spectre's `q=`: a branch current, a braced value, `time`, `temper`, `hertz`; VACASK's `i=ddt(q)`: what W0102 lists).
            W0103 Refusal {
                title: "a charge expression the target cannot spell",
                cite: "Spectre Reference 19.1 p.459 (bsource `q=`); VACASK 36be560 docs/dev-builtin-behavioral.md (`ddt`, its function table)",
                explain: "The charge capacitor's expression reads something the target's form cannot: Spectre's `q=` a branch current, `time`, `temper` or `hertz`; VACASK's `i=ddt(q)` line `hertz`, a function it has no spelling for, or the current of a device that is no voltage source. Model the capacitor with a behavioural current source.",
            }
            /// A `.model` whose kind (and level) has no master in the target.
            W0104 Refusal {
                title: "a model kind with no master in the target",
                cite: "Spectre Reference 19.1 p.498 (model masters); VACASK spice/*.osdi; ngspice-45 spicelib/parser/inpdomod.c (the level switch), inpfindl.c:51-57",
                explain: "No master of the target has this model kind and level's equations; the nearest master would compute something else. Use a model the target ships, or load the model as Verilog-A.",
            }
            /// A device the target has no master or terminal list for (a MOSFET without its bulk or with extra nodes, a current-controlled switch; in VACASK a charge capacitor with instance parameters, a switch, a POLY source whose term order or lone coefficient the tree does not settle).
            W0105 Refusal {
                title: "a device the target has no master or terminals for",
                cite: "Spectre Reference 5.0 (mos, relay, vsource chapters); HSPICE UG, W element (RLGC lossy lines): the ngspice manual, Xyce RG and LTspice help list no W element (ngspice's `W` is a current switch, its coupled lines `P` take other parameters)",
                explain: "The target has no device with this element's terminals (a MOSFET with three or five nodes, a current-controlled switch, a POLY source). Connect the missing terminal, or model the device another way.",
            }
            /// A function the target cannot define (a body over `v()`/`i()`; VACASK has none).
            W0106 Refusal {
                title: "a function the target cannot define",
                cite: "Spectre Reference R5 pp.870-871 (`real f(real x)`; \"declared at the top level only, and must not be declared within subcircuits\"); VACASK has no user functions; LTspice help `.FUNC` (no scope stated for one in a subcircuit)",
                explain: "The target's functions cannot read node voltages or currents (Spectre), or it has no user functions (VACASK). Inline the function where it is called.",
            }
            /// A statistical call with no form in the target (HSPICE's multiplier, a wrong argument count).
            W0107 Refusal {
                title: "a statistical call with no form in the target",
                cite: "HSPICE UG B-2008.09 App. A (`multiplier`); ngspice manual §2.11.5; Xyce RG Table 2-33",
                explain: "The call's arguments have no meaning in the target's statistical form (HSPICE's four-argument multiplier, a count no dialect reads). Write the call with ngspice's arguments.",
            }
            /// A source waveform the target cannot run as SPICE does (Spectre's AM; VACASK's PWL repeat, a SIN phase, an EXP delay that is no number).
            W0108 Refusal {
                title: "a source waveform the target cannot run",
                cite: "Spectre Reference 5.0 p.685 (no AM equation); VACASK vsource (`sine` ignores a phase; a PWL `period` repeats from time 0 with the last value the first: `docs/dev-builtin-src.md`)",
                explain: "The target's source runs this waveform otherwise (Spectre has no AM; VACASK ignores a SIN phase and repeats a PWL only whole). Write the waveform as a behavioural source or a PWL without the repeat.",
            }
            /// A `.measure` the target has no statement for (VACASK).
            W0109 Refusal {
                title: "a measurement the target has no statement for",
                cite: "VACASK docs (no `measure`: post-process the raw file)",
                explain: "VACASK has no measurement statement. Compute the measurement from the raw file after the run.",
            }
            /// Retired (phase 7): a `.lib file` with no section is `include
            /// "file"`, no longer `include "file" section=`.
            W0110 Note {
                title: "a sectionless library call written with an empty section",
                cite: "Spectre Reference 19.1 p.493 (include ... section=)",
                explain: "Retired: no writer draws it. The old writers wrote a SPICE `.lib file` with no section as `include \"file\" section=` with an empty section; Spectre and VACASK now write `include \"file\"` (Spectre Reference 5.0 p.875), which reads the whole file.",
            }
            /// A `.model` level that is no number, mapped as level 1.
            W0111 Note {
                title: "a model level that is no number, read as level 1",
                cite: "Spectre Reference 19.1 p.498 (model masters)",
                explain: "The model's `level` is an expression, so the master is chosen as for level 1 (`mos1`, `bjt`). Write the level as a number so the master matches the model's equations.",
            }
            /// A parameter ngspice reads without effect on the solution (or not at all), not written.
            W0112 Note {
                title: "a parameter ngspice ignores, not written",
                cite: "ngspice-45 `parser/inpgmod.c` (\"unrecognized parameter - ignored\"); `render::models` (the safe-operating-area limits)",
                explain: "ngspice does not read this parameter (no row of the device's table names it), or reads it only for a warning (a safe-operating-area limit), so the solution is the same without it; the target would reject or misread it. Remove it from the card.",
            }
            /// A voltage-dependent resistance written as ngspice runs it: its model and its other parameters left out.
            W0113 Note {
                title: "a voltage-dependent resistor's model and parameters, not written",
                cite: "ngspice-45 `frontend/inpcom.c` L6621-L6680 (`R … r={v(…)}` → `B … i=v(…)/(…)` with `tc1`, `tc2`, `m` only); Spectre Reference 5.0 p.859 (`resistor r=` of `v(a,b)`)",
                explain: "ngspice runs a resistance that reads a node voltage or a branch current as a behavioural source carrying only `tc1`, `tc2` and `m`: the model and every other parameter are dropped. The target's card keeps the same three.",
            }
            /// A model or element parameter no parameter of the target computes the same way.
            W0114 Refusal {
                title: "a parameter with no form in the target",
                cite: "`render::models` (each row's ngspice-45 and Spectre Reference 5.0 clause); `render::spice::ngspice_models` (Xyce RG 7.10 `C` AGE, BSIM-SOI instance `shmod`; ngspice-45 `cap.c`, `b4soi.c` instance tables; the diode knee: Xyce `N_DEV_Diode.C:2396-2401` id·sqrt(IKF/(IKF+id)) against ngspice-45 `dio/dioload.c:382-386` id/(1+sqrt(id/IKF)))",
                explain: "The parameter at this position has no parameter in the target with ngspice's equation (a tunnelling current, an exponential temperature coefficient, a knee current Spectre reads only with `hcomp=1`), or ngspice and the target rank it against another one in opposite orders. Set it to its off value, or model the effect another way.",
            }
            /// A Spectre `montecarlo` whose `variations` leaves out a scope the deck draws in, on a target that varies every call (VACASK's `mc`).
            W0115 Refusal {
                title: "a Monte Carlo the target varies wider",
                cite: "Spectre Reference 19.1 p.166 (`variations`, default `process`); VACASK docs/cmd-analysis-mc.md (`mc` draws at every call site)",
                explain: "The montecarlo varies only the process (top-level) or the mismatch (in-subcircuit) draws, and the target's Monte Carlo varies every draw. Write `variations=all`, or keep the deck's draws to the scope the montecarlo names.",
            }
            // W0120–W0124: the Spectre and VACASK reader's (`render::read::named*`).
            /// A VACASK control command or `embed` that only steers a VACASK run
            /// and its scripts, not written on another target.
            W0120 Note {
                title: "a VACASK script line with no circuit meaning, not written",
                cite: "VACASK docs cmd-errorhandling.md (`abort`), cmd-print.md, cmd-postprocess.md, input-embed.md (a file written for a script)",
                explain: "`abort` sets VACASK's error policy, `print` writes to its console, `postprocess` runs a program after the run, and an `embed` that no `load` or `include` of the deck reads writes a file for such a program: none changes a number of the circuit, and the target has no such step, so it is left out. Run the script on the target's own results.",
            }
            /// A loaded module named as one of the target's built-in devices.
            W0121 Refusal {
                title: "a Verilog-A module named as a built-in device of the target",
                cite: "ngspice-45 run: `.model dmod diode` with VACASK's `diode.osdi` loaded reads `diode` as its built-in `Diode` (\"unrecognized parameter (diode)\") and the `N` instance fails (\"incorrect model type! Expected OSDI device\"); `devhelp` lists the built-in names",
                explain: "The target looks a model's type up among its built-in devices before the OSDI modules it loaded, so a module named as one of them (`diode`, `resistor`, `bjt`, …) cannot be instantiated. Rename the module in its Verilog-A source and load that build.",
            }
            /// A VACASK run that elaborates more than one circuit.
            W0122 Refusal {
                title: "a VACASK run of more than one circuit",
                cite: "VACASK docs cmd-elaboration.md (`elaborate circuit(\"a\")` builds the circuit the analyses after it run); ngspice manual §2.1 (one circuit, the title line to `.end`); Spectre Reference 5.0 (one netlist a run)",
                explain: "The control block elaborates a circuit after an analysis has run, or more than once (`elaborate circuit(\"ntest\")` … `elaborate circuit(\"ptest\")`), so its analyses run on different circuits; the target's deck holds one (a promoted subcircuit with terminals is not read either). Split the deck, one per circuit, each promoting its subcircuit once before its first analysis.",
            }
            /// A `.tf` written as Spectre's `xf` at frequency 0: its input and
            /// output resistances are not computed.
            W0125 Note {
                title: "a transfer function's input and output resistance not computed",
                cite: "Spectre Reference 19.1 pp.438-439 (xf: transfer functions from every source)",
                explain: "SPICE's `.tf` gives the gain and the input and output resistances at the operating point; Spectre's `xf` at `values=[0]` gives the transfer function from every source to the output, no resistance. Add a test source and compute the resistance from its transfer function.",
            }
            /// A SPICE `.include` or `.lib` inside a `.subckt`, on Spectre.
            W0128 Refusal {
                title: "a file read inside a subcircuit, on Spectre",
                cite: "ngspice manual §2.7 (a `.lib` in a `.subckt` is local to it); Spectre Reference 19.1 pp.533-535 (`subckt`: no clause on an `include` in its body)",
                explain: "ngspice scopes what a file read inside a `.subckt` defines to that subcircuit; no Spectre clause here says an `include` in a `subckt` body does the same, and leaving the file out changes which subcircuits and models the body sees. Read the file at the top level, renaming what collides.",
            }
            /// A model parameter the VACASK module the card maps to does not declare.
            W0129 Refusal {
                title: "a model parameter the VACASK module lacks",
                cite: "VACASK 36be560 `devices/spice/<module>.va` (its `parameter` and `aliasparam` names; VACASK stops on another: \"Parameter '…' not found.\"); ngspice-45 `devices/<dev>/*.c` (the names the device reads)",
                explain: "The card sets a parameter the source simulator reads (or, from a dialect other than ngspice, may read) but the `sp_*` module VACASK builds from ngspice's device code does not declare (an initial condition, a self-heating or a version switch), so VACASK has no equation for it. Remove the parameter, or set the condition the VACASK way (`ic=` on the analysis).",
            }
            // W0130–W0139: codes the ledger assigned one by one (ZERO-LEDGER "Code
            // ranges"): W0130 S2 StepStates, W0131 S1 REFUSE-SOURCE (P4-096), W0132
            // S2 no line fit, W0133 S2 pac without pss, W0134 S1 (P4-103), W0135 S1
            // (P4-104), W0136-W0137 S3 (P8-149); W0138 free.
            /// A circuit changed between two analyses, on a target whose deck is one circuit.
            W0130 Refusal {
                title: "a circuit changed between analyses, which the target runs on one circuit",
                cite: "Xyce RG `.STEP` and LTspice help `.step` (a step runs every analysis of the deck); HSPICE cmdref `.ALTER` (E-2010.12 pp.32-34); ESPice 0.3.0 (`.control` commands ignored, run); ngspice manual §13.5.5 (`alterparam`); VACASK `docs/cmd-alter.md`",
                explain: "The source runs an analysis, changes a parameter (VACASK `alter`, ngspice `alterparam`) and runs another: two circuit states in one deck. This target's deck is one circuit and every analysis in it runs on that one. Write one deck per state, or netlist for ngspice, whose `.control` script changes the parameter between the runs.",
            }
            // W0136 is not a code: ngspice-45 linearizes `exp` in a B source only under
            // `ngbehavior=ps`, where `pow(exp(1), x)` is exact (P8-149, P8-146).
            /// A neutral power, `sqrt` or `log` whose base or argument may leave the
            /// domain, on a target that then computes a number instead of failing.
            W0137 Refusal {
                title: "an expression whose domain the target evaluates differently",
                cite: "ngspice manual §5.1 and ngspice-45 `ptfuncs.c` `PTpower` (`pow(fabs(x1), x2)`); Xyce 7.10 runs: `(-2)**0.5` = 8.66e-17, `sqrt(-2)` = 0, `ln(-2)` = 0.693 (the real part of a complex value); Spectre 25.1 run: `(-2)**0.5` = 0 (settle-remote P4-003); recorded under `research/impl/evidence/P8-146/`",
                explain: "The neutral expression has a real value only inside its domain (`x**y` with x < 0 needs an integer y; `sqrt` and `log` need x >= 0). Outside it this target does not fail: it returns the power of |x|, or the real part of a complex result, so the deck would run to a wrong number. The writer lowers a base or argument it can prove non-negative (a non-negative constant, `abs(...)`, a square `x*x` or `x**2`, `exp(...)`) and refuses the rest. Wrap the base or argument in `abs(...)`.",
            }
        }
    };
}
