//! A named call in an HSPICE expression → its tree node (split from `expr_read.rs`): a function,
//! a draw, a user function (`.func`, `.param f(x)=…`); a name with no settled member is
//! `Unread` (`tables::NO_MEMBER`).
//!
//! Contents: `Parser::call(&mut self, name: usize, args: &[usize]) -> Result<(), Fail>`

use super::super::expr_lex::Tok;
use super::super::tables::{self, Named};
use super::{Fail, N, Parser};

impl Parser<'_> {
    /// Pushes the call of the name at token `name` on `args`.
    pub(super) fn call(&mut self, name: usize, args: &[usize]) -> Result<(), Fail> {
        let Tok::Ident(word) = &self.toks[name] else {
            unreachable!("a call is named by an identifier")
        };
        let within = |(min, max): (u8, u8)| {
            args.len() >= usize::from(min) && (max == u8::MAX || args.len() <= usize::from(max))
        };
        let node = match tables::named(word) {
            Some(Named::Func(f)) if within(f.arity()) => N::Call(f, args.to_vec()),
            Some(Named::Draw(d)) if within(d.arity()) => N::Draw(d, args.to_vec()),
            Some(_) => return Err(Fail::invalid()),
            None if tables::no_member(word) => return Err(Fail::unread()),
            None => N::User(word.clone(), args.to_vec()),
        };
        let k = self.push(node);
        self.out.push(k);
        Ok(())
    }
}
