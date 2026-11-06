//! LTspice's lexicon rules, pinned by tests: the rows `dialects/ltspice.rs`
//! declares.
//!
//! Cites (as the old LTspice grammar did): ADI ltspice-reference `ai_ref/`
//! SPICE-SYNTAX-REFERENCE.md (SSR) "Netlist Structure" (title), "Comments"
//! (`*` lines, `;` anywhere), "Engineering Multipliers" (T G Meg K mil m u
//! n p f, `µ`, `6K34`); SIMULATION-COMMANDS-REFERENCE.md `.END`;
//! CIRCUIT-ELEMENTS-REFERENCE.md "Supported Operators" (`& | ^` Boolean,
//! `**`); LTspice XVII help DotParam.htm (`{expr}`).
//!
//! Contents: tests only.

#[cfg(test)]
mod tests {
    use crate::syntax::Lang;
    use crate::syntax::lexicon::Lexicon;
    use crate::syntax::{test_numbers as numbers, test_words as words};

    #[test]
    fn semicolon_comments_and_dollar_is_text() {
        // SSR "Comments": `*` lines and `;` anywhere; `$` and `//` are not.
        let w = words(
            "t\n* c\nR1 a$b 0 1k ; c\nR2 a b 2k $ x // y\n;c\n",
            lex(Lang::LTSPICE),
        );
        assert_eq!(w, ["R1 a$b 0 1k", "R2 a b 2k $ x / / y"]);
    }

    #[test]
    fn plus_reads_past_semicolon_lines() {
        let w = words("t\nR1 a\n; c\n+ b 1\n", lex(Lang::LTSPICE));
        assert_eq!(w, ["R1 a b 1"]);
    }

    #[test]
    fn multipliers_mu_and_fractions() {
        // SSR "Engineering Multipliers": `6K34` is 6.34k, `µ` micro, no atto.
        assert_eq!(
            numbers("t\nR1 a b 6K34 1µ 1Meg 2a\n", lex(Lang::LTSPICE)),
            [6340.0, 1e-6, 1e6, 2.0]
        );
    }

    fn lex(l: Lang) -> &'static Lexicon {
        l.lexicon().expect("a reader")
    }
}
