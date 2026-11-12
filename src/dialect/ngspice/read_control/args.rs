//! A script command's arguments → its target and value words: `alter`/`altermod`
//! (`device.c:1249-1420`), `alterparam` (`inp.c:1752-1776`), `set`/`option` assignments
//! (`cp_setparse`), `repeat`'s count, and the loop-word substitution (`variable.c`: a loop word
//! as written; any other `$` or a backquote is a reprinted value). Text in, words out.
//!
//! Cites: `rewrite/S3-CONTROL.md` S2–S4, V1–V3.
//!
//! Contents: `pub(super) fn is_number(&str) -> bool`, `pub(super) fn subst(&str,
//! &[(String, String)]) -> Result<String, ir::Code>`, `pub(super) fn assignments(&str) ->
//! Vec<(String, Option<String>)>`, `pub(super) fn alter(&str, bool) -> Result<(Target, String),
//! ir::Code>`, `pub(super) fn alterparam(&str) -> Result<(Target, String), ir::Code>`,
//! `pub(super) fn repeat_count(&str) -> Option<usize>`, `pub(super) fn class_of(ir::Code) ->
//! ir::OpaqueClass`

use super::plan::{MARK, Target, UNROLL_MAX};
use super::{FORM, READS, REJECTED};
use crate::dialect::ngspice::read_run;
use crate::ir;

/// Whether `word` is one whole card number (`INPevaluate`'s reading, its unit tail included).
pub(super) fn is_number(word: &str) -> bool {
    use crate::dialect::ngspice::number::{Grammar, scan};
    let digits = word.strip_prefix(['-', '+']).unwrap_or(word);
    scan(digits, Grammar::Inp).is_some_and(|s| s.len == digits.len())
}

/// `line` with each `$var` of `env` replaced by its word; any other `$`, a backquote, is a
/// reprinted value (`READS`).
pub(super) fn subst(line: &str, env: &[(String, String)]) -> Result<String, ir::Code> {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.char_indices().peekable();
    // Bounded: each step consumes at least one character.
    while let Some((at, ch)) = chars.next() {
        if ch == '`' {
            return Err(READS);
        }
        if ch != '$' {
            out.push(ch);
            continue;
        }
        let tail = &line[at + 1..];
        let len = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(tail.len());
        let name = &tail[..len];
        let word = env.iter().rev().find(|(v, _)| v == name).ok_or(READS)?;
        out.push_str(&word.1);
        for _ in 0..name.chars().count() {
            chars.next();
        }
    }
    Ok(out)
}

/// The `(name, value)` pairs of a `set`/`option` argument text (`cp_setparse`: `name`,
/// `name=value`, `name = value`), names lowercase.
pub(super) fn assignments(rest: &str) -> Vec<(String, Option<String>)> {
    let words = read_run::split_words(rest);
    let mut out = Vec::new();
    let mut k = 0;
    // Bounded: each step consumes one or three words.
    while k < words.len() {
        let name = words[k].to_ascii_lowercase();
        if words.get(k + 1).is_some_and(|w| w == "=") {
            out.push((name, words.get(k + 2).cloned()));
            k += 3;
        } else {
            out.push((name, None));
            k += 1;
        }
    }
    out
}

/// `alter` / `altermod` arguments → target and one-word value (`device.c:1249-1420`).
pub(super) fn alter(rest: &str, model: bool) -> Result<(Target, String), ir::Code> {
    let (lhs, rhs) = match rest.split_once('=') {
        Some((l, r)) => (
            l.split_whitespace().collect::<Vec<_>>(),
            r.trim().to_string(),
        ),
        None => {
            let mut w: Vec<&str> = rest.split_whitespace().collect();
            let v = w.pop().ok_or(REJECTED)?.to_string();
            (w, v)
        }
    };
    if rhs.is_empty() || lhs.is_empty() {
        return Err(REJECTED);
    }
    if rhs.starts_with('[')
        || rhs.contains(char::is_whitespace)
        || !(rhs == MARK || is_number(&rhs))
        || rest.contains(" file")
    {
        return Err(FORM);
    }
    let (dev, param) = match lhs.as_slice() {
        [d] if d.starts_with('@') => {
            let inner = d[1..].strip_suffix(']').ok_or(FORM)?;
            let (dv, p) = inner.split_once('[').ok_or(FORM)?;
            (dv.to_string(), Some(p.to_ascii_lowercase()))
        }
        [d] => (d.to_string(), None),
        [d, p] => (d.to_string(), Some(p.to_ascii_lowercase())),
        _ => return Err(REJECTED),
    };
    if dev.is_empty() || dev.contains('.') {
        return Err(FORM);
    }
    let target = match (model, param) {
        (true, Some(p)) => Target::Model(dev, p),
        (true, None) => return Err(FORM),
        (false, Some(p)) => Target::Device(dev, p),
        (false, None) => match dev.as_bytes()[0].to_ascii_lowercase() {
            b'r' | b'c' | b'l' => Target::Element(dev),
            b'v' | b'i' => Target::Source(dev),
            _ => return Err(FORM),
        },
    };
    Ok((target, rhs))
}

/// `alterparam p=v` → the parameter and its value text (`inp.c:1752-1776`: the first token
/// after `=`); `alterparam sub p=v` is a form the IR may type later.
pub(super) fn alterparam(rest: &str) -> Result<(Target, String), ir::Code> {
    let (lhs, rhs) = rest.split_once('=').ok_or(REJECTED)?;
    let value = rhs.split_whitespace().next().ok_or(REJECTED)?.to_string();
    match lhs.split_whitespace().collect::<Vec<_>>().as_slice() {
        [p] => Ok((Target::Param(p.to_string()), value)),
        [_, _] => Err(FORM),
        _ => Err(REJECTED),
    }
}

/// `repeat`'s count: a literal non-negative integer (`control.c` CO_REPEAT); `repeat` alone
/// loops for ever, which is no plan.
pub(super) fn repeat_count(rest: &str) -> Option<usize> {
    rest.trim()
        .parse::<usize>()
        .ok()
        .filter(|&n| n <= UNROLL_MAX)
}

/// The class a command-level code keeps.
pub(super) fn class_of(code: ir::Code) -> ir::OpaqueClass {
    if code == FORM {
        ir::OpaqueClass::Unread
    } else {
        ir::OpaqueClass::InvalidInSource
    }
}
