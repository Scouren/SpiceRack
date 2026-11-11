//! The catalogue's W0180–W0199 rows, stream S3's (none yet: a
//! `// slot <CODE>` marker each). A block of `catalogue!`'s list
//! (`mod.rs` chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_s3`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_s3 {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            // W0180–W0199: stream S3 (Phases 8, 6).
            /// A deck analysis the simulator chosen for the run does not declare.
            W0180 Refusal {
                title: "an analysis the chosen simulator does not declare",
                cite: "no clause: the simulator's declared analyses (`backend::SimDef::analyses`, `backend::select`)",
                explain: "The simulator named for the run (`backend=`, `$SPICERACK_BACKEND`) does not run one of the deck's analyses, or no installed simulator does. The message names the analysis and a simulator that runs the deck's analyses: name that one, or leave the choice to the router.",
            }
            /// A feature the deck needs that the simulator chosen for the run does not declare.
            W0181 Refusal {
                title: "a feature the chosen simulator does not declare",
                cite: "no clause: the simulator's declared features (`backend::SimDef::features`, `backend::features`)",
                explain: "The deck needs a feature (`.meas`, `.step`, Verilog-A, ...) outside the simulator's declared `features`: a registered simulator named for the run, or every installed one that runs the deck's analyses. The message names the feature and who declares it.",
            }
            /// A deck whose analyses no one simulator runs together.
            W0182 Refusal {
                title: "no simulator runs every analysis of the deck",
                cite: "no clause: the routes (`backend::Kind::spec`) and each registered simulator's declared analyses",
                explain: "Each analysis runs somewhere, but no simulator runs them all (a pole-zero beside a harmonic balance). The message names the first analysis that leaves none. Split the deck by simulator.",
            }
            // slot W0183
            // slot W0184
            // slot W0185
            // slot W0186
            // slot W0187
            // slot W0188
            // slot W0189
            // slot W0190
            // slot W0191
            // slot W0192
            // slot W0193
            // slot W0194
            // slot W0195
            // slot W0196
            // slot W0197
            // slot W0198
            // slot W0199
        }
    };
}
