//! A plugin target's written deck → its own heads, masters and option
//! names, and its hooked cards answered: one pass over the text the
//! family's writer wrote (`DIALECT-PLUGIN.md` §6.2, §6.3). It runs only
//! for a target whose row declares a map or a hook; no built-in does, so
//! their decks never reach it.
//!
//! A SPICE card is a line and its `+` lines, its head the first word; a
//! named statement is a line and the lines a trailing `\` joins, its
//! master the word after `name [(nodes)]` (`model name master`). Comment
//! lines are left as written. Text a region keeps as written is respelled
//! too (right for a plugin target: its text is the target's).
//!
//! Cites: none.
//!
//! Contents:
//! - `pub struct Request`, `pub struct Pending`
//! - `pub fn card_fields(text: &str) -> (&str, Vec<&str>, Vec<(&str, &str)>)`
//! - `pub(super) fn applies(target: Lang) -> bool`
//! - `pub(super) fn pass(text: &str, target: Lang, src: &Tokens, diags: &mut Vec<Diag>) -> (String, Vec<Request>)`
//! - `pub(super) fn refusal(target: Lang, id: &str, token: u32) -> Diag`

use std::ops::Range;

use crate::syntax::Lang;
use crate::syntax::behaviour::Writer;
use crate::syntax::def::{Answer, Card, Hook};
use crate::syntax::diag::{Code, Diag};
use crate::syntax::token::{Tag, Tokens};

/// A hooked card an external hook answers ([`Hook::External`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The hook's id.
    pub hook: u16,
    /// The card's bytes in [`Pending::text`].
    pub span: Range<u32>,
    /// The source token a refusal of it is anchored at.
    pub anchor: u32,
}

/// A deck written but for its external hooks' answers
/// (`render::render_pending`); `render::splice` completes it.
#[derive(Debug)]
pub struct Pending {
    pub text: String,
    /// One per card an external hook answers, in output order.
    pub requests: Vec<Request>,
    pub(super) target: Lang,
    pub(super) diags: Vec<Diag>,
}

/// One hooked card: its span, the hook row it is answered by, its joined
/// text and its anchor.
struct Hooked {
    span: Range<usize>,
    hook: usize,
    text: String,
    anchor: u32,
}

/// A card's text → its head, its words and its `name=value` words.
pub fn card_fields(text: &str) -> (&str, Vec<&str>, Vec<(&str, &str)>) {
    let mut it = text.split_ascii_whitespace();
    let head = it.next().unwrap_or("");
    let (mut words, mut params) = (Vec::new(), Vec::new());
    for w in it {
        match w.split_once('=') {
            Some((k, v)) if !k.is_empty() => params.push((k, v)),
            _ => words.push(w),
        }
    }
    (head, words, params)
}

/// Whether `target`'s written decks go through the pass.
pub(super) fn applies(target: Lang) -> bool {
    let r = target.row();
    !(r.heads.is_empty()
        && r.write_masters.is_empty()
        && r.options.is_empty()
        && r.hooks.is_empty())
}

/// `text` (written for `target`) respelled and its hooked cards answered:
/// a Rust hook's answers spliced in (its refusals pushed onto `diags`),
/// an external hook's cards returned as requests.
///
/// # Panics
/// When a Rust hook answers a different number of cards than it was
/// given, or refuses with a code its definition did not declare.
pub(super) fn pass(
    text: &str,
    target: Lang,
    src: &Tokens,
    diags: &mut Vec<Diag>,
) -> (String, Vec<Request>) {
    let named = matches!(target.behaviours().writer, Writer::Spectre | Writer::Vacask);
    let cards = cards(text, named, target.spelling().comment);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for c in &cards {
        out.push_str(&text[at..c.start]);
        out.push_str(&respell(&text[c.clone()], named, target));
        at = c.end;
    }
    out.push_str(&text[at..]);
    hooks(&out, named, target, src, diags)
}

