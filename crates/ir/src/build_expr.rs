//! The builder's expression constructors: a reader's or builder's values in, expression rows
//! out (the encoding of `expr.rs`'s header). Each constructor asserts its children exist (they
//! have lower ids, so trees are acyclic by construction), its arity, and the literal's
//! invariants or a built number's finiteness (the NaN gate, C9). Split from `build.rs` along its
//! transforms; the door and its rules are that file's.
//!
//! Cites: `rewrite/IR-KERNEL.md` §7, C7, C9, Addenda A1–A3.
//!
//! Contents: `Builder::literal`, `Builder::num`, `Builder::string`, `Builder::param_ref`,
//! `Builder::call`, `Builder::user_call`, `Builder::unary`, `Builder::binary`,
//! `Builder::ternary`, `Builder::v`, `Builder::i`, `Builder::time`, `Builder::temp`,
//! `Builder::freq`, `Builder::tstep`, `Builder::tstop`, `Builder::tnom`, `Builder::draw`,
//! `Builder::list` (each `-> ExprId`); crate: `Builder::rebuild`, `Builder::expr_row`, `Builder::assert_expr`

use crate::build::Builder;
use crate::expr::{
    BinOp, Dist, DrawPer, DrawWhen, ExprTag, Func, Literal, NumForm, UnOp, UserCall,
};
use crate::id::{DefId, EXPRS_MAX, ExprId, NameId, Ref};

