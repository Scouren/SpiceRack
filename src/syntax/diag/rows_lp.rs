//! The catalogue's tokenize rows (L0001–L0100, none yet) and parse rows
//! (P0001–P0100), with the P streams' block headers and their free codes'
//! `// slot <CODE>` markers: the first block of `catalogue!`'s list
//! (`mod.rs` chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_lp`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_lp {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            /// A statement no rule reads; kept as a `Verbatim` node.
            P0001 Note {
                title: "a statement no rule reads",
                cite: "the dialect's statement grammar (ngspice manual ch.2; Spectre UG ch.3)",
                explain: "The parser has no rule for this statement's shape, so it keeps the statement's text as written. A writer for the same dialect family copies it; another family refuses it (W0001).",
            }
            /// An expression that does not parse; kept as a `Bad` node.
            P0002 Note {
                title: "an expression that does not parse",
                cite: "ngspice manual §2.8.5; HSPICE UG08 pp.279-283; Spectre UG p.87",
                explain: "The value's text is no expression of the dialect's grammar (a stray operator, an unbalanced parenthesis). Its text is kept as written; check the value's spelling against the manual.",
            }
            /// A block the file ends inside (no `.ends`, `.endl`, `.endif`, `}` …).
            P0003 Note {
                title: "a block the file ends inside",
                cite: "ngspice manual §2.5 (.subckt ... .ends), §2.7 (.lib ... .endl)",
                explain: "A subcircuit, library section, conditional or brace block is still open at the end of the file. Add its closer; the block's statements are read as if it closed at the end.",
            }
            /// A block closer with no block open (`.ends` at the top).
            P0004 Note {
                title: "a block closer with no block open",
                cite: "ngspice manual §2.5",
                explain: "A `.ends`, `.endl`, `.endif` or `}` closes nothing. It is ignored; remove it, or open the block it was meant to close.",
            }
            /// A number whose spelling the dialect's number grammar does not read
            /// (`1k5` in Spectre, `0x1F` in VACASK): its token's value is NaN.
            P0005 Note {
                title: "a number the dialect does not read",
                cite: "ngspice manual §2.1.3; the old Spectre grammar's §2.4 number rule (`9d1d6bb:grammar/spectre.peg`)",
                explain: "The token starts like a number but the dialect's suffix table does not read it. Spell it with the dialect's suffixes (Spectre has no `1k5`; SPICE's `M` is milli, `meg` mega).",
            }
            /// Nesting deeper than the parser's cap: the rest is kept as tokens.
            P0006 Note {
                title: "nesting deeper than the parser's cap",
                cite: "no clause: the parser's own limit, syntax::parse DEPTH_MAX",
                explain: "Expressions or blocks nest deeper than the parser walks (`DEPTH_MAX`), so the part below the cap is kept as tokens, unread. No real deck nests this deep; split the expression.",
            }
            /// A byte no token rule reads (the tokenizer's `Invalid`).
            P0007 Note {
                title: "a byte no token reads",
                cite: "the dialect's lexical grammar",
                explain: "A character the dialect's lexicon has no token for (a control byte, a stray symbol). The statement holding it is kept as text; remove the character or check the file's encoding.",
            }
            /// A delimiter with no closer on its statement: `{`, `'`, `(`.
            P0008 Note {
                title: "a delimiter with no closer",
                cite: "ngspice manual §2.1 ({...}, '...'); Spectre UG p.87",
                explain: "An expression delimiter (`{`, `'`, `(`) is not closed before the statement ends. Close it on the same logical line (continuations count).",
            }
            /// A section opened inside a section: sections do not nest.
            P0009 Note {
                title: "a section inside a section",
                cite: "ngspice manual §2.7; Spectre Reference 19.1 p.498",
                explain: "A library section opens before the previous one closed. Close the first section (`.endl`, `endsection`) before opening the next.",
            }
            // P0010–P0014: stream D's reader codes (P1-022, P1-042, P4-100).
            // P0020–P0026: stream S3 (P8-149). P0030–P0039: S1. P0040–P0049: S2.
            // P0050–P0059: S3.
            /// A line the source's own simulator (Xyce) rejects: left out, noted.
            P0010 Note {
                title: "source text its simulator rejects",
                cite: "Xyce 7.10 `-norun`: \".TRAN line has an unexpected number of fields\", \".DC line not formatted correctly\" / \"Extraneous values on .DC line\", \".AC line has an unexpected number of fields\", \".NOISE line has an unexpected number of fields\", \".FFT line needs at least 2 arguments\", \".FOUR line needs at least 3 arguments\", \"Not enough fields on input line for device\", \"Unrecognized parameter\", \"PWL device missing source parameters\", \"Could not parse time/value pairs for PWL function\", \"PWL source repeat value (R) must be >= 0 and < last value\"",
                explain: "Xyce stops on this line before it runs the deck (a field count its card does not take, a second value on an R, L or C, a PWL with no points or a repeat time outside them), so the line has no meaning to write. It is left out and the rest of the deck is written; fix the line in the source.",
            }
            /// An ngspice PWL `r=` that is not one of the wave's time points before
            /// the last: left out, noted.
            P0011 Note {
                title: "a PWL repeat time no point of the wave",
                cite: "ngspice-45 run: \"repeat start time value … does not match any time point given!\", \"… must be smaller than final time point given!\" (ngspice manual §4.1.4)",
                explain: "ngspice repeats a PWL wave from the time point `r=` names, which must be one of the wave's time points before the last; any other value stops the run. The source is left out and the rest of the deck is written; set `r=` to one of the wave's time points.",
            }
            // slot P0015
            // slot P0016
            // slot P0017
            // slot P0018
            // slot P0019
            // slot P0030
            // slot P0031
            // slot P0032
            // slot P0033
            // slot P0034
            // slot P0035
            // slot P0036
            // slot P0037
            // slot P0038
            // slot P0039
            // slot P0040
            // slot P0041
            // slot P0042
            // slot P0043
            // slot P0044
            // slot P0045
            // slot P0046
            // slot P0047
            // slot P0048
            // slot P0049
            // slot P0050
            // slot P0051
            // slot P0052
            // slot P0053
            // slot P0054
            // slot P0055
            // slot P0056
            // slot P0057
            // slot P0058
            // slot P0059
        }
    };
}
