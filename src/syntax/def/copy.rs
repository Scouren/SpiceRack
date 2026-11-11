//! A registered lexicon → its rows, owned ([`Lex`]): what
//! [`DialectDef::extends`](super::DialectDef::extends) copies when a
//! definition changes a lexicon it shared. A table lists its rows in the
//! order the lexicon reads them, each key once (the row a lookup finds).
//!
//! Cites: none.
//!
//! Contents:
//! - `Lex::of(l: &Lexicon) -> Lex`

use super::Lex;
use crate::syntax::lexicon::Lexicon;

impl Lex {
    /// `l`'s rows, owned; a table lists its rows in the order the lexicon
    /// reads them, a word only the first time (the one it finds).
    pub(super) fn of(l: &Lexicon) -> Lex {
        let owned = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut lex = Lex {
            title: l.title,
            fold_case: l.fold_case,
            comment_lines: owned(l.comment_lines),
            comment_col1: l.comment_col1.to_vec(),
            blank_led_comment: l.blank_led_comment,
            cont_skip: owned(l.cont_skip),
            comment_tails: owned(l.comment_tails),
            block_comment: l.block_comment,
            long_strings: l.long_strings,
            dollar: Some(l.dollar),
            plus: l.plus,
            backslash: Some(l.backslash),
            end: l.end,
            stop: l.stop,
            join: Some(l.join),
            quote: Some(l.quote),
            string: Some(l.string),
            string_expr: l.string_expr,
            name_escape: l.name_escape,
            ident_start: l.ident_start.to_vec(),
            ident_continue: l.ident_continue.to_vec(),
            directive: l.directive,
            head_words: l.head_words,
            heads_after: l.heads_after.to_vec(),
            operators: Vec::new(),
            expr_braces: l.expr_braces,
            caret: Some(l.caret),
            infix: l.ops.infix.to_vec(),
            right: l.ops.right.to_vec(),
            nonassoc: l.ops.nonassoc.to_vec(),
            ternary: l.ops.ternary,
            prefix: l.ops.prefix.to_vec(),
            prefix_bp: l.ops.prefix_bp,
            probes: owned(l.ops.probes),
            index: l.ops.index,
            lib_sections: l.lib_sections,
            exponent: l.numbers.exponent.to_vec(),
            scales: Vec::new(),
            tail: Some(l.numbers.tail),
            arithmetic: Some(l.numbers.arithmetic),
            keywords: Vec::new(),
            named: l.named,
            devices: Vec::new(),
            masters: Vec::new(),
            analyses: Vec::new(),
            regions: l
                .regions
                .iter()
                .map(|r| (r.0.to_string(), r.1.lang))
                .collect(),
            regions_registered: l.regions_registered,
        };
        for &(s, t) in l.operators {
            first(&mut lex.operators, s.to_string(), t);
        }
        for &(s, k) in l.numbers.scales {
            first(&mut lex.scales, s.to_string(), k);
        }
        for &(s, t) in l.keywords.iter().flat_map(|t| t.iter()) {
            first(&mut lex.keywords, s.to_string(), t);
        }
        for d in l.devices.iter().flat_map(|t| t.iter()) {
            lex.devices.push((d.word.to_string(), *d));
        }
        for &(s, m) in l.masters {
            first(&mut lex.masters, s.to_string(), m);
        }
        for &(s, a) in l.analyses {
            first(&mut lex.analyses, s.to_string(), a);
        }
        lex
    }
}

// ---- private ----

/// Adds `key`'s row to `v` unless `v` holds the key: the row a reader
/// finds first.
fn first<K: PartialEq, V>(v: &mut Vec<(K, V)>, key: K, val: V) {
    if !v.iter().any(|r| r.0 == key) {
        v.push((key, val));
    }
}
