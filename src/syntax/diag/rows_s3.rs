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
            // slot W0180
            // slot W0181
            // slot W0182
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
