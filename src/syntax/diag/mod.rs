//! The diagnostic catalogue: one [`Code`] per refusal or drop, each a row
//! of [`Info`] (its title, the manual clause it cites, its explanation and
//! its [`Kind`]), and the [`Diag`] row a stage appends (a code and the
//! token it anchors to). Messages are rendered late, from the rows
//! ([`messages`]: line and column from the token, sorted by source
//! position), never stored.
//!
//! Codes are never renumbered or reused; a retired code keeps its row. The
//! first letter is the stage class and the number its range:
//!
//! | Range | Stage |
//! |---|---|
//! | L0001–L0100 | tokenize |
//! | P0001–P0100 | parse |
//! | R0001–R0100 | resolve |
//! | W0001–W0100 | render: the SPICE-family writer and the rules both writers share |
//! | W0101–W0200 | render: the named writer (Spectre, VACASK) |
//! | W0201–W0300 | render: rules a dialect definition declares |
//!
//! A `W0201`/`W0202` diagnostic carries a `detail`: which of the codes a
//! definition declared (`syntax::def::DialectDef::code`) it is, its
//! message `W0201 <lang>:<id> <message>`.
//!
//! The catalogue is one table (`catalogue!`): a code's variant and its
//! row are one entry, so no code lacks a row; `tests` checks each row and
//! the ranges. Its rows sit one block a file, chained in code order into
//! one `catalogue!` list: `rows_lp.rs` (L, P), `rows_r.rs` (R),
//! `rows_w_spice.rs` (W0001–W0100), `rows_w_named.rs` (W0101–W0139),
//! `rows_s1.rs` (W0140–W0159), `rows_s2.rs` (W0160–W0179), `rows_s3.rs`
//! (W0180–W0199), `rows_w_def.rs` (W0201–W0300).
//!
//! Adding a code is one entry in its block's file: take the next free
//! number of your block (the `// Wxxxx–Wyyyy:` headers in each file name
//! each block's owner, from `research/ZERO-LEDGER.md` "Code ranges"), write
//! `/// <one line>` then `Wnnnn Refusal|Note { title, cite, explain }` at
//! its numeric place (the codes stay ascending: `Code`'s `Ord` is the
//! order two diagnostics on one token are rendered in, and
//! `codes_follow_their_stage_ranges` fails out of order). A stream's free
//! code (S1, S2, S3, and P0015–P0019) holds a `// slot <CODE>` line at its
//! numeric place: write the row directly after its slot and delete the
//! marker, so the codes stay ascending whatever order rows land in. `cite`
//! names the target's manual clause (or the run) that makes it a refusal;
//! `Kind::Refusal` stops the deck, `Kind::Note` keeps it. Raise it from a
//! capability cell (`DialectDef::refuse`/`omit`/`copy`, in
//! `syntax::dialects`) or push `Diag::new(code, token, 0)` from a stage.
//!
//! Cites: Zig 0.17 `lib/std/zig/Ast.zig` (`Ast.Error`); VerA
//! `lib/diag/code.zig`.
//!
//! Contents:
//! - `pub enum Code`, `Code::ALL: &[Code]`, `pub enum Kind`, `pub struct Info`,
//!   `pub const fn info(c: Code) -> Info`
//! - `pub struct Diag { pub code: Code, pub token: u32, pub detail: u16 }`, `Diag::new(code: Code, token: u32, detail: u16) -> Diag`
//! - `pub fn sort(diags: &mut [Diag])`, `pub fn messages(diags: &[Diag], t: &Tokens, file: &str) -> String`
//!   (`messages.rs`)

mod messages;
#[macro_use]
mod rows_lp;
#[macro_use]
mod rows_r;
#[macro_use]
mod rows_s1;
#[macro_use]
mod rows_s2;
#[macro_use]
mod rows_s3;
#[macro_use]
mod rows_w_def;
#[macro_use]
mod rows_w_named;
#[macro_use]
mod rows_w_spice;
#[macro_use]
mod table;

pub use messages::{messages, sort};

/// What a diagnostic means for the file it is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// The stage kept going and kept the meaning (text kept as written, a
    /// binding left open): the deck is still written.
    Note,
    /// The target cannot say this with its meaning: the deck is not written.
    Refusal,
}

/// A code's fixed text.
pub struct Info {
    /// What went wrong, in a few words.
    pub title: &'static str,
    /// The manual clause, or the reason no clause applies.
    pub cite: &'static str,
    /// What triggers it and what the user can do (`--explain`).
    pub explain: &'static str,
    /// Whether the deck is still written.
    pub kind: Kind,
}

/// One diagnostic: `code` at token `token` of the file's [`Tokens`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Diag {
    /// What it is.
    pub code: Code,
    /// The token it anchors to.
    pub token: u32,
    /// `W0201`, `W0202`: the declared code it is (`lang::plugin_code`);
    /// 0 for every other code.
    pub detail: u16,
}

// A stage appends one per finding; 8 bytes holds a code, a token and a
// detail (P9-018).
const _: () = assert!(size_of::<Diag>() == 8);

// The catalogue: each block's rows appended in code order, then
// `catalogue!` over them all.
rows_lp! { rows_r rows_w_spice rows_w_named rows_s1 rows_s2 rows_s3 rows_w_def catalogue_end ; }

impl Diag {
    /// `code` at `token`, with `detail` (0 but for `W0201`/`W0202`).
    pub const fn new(code: Code, token: u32, detail: u16) -> Diag {
        Diag {
            code,
            token,
            detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_has_a_row_with_a_title_a_cite_and_an_explanation() {
        assert!(!Code::ALL.is_empty());
        for (k, &c) in Code::ALL.iter().enumerate() {
            // `ALL` is in declaration order, so it holds every variant once.
            assert_eq!(c as usize, k, "{c:?}");
            let i = info(c);
            assert!(!i.title.is_empty(), "{c:?}");
            assert!(!i.cite.trim().is_empty(), "{c:?}");
            assert!(i.explain.len() > i.title.len(), "{c:?}");
        }
        assert_eq!(size_of::<Diag>(), 8);
    }

    #[test]
    fn codes_follow_their_stage_ranges() {
        // The module header's table: (letter, first, last, stage).
        const RANGES: [(char, u32, u32, &str); 6] = [
            ('L', 1, 100, "tokenize"),
            ('P', 1, 100, "parse"),
            ('R', 1, 100, "resolve"),
            ('W', 1, 100, "render: SPICE and shared"),
            ('W', 101, 200, "render: named"),
            ('W', 201, 300, "render: a definition's rules"),
        ];
        let mut last: Option<(char, u32)> = None;
        for &c in Code::ALL {
            let name = format!("{c:?}");
            let (letter, digits) = name.split_at(1);
            let letter = letter.chars().next().expect("a code has a letter");
            assert_eq!(digits.len(), 4, "{name}");
            let n: u32 = digits.parse().expect("four digits");
            assert!(
                RANGES
                    .iter()
                    .any(|&(l, lo, hi, _)| l == letter && (lo..=hi).contains(&n)),
                "{name} is in no stage's range"
            );
            // Stage order (L, P, R, W), each class ascending: a new code is
            // appended to its range.
            let rank = |l: char| "LPRW".find(l).expect("a stage letter");
            if let Some((pl, pn)) = last {
                assert!((rank(pl), pn) < (rank(letter), n), "{name} out of order");
            }
            last = Some((letter, n));
            // Parse, tokenize and resolve keep going; only a writer refuses.
            if letter != 'W' {
                assert_eq!(info(c).kind, Kind::Note, "{name}");
            }
        }
    }
}
