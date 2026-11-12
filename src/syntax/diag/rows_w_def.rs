//! The catalogue's W0201–W0300 rows: the rules a dialect definition
//! declares, and W0203–W0204, the IR pipeline's (`dialect::facade`). A block of `catalogue!`'s list (`mod.rs` chains every block,
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
            // W0203–W0204: the IR pipeline's (`dialect::facade`), for a routed pair.
            /// A refusal of the IR pipeline with no old code: the message carries the new code and its title.
            W0203 Refusal {
                title: "refused by the IR pipeline",
                cite: "the new code's own row (`dialect::info`); `rewrite/SPINE.md` §5",
                explain: "The pair is written by the IR pipeline (`dialect::facade::ROUTES`, or SPICERACK_PIPELINE=new), which refused the deck with a code of its own that has no code of the old catalogue; the message names that code and its title.",
            }
            /// A note of the IR pipeline with no old code: the message carries the new code and its title.
            W0204 Note {
                title: "noted by the IR pipeline",
                cite: "the new code's own row (`dialect::info`); `rewrite/SPINE.md` §5",
                explain: "The pair is written by the IR pipeline (`dialect::facade::ROUTES`, or SPICERACK_PIPELINE=new), which noted something with a code of its own that has no code of the old catalogue; the message names that code and its title. The deck is still written.",
            }
        }
    };
}
