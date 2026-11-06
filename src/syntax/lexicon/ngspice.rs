//! ngspice's lexicon rules, pinned by tests: the rows `dialects/ngspice.rs`
//! declares (ESPice reads the same text).
//!
//! Cites (the § are the labels the old ngspice grammar gives its rules,
//! `git show 9d1d6bb:grammar/ngspice.peg`): §1.2 title; §1.3 `@` ends the file,
//! blank lines; §1.5 comment lines and `$` `;` `//` tails; §1.6 `+` and `\\`
//! continuation; §1.7 `.end`; §1.9 quotes; §1.10 numbers (`INPevaluate`,
//! inpeval.c); §2.2 operators; ngspice manual §2.1.3 (scale factors). ESPice
//! `src/frontend/lines_ngspice.zig`.
//!
//! Contents: tests only.

#[cfg(test)]
mod tests {
    use crate::syntax::Lang;
    use crate::syntax::lexicon::Lexicon;
    use crate::syntax::token::Tag;
    use crate::syntax::tokenize;
    use crate::syntax::{test_numbers as numbers, test_words as words};

    #[test]
    fn comment_lines_and_tails() {
        // §1.5: `*` and the punctuation lines are comments; `$` after a
        // blank, `;` and `//` anywhere end the line; `a$b` is a name.
        let w = words(
            "t\n* c\n= c\nR1 a$b 0 1k $ c\nR2 a b 2k;c\nR3 a b 3k//c\n",
            lex(Lang::NGSPICE),
        );
        assert_eq!(w, ["R1 a$b 0 1k", "R2 a b 2k", "R3 a b 3k"]);
    }

    #[test]
    fn plus_reads_past_star_lines_but_not_semicolon_lines() {
        // §1.6 (`l_skip_cont`): `*`, `#`, `$` and blank lines; a `;` line
        // ends the statement, and the `+` after it continues nothing.
        let w = words(
            "t\nR1 a\n* c\n\n+ b 1\nR2 a\n; c\n+ b 2\n",
            lex(Lang::NGSPICE),
        );
        assert_eq!(w, ["R1 a b 1", "R2 a"]);
    }

    #[test]
    fn glue_continues_even_after_a_comment() {
        // §1.6: a trailing `\\` continues the statement with no `+`, past
        // an end-of-line comment.
        let w = words(
            "t\nR1 a b \\\\\n 1k\nR2 a $ c \\\\\nb 2k\n",
            lex(Lang::NGSPICE),
        );
        assert_eq!(w, ["R1 a b 1k", "R2 a b 2k"]);
    }

    #[test]
    fn end_is_dropped_and_reading_goes_on_and_at_stops() {
        // §1.7, §1.3.
        let t = tokenize("t\n.end\nR1 a b 1\n@\nR2 a b 2\n", lex(Lang::NGSPICE));
        let w = words("t\n.end\nR1 a b 1\n@\nR2 a b 2\n", lex(Lang::NGSPICE));
        assert_eq!(w, ["R1 a b 1"]);
        assert_eq!(t.tags()[2], Tag::KwEnd);
    }

    #[test]
    fn quotes_protect_comments() {
        // §1.9: `$` and `;` inside `'…'` and `"…"` are text.
        let w = words("t\n.param a='1 $2' b=\"x;y\"\n", lex(Lang::NGSPICE));
        assert_eq!(w, [".param a = ' 1 $2 ' b = \"x;y\""]);
    }

    #[test]
    fn numbers_read_as_inpevaluate() {
        // §1.10: `M` is milli, `meg` mega, the tail is ignored (`1k5`),
        // `d` is an exponent letter.
        let v = numbers("t\nR1 a b 1M 1meg 1k5 10v 1d3 0.1n\n", lex(Lang::NGSPICE));
        assert_eq!(v, [1e-3, 1e6, 1e3, 10.0, 1e3, 1e-10]);
        let mil = numbers("t\nR1 a b 1mil\n", lex(Lang::NGSPICE))[0];
        assert!((mil - 25.4e-6).abs() < 1e-20);
    }

    #[test]
    fn keywords_fold_and_letters_map() {
        let t = tokenize("t\n.PARAM a=1\n.Control\n", lex(Lang::NGSPICE));
        assert_eq!(t.tags()[2], Tag::KwParam);
        assert_eq!(t.tags()[7], Tag::KwControl);
        let x = lex(Lang::NGSPICE).device("X").map(|d| d.primitive);
        assert_eq!(x, Some(crate::syntax::device::Primitive::Subckt));
    }

    fn lex(l: Lang) -> &'static Lexicon {
        l.lexicon().expect("a reader")
    }
}
