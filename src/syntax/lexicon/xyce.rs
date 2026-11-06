//! Xyce's lexicon rules, pinned by tests: the rows `dialects/xyce.rs`
//! declares.
//!
//! Cites (as the old Xyce grammar did; RG = Xyce Reference Guide 7.9, checked
//! on Xyce 7.10): §2.1.39 p.163 (line 1 the title); §2.1.39.1 (`*` comment
//! lines, a blank-led line is a comment); §2.1.39.2 (`;` anywhere, quotes
//! and braces included); §2.1.39.3 (`+` continuation); §2.1.6 (`.END`);
//! §3.1 p.837 and Table 6-2 p.852 (scale factors, `X` mega, no atto); §2.2
//! p.164 (`{expr}` and `'expr'`); Table 2-32 (operators); p.165 (`:` in
//! names).
//!
//! Contents: tests only.

#[cfg(test)]
mod tests {
    use crate::syntax::Lang;
    use crate::syntax::lexicon::Lexicon;
    use crate::syntax::{test_numbers as numbers, test_words as words};

    #[test]
    fn blank_led_lines_are_comments_and_semicolons_cut_quotes() {
        // §2.1.39.1, §2.1.39.2.
        let w = words(
            "t\n  R0 a b 1\nR1 a b 1 ; c\nR2 a b \"x;y\"\n  + 2\n",
            lex(Lang::XYCE),
        );
        assert_eq!(w, ["R1 a b 1", "R2 a b \"x 2"]);
    }

    #[test]
    fn end_only_in_column_one() {
        // §2.1.6: ` .end` is a blank-led comment line, `.end` ends the netlist.
        assert_eq!(
            words(
                "t\nR1 a b 1\n .end\nR2 a b 2\n.END\nR3 a b 3\n",
                lex(Lang::XYCE)
            ),
            ["R1 a b 1", "R2 a b 2"]
        );
    }

    #[test]
    fn x_is_mega_and_a_is_no_scale() {
        // §3.1 p.837: `1a` is 1 with a unit.
        assert_eq!(
            numbers("t\nR1 a b 1x 1a 1M\n", lex(Lang::XYCE)),
            [1e6, 1.0, 1e-3]
        );
    }

    fn lex(l: Lang) -> &'static Lexicon {
        l.lexicon().expect("a reader")
    }
}