impl Builder<'_> {
    /// A number as a reader read it. Asserts `Literal::is_valid` (a reader rounds a literal
    /// past 18 significant digits, with a note, and folds `1e3k` into `Exponent`, before it
    /// calls).
    pub fn literal(&mut self, literal: Literal) -> ExprId {
        assert!(literal.is_valid(), "a literal meets its invariants");
        let nums = &mut self.design.exprs.nums;
        assert!(nums.len() < EXPRS_MAX, "at most EXPRS_MAX literals");
        nums.push(literal);
        let id = (nums.len() - 1) as u32;
        self.expr_row(ExprTag::Num, 0, id, 0)
    }

    /// A literal whose significand has more significant digits than the mantissa holds (A18):
    /// `significand` as written (`[-]digits[.digits]`), `exp10` the power written or implied by
    /// a scale letter, `written` the form it was written in (`Plain` with `exp10 == 0`,
    /// `Exponent`, or `Scale` with a letter's power). Asserts the significand is that long
    /// (`literal::exceeds_mantissa`; a shorter one is built with `literal`) and well formed.
    pub fn long_literal(&mut self, significand: &str, exp10: i16, written: NumForm) -> ExprId {
        todo!()
    }

    /// A built number (Python, Schemify, passes), stored as `Literal::from_f64(value)`. Asserts
    /// it is finite (the NaN gate, C9).
    pub fn num(&mut self, value: f64) -> ExprId {
        assert!(value.is_finite(), "a built number is finite");
        self.literal(Literal::from_f64(value))
    }

    /// A string value.
    pub fn string(&mut self, value: &str) -> ExprId {
        let id = self.design.names.intern_any(value);
        self.expr_row(ExprTag::Str, 0, id.0, 0)
    }

    /// A parameter read, unresolved. (`param` is the item push.)
    pub fn param_ref(&mut self, name: &str) -> ExprId {
        let id = self.name(name);
        self.expr_row(ExprTag::Param, 0, id.0, DefId::NONE.0)
    }

    /// A built-in function call. Asserts the argument count is within `func.arity()`.
    pub fn call(&mut self, func: Func, args: &[ExprId]) -> ExprId {
        let (min, max) = func.arity();
        let n = args.len();
        assert!(
            n >= min as usize && (max == u8::MAX || n <= max as usize),
            "argument count within the function's arity"
        );
        let list = self.exprs(args);
        self.expr_row(ExprTag::Call, func as u8, list.start, list.len)
    }

    /// A user-function call, unresolved.
    pub fn user_call(&mut self, name: &str, args: &[ExprId]) -> ExprId {
        let func = Ref::unresolved(self.name(name));
        let args = self.exprs(args);
        let calls = &mut self.design.exprs.calls;
        calls.push(UserCall { func, args });
        let index = (calls.len() - 1) as u32;
        self.expr_row(ExprTag::UserCall, 0, index, 0)
    }

    /// A unary operator.
    pub fn unary(&mut self, op: UnOp, operand: ExprId) -> ExprId {
        self.assert_expr(operand);
        self.expr_row(ExprTag::Unary, op as u8, operand.0, 0)
    }

    /// A binary operator.
    pub fn binary(&mut self, op: BinOp, left: ExprId, right: ExprId) -> ExprId {
        self.assert_expr(left);
        self.assert_expr(right);
        self.expr_row(ExprTag::Binary, op as u8, left.0, right.0)
    }

    /// `cond ? then : other`.
    pub fn ternary(&mut self, cond: ExprId, then: ExprId, other: ExprId) -> ExprId {
        self.assert_expr(cond);
        let pair = self.exprs(&[then, other]);
        self.expr_row(ExprTag::Ternary, 0, cond.0, pair.start)
    }

    /// `v(net)` (`reference` `None`: ground) or `v(net, reference)`.
    pub fn v(&mut self, net: &str, reference: Option<&str>) -> ExprId {
        let net = self.name(net);
        let reference = reference.map_or(NameId::NONE, |r| self.name(r));
        self.expr_row(ExprTag::V, 0, net.0, reference.0)
    }

    /// `i(element)`: the element's branch current, positive into its first terminal.
    pub fn i(&mut self, element: &str) -> ExprId {
        let element = self.name(element);
        self.expr_row(ExprTag::I, 0, element.0, 0)
    }

    /// The simulation time.
    pub fn time(&mut self) -> ExprId {
        self.expr_row(ExprTag::Time, 0, 0, 0)
    }

    /// The circuit temperature.
    pub fn temp(&mut self) -> ExprId {
        self.expr_row(ExprTag::Temp, 0, 0, 0)
    }

    /// The analysis frequency.
    pub fn freq(&mut self) -> ExprId {
        self.expr_row(ExprTag::Freq, 0, 0, 0)
    }

    /// The transient step leaf.
    pub fn tstep(&mut self) -> ExprId {
        self.expr_row(ExprTag::TStep, 0, 0, 0)
    }

    /// The transient stop leaf.
    pub fn tstop(&mut self) -> ExprId {
        self.expr_row(ExprTag::TStop, 0, 0, 0)
    }

    /// The nominal temperature leaf.
    pub fn tnom(&mut self) -> ExprId {
        self.expr_row(ExprTag::Tnom, 0, 0, 0)
    }

    /// A statistical draw: when it samples and how often are the source simulator's rule, made
    /// explicit. Asserts the argument count is within `dist.arity()`.
    pub fn draw(&mut self, dist: Dist, when: DrawWhen, per: DrawPer, args: &[ExprId]) -> ExprId {
        let (min, max) = dist.arity();
        let n = args.len();
        assert!(
            n >= min as usize && n <= max as usize,
            "argument count within the distribution's arity"
        );
        let list = self.exprs(args);
        // sub: bits 0-3 the Dist, bit 4 the DrawWhen, bits 5-6 the DrawPer (expr.rs header)
        let sub = dist as u8 | (when as u8) << 4 | (per as u8) << 5;
        self.expr_row(ExprTag::Draw, sub, list.start, list.len)
    }

    /// A vector value.
    pub fn list(&mut self, elements: &[ExprId]) -> ExprId {
        let list = self.exprs(elements);
        self.expr_row(ExprTag::List, 0, list.start, list.len)
    }

    /// Appends a copy of `e` with `kids` for its children, in `Design::children`'s order (A23:
    /// a changed spine is appended again, so children stay below parents). A leaf is returned
    /// as is. Asserts `kids` has `e`'s child count.
    pub(crate) fn rebuild(&mut self, e: ExprId, kids: &[ExprId]) -> ExprId {
        let x = &self.design.exprs;
        let i = e.index();
        let (tag, sub, a, b) = (x.tags[i], x.subs[i], x.a[i], x.b[i]);
        let mut old = Vec::new();
        self.design.children(e, &mut old);
        assert!(old.len() == kids.len(), "a rebuild keeps the child count");
        match tag {
            ExprTag::Call | ExprTag::Draw | ExprTag::List => {
                let list = self.exprs(kids);
                self.expr_row(tag, sub, list.start, list.len)
            }
            ExprTag::UserCall => {
                let func = self.design.exprs.calls[a as usize].func;
                let args = self.exprs(kids);
                let calls = &mut self.design.exprs.calls;
                calls.push(UserCall { func, args });
                let index = (calls.len() - 1) as u32;
                self.expr_row(tag, sub, index, b)
            }
            ExprTag::Unary => self.expr_row(tag, sub, kids[0].0, b),
            ExprTag::Binary => self.expr_row(tag, sub, kids[0].0, kids[1].0),
            ExprTag::Ternary => {
                let pair = self.exprs(&kids[1..]);
                self.expr_row(tag, sub, kids[0].0, pair.start)
            }
            ExprTag::Num
            | ExprTag::Str
            | ExprTag::Param
            | ExprTag::V
            | ExprTag::I
            | ExprTag::Time
            | ExprTag::Temp
            | ExprTag::Freq
            | ExprTag::TStep
            | ExprTag::TStop
            | ExprTag::Tnom => e,
        }
    }

    /// Appends one expression row. Asserts `EXPRS_MAX` and that the columns stay parallel.
    pub(crate) fn expr_row(&mut self, tag: ExprTag, sub: u8, a: u32, b: u32) -> ExprId {
        self.design.classes.take();
        let x = &mut self.design.exprs;
        assert!(x.tags.len() < EXPRS_MAX, "at most EXPRS_MAX expressions");
        x.tags.push(tag);
        x.subs.push(sub);
        x.a.push(a);
        x.b.push(b);
        debug_assert!(
            x.subs.len() == x.tags.len() && x.a.len() == x.tags.len() && x.b.len() == x.tags.len()
        );
        ExprId((x.tags.len() - 1) as u32)
    }

    /// Asserts `e` is an existing expression row (children exist before their parent).
    pub(crate) fn assert_expr(&self, e: ExprId) {
        assert!(
            !e.is_none() && e.index() < self.design.exprs.tags.len(),
            "an expression exists before it is used"
        );
    }
}
