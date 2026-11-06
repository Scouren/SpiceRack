//! Xyce's `E`/`G` `TABLE` source → [`Card::Table`]: the controlling
//! expression and the `(x, y)` points.
//!
//! Cites: Xyce RG, E and G sources (`TABLE {expr} = (x1,y1) (x2,y2) …`; the
//! braces, the `=` and an outer parenthesis optional; the output constant
//! past the first and last point: Xyce 7.10 run, `(0,1) (1,3)` gives 1 at
//! -1 and 3 at 2).
//!
//! Contents:
//! - `Rd::table_source(&self, n: u32, current: bool, name: String, np: String, nm: String) -> Option<Card>`

use super::{Card, Rd};
use crate::syntax::behaviour::CardRules;

impl Rd<'_> {
    /// The `TABLE` form of the E or G card `n` (its items from the third),
    /// when it is Xyce's and every point is a number, the abscissae
    /// increasing; `None` for another card. ngspice's own `TABLE` is not
    /// read: ngspice smooths each corner (ngspice-45 run, 1.05 at the first
    /// point of `(0,1) (1,3)`), which no other target's form does.
    pub(in crate::syntax::render) fn table_source(
        &self,
        n: u32,
        current: bool,
        name: String,
        np: String,
        nm: String,
    ) -> Option<Card> {
        let items = self.items(n);
        let &kw = items.get(2)?;
        if self.b.card_rules != CardRules::Xyce || !self.is_word(kw) || !self.word_is(kw, "table") {
            return None;
        }
        let last = self.line_last(self.main(n));
        let first = self.main(kw) + 1;
        let text = self.span(first, last);
        let (expr, rest) = split_expr(text.trim())?;
        let lex = self.t.lexicon_at(self.main(n));
        let num = |s: &str| self.number_text(s.trim(), lex);
        let mut points: Vec<(f64, f64)> = Vec::new();
        // Each point's `(x,y)` or `(x y)`: the numbers inside the innermost
        // parentheses, two at a time.
        let flat: String = rest
            .chars()
            .map(|c| if "(),".contains(c) { ' ' } else { c })
            .collect();
        let words: Vec<&str> = flat.split_whitespace().collect();
        if words.len() < 4 || !words.len().is_multiple_of(2) {
            return None;
        }
        for p in words.chunks(2) {
            points.push((num(p[0])?, num(p[1])?));
        }
        if points.windows(2).any(|w| w[0].0 >= w[1].0) {
            return None;
        }
        Some(Card::Table {
            current,
            name,
            np,
            nm,
            expr,
            points,
        })
    }
}

// ---- private ----

/// The controlling expression of `text` (after `TABLE`) and the text of its
/// points: a braced expression, else everything before a top-level `=`,
/// else one `name(…)` call. `None` when the expression reads more than
/// node voltages and branch currents (`v(…)`, `i(…)`), numbers and
/// arithmetic, which every target spells alike.
fn split_expr(text: &str) -> Option<(String, &str)> {
    let close = |open: char, close: char, s: &str| -> Option<usize> {
        let mut depth = 0;
        for (i, c) in s.char_indices() {
            match c {
                _ if c == open => depth += 1,
                _ if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    };
    let (expr, rest) = if let Some(body) = text.strip_prefix('{') {
        let end = close('{', '}', text)?;
        (&body[..end - 1], &text[end + 1..])
    } else if let Some(eq) = text.find('=') {
        (&text[..eq], &text[eq..])
    } else {
        let end = close('(', ')', text)?;
        (&text[..=end], &text[end + 1..])
    };
    let rest = rest.trim_start().strip_prefix('=').unwrap_or(rest);
    let expr = expr.trim();
    // Each word followed by `(` is `v` or `i`; no other name.
    let b = expr.as_bytes();
    let mut i = 0;
    // Each pass moves past one character, so it ends by the text's length.
    while i < b.len() {
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &expr[start..i];
            let call = expr[i..].trim_start().starts_with('(');
            let numeric = start > 0 && b[start - 1].is_ascii_digit();
            if !numeric
                && !(call && (word.eq_ignore_ascii_case("v") || word.eq_ignore_ascii_case("i")))
            {
                return None;
            }
            if call {
                // The probe's arguments are node or branch names.
                let end = i + expr[i..].find(')')?;
                i = end + 1;
            }
            continue;
        }
        if !b"0123456789.+-*/() ,".contains(&b[i]) {
            return None;
        }
        i += 1;
    }
    (!expr.is_empty()).then(|| (expr.to_string(), rest))
}

#[cfg(test)]
mod tests {
    use super::split_expr;

    #[test]
    fn each_xyce_table_head_splits_at_its_points() {
        // The four Xyce_Regression forms. Checks: 4 + 1.
        for (t, e) in [
            ("{V(Vrev)} = (-0.1,1) (0,0)", "V(Vrev)"),
            ("V(1,0) = (0,1) (1,2)", "V(1,0)"),
            ("V(1,0) (0,1) (1,2)", "V(1,0)"),
            ("{ V(1, 0) }  ( (-2,-3) (2,3) )", "V(1, 0)"),
        ] {
            assert_eq!(split_expr(t).map(|x| x.0), Some(e.to_string()), "{t}");
        }
        assert_eq!(split_expr("{sin(V(1))} = (0,1) (1,2)"), None);
    }
}
