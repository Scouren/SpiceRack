//! A file's diagnostics as text: sorted by source position, each line
//! `file:line:column: CODE title` (a definition's own code: `CODE
//! <lang>:<id> <message>`), the line and column computed from the token.
//!
//! Cites: none.
//!
//! Contents:
//! - `pub fn sort(diags: &mut [Diag])`
//! - `pub fn messages(diags: &[Diag], t: &Tokens, file: &str) -> String`

use std::fmt::Write as _;

use super::{Code, Diag, info};
use crate::syntax::token::Tokens;

/// Sorts `diags` by source position (token), then code: the order their
/// messages are rendered in.
pub fn sort(diags: &mut [Diag]) {
    diags.sort_by_key(|d| (d.token, d.code));
}

/// `diags` (one file's) as messages, one a line, sorted by source
/// position: `file:line:column: CODE title`, the line and column computed
/// here from each diagnostic's token in `t`.
///
/// # Panics
/// When a diagnostic's token is not one of `t`'s.
pub fn messages(diags: &[Diag], t: &Tokens, file: &str) -> String {
    let mut sorted = diags.to_vec();
    sort(&mut sorted);
    let mut out = String::new();
    for d in &sorted {
        assert!(
            (d.token as usize) < t.len(),
            "a diagnostic on a token of `t`"
        );
        let at = t.location(d.token);
        let r = match d.code {
            Code::W0201 | Code::W0202 => {
                let p = crate::syntax::lang::plugin_code(d.detail);
                writeln!(
                    out,
                    "{file}:{}:{}: {:?} {}:{} {}",
                    at.line,
                    at.column,
                    d.code,
                    p.lang.name(),
                    p.id,
                    p.message
                )
            }
            _ => writeln!(
                out,
                "{file}:{}:{}: {:?} {}",
                at.line,
                at.column,
                d.code,
                info(d.code).title
            ),
        };
        r.expect("a String takes writes");
    }
    out
}
