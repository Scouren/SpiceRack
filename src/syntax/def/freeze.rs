//! A [`DialectDef`] → checked, then frozen into the `&'static` tables of a
//! new row ([`lang::append`]): its lexicon and spelling leaked once, every
//! text a `&'static str`, so the tokenizer, parser and writers read a
//! registered language's tables as they read a built-in's.
//!
//! Cites: none.
//!
//! Contents:
//! - `pub fn register(def: DialectDef) -> Result<Lang, RegisterError>`, `pub enum RegisterError`

use super::{DialectDef, Lex, leak};
use crate::syntax::Lang;
use crate::syntax::diag::{self, Kind};
use crate::syntax::lang::{self, CONSTRUCTS, Cells, Row};
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, Join, Lexicon, Numbers, Ops, Quote, Quoting, Tail,
};
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::spelling::Spelling;

/// Why a definition was not registered.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegisterError {
    /// A language of this name is registered.
    #[error("a language named '{0}' is registered")]
    Taken(String),
    /// The name is empty.
    #[error("a language needs a name")]
    Unnamed,
    /// No cell answers the construct.
    #[error("'{0}': no cell for {1:?}")]
    Unanswered(String, Construct),
    /// A cell's code is not a code of the kind it records.
    #[error("'{0}': {1:?} records {2:?}, not a {3:?} code")]
    WrongKind(String, Construct, diag::Code, Kind),
    /// A region names a language with no lexicon.
    #[error("'{0}': region '{1}' names a language with no reader")]
    Readerless(String, String),
    /// Every slot is taken.
    #[error("{} languages are registered", lang::LANGS_MAX)]
    Full,
}

/// Registers `def`: checks it, freezes its tables and returns the handle
/// of its new row.
///
/// # Errors
/// [`RegisterError`]: the name is empty or registered, a construct has no
/// cell, a cell's code is of the wrong kind, a region names a language
/// with no lexicon, or the registry is full. Nothing is registered then.
pub fn register(def: DialectDef) -> Result<Lang, RegisterError> {
    let name = def.name.clone();
    if name.is_empty() {
        return Err(RegisterError::Unnamed);
    }
    if Lang::from_name(&name).is_some() {
        return Err(RegisterError::Taken(name));
    }
    let cells = cells(&def)?;
    if let Some(l) = &def.lex
        && let Some(r) = l.regions.iter().find(|r| r.1.lexicon().is_none())
    {
        return Err(RegisterError::Readerless(name, r.0.clone()));
    }
    lang::append(|me| row(def, me, cells)).ok_or(RegisterError::Full)
}

// ---- private ----

/// The cells, each construct answered with a code of its kind.
fn cells(def: &DialectDef) -> Result<Cells, RegisterError> {
    let mut out = [Action::Native; lang::CONSTRUCT_COUNT];
    for (i, c) in CONSTRUCTS.into_iter().enumerate() {
        let a = def.cells[i].ok_or_else(|| RegisterError::Unanswered(def.name.clone(), c))?;
        let want = match a {
            Action::Refuse(code) => Some((code, Kind::Refusal)),
            Action::Spell(Alt::Omit(code) | Alt::Copy(code)) => Some((code, Kind::Note)),
            Action::Native | Action::Spell(_) | Action::Lift(_) => None,
        };
        if let Some((code, kind)) = want
            && diag::info(code).kind != kind
        {
            return Err(RegisterError::WrongKind(def.name.clone(), c, code, kind));
        }
        out[i] = a;
    }
    Ok(out)
}

/// `def`'s row, at handle `me`.
fn row(def: DialectDef, me: Lang, cells: Cells) -> Row {
    // Its text is its text family's: a lexicon reads that language's text.
    let family = def.text_family.unwrap_or(me);
    let lexicon = match def.lex {
        Some(l) => Some(&*Box::leak(Box::new(lexicon(l, family)))),
        None => def.shared,
    };
    let write_lexicon = match def.sp.write_lexicon {
        Some(l) => l.lexicon(),
        None => lexicon,
    };
    let sp = def.sp;
    let spelling = Spelling {
        lang: me,
        lexicon: write_lexicon.expect("a writer spells from some lexicon"),
        comment: leak(&sp.comment),
        path_quote: leak(&sp.path_quote),
        expr: (leak(&sp.expr.0), leak(&sp.expr.1)),
        func: sp.func,
        subckt_params: sp.subckt_params.as_deref().map(leak),
        ngspice_numbers: sp.ngspice_numbers,
    };
    let mut behaviours = def.b;
    behaviours.text_family = family;
    Row {
        name: leak(&def.name),
        lexicon,
        spelling: Box::leak(Box::new(spelling)),
        cells,
        behaviours,
    }
}

/// `l` frozen as a lexicon of `lang`'s text.
fn lexicon(l: Lex, lang: Lang) -> Lexicon {
    let strs = |v: Vec<String>| slice(v.iter().map(|s| leak(s)).collect());
    Lexicon {
        lang,
        title: l.title,
        fold_case: l.fold_case,
        comment_lines: strs(l.comment_lines),
        comment_col1: slice(l.comment_col1),
        blank_led_comment: l.blank_led_comment,
        cont_skip: strs(l.cont_skip),
        comment_tails: strs(l.comment_tails),
        block_comment: l.block_comment,
        long_strings: l.long_strings,
        dollar: l.dollar.unwrap_or(Dollar::Text),
        plus: l.plus,
        backslash: l.backslash.unwrap_or(Backslash::None),
        end: l.end,
        stop: l.stop,
        join: l.join.unwrap_or(Join::None),
        quote: l.quote.unwrap_or(Quote::Plain),
        string: l.string.unwrap_or(Quoting {
            escapes: false,
            protects: false,
            across_breaks: false,
        }),
        string_expr: l.string_expr,
        name_escape: l.name_escape,
        ident_start: slice(l.ident_start),
        ident_continue: slice(l.ident_continue),
        directive: l.directive,
        head_words: l.head_words,
        heads_after: slice(l.heads_after),
        operators: pairs(l.operators),
        expr_braces: l.expr_braces,
        caret: l.caret.unwrap_or(Caret::None),
        ops: Ops {
            infix: slice(l.infix),
            right: slice(l.right),
            prefix: slice(l.prefix),
            prefix_bp: l.prefix_bp,
            probes: strs(l.probes),
            index: l.index,
        },
        lib_sections: l.lib_sections,
        numbers: Numbers {
            exponent: slice(l.exponent),
            scales: pairs(l.scales),
            tail: l.tail.unwrap_or(Tail::Spice { dot: false }),
            arithmetic: l.arithmetic.unwrap_or(Arithmetic::Decimal),
        },
        keywords: slice(vec![pairs(l.keywords)]),
        named: l.named,
        devices: slice(vec![slice(
            l.devices
                .into_iter()
                .map(|(w, mut d)| {
                    d.word = leak(&w);
                    d
                })
                .collect(),
        )]),
        masters: pairs(l.masters),
        analyses: pairs(l.analyses),
        regions: slice(
            l.regions
                .into_iter()
                .map(|(s, lang)| (leak(&s), lang.lexicon().expect("checked: a reader")))
                .collect(),
        ),
    }
}

/// `v`'s rows, their keys leaked.
fn pairs<T>(v: Vec<(String, T)>) -> &'static [(&'static str, T)] {
    slice(v.into_iter().map(|(s, x)| (leak(&s), x)).collect())
}

fn slice<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}
