//! Spectre's lexicon rules, pinned by tests: the rows `dialects/spectre.rs`
//! declares, with the `simulator lang=spice` hook.
//!
//! Cites (as the old Spectre grammar did; § = the labels it gives its rules,
//! `git show 9d1d6bb:grammar/spectre.peg`; UG = Spectre User Guide 5.1.41, R19 = Spectre Reference 19.1): §1.2 title
//! (UG p.29, p.50); §1.3 `*` in column 1 and `//` comments (UG p.59); §1.4
//! `\` and `+` continuation (UG p.59); §2.1 case kept; §2.2 names (`\`
//! escapes, `!`); §2.4 numbers (case-sensitive scales, `M` mega, `m` milli,
//! units after the scale); §2.5 strings; §3.1 the `simulator lang=` switch
//! (UG p.53); §6.1 operators (UG p.87).
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
    fn star_only_in_column_one_and_slashes() {
        // §1.3: `*` in column 1; ` *` is text; `//` anywhere.
        let w = words(
            "t\n* c\n *x\nr1 (a b) resistor r=1k // c\n// c\n",
            lex(Lang::SPECTRE),
        );
        assert_eq!(w, ["* x", "r1 ( a b ) resistor r = 1k"]);
    }

    #[test]
    fn backslash_and_plus_continue() {
        // §1.4 (UG p.59).
        let w = words("t\nr1 (a b) \\\n resistor\n+ r=1\n", lex(Lang::SPECTRE));
        assert_eq!(w, ["r1 ( a b ) resistor r = 1"]);
    }

    #[test]
    fn escaped_names_keep_their_backslash() {
        // §2.2: `\2N2222` is a name.
        let t = tokenize("t\nq1 (c b e) \\2N2222\n", lex(Lang::SPECTRE));
        assert_eq!(t.tags()[8], Tag::Ident);
        assert_eq!(t.text(8), "\\2N2222");
    }

    #[test]
    fn keywords_at_heads_only() {
        // `section=tt` is a parameter, `} else {` a head again.
        let t = tokenize(
            "t\ninclude \"a\" section=tt\n} else {\n",
            lex(Lang::SPECTRE),
        );
        let tags = t.tags();
        assert_eq!(tags[2], Tag::KwInclude);
        assert_eq!(tags[4], Tag::Ident);
        assert_eq!(tags[9], Tag::KwElse);
    }

    #[test]
    fn open_parentheses_join_the_next_line() {
        // R19 p.491: a condition runs on over lines; comment lines between
        // are skipped; a statement holds one end of statement, its last.
        let src = "t\nif (a &&\n// c\n b) {\nr1 (x y) resistor\n}\n";
        assert_eq!(
            words(src, lex(Lang::SPECTRE)),
            ["if ( a && b ) {", "r1 ( x y ) resistor", "}"]
        );
        let t = tokenize(src, lex(Lang::SPECTRE));
        let ends = t
            .tags()
            .iter()
            .filter(|&&x| x == Tag::EndOfStatement)
            .count();
        assert_eq!(ends, 4, "the title and three statements");
    }

    #[test]
    fn scales_are_case_sensitive() {
        // §2.4: `M` mega, `m` milli, units after the scale; `1k5` is none.
        let v = numbers(
            "t\np a=1M b=1m c=1pf d=5% e=50_Ohms f=1k5\n",
            lex(Lang::SPECTRE),
        );
        assert_eq!(&v[..5], [1e6, 1e-3, 1e-12, 0.05, 50.0]);
        assert!(v[5].is_nan());
    }

    #[test]
    fn lang_spice_region_reads_as_hspice() {
        // §3.1 (UG p.53): the region's `.param` is a keyword, `$` a comment,
        // `+` a continuation; `simulator lang=spectre` (any case) goes back.
        let src =
            "t\nsimulator lang=spice\n.param a=1M $ c\n+ b=2\nSIMULATOR LANG=SPECTRE\np c=1M\n";
        let t = tokenize(src, lex(Lang::SPECTRE));
        assert_eq!(
            words(src, lex(Lang::SPECTRE)),
            [
                "simulator lang = spice",
                ".param a = 1M b = 2",
                "SIMULATOR LANG = SPECTRE",
                "p c = 1M"
            ]
        );
        let at = |tag| t.tags().iter().position(|&x| x == tag).unwrap() as u32;
        assert!(std::ptr::eq(
            t.lexicon_at(at(Tag::KwParam)),
            lex(Lang::HSPICE)
        ));
        assert_eq!(numbers(src, lex(Lang::SPECTRE)), [1e-3, 2.0, 1e6]);
        assert!(at(Tag::LangSpice) < at(Tag::KwParam) && at(Tag::KwParam) < at(Tag::LangSpectre));
    }

    fn lex(l: Lang) -> &'static Lexicon {
        l.lexicon().expect("a reader")
    }
}
