//! `.func` cards → [`Func`] rows: the head group, the body, and where a
//! subcircuit's `.func` goes (the subcircuit, or the deck's top).
//!
//! Cites: the old SPICE grammar's `Func`; Spectre Reference p.871
//! (functions at the top level only).
//!
//! Contents:
//! - `Rd::func_view(&self, f: Func) -> (String, Vec<String>, String)`,
//!   `Rd::func_span(&self, f: Func) -> Option<(u32, u32)>`
//! - `Rd::func(&mut self, n: u32, here: &mut Block, inside: bool, acc: &mut Acc) -> Result<(), Kept>`,
//!   `Rd::func_head_ok(&self, g: u32) -> bool`

use super::{Acc, Block, Func, Kept, Rd};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::token::Tag as T;

/// Where a `.func` card's body is: a delimited expression's inside, or
/// tokens `a..=b` (empty when `a > b`).
enum Body {
    Expr(u32),
    Span(u32, u32),
}

impl Rd<'_> {
    /// A function's name, arguments and body text.
    pub(in crate::syntax::render) fn func_view(&self, f: Func) -> (String, Vec<String>, String) {
        let (g, body) = match f {
            Func::Param(a) => (self.ast.lhs(Node(a)), self.body_text(self.ast.rhs(Node(a)))),
            Func::Card(n) => {
                let g = self.func_group(n).expect("func read it");
                (g, self.func_body(n, g).expect("func read it"))
            }
            Func::Real(n) => return self.real_func(n),
        };
        (
            self.text(self.ast.lhs(Node(g))),
            self.group_items(g).iter().map(|&k| self.text(k)).collect(),
            body,
        )
    }

    /// The tokens function `f`'s body ([`Rd::func_view`]'s) is spelled
    /// from; `None` when it is empty or a string's text.
    pub(in crate::syntax::render) fn func_span(&self, f: Func) -> Option<(u32, u32)> {
        let v = match f {
            Func::Param(a) => self.ast.rhs(Node(a)),
            Func::Card(n) => match self.func_body_at(n, self.func_group(n)?)? {
                Body::Expr(e) => return self.inner_span(e),
                Body::Span(a, b) => return (a <= b).then_some((a, b)),
            },
            Func::Real(n) => *self.fields(self.real_return(n)?).first()?,
        };
        match self.tag(v) {
            Tag::Delimited => self.inner_span(v),
            _ => Some((self.first(v), self.last(v))),
        }
    }

    /// `.func name(args) [=] body` (the old SPICE grammar's `Func`). One in
    /// a subcircuit whose body reads only its arguments (and calls), under
    /// a name no other `.func` has, means the same at the deck's top, where
    /// every target defines functions (Spectre Reference p.871: "must be
    /// declared at the top level only"); any other is the subcircuit's.
    pub(super) fn func(
        &mut self,
        n: u32,
        here: &mut Block,
        inside: bool,
        acc: &mut Acc,
    ) -> Result<(), Kept> {
        let Some(g) = self.func_group(n) else {
            return Err(Kept::Unparsed);
        };
        if self.func_body(n, g).is_none() {
            return Err(Kept::Unparsed);
        }
        match inside && !self.func_closed(n, g) {
            true => here.funcs.push(Func::Card(n)),
            false => acc.funcs.push(Func::Card(n)),
        }
        Ok(())
    }

    /// the old SPICE grammar's `Key s? '(' s? (Arg (s? ',' s? Arg)*)? s? ')'`: a key and
    /// keys, closed.
    pub(super) fn func_head_ok(&self, g: u32) -> bool {
        let name = self.ast.lhs(Node(g));
        let key = |k: u32| {
            self.is_word_item(k)
                && self
                    .text(k)
                    .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        };
        name != NONE
            && key(name)
            && self.group_closed(g)
            && self.group_items(g).iter().all(|&k| key(k))
    }
}

// ---- private ----

