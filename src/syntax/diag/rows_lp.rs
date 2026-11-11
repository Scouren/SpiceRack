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
                cite: "Xyce 7.10 `-norun`: \".TRAN line has an unexpected number of fields\", \".DC line not formatted correctly\" / \"Extraneous values on .DC line\", \".AC line has an unexpected number of fields\", \".NOISE line has an unexpected number of fields\", \".FFT line needs at least 2 arguments\", \".FOUR line needs at least 3 arguments\", \"Not enough fields on input line for device\", \"Unrecognized parameter\", \"PWL device missing source parameters\", \"Could not parse time/value pairs for PWL function\", \"PWL source repeat value (R) must be >= 0 and < last value\"; an element: \"Invalid device type for device\", \"Unrecognized line\", \"Model is required for device\", \"Specified model not found for device\", \"Invalid DC value\", a PAT source's \"TR, TF and TSAMPLE must all be non-negative\", \"VHI, VLO, TD, TR, TF, TSAMPLE and DATA parameters are all required\", \"Invalid DATA field\", \"Invalid bit symbol in DATA field\", \"Only RB=1 is supported\"; a card: \"Invalid WINDOW type\", \"Invalid FORMAT type\", \"NP value on .FFT line must be a power of 2, and >=4\", \"Illegal name in .MEASURE line\", \"Unknown mode in .MEASURE line\", \"has incomplete MEASURE line\", \"has invalid MEASURE line\", \"Unmatched expression delimiter on .MEASURE line\", \"Invalid PAR() expression syntax on .MEASURE line\", \"Invalid () expression syntax on .MEASURE line\", \"Incomplete .MEASURE line\", \"Invalid Measure Line\", \"Could not parse measured variable\", \"Only ENOB, EQN/PARAM, FIND, SDFR, SNDR, SNR and THD measure types are supported for FFT measure mode\", \"Only FIND-AT supported for FFT measure\", \".SENS line is mis-formatted\"; a spelling: \"Invalid notation encountered\", \"Syntax error in number of nodes in expression\", \"Unrecognized character\". Xyce Reference Guide 7.10 §2.1.8 (`.FFT`: WINDOW, FORMAT, NP)",
                explain: "Xyce stops on this line before it runs the deck (a field count its card does not take, a second value on an R, L or C, a PWL with no points or a repeat time outside them, a letter Xyce has no device for, a diode with no model, a second coupling value, a PAT source's times or data, an `.FFT` window, a `.MEASURE` or `.SENS` Xyce cannot read, an unclosed brace or quote), so the line has no meaning to write. It is left out and the rest of the deck is written; fix the line in the source.",
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
            /// Letters after a neutral number's suffix (`1kohm`, `10uF`).
            P0020 Note {
                title: "letters after a number",
                cite: "docs/expressions.md \"Numbers\" (D16): a number ends at its SI suffix `f p n u m k meg g t`; ngspice manual §2.1.3 and LTspice help \"General Structure and Conventions\" disagree on what trailing letters mean (`1a`, `1M`)",
                explain: "A neutral number is a decimal, an optional exponent and at most one SI suffix (`f p n u m k meg g t`, any case; `m` is milli, `meg` mega). Nothing may follow the suffix, because the simulators read trailing letters differently. Write `1k`, not `1kohm`.",
            }
            /// A ternary used as another ternary's condition or true branch, bare.
            P0021 Note {
                title: "a nested ternary without parentheses",
                cite: "docs/expressions.md \"Operators\" (D16); ngspice-45 numparam reads `a?b?c:d:e` as `a?c:e`, C as `a?(b?c:d):e` (`behaviour::ExprEval::Ngspice`)",
                explain: "A ternary used as the condition or as the true branch of another ternary must be in parentheses: the simulators group the bare form differently. Write `a ? (b ? c : d) : e`. A ternary in the else branch needs none (`a ? b : c ? d : e`).",
            }
            /// Two comparisons of one level chained (`a==b==c`, `a<b<c`).
            P0022 Note {
                title: "a comparison chained without parentheses",
                cite: "docs/expressions.md \"Operators\" (D16): `== !=` and `< <= > >=` do not associate",
                explain: "`a==b==c` and `a<b<c` compare the 0 or 1 of the first comparison with the third operand, which is rarely what the writer of the expression meant, so the neutral syntax has no chained comparison. Write `(a==b)==c` for that reading, or `a<b && b<c` for a range.",
            }
            /// `^` in a neutral expression: power in some dialects, exclusive or in others.
            P0023 Note {
                title: "`^` in a neutral expression",
                cite: "docs/expressions.md \"Operators\" (D16); the definitions' `caret` rows: power in ngspice (manual §2.8.5) and HSPICE, exclusive or in Xyce, LTspice and VACASK, no operator in Spectre",
                explain: "`^` is a power in ngspice and HSPICE and an exclusive or in Xyce, LTspice and VACASK, so it has no neutral meaning. Write `**` for a power.",
            }
            /// A constant argument outside its operator's or function's domain.
            P0024 Note {
                title: "a constant outside its function's domain",
                cite: "docs/expressions.md \"Domains\" (D16): reals only; C99 7.12.7.4 (`pow`: a domain error for a negative base and a non-integer exponent), 7.12.7.5 (`sqrt`), 7.12.6.7 (`log`)",
                explain: "The expression has no real value whatever the circuit does: a negative constant raised to a non-integer power (`(-8)**0.5`), `sqrt` or `log` of a negative constant, `log(0)`, or a division by the constant 0. Fix the constant. An argument that depends on the circuit is checked when the deck is written (W0137).",
            }
            /// `ddt` where no time derivative exists: outside a B source and a charge.
            P0025 Note {
                title: "`ddt` outside a B source or a charge",
                cite: "docs/expressions.md \"Functions\" (D16); ngspice manual §5.1 (`ddt` in a B source); Xyce Reference Guide 7.10 §2.2 (`ddt` in device expressions, not in `.param`)",
                explain: "`ddt(x)` is the time derivative of a circuit quantity, so it has a value only in a behavioural source or a charge expression. A parameter, a model card or an analysis argument has no time. Move the derivative into a B source and name its output node.",
            }
            /// A call whose name is no neutral function and no `.func` of the deck.
            P0026 Note {
                title: "a function the neutral syntax does not define",
                cite: "docs/expressions.md \"Functions\" (D16): the neutral function set (`render::spelling::Func`)",
                explain: "The name called is none of the neutral functions and none of the deck's own `.func` definitions, so no target spelling is known for it. Spell it with the neutral set (`log` is natural, `round` rounds half away from zero, `int` truncates), or define it with `.func`.",
            }
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
