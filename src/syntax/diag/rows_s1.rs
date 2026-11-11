//! The catalogue's W0140–W0159 rows, stream S1's (none yet: a
//! `// slot <CODE>` marker each). A block of `catalogue!`'s list
//! (`mod.rs` chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_s1`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_s1 {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            // W0140–W0159: stream S1 (Phases 3, 4).
            /// An `X` line with no parameters that Xyce computes with the values an earlier line of its subcircuit passed.
            W0140 Refusal {
                title: "a subcircuit instance Xyce gives another instance's parameters",
                cite: "Xyce 7.10 runs (`.param R=1k`, `.subckt s a b` reading `{R}`, `X1 1 0 s PARAMS: R=100k`, `X4 4 0 s`: both compute with 100k; with `X4` first, or passing any parameter, it computes with 1k); ngspice manual §2.4.3 (a subcircuit's parameters are its head's, each with one default)",
                explain: "Xyce sets a parameter an `X` line passes even where the `.subckt` line does not declare it, and an `X` line of the same subcircuit that passes no parameter at all then computes with the values the line before it passed, not with the top-level `.param` or the subcircuit's own: the result depends on the order of the lines. The target gives every instance the head's one default. Pass the parameter on this line too, or declare it on the `.subckt` line with its default.",
            }
            // slot W0141
            // slot W0142
            // slot W0143
            // slot W0144
            // slot W0145
            // slot W0146
            // slot W0147
            // slot W0148
            // slot W0149
            // slot W0150
            // slot W0151
            // slot W0152
            // slot W0153
            // slot W0154
            // slot W0155
            // slot W0156
            // slot W0157
            // slot W0158
            // slot W0159
        }
    };
}
