//! HSPICE's lexicon rules, pinned by tests: the rows `dialects/hspice.rs`
//! declares; it is also read inside a Spectre `simulator lang=spice`
//! region.
//!
//! Cites (the rule ids the old HSPICE grammar labels its rules with,
//! `git show 9d1d6bb:grammar/hspice.peg`; UG08 = HSPICE User Guide 2008, CR10 =
//! Command Reference 2010): L1 title; L2 comment lines; L3 the `$` rule; L5
//! `+` continuation; L6 `\` and `\\` breaks; L9 `.END`; T1 separators; T2
//! quotes (`$` is text inside); T3 numbers (`X` is mega, `D` an exponent);
//! X2 operators (UG08 pp.279-283). ESPice `src/frontend/lines_hspice.zig`
//! (`stripComment`, `isNumberToken`: the `$` hook).
//!
//! Contents: tests only.

#[cfg(test)]
mod tests {
    use crate::syntax::Lang;
    use crate::syntax::lexicon::Lexicon;
    use crate::syntax::{test_numbers as numbers, test_words as words};

    #[test]
    fn dollar_is_a_comment_after_a_separator_or_a_number() {
        // L3: `$` after a blank, `=` or a number (`1k$c` is `1k`, `-1w$c` is
        // `-1w`); inside a name it is a name byte (`a$b`).
        let w = words(
            "t\nR1 a$b 0 1k$c\nR2 a b r=$c\nV1 a b -1w$c\nR3 a b r1k_x$c\n",
            lex(Lang::HSPICE),
        );
        assert_eq!(
            w,
            ["R1 a$b 0 1k", "R2 a b r =", "V1 a b - 1w", "R3 a b r1k_x$c"]
        );
    }

    #[test]
    fn dollar_is_text_inside_quotes() {
        // T2.
        let w = words("t\n.param a='1 $2' $ c\n", lex(Lang::HSPICE));
        assert_eq!(w, [".param a = ' 1 $2 '"]);
    }

    #[test]
    fn backslash_breaks_glue_or_keep_the_blank() {
        // L6: `\\` joins with nothing between, a blank and `\` keeps it;
        // neither needs a `+` on the next line.
        let src = "t\nR1 ab\\\\\ncd 1\nR2 a \\\nb 2\n";
        let t = crate::syntax::tokenize(src, lex(Lang::HSPICE));
        assert_eq!(words(src, lex(Lang::HSPICE)), ["R1 ab cd 1", "R2 a b 2"]);
        // `ab` and `cd` are two tokens with only the glue between them.
        assert_eq!(&src[t.end(3) as usize..t.start(4) as usize], "\\\\\n");
    }

    #[test]
    fn end_stops_the_run() {
        // L9.
        assert_eq!(
            words("t\nR1 a b 1\n.END\nR2 a b 2\n", lex(Lang::HSPICE)),
            ["R1 a b 1"]
        );
    }

    #[test]
    fn x_is_mega_and_d_an_exponent() {
        // T3.
        assert_eq!(
            numbers("t\nR1 a b 1x 2.5D-3 1M 10pf\n", lex(Lang::HSPICE)),
            [1e6, 2.5e-3, 1e-3, 1e-11]
        );
    }

    fn lex(l: Lang) -> &'static Lexicon {
        l.lexicon().expect("a reader")
    }
}
