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
            // slot W0140
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