impl Rd<'_> {
    /// Whether `.func` card `n` (head group `g`) reads no name but its
    /// arguments, and no other `.func` card of the file has its name.
    fn func_closed(&self, n: u32, g: u32) -> bool {
        let name = self.text(self.ast.lhs(Node(g)));
        let args: Vec<String> = self.group_items(g).iter().map(|&k| self.text(k)).collect();
        let Some((a, b)) = self.func_span(Func::Card(n)) else {
            return true;
        };
        // Each pass looks at one token, so it ends by the file's length.
        let funcs: Vec<&str> = (1..self.t.len() as u32)
            .filter(|&k| self.ttag(k - 1) == T::KwFunc)
            .map(|k| self.t.text(k))
            .collect();
        let same_name = funcs
            .iter()
            .filter(|f| f.eq_ignore_ascii_case(&name))
            .count();
        // A word is an argument, or a call (a `(` follows it) of no `.func`.
        let reads_only_args = (a..=b).all(|k| {
            let w = self.t.text(k);
            self.ttag(k) != T::Ident
                || args.iter().any(|x| x.eq_ignore_ascii_case(w))
                || (k < b
                    && self.ttag(k + 1) == T::LParen
                    && !funcs.iter().any(|f| f.eq_ignore_ascii_case(w)))
        });
        reads_only_args && same_name == 1
    }

    /// A `.func` card's `name(args)` group, when its head reads.
    fn func_group(&self, n: u32) -> Option<u32> {
        let &first = self.fields(n).first()?;
        let g = match self.tag(first) {
            Tag::Group => first,
            Tag::Assign if self.tag(self.ast.lhs(Node(first))) == Tag::Group => {
                self.ast.lhs(Node(first))
            }
            _ => return None,
        };
        self.func_head_ok(g).then_some(g)
    }

    /// A `.func` card's body text (`g` its head group), when it reads.
    fn func_body(&self, n: u32, g: u32) -> Option<String> {
        Some(match self.func_body_at(n, g)? {
            Body::Expr(e) => self.inner(e).trim().to_string(),
            Body::Span(a, b) if a > b => String::new(),
            Body::Span(a, b) => self.span(a, b).trim().to_string(),
        })
    }

    /// Where a `.func` card's body is (`g` its head group), when it reads.
    fn func_body_at(&self, n: u32, g: u32) -> Option<Body> {
        let items = self.fields(n);
        // The body: after `)` and an optional `=`, to the card's end or a
        // ` key=` (the old SPICE grammar's `Body`).
        let close = self.last(g);
        let mut start = close + 1;
        if start <= self.line_last(self.main(n)) && self.ttag(start) == T::Equal {
            start += 1;
        }
        let last = self.line_last(self.main(n));
        if start > last {
            return None;
        }
        for k in start + 1..=last {
            if self.t.end(k - 1) != self.t.start(k)
                && self.ttag(k) == T::Ident
                && k < last
                && self.ttag(k + 1) == T::Equal
            {
                return None;
            }
        }
        // quirk: the old SPICE grammar's `Body`: an `expr` to the end, a
        // braced body to the end, else the text.
        let expr = items
            .iter()
            .copied()
            .chain(
                items
                    .iter()
                    .filter(|&&k| self.tag(k) == Tag::Assign)
                    .map(|&k| self.ast.rhs(Node(k))),
            )
            .find(|&k| k != NONE && self.first(k) == start && self.last(k) == last);
        Some(if expr.is_some_and(|e| self.expr_ok(e)) {
            Body::Expr(expr.expect("checked"))
        } else if self.ttag(start) == T::LBrace
            && self.close_of(start, T::LBrace, T::RBrace) == Some(last)
        {
            Body::Span(start + 1, last - 1)
        } else {
            Body::Span(start, last)
        })
    }

    /// The tokens a delimited expression's inside ([`Rd::inner`]) is
    /// spelled from; `None` when it is empty or a string's text (`{"…"}`).
    fn inner_span(&self, n: u32) -> Option<(u32, u32)> {
        let a = self.ast;
        let (open, close) = (a.main_token(Node(n)), a.rhs(Node(n)));
        let e = a.lhs(Node(n));
        let string = e != NONE && self.tag(e) == Tag::Str && self.ttag(open) == T::LBrace;
        (!string && close > open + 1).then(|| (open + 1, close - 1))
    }
}
