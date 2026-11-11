//! The catalogue's resolve rows (R0001–R0100), with the R streams' block
//! headers and their free codes' `// slot <CODE>` markers: a block of
//! `catalogue!`'s list (`mod.rs` chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_r`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_r {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            /// An instance whose master names nothing the kit declares: no
            /// subcircuit, model or primitive. The instance keeps its name.
            R0001 Note {
                title: "a master the kit does not declare",
                cite: "ngspice manual §2.4.3 (X ... SUBNAM), ch.7-ch.11 (mname); Spectre UG p.66",
                explain: "No file of the kit declares the subcircuit or model this instance names. The instance keeps the name and the dialect's default letter; read the file in its kit (the files that define the master), or check the name's spelling.",
            }
            /// A name declared twice in one scope (two subcircuits, two models, two
            /// instances of one block); the first is the one a lookup finds.
            R0002 Note {
                title: "a name declared twice in one scope",
                cite: "ngspice manual §2.5; Spectre UG p.94 (names unique in a subcircuit)",
                explain: "Two declarations of one kind share a name in one scope. A reference binds the first in read order; rename or remove the other.",
            }
            /// A master that names declarations of different kinds equally near
            /// (a subcircuit and a model); the first in read order is bound.
            R0003 Note {
                title: "a master naming declarations of different kinds",
                cite: "Spectre UG p.66 (a master is a model, a subcircuit or a primitive)",
                explain: "The master names both a subcircuit and a model at the same distance. The first in read order is bound, which picks the element's letter; rename one of them.",
            }
            /// An instance whose node count differs from its subcircuit's ports.
            R0004 Note {
                title: "an instance's node count differs from its subcircuit's ports",
                cite: "ngspice manual §2.4.3; Spectre UG p.94",
                explain: "The instance connects more or fewer nodes than its subcircuit has ports. A simulator refuses it; connect one node per port.",
            }
            /// A master that names a parameter or a function, not a device.
            R0005 Note {
                title: "a master naming a parameter or a function",
                cite: "ngspice manual §2.4.3, §2.8; Spectre UG p.66",
                explain: "The instance's master is declared, but as a parameter or a function, not a subcircuit or model. Check which name the instance means.",
            }
            /// An include, `.lib` call or Verilog-A source the kit lacks.
            R0006 Note {
                title: "an included file the kit lacks",
                cite: "ngspice manual §2.6 (.include), §2.7 (.lib); Spectre UG p.98 (include)",
                explain: "The included path does not exist under the kit's root as the loader resolves it. The statement is written as is; the names it would declare stay unbound (R0001).",
            }
            /// An include the loader refused (a path outside the kit's root).
            R0007 Note {
                title: "an include the loader refused",
                cite: "no clause: the kit's loader confines paths (syntax::resolve::Kit::load)",
                explain: "The loader does not read files outside the kit's root. Move the file into the kit, or load the kit from a root that holds it.",
            }
            /// An include past the kit's nesting cap (`resolve::DEPTH_MAX`): not
            /// read.
            R0008 Note {
                title: "an include past the nesting cap",
                cite: "no clause: the kit's own limit, syntax::resolve::DEPTH_MAX",
                explain: "Includes nest deeper than the kit reads (`DEPTH_MAX`), usually an include cycle. The file below the cap is not read; break the cycle.",
            }
            /// An include past the kit's file cap (`resolve::FILES_MAX`): not read.
            R0009 Note {
                title: "an include past the file cap",
                cite: "no clause: the kit's own limit, syntax::resolve::FILES_MAX",
                explain: "The kit holds more files than it reads (`FILES_MAX`). The rest are not read; load a smaller entry set.",
            }
            /// A `.lib file section` or `include … section=` naming a section the
            /// file does not hold.
            R0010 Note {
                title: "a section the file does not hold",
                cite: "ngspice manual §2.7; Spectre Reference 19.1 p.498 (include ... section=)",
                explain: "The file is read but declares no section of that name. Check the section's spelling (corner names such as `tt` are case-sensitive in Spectre).",
            }
            // R0011–R0019: R0011 S2 (import-files). R0020–R0029: S1.
            // R0030–R0039: S2. R0040–R0049: S3.
            // slot R0020
            // slot R0021
            // slot R0022
            // slot R0023
            // slot R0024
            // slot R0025
            // slot R0026
            // slot R0027
            // slot R0028
            // slot R0029
            // slot R0030
            // slot R0031
            // slot R0032
            // slot R0033
            // slot R0034
            // slot R0035
            // slot R0036
            // slot R0037
            // slot R0038
            // slot R0039
            /// A simulator descriptor (`backend::SimDef`, Python `Simulator(...)`, a
            /// `spicerack.simulator/1` file) with a field that does not validate.
            R0040 Note {
                title: "a simulator descriptor that does not validate",
                cite: "no clause: the registry's edge checks (`backend::register_simulator`, `backend::SimRegisterError`)",
                explain: "The message opens with the field at fault (`name`, `dialect`, `command`, `output`, `analyses`, `features`, `verilog`, an unknown key) and says what it must be. Nothing is registered.",
            }
            /// A registered command simulator whose program is not on `$PATH` at run time.
            R0041 Note {
                title: "the simulator's command is not installed",
                cite: "no clause: the simulator's `installed` probe (`backend::Installed::OnPath`)",
                explain: "The deck was written, and the program that runs it (the command's first word) is not on `$PATH`. Install it, or name another simulator.",
            }
            /// A registration past the registry's capacity (`backend::SIMS_MAX`).
            R0042 Note {
                title: "the simulator registry is full",
                cite: "no clause: the registry's own limit, backend::SIMS_MAX",
                explain: "The registry holds 32 simulators, the five built-ins among them, for the life of the process. Register fewer; an equal definition registered again takes no new slot.",
            }
            // slot R0043
            // slot R0044
            // slot R0045
            // slot R0046
            // slot R0047
            // slot R0048
            // slot R0049
        }
    };
}
