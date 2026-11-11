//! `Build` (`build.rs`) against the reader: a deck appended chunk by chunk
//! is, row for row, the parse of the text it wrote, and passes the
//! structural validator (`Ast::check`). The oracle is the tokenizer and
//! parser over the built text (DESIGN-python-ast-builders §3, the
//! differential); `unlike_parse` itself is held to a hand-broken tree.
//!
//! Contents: none (tests only).

use crate::syntax::Lang;
use crate::syntax::ast::{NONE, Tag};
use crate::syntax::build::{Build, unlike_parse};
use crate::syntax::token::Tokens;

/// A deck's lines after its title, cut where no block or region is open:
/// every statement tag an ngspice-given deck reads, with each word kind a
/// row can hold (a list, a header, a body with a chained branch and a
/// closer, a group, a call, a probe, a ternary's pair, VACASK's index
/// tail and vectors in its region, a `Verbatim`, a `Bad`).
const BLOCKS: [&str; 16] = [
    "* a comment line\n\n.param w=1u r = a + 1 s='-1*x'\n",
    ".func f(x) {x*2}\n.model m cpl (c=1e-10 -6e-11 r=1)\n",
    ".subckt cell a b params: k=2\nR1 a b {k ? 1k : 2k}\n.subckt inner p\nC1 p 0 1p ic=0,-5\n.ends inner\nX1 a b inner\n.ends cell\n",
    ".if (w == 1)\nR2 a 0 1\n.elseif (w)\n.else\nR3 a 0 {max(w, 1)}\n.endif\n",
    "V1 in 0 dc 0 ac 1\n+ sin(0 1 1k)\n",
    "B1 out 0 v={v(in,0)*2}\n",
    ".include \"m.lib\"\n.lib 'c.lib' tt\n",
    ".lib tt\n.param q=1\n.endl tt\n",
    ".control\ntran 1n 10n\nrun\n.endc\n",
    "5 a b\n",
    ".param bad={1 2} worse={1+\n",
    "simulator lang=spectre\nsweep1 sweep param=r start=1k stop=2k step=1k {\n  ac1 ac\n}\nsimulator lang=spice\n",
    "simulator lang=vacask\nparameters v=[1, 2] l={1, 2} y=v[1]\nsimulator lang=spice\n",
    ".options reltol=1e-3 klu\n.save v(a)\n.meas tran x find v(a) at=1n\n.ic v(a)=1\n",
    "X2 in out cell k=3",
    ".tran 1n 10n\n.end\n",
];

// ---- private ----

/// A deck titled `my deck` built from `chunks` in order: where its rows
/// part from the parse of its own text (`None`: nowhere).
fn built(chunks: &[&str]) -> Option<String> {
    let lex = Lang::NGSPICE_GIVEN.lexicon().expect("a reader");
    let mut b = Build::new("my deck", lex);
    for c in chunks {
        b.text(c);
    }
    let (src, rows, ast) = b.finish();
    let t = Tokens::from_rows(&src, lex, rows);
    assert_eq!(ast.check(&t), Ok(()), "{src}");
    unlike_parse(&t, &ast)
}

#[test]
fn p8_140_a_deck_built_as_one_chunk_is_the_parse_of_its_text() {
    // Oracle: `parse(tokenize(text))`, row for row. One chunk: the rows
    // move by the title's two tokens and one node only. Checks: 2.
    let whole: String = BLOCKS
        .iter()
        .map(|b| format!("{}\n", b.trim_end()))
        .collect();
    assert_eq!(built(&[whole.as_str()]), None);
}

#[test]
fn p8_140_a_deck_built_chunk_by_chunk_is_the_parse_of_its_text() {
    // Oracle: the same. Sixteen chunks: every later chunk's tokens, nodes,
    // extra words and names land past the earlier ones', so each word kind
    // is moved by a base that is not zero. Checks: 2.
    assert_eq!(built(&BLOCKS), None);
}

#[test]
fn p8_140_a_title_alone_and_an_empty_chunk_build() {
    // ngspice manual §1.2: line 1 is the title, whatever it holds.
    // Checks: 4.
    assert_eq!(built(&[]), None);
    assert_eq!(built(&["", "* only a comment\n", ""]), None);
}

#[test]
fn p8_140_a_name_met_again_keeps_its_first_id() {
    // `ast.rs`: names are numbered in first-seen order. `a` and `R1` are
    // met in two chunks and `out` only in the second: the reader's order
    // over the whole text is the oracle. Checks: 2.
    assert_eq!(built(&["R1 a b 1k\n", "R2 out a {R1}\nR1 b a 1\n"]), None);
}

#[test]
fn p8_140_unlike_parse_names_the_first_row_that_differs() {
    // Oracle: a tree broken by hand. `t` / `R1 a b 1k` is the root, the
    // title, four words and the instance (nodes 0..=6); a seventh row is
    // one the reader does not make of that text. Checks: 2.
    let lex = Lang::NGSPICE_GIVEN.lexicon().expect("a reader");
    let mut b = Build::new("t", lex);
    b.text("R1 a b 1k\n");
    let (src, rows, mut ast) = b.finish();
    let t = Tokens::from_rows(&src, lex, rows);
    assert_eq!(unlike_parse(&t, &ast), None);
    ast.push(Tag::End, 0, NONE, NONE);
    let why = unlike_parse(&t, &ast).expect("a row the reader does not make");
    assert!(why.starts_with("node 7: built Some("), "{why}");
}
