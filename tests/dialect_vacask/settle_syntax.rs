//! The settle decks of the reader's syntax rules (`tests/fixtures/dialect/vacask/settle/`):
//! what VACASK 36be560 printed for an instance's terminal count (R61), an unclosed bracket or
//! string (P2012), and a statement it has no syntax for (P2013, P2014), asserted.
//!
//! Contents: tests only.

use super::settle::{run, says};

// ---- private ----

#[test]
fn fwd_an_instance_with_too_many_terminals_is_rejected_a_short_one_runs() {
    // R61, A36: more terminals than the subcircuit's ports is VACASK's error (InvalidInSource
    // P2109); fewer gives the missing ones internal nodes and runs (Private P2104).
    assert!(says(
        &run("vacask", "fwd_long.sim"),
        "Too many terminals specified at instantiation."
    ));
    assert!(says(
        &run("vacask", "fwd_short.sim"),
        "Running analysis 'op1'."
    ));
}

#[test]
fn unclosed_a_bracket_joins_lines_until_it_closes_and_an_unclosed_one_is_rejected() {
    // P2012: VACASK reads on across line breaks while (, [ or { is open (p2, b2, c2 run), and
    // rejects a statement whose bracket or string never closes (pair agent's paren deck).
    for (deck, message) in [
        (
            "p1",
            "Parser syntax error, unexpected =, expecting identifier or integer or )",
        ),
        (
            "b1",
            "Parser syntax error, unexpected identifier, expecting ] or \",\"",
        ),
        (
            "c1",
            "Parser syntax error, unexpected identifier, expecting } or \",\"",
        ),
        ("q1", "Syntax error, unexpected newline in string literal."),
        ("e1", "Parser syntax error, unexpected identifier"),
    ] {
        let r = run("vacask", &format!("unclosed_{deck}.sim"));
        assert!(says(&r, message), "{deck}");
    }
    for deck in ["p2", "b2", "c2"] {
        let r = run("vacask", &format!("unclosed_{deck}.sim"));
        assert!(says(&r, "Running analysis 'op1'."), "{deck}");
    }
}

#[test]
fn star_a_statement_vacask_has_no_syntax_for_is_rejected_except_outside_an_included_section() {
    // Item 6b: `* a star line` (and every ngspice card: run fx) is "Syntax error, unexpected
    // string." in a deck and inside the section a deck includes; outside every section of a
    // library included by `section=`, VACASK skips it, junk included (`foo bar baz(`, `.model`).
    for deck in ["star_deck.sim", "star_in_section.sim"] {
        let r = run("vacask", deck);
        assert!(says(&r, "Syntax error, unexpected string."), "{deck}");
    }
    for deck in ["star_outside_section.sim", "junk_outside_section.sim"] {
        let r = run("vacask", deck);
        assert!(says(&r, "Running analysis 'op1'."), "{deck}");
    }
}

#[test]
fn str_escapes_a_string_reads_backslash_escapes_and_holds_no_line_break() {
    // Item 6c: `\"`, `\\`, `\n`, `\t` are a quote, a backslash, a line break and a tab; any other
    // escaped character is itself (`\q` is q); a raw line break ends the string with an error
    // (settle unclosed_q1). One quoter writes `"`, `\` and a line break escaped.
    let r = run("vacask", "str_escapes.sim");
    assert!(says(&r, "[q\"q] [b\\b] [n\nn] [t\tt] [xqx]"));
}
