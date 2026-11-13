//! A named call in an ngspice expression → its tree node (split from `expr_read.rs`): a
//! function, a draw, the ternary function, `u2`'s clamp, a user `.func`; numparam's two-argument
//! function called with three arguments reads the first and the last (`xpressn.c` `formula`),
//! and its min or max of one argument is that argument (settle `max_one_arg.sp`).
//!
//! Contents: `Parser::call(&mut self, name: usize, args: &[usize]) -> Result<(), Fail>`

use super::super::codes;
use super::super::expr_lex::Tok;
use super::super::tables::{self, Eval, Named};
use super::{Fail, N, Parser};
use crate::ir;

impl Parser<'_> {
    /// Pushes the call of the name at token `name` on `args`.
    pub(super) fn call(&mut self, name: usize, args: &[usize]) -> Result<(), Fail> {
        let Tok::Ident(word) = &self.toks[name] else {
            unreachable!("a call is named by an identifier")
        };
        // numparam hands a function its first and last argument (`formula`: `v` the first, `u`
        // the last), so a two-argument one called with three reads the first and the third
        // (run: `{limit(3,0,2)}` = 5, a draw of 3 ± 2)
        let named = tables::named(self.eval, word);
        let two = match named {
            Some(Named::Func(f)) => f.arity() == (2, 2),
            Some(Named::Draw(d)) => d.arity() == (2, 2),
            _ => false,
        };
        let picked;
        // numparam's min or max of one argument is that argument, its first and its last
        // (settle max_one_arg.sp: `max(f(1,2,3,4))` is f's value, the 38 decks' func_cap.sp)
        if let (Eval::Numparam, [arg], Some(Named::Func(ir::Func::Min | ir::Func::Max))) =
            (self.eval, args, named)
        {
            self.out.push(*arg);
            return Ok(());
        }
        let args = if self.eval == Eval::Numparam && two && args.len() == 3 {
            self.note(codes::FIRST_LAST);
            picked = [args[0], args[2]];
            &picked[..]
        } else {
            args
        };
        let within = |(min, max): (u8, u8)| {
            args.len() >= usize::from(min) && (max == u8::MAX || args.len() <= usize::from(max))
        };
        let node = match named {
            Some(Named::Func(f)) if within(f.arity()) => N::Call(f, args.to_vec()),
            Some(Named::Draw(d)) if within(d.arity()) => N::Draw(d, args.to_vec()),
            Some(Named::Ternary) if args.len() == 3 => N::Tern(args[0], args[1], args[2]),
            // B `u2(x)` (PTustep2: 0, x, 1) is `min(max(x, 0), 1)` (sugar, R13)
            Some(Named::Clamp01) if args.len() == 1 => {
                let zero = self.push(N::Num(ir::Literal::ZERO));
                let max = self.push(N::Call(ir::Func::Max, vec![args[0], zero]));
                let one = ir::Literal {
                    mantissa: 1,
                    ..ir::Literal::ZERO
                };
                let one = self.push(N::Num(one));
                N::Call(ir::Func::Min, vec![max, one])
            }
            Some(_) => return Err(Fail::invalid()),
            None if tables::no_member(self.eval, word) => return Err(Fail::unread()),
            None => N::User(word.clone(), args.to_vec()),
        };
        let k = self.push(node);
        self.out.push(k);
        Ok(())
    }
}
