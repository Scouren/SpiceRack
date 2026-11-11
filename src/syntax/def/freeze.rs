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
use crate::syntax::behaviour::DotRule;
use crate::syntax::diag::{self, Code, Kind};
use crate::syntax::lang::{self, CONSTRUCTS, Cells, Row};
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, Join, Lexicon, Numbers, Ops, Quote, Quoting, Tail,
};
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::spelling::Spelling;

/// Why a definition was not registered.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegisterError {
    /// A language of this name is registered with another definition.
    #[error("a language named '{0}' is registered with another definition")]
    Redefined(String),
    /// The name is empty.
    #[error("a language needs a name")]
    Unnamed,
    /// No cell answers the construct.
    #[error("'{0}': no cell for {1:?}")]
    Unanswered(String, Construct),
    /// A cell's code is not a code of the kind it records.
    #[error("'{0}': {1:?} records {2:?}, not a {3:?} code")]
    WrongKind(String, Construct, diag::Code, Kind),
    /// A cell names a code the definition did not declare (`refuse_code`,
    /// `omit_code`), of the kind it records; or raises `W0201`/`W0202`
    /// with no code of its own (`refuse`, `omit`).
    #[error("'{0}': {1:?} names no declared {3:?} code '{2}'")]
    UnknownCode(String, Construct, String, Kind),
    /// A `dot_rule`'s `DotRule::Note` names a code that is no note.
    #[error("'{0}': dot_rule '{1}' notes {2:?}, not a Note code")]
    DotRuleKind(String, String, diag::Code),
    /// A region names a language with no lexicon.
    #[error("'{0}': region '{1}' names a language with no reader")]
    Readerless(String, String),
    /// `runs_on` names a simulator whose language the definition does not
    /// extend: the definition, the simulator, the simulator's language.
    #[error(
        "'{0}': runs_on('{1}'), but it does not extend '{2}', the language simulator '{1}' reads"
    )]
    RunsOn(String, String, &'static str),
    /// Every slot is taken (or the code table is full).
    #[error("{} languages are registered", lang::LANGS_MAX)]
    Full,
}

/// Registers `def`: checks it, freezes its tables and returns the handle
/// of its new row. A name registered with an equal definition (the same
/// [`Lang::dump`]) returns that language: registration is idempotent.
///
/// # Errors
/// [`RegisterError`]: the name is empty or registered with another
/// definition, a construct has no cell, a cell's code is of the wrong kind
/// or undeclared, a `dot_rule` notes with a code that is no note, a region
/// names a language with no lexicon, `runs_on`
/// names a simulator outside the `extends` chain, or the registry is full.
/// Nothing is registered then.
pub fn register(def: DialectDef) -> Result<Lang, RegisterError> {
    let name = def.name.clone();
    if name.is_empty() {
        return Err(RegisterError::Unnamed);
    }
    let cells = cells(&def)?;
    if let Some(&(w, DotRule::Note(code))) = def
        .b
        .dot_rules
        .iter()
        .find(|r| matches!(r.1, DotRule::Note(c) if diag::info(c).kind != Kind::Note))
    {
        return Err(RegisterError::DotRuleKind(name, w.into(), code));
    }
    if let Some(l) = &def.lex
        && let Some(r) = l.regions.iter().find(|r| r.1.lexicon().is_none())
    {
        return Err(RegisterError::Readerless(name, r.0.clone()));
    }
    if let Some(on) = def.runs_on {
        let chain = std::iter::successors(def.base, |l| l.base());
        if !chain.into_iter().any(|l| l == on.dialect) {
            let reads = on.dialect.name();
            return Err(RegisterError::RunsOn(name, on.sim.to_string(), reads));
        }
    }
    // ponytail: a held name's candidate row is built (and leaked) to be
    // dumped; bounded by how often a caller re-registers a name.
    let held = match lang::append(&name, |me, first| row(def.clone(), me, cells, first)) {
        Ok(me) => return Ok(me),
        Err(None) => return Err(RegisterError::Full),
        Err(Some(held)) => held,
    };
    match super::dump(&row(def, held, cells, 0)) == held.dump() {
        true => Ok(held),
        false => Err(RegisterError::Redefined(name)),
    }
}

// ---- private ----

/// The cells, each construct answered with a code of its kind; a
/// definition's own code declared, of the kind its cell records.
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
        let own = matches!(
            a,
            Action::Refuse(Code::W0201) | Action::Spell(Alt::Omit(Code::W0202))
        );
        if let Some((code, kind)) = want
            && (own || matches!(code, Code::W0201 | Code::W0202))
        {
            let id = def.code_ids[i].as_deref().unwrap_or(match code {
                Code::W0201 => "W0201",
                _ => "W0202",
            });
            let declared = def.codes.iter().any(|d| d.0 == id && d.1 == kind);
            if !own || !declared {
                return Err(RegisterError::UnknownCode(
                    def.name.clone(),
                    c,
                    id.into(),
                    kind,
                ));
            }
        }
        out[i] = a;
    }
    Ok(out)
}

/// `def`'s row, at handle `me`, its codes at global indices `first..`.
fn row(def: DialectDef, me: Lang, cells: Cells, first: u16) -> Row {
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
    let details = std::array::from_fn(|i| match &def.code_ids[i] {
        Some(id) => {
            let at = def.codes.iter().position(|c| &c.0 == id);
            first + at.expect("checked: a declared code") as u16
        }
        None => 0,
    });
    let codes = def.codes.iter().map(|c| (leak(&c.0), c.1, leak(&c.2)));
    let pairs2 =
        |v: &[(String, String)]| slice(v.iter().map(|m| (leak(&m.0), leak(&m.1))).collect());
    Row {
        name: leak(&def.name),
        lexicon,
        spelling: Box::leak(Box::new(spelling)),
        cells,
        details,
        behaviours,
        codes: slice(codes.collect()),
        first_code: first,
        heads: slice(def.heads.iter().map(|h| (h.0, leak(&h.1))).collect()),
        write_masters: pairs2(&def.write_masters),
        options: pairs2(&def.options),
        hooks: slice(def.hooks.iter().map(|h| (leak(&h.0), h.1)).collect()),
        runs_on: def.runs_on,
        base: def.base,
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
            nonassoc: slice(l.nonassoc),
            ternary: l.ternary,
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
        regions_registered: l.regions_registered,
    }
}

/// `v`'s rows, their keys leaked.
fn pairs<T>(v: Vec<(String, T)>) -> &'static [(&'static str, T)] {
    slice(v.into_iter().map(|(s, x)| (leak(&s), x)).collect())
}

fn slice<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}
