//! VACASK's lexicon rules, pinned by tests: the rows `dialects/vacask.rs`
//! declares.
//!
//! Cites (as the old VACASK grammar did; L = `lib/dfllexer.l` line, P =
//! `lib/dflparser.y` line): L159 title; L176-L187 `//` and `/* */`
//! comments; L913-L921 a newline inside brackets is no statement end, `\`
//! before the newline joins; L594-L693 numbers (`M` `x` `X` `meg` mega, `m`
//! milli, `mil`, letters after the scale, `1e3k` an error); L695 long
//! strings; L705 strings; L869 names (`'…'` parts); L783-L868 keywords at a
//! statement's head; P216-P230 and docs/expr-operators.md operators.
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
    fn brackets_hold_newlines_and_comments_go_anywhere() {
        // L913-L919, L176-L187.
        let w = words(
            "t\nr1 (a /* x\n y */ b) r // c\nr2 (a\n b) r\n",
            lex(Lang::VACASK),
        );
        assert_eq!(w, ["r1 ( a b ) r", "r2 ( a b ) r"]);
    }

    #[test]
    fn quoted_name_parts_and_long_strings() {
        // L869, L695.
        let src = "t\nsave p('x1:m1', id)\nembed \"f\" <<<E\nline // kept\n>>>E\n";
        let t = tokenize(src, lex(Lang::VACASK));
        // Title, end, `save`, `p`, `(`, then the name.
        assert_eq!(t.text(5), "'x1:m1'");
        assert_eq!(t.tags()[5], Tag::Ident);
        assert_eq!(t.tags()[12], Tag::String);
        assert_eq!(t.text(12), "<<<E\nline // kept\n>>>E");
    }

    #[test]
    fn directives_and_head_keywords() {
        let t = tokenize("t\n@if a\nmodel m r\n@end\n", lex(Lang::VACASK));
        assert_eq!(t.tags()[2], Tag::KwIf);
        assert_eq!(t.tags()[5], Tag::KwModel);
        assert_eq!(t.tags()[9], Tag::KwEndif);
    }

    #[test]
    fn numbers_and_their_errors() {
        // L594-L693: `M` mega, `1kOhm`; `1e3k` and hex are no decimal.
        let v = numbers("t\np a=1M b=1x c=1kOhm d=1e3k e=0x1F\n", lex(Lang::VACASK));
        assert_eq!(&v[..3], [1e6, 1e6, 1e3]);
        assert!(v[3].is_nan() && v[4].is_nan());
    }

    fn lex(l: Lang) -> &'static Lexicon {
        l.lexicon().expect("a reader")
    }
}