/// `W0201` with `target`'s code `id`, at `token`.
///
/// # Panics
/// When `target` declared no refusal `id`.
pub(super) fn refusal(target: Lang, id: &str, token: u32) -> Diag {
    let r = target.row();
    let at = r.codes.iter().position(|c| c.0 == id);
    let at = at.unwrap_or_else(|| panic!("{}: no code '{id}'", r.name));
    Diag {
        code: Code::W0201,
        token,
        detail: r.first_code + at as u16,
    }
}

// ---- private ----

/// The byte ranges of `text`'s cards (statements), comments and blank
/// lines left out; each ends before its last line's newline.
fn cards(text: &str, named: bool, comment: &str) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut joined = false;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let trimmed = body.trim_start();
        let span = at..at + body.len();
        at += line.len();
        let skip = trimmed.is_empty() || trimmed.starts_with(comment) || trimmed.starts_with("//");
        let cont = match named {
            true => joined,
            false => trimmed.starts_with('+'),
        };
        joined = named && body.ends_with('\\');
        match (cont, out.last_mut()) {
            (true, Some(last)) => last.end = span.end,
            _ if skip => {}
            _ => out.push(span),
        }
    }
    out
}

/// One card respelled: its head (SPICE) or master (named), and its option
/// names on an options card.
fn respell(card: &str, named: bool, target: Lang) -> String {
    let r = target.row();
    let words = words(card);
    let Some(first) = words.first().cloned() else {
        return card.to_string();
    };
    let lex = target.spelling().lexicon;
    let mut swaps: Vec<(Range<usize>, &str)> = Vec::new();
    let options = match named {
        false => {
            let tag = lex.keyword(&card[first.clone()]);
            if let Some(h) = r.heads.iter().find(|h| Some(h.0) == tag) {
                swaps.push((first.clone(), h.1));
            }
            tag == Some(Tag::KwOptions)
        }
        true => {
            let master = master(card, &words);
            if let Some(m) = &master
                && let Some(w) = r.write_masters.iter().find(|w| w.0 == &card[m.clone()])
            {
                swaps.push((m.clone(), w.1));
            }
            master.is_some_and(|m| &card[m] == "options") || &card[first.clone()] == "options"
        }
    };
    if options {
        for w in &words[1..] {
            let word = &card[w.clone()];
            let key = word.split_once('=').map_or(word, |(k, _)| k);
            let row = r.options.iter().find(|o| o.0.eq_ignore_ascii_case(key));
            if let Some(o) = row {
                swaps.push((w.start..w.start + key.len(), o.1));
            }
        }
    }
    let mut out = card.to_string();
    swaps.sort_by_key(|s| s.0.start);
    for (span, to) in swaps.into_iter().rev() {
        out.replace_range(span, to);
    }
    out
}

/// A named statement's master word: after `name [(nodes)]`, or the third
/// word of `model name master`.
fn master(card: &str, words: &[Range<usize>]) -> Option<Range<usize>> {
    let text = |i: usize| words.get(i).map(|w| &card[w.clone()]);
    if text(0)? == "model" {
        return words.get(2).cloned();
    }
    match text(1)?.starts_with('(') {
        true => {
            let close = words
                .iter()
                .skip(1)
                .position(|w| card[w.clone()].ends_with(')'))?;
            words.get(close + 2).cloned()
        }
        false => words.get(1).cloned(),
    }
}

