//! The catalogue's W0201–W0300 rows: the rules a dialect definition
//! declares. A block of `catalogue!`'s list (`mod.rs` chains every block,
//! in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_w_def`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_w_def {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            // W0201–W0300: the rules a dialect definition declares.
            /// A refusal a dialect definition declared (`DialectDef::code`): its id and message are the definition's.
            W0201 Refusal {
                title: "refused by the target dialect's definition",
                cite: "the target's definition (`syntax::def`, `DialectDef::code`)",
                explain: "The target is a dialect a definition declared (a plugin), and that definition refuses this construct or card with a code of its own; the message names the code and says why. Change the deck, or write it for a dialect that says the construct.",
            }
            /// A note a dialect definition declared: something its writer left out or kept.
            W0202 Note {
                title: "noted by the target dialect's definition",
                cite: "the target's definition (`syntax::def`, `DialectDef::code`)",
                explain: "The target is a dialect a definition declared (a plugin), and that definition leaves this construct out with a note of its own; the message names the code and says what was left out. The deck is still written.",
            }
        }
    };
}
