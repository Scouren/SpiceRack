//! The catalogue's W0160–W0179 rows, stream S2's (none yet: a
//! `// slot <CODE>` marker each). A block of `catalogue!`'s list
//! (`mod.rs` chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_s2`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_s2 {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            // W0160–W0179: stream S2 (Phase 5).
            /// A `.control` line that changes the circuit or the script's flow, not read.
            W0160 Refusal {
                title: "a control-script line that changes the circuit is not read",
                cite: "ngspice-45 manual §13.5.3 `alter`, §13.5.4 `altermod`, §13.5.5 `alterparam` (the subcircuit form), §13.5.55 `option`, §13.5.69 `run`, §13.5.73 `set`, §13.6 (the control structures); each run on ngspice-45: the line changes the next analysis's result",
                explain: "The `.control` script changes what its later analyses run on in a way the reader does not read: a command other than `alterparam name = number` followed by `reset` (`alter`, `altermod`, `option`, a `set` of a simulator variable such as `temp`, a loop or an `if`, `source` …), an `alterparam` of a subcircuit's parameter or to an expression, or a parameter change in a deck whose dot-card analyses run too (a card before the block, or `run`: ngspice runs those after the script, on the state it left). Leaving the line out would give other numbers, so the deck is refused. Fold the change into the netlist (one deck per state), or keep to `alterparam name = number` and `reset` between the script's own analysis commands.",
            }
            // slot W0161
            // slot W0162
            // slot W0163
            // slot W0164
            // slot W0165
            // slot W0166
            // slot W0167
            // slot W0168
            // slot W0169
            // slot W0170
            // slot W0171
            // slot W0172
            // slot W0173
            // slot W0174
            // slot W0175
            // slot W0176
            // slot W0177
            // slot W0178
            // slot W0179
        }
    };
}