/// The byte ranges of `s`'s whitespace-separated words.
fn words(s: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, b) in s
        .bytes()
        .enumerate()
        .chain(std::iter::once((s.len(), b' ')))
    {
        match (b.is_ascii_whitespace(), start) {
            (true, Some(st)) => {
                out.push(st..i);
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    out
}

/// `text`'s hooked cards answered: a Rust hook once per head with all its
/// cards, an external one's cards returned as requests.
fn hooks(
    text: &str,
    named: bool,
    target: Lang,
    src: &Tokens,
    diags: &mut Vec<Diag>,
) -> (String, Vec<Request>) {
    let r = target.row();
    if r.hooks.is_empty() {
        return (text.to_string(), Vec::new());
    }
    let mut hooked: Vec<Hooked> = Vec::new();
    for span in cards(text, named, target.spelling().comment) {
        let card = &text[span.clone()];
        let head = card.split_ascii_whitespace().next().unwrap_or("");
        if let Some(hook) = r.hooks.iter().position(|h| h.0.eq_ignore_ascii_case(head)) {
            let joined = joined(card, named);
            hooked.push(Hooked {
                span,
                hook,
                text: joined,
                anchor: 0,
            });
        }
    }
    anchors(&mut hooked, target, src);
    // Each Rust hook once, with all its cards; its answers by card.
    let mut answers: Vec<Option<Answer>> = vec![None; hooked.len()];
    let mut out_answers = Vec::new();
    for (k, (head, hook)) in r.hooks.iter().enumerate() {
        let Hook::Rust(f) = *hook else { continue };
        let mine: Vec<usize> = (0..hooked.len()).filter(|&i| hooked[i].hook == k).collect();
        if mine.is_empty() {
            continue;
        }
        let fields: Vec<_> = mine.iter().map(|&i| card_fields(&hooked[i].text)).collect();
        let views: Vec<Card> = mine
            .iter()
            .zip(&fields)
            .map(|(&i, f)| Card {
                head: f.0,
                words: &f.1,
                params: &f.2,
                text: &hooked[i].text,
            })
            .collect();
        out_answers.clear();
        f(&views, &mut out_answers);
        assert_eq!(
            out_answers.len(),
            mine.len(),
            "hook '{head}': one answer per card"
        );
        for (&i, a) in mine.iter().zip(out_answers.drain(..)) {
            answers[i] = Some(a);
        }
    }
    let mut out = String::with_capacity(text.len());
    let mut requests = Vec::new();
    let mut at = 0;
    for (h, a) in hooked.iter().zip(answers) {
        out.push_str(&text[at..h.span.start]);
        match (a, r.hooks[h.hook].1) {
            (Some(Answer::Text(t)), _) => out.push_str(t.trim_end_matches('\n')),
            (Some(Answer::Refuse(id)), _) => {
                diags.push(refusal(target, id, h.anchor));
                out.push_str(&text[h.span.clone()]);
            }
            (None, Hook::External(id)) => {
                let start = out.len() as u32;
                out.push_str(&text[h.span.clone()]);
                requests.push(Request {
                    hook: id,
                    span: start..out.len() as u32,
                    anchor: h.anchor,
                });
            }
            (None, Hook::Rust(_)) => unreachable!("a Rust hook answered every card"),
        }
        at = h.span.end;
    }
    out.push_str(&text[at..]);
    (out, requests)
}

/// A card's lines joined by one space, continuation marks left out.
fn joined(card: &str, named: bool) -> String {
    let lines = card.lines().map(|l| {
        let l = l.trim();
        match named {
            true => l.trim_end_matches('\\').trim_end(),
            false => l.strip_prefix('+').unwrap_or(l).trim_start(),
        }
    });
    lines.collect::<Vec<_>>().join(" ")
}

/// Each hooked card's anchor: the source statement it came from, read as
/// the k-th card of its keyword ↔ the k-th source token of that keyword
/// when the counts agree; else the deck's first token.
// ponytail: matched by count, not traced through the writer; a writer
// that splits or merges a keyword's cards anchors them at token 0.
fn anchors(hooked: &mut [Hooked], target: Lang, src: &Tokens) {
    let lex = target.spelling().lexicon;
    let tag = |h: &Hooked| lex.keyword(h.text.split_ascii_whitespace().next().unwrap_or(""));
    let tags: Vec<Option<Tag>> = hooked.iter().map(tag).collect();
    let mut wants: Vec<Tag> = Vec::new();
    for &t in tags.iter().flatten() {
        if !wants.contains(&t) {
            wants.push(t);
        }
    }
    for want in wants {
        let at: Vec<u32> = (0..src.len() as u32)
            .filter(|&k| src.tags()[k as usize] == want)
            .collect();
        let mine: Vec<usize> = (0..hooked.len())
            .filter(|&i| tags[i] == Some(want))
            .collect();
        if at.len() == mine.len() {
            for (&i, &k) in mine.iter().zip(&at) {
                hooked[i].anchor = k;
            }
        }
    }
}
