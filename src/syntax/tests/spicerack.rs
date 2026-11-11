//! SpiceRack's own text (`Lang::SPICERACK`, decision D16): the neutral
//! expression grammar's parse oracle (N4 of the design note). For each
//! parse code the grammar adds, a refused fixture and its legal neighbour
//! (rule 10 of the compiler-construction skill), and the groupings the
//! levels give. The trees are derived by hand from the level table in
//! `dialects/spicerack.rs`'s header and printed as `mod.rs` prints them.
//!
//! Contents: none (tests only).

use crate::syntax::Lang;
use crate::syntax::diag::Code;

use super::{lex, read};

// ---- private ----

/// Asserts that `expr`, as a `.param` value in SpiceRack's text, reads as
/// `tree` with exactly `codes`.
fn reads(expr: &str, tree: &str, codes: &[Code]) {
    let src = format!("t\n.param p={{{expr}}}\n");
    let (top, noted) = read(&src, lex(Lang::SPICERACK));
    assert_eq!(top.len(), 1, "{expr}: one statement");
    let got = top[0]
        .strip_prefix("Param\".param\"[w\"p\"=Delimited\"{\"[")
        .and_then(|s| s.strip_suffix("]]"))
        .expect("a braced .param value");
    assert_eq!((got, noted.as_slice()), (tree, codes), "{expr}");
}

#[test]
fn p8_003_power_groups_right_and_binds_tighter_than_the_sign() {
    // Levels 8 and 9: `-2**2` is -(2**2), `2**3**2` is 2**(3**2), the
    // exponent may carry a sign, a sign binds tighter than `*`; levels 4
    // and 5: an ordering binds tighter than an equality. Checks: 5.
    reads("-2**2", "(- (** n\"2\" n\"2\"))", &[]);
    reads("2**3**2", "(** n\"2\" (** n\"3\" n\"2\"))", &[]);
    reads("2**-1", "(** n\"2\" (- n\"1\"))", &[]);
    reads("-a*b", "(* (- i\"a\") i\"b\")", &[]);
    reads("a==b<c", "(== i\"a\" (< i\"b\" i\"c\"))", &[]);
}

#[test]
fn p8_003_p0020_letters_after_a_number() {
    // A number ends at its scale: `1kohm`, atto `1a` and `1mil` are
    // P0020 as operands; `1k` and `1meg` are numbers. Outside an
    // expression the same token is a name (a model `2n3904`): no code.
    // Checks: 6.
    reads("1kohm", "n\"1kohm\"", &[Code::P0020]);
    reads("1a", "n\"1a\"", &[Code::P0020]);
    reads("1mil", "n\"1mil\"", &[Code::P0020]);
    reads("1k", "n\"1k\"", &[]);
    reads("2*1meg", "(* n\"2\" n\"1meg\")", &[]);
    let (_, codes) = read("t\nQ1 c b e 2n3904\n", lex(Lang::SPICERACK));
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn p8_003_p0021_a_nested_ternary_needs_parentheses() {
    // Level 1: a ternary as the then-branch is P0021 (the tree is still
    // read); in parentheses, or as the else branch, it is legal.
    // Checks: 3.
    reads(
        "a?b?c:d:e",
        "(? i\"a\" (? i\"b\" i\"c\" i\"d\") i\"e\")",
        &[Code::P0021],
    );
    reads(
        "a?(b?c:d):e",
        "(? i\"a\" Paren\"(\"[(? i\"b\" i\"c\" i\"d\")] i\"e\")",
        &[],
    );
    reads(
        "a?b:c?d:e",
        "(? i\"a\" i\"b\" (? i\"c\" i\"d\" i\"e\"))",
        &[],
    );
}

#[test]
fn p8_003_p0022_comparisons_do_not_associate() {
    // Levels 4 and 5: a second operator of the level is P0022 (read
    // left to right); in parentheses it is legal. Checks: 4.
    reads("a==b==c", "(== (== i\"a\" i\"b\") i\"c\")", &[Code::P0022]);
    reads("a<b<=c", "(<= (< i\"a\" i\"b\") i\"c\")", &[Code::P0022]);
    reads("a==b!=c", "(!= (== i\"a\" i\"b\") i\"c\")", &[Code::P0022]);
    reads(
        "(a==b)==c",
        "(== Paren\"(\"[(== i\"a\" i\"b\")] i\"c\")",
        &[],
    );
}

#[test]
fn p8_003_p0023_caret_is_refused_and_the_dropped_rows_are_no_operators() {
    // `^` is read as `**` and noted P0023; `**` is legal; ngspice's `<>`
    // has no row, so the expression does not parse (P0002). Checks: 3.
    reads("a^b", "(^ i\"a\" i\"b\")", &[Code::P0023]);
    reads("a**b", "(** i\"a\" i\"b\")", &[]);
    reads("a<>b", "Bad\"a\"", &[Code::P0002]);
}
