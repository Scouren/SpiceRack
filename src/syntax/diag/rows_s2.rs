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
            // slot W0160
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
