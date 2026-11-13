//! HSPICE expression text → an `ir::Expr` tree. Two steps, so a failure leaves no row behind:
//! [`parse`] builds a local tree (or the reason the statement is `Opaque`), [`Tree::emit`]
//! appends its rows through the builder and records each parameter read and user call for the
//! reader's bind step.
//!
//! The grammar is one operator-precedence parse on explicit stacks (no recursion: a 10,000-deep
//! nest parses in a debug build), driven by `tables.rs`'s levels: every binary operator
//! left-associative, the power too; a prefix `-` looser than the power (`-2**2` = −4); the
//! ternary grouped as C does. `x**y` and `x^y` are `Pow` (`(-2)**3` = −8). A `-` directly before
//! a number whose operand is that number alone is the literal's sign (`-2` is the literal −2;
//! `-(2)` is the negation of 2), so the writer can spell both trees.
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §3 (branch `differences`); settle `expr_*.sp` (ESPice
//! 0.3.0 in HSPICE mode: ESPice is not HSPICE); the lexer is `expr_lex.rs`.
//!
//! Contents: `pub(super) struct Tree`, `pub(super) struct Fail`,
//! `pub(super) fn parse(&str) -> Result<Tree, Fail>`,
//! `Fail::invalid() -> Fail`, `Fail::unread() -> Fail`,
//! `Tree::emit(&self, b, names, scope, pending) -> ir::ExprId`

use super::codes;
use super::expr_lex::{self, Op, Tok};
use super::name::Folder;
use super::number::{self, Fit};
use super::tables;
use crate::ir;

#[path = "expr_call.rs"]
mod call;

/// An expression read but not yet appended: its nodes in creation order (children first).
#[derive(Clone, Debug)]
pub(super) struct Tree {
    nodes: Vec<N>,
    root: usize,
}

/// Why an expression has no tree: the code and the class of the `Opaque` its statement becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Fail {
    /// The coded note (`codes.rs`).
    pub(super) code: ir::Code,
    /// `InvalidInSource` (HSPICE rejects it) or `Unread` (no IR member yet).
    pub(super) class: ir::OpaqueClass,
}

/// One node of a [`Tree`]; children are indices of earlier nodes.
#[derive(Clone, Debug)]
enum N {
    Num(ir::Literal),
    /// A number longer than a literal's mantissa (A18).
    Long(Box<number::Long>),
    Param(String),
    Call(ir::Func, Vec<usize>),
    User(String, Vec<usize>),
    Draw(ir::Dist, Vec<usize>),
    Un(ir::UnOp, usize),
    Bin(ir::BinOp, usize, usize),
    Tern(usize, usize, usize),
    V(String, Option<String>),
    I(String),
    Time,
    Temp,
    Freq,
}

/// An entry of the operator stack.
#[derive(Clone, Copy, Debug)]
enum Frame {
    Bin(Op, u8),
    /// A prefix operator, its level, and the node of the number right after it (the fold).
    Prefix(Option<ir::UnOp>, u8, Option<usize>),
    Paren,
    /// A call: its name's token index, and the output length when its arguments began.
    Call(usize, usize),
    Question,
    Ternary,
}

/// The parser's state over one expression's tokens.
struct Parser<'t> {
    toks: &'t [Tok],
    nodes: Vec<N>,
    /// Per node: whether parentheses closed around it (`-(2)` is no literal).
    paren: Vec<bool>,
    out: Vec<usize>,
    ops: Vec<Frame>,
}

/// The tree of `text`.
pub(super) fn parse(text: &str) -> Result<Tree, Fail> {
    let toks = expr_lex::lex(text)?;
    let mut p = Parser {
        toks: &toks,
        nodes: Vec::new(),
        paren: Vec::new(),
        out: Vec::new(),
        ops: Vec::new(),
    };
    let root = p.run()?;
    debug_assert!(root < p.nodes.len(), "the root is a node");
    Ok(Tree {
        nodes: p.nodes,
        root,
    })
}

impl Tree {
    /// Appends the reachable nodes as expression rows (children first) and returns the root's
    /// id. Names fold through `names` (nodes: `gnd` is `0`); each parameter read and user call
    /// goes to `pending` with `scope`, for the reader's bind step (ruling R3).
    pub(super) fn emit(
        &self,
        b: &mut ir::Builder<'_>,
        names: &mut Folder,
        scope: ir::ScopeId,
        pending: &mut Vec<(ir::ExprId, ir::ScopeId)>,
    ) -> ir::ExprId {
        let mut reach = vec![false; self.nodes.len()];
        let mut stack = vec![self.root];
        // Bounded: each node is marked once.
        while let Some(n) = stack.pop() {
            if std::mem::replace(&mut reach[n], true) {
                continue;
            }
            stack.extend(children(&self.nodes[n]));
        }
        let mut ids = vec![ir::ExprId::NONE; self.nodes.len()];
        for (k, node) in self.nodes.iter().enumerate() {
            if !reach[k] {
                continue;
            }
            let id = |c: &usize| {
                debug_assert!(*c < k, "children come first");
                ids[*c]
            };
            let e = match node {
                N::Num(lit) => b.literal(*lit),
                N::Long(l) => b.long_literal(&l.significand, l.exp10, l.written),
                N::Param(name) => {
                    let e = b.param_ref(&names.fold(name));
                    pending.push((e, scope));
                    e
                }
                N::Call(f, args) => b.call(*f, &args.iter().map(id).collect::<Vec<_>>()),
                N::User(name, args) => {
                    let args: Vec<_> = args.iter().map(id).collect();
                    let e = b.user_call(&names.fold(name), &args);
                    pending.push((e, scope));
                    e
                }
                N::Draw(d, args) => {
                    let args: Vec<_> = args.iter().map(id).collect();
                    b.draw(*d, tables::DRAW.0, tables::DRAW.1, &args)
                }
                N::Un(op, a) => b.unary(*op, id(a)),
                N::Bin(op, l, r) => b.binary(*op, id(l), id(r)),
                N::Tern(c, t, f) => b.ternary(id(c), id(t), id(f)),
                N::V(n, m) => {
                    let (n, m) = (names.node(n), m.as_deref().map(|m| names.node(m)));
                    b.v(&n, m.as_deref())
                }
                N::I(e) => b.i(&names.fold(e)),
                N::Time => b.time(),
                N::Temp => b.temp(),
                N::Freq => b.freq(),
            };
            ids[k] = e;
        }
        assert!(!ids[self.root].is_none(), "the root is emitted");
        ids[self.root]
    }
}

impl Fail {
    /// An expression HSPICE rejects (`P4002`, `InvalidInSource`).
    pub(super) fn invalid() -> Fail {
        Fail {
            code: codes::EXPR_INVALID,
            class: ir::OpaqueClass::InvalidInSource,
        }
    }

    /// A construct the IR has no member for yet (`P4003`, `Unread`).
    pub(super) fn unread() -> Fail {
        Fail {
            code: codes::EXPR_NO_MEMBER,
            class: ir::OpaqueClass::Unread,
        }
    }
}

// ---- private ----

/// A binary operator's level.
fn level(op: Op) -> u8 {
    let bin = |b| tables::level(b).expect("HSPICE has it");
    match op {
        Op::Pow => tables::POWER,
        Op::Plus => bin(ir::BinOp::Add),
        Op::Minus => bin(ir::BinOp::Sub),
        Op::Star => bin(ir::BinOp::Mul),
        Op::Slash => bin(ir::BinOp::Div),
        Op::Eq => bin(ir::BinOp::Eq),
        Op::Ne => bin(ir::BinOp::Ne),
        Op::Lt => bin(ir::BinOp::Lt),
        Op::Le => bin(ir::BinOp::Le),
        Op::Gt => bin(ir::BinOp::Gt),
        Op::Ge => bin(ir::BinOp::Ge),
        Op::And => bin(ir::BinOp::And),
        Op::Or => bin(ir::BinOp::Or),
        Op::Question | Op::Colon => unreachable!("not a binary operator"),
    }
}

/// A name that is no call: `time`, `temper`, `hertz` (any case), else a parameter read.
fn ident(name: &str) -> N {
    match name.to_ascii_lowercase().as_str() {
        "time" => N::Time,
        "temper" => N::Temp,
        "hertz" => N::Freq,
        _ => N::Param(name.to_string()),
    }
}

/// The child indices of a node.
fn children(n: &N) -> Vec<usize> {
    match n {
        N::Call(_, a) | N::User(_, a) | N::Draw(_, a) => a.clone(),
        N::Un(_, a) => vec![*a],
        N::Bin(_, l, r) => vec![*l, *r],
        N::Tern(c, t, f) => vec![*c, *t, *f],
        N::Num(_) | N::Long(_) | N::Param(_) | N::V(..) | N::I(_) | N::Time | N::Temp | N::Freq => {
            Vec::new()
        }
    }
}

impl Parser<'_> {
    /// The whole token list as one expression: the root node.
    fn run(&mut self) -> Result<usize, Fail> {
        let mut operand = true;
        let mut k = 0;
        // Bounded: every arm consumes at least one token.
        while k < self.toks.len() {
            let tok = self.toks[k].clone();
            let next_num = matches!(self.toks.get(k + 1), Some(Tok::Num(..)));
            k += 1;
            if operand {
                match tok {
                    Tok::Op(op @ (Op::Minus | Op::Plus)) => {
                        let un = (op == Op::Minus).then_some(ir::UnOp::Neg);
                        let fold = (op == Op::Minus && next_num).then_some(self.nodes.len());
                        self.ops.push(Frame::Prefix(un, tables::UNARY, fold));
                    }
                    Tok::Open => self.ops.push(Frame::Paren),
                    Tok::Ident(_) if matches!(self.toks.get(k), Some(Tok::Open)) => {
                        k += 1;
                        if matches!(self.toks.get(k), Some(Tok::Close)) {
                            k += 1;
                            self.call(k - 3, &[])?;
                            operand = false;
                            continue;
                        }
                        self.ops.push(Frame::Call(k - 2, self.out.len()));
                    }
                    Tok::Num(lit, r) => {
                        self.number(lit, r);
                        operand = false;
                    }
                    Tok::Ident(name) => {
                        let node = ident(&name);
                        self.leaf(node);
                        operand = false;
                    }
                    Tok::V(n, m) => {
                        self.leaf(N::V(n, m));
                        operand = false;
                    }
                    Tok::I(e) => {
                        self.leaf(N::I(e));
                        operand = false;
                    }
                    Tok::Close | Tok::Comma | Tok::Op(_) => return Err(Fail::invalid()),
                }
                continue;
            }
            match tok {
                Tok::Op(Op::Question) => {
                    self.reduce(tables::TERNARY - 1)?;
                    self.ops.push(Frame::Question);
                }
                Tok::Op(Op::Colon) => {
                    self.reduce_to_question()?;
                    self.ops.push(Frame::Ternary);
                }
                Tok::Op(op) => {
                    let level = level(op);
                    self.reduce(level)?;
                    self.ops.push(Frame::Bin(op, level));
                }
                Tok::Close => self.close()?,
                Tok::Comma => {
                    self.reduce(tables::TERNARY)?;
                    if !matches!(self.ops.last(), Some(Frame::Call(..))) {
                        return Err(Fail::invalid());
                    }
                    operand = true;
                    continue;
                }
                Tok::Num(..) | Tok::Ident(_) | Tok::V(..) | Tok::I(_) | Tok::Open => {
                    return Err(Fail::invalid());
                }
            }
            operand = !matches!(tok, Tok::Close);
        }
        if operand {
            return Err(Fail::invalid());
        }
        self.reduce(tables::TERNARY)?;
        if !self.ops.is_empty() || self.out.len() != 1 {
            return Err(Fail::invalid());
        }
        Ok(self.out[0])
    }

    /// Appends a number read by the lexer as a leaf (a long one as its digits, A18).
    fn number(&mut self, lit: ir::Literal, fit: Fit) {
        self.leaf(match fit {
            Fit::Long(l) => N::Long(l),
            Fit::Exact => N::Num(lit),
        });
    }

    /// Appends a leaf and pushes it as an operand.
    fn leaf(&mut self, n: N) {
        let k = self.push(n);
        self.out.push(k);
    }

    /// Appends a node; returns its index.
    fn push(&mut self, n: N) -> usize {
        self.nodes.push(n);
        self.paren.push(false);
        debug_assert_eq!(self.nodes.len(), self.paren.len());
        self.nodes.len() - 1
    }

    /// Pops operators that bind at `level` or tighter (left-associative), and prefixes whose
    /// operand ends here; stops at a paren, a call, a `?` or a pending ternary's else.
    fn reduce(&mut self, level: u8) -> Result<(), Fail> {
        // Bounded: each step pops one frame.
        while let Some(&top) = self.ops.last() {
            match top {
                Frame::Bin(op, l) if l <= level => {
                    self.ops.pop();
                    self.binary(op)?;
                }
                Frame::Prefix(un, l, fold) if l <= level => {
                    self.ops.pop();
                    self.prefix(un, fold)?;
                }
                Frame::Ternary if level >= tables::TERNARY => {
                    self.ops.pop();
                    self.ternary()?;
                }
                _ => return Ok(()),
            }
        }
        Ok(())
    }

    /// At a `:`: completes everything back to its `?`.
    fn reduce_to_question(&mut self) -> Result<(), Fail> {
        // Bounded: each step pops one frame.
        loop {
            self.reduce(tables::TERNARY - 1)?;
            match self.ops.pop() {
                Some(Frame::Question) => return Ok(()),
                Some(Frame::Ternary) => self.ternary()?,
                _ => return Err(Fail::invalid()),
            }
        }
    }

    /// At a `)`: completes the group or the call it closes.
    fn close(&mut self) -> Result<(), Fail> {
        self.reduce(tables::TERNARY)?;
        match self.ops.pop() {
            Some(Frame::Paren) => {
                let top = *self.out.last().ok_or_else(Fail::invalid)?;
                self.paren[top] = true;
                Ok(())
            }
            Some(Frame::Call(name, base)) => {
                let args = self.out.split_off(base);
                self.call(name, &args)
            }
            _ => Err(Fail::invalid()),
        }
    }

    /// Pops two operands and pushes `op` applied to them.
    fn binary(&mut self, op: Op) -> Result<(), Fail> {
        let r = self.out.pop().ok_or_else(Fail::invalid)?;
        let l = self.out.pop().ok_or_else(Fail::invalid)?;
        use ir::BinOp as B;
        let node = match op {
            Op::Pow => N::Call(ir::Func::Pow, vec![l, r]),
            Op::Plus => N::Bin(B::Add, l, r),
            Op::Minus => N::Bin(B::Sub, l, r),
            Op::Star => N::Bin(B::Mul, l, r),
            Op::Slash => N::Bin(B::Div, l, r),
            Op::Eq => N::Bin(B::Eq, l, r),
            Op::Ne => N::Bin(B::Ne, l, r),
            Op::Lt => N::Bin(B::Lt, l, r),
            Op::Le => N::Bin(B::Le, l, r),
            Op::Gt => N::Bin(B::Gt, l, r),
            Op::Ge => N::Bin(B::Ge, l, r),
            Op::And => N::Bin(B::And, l, r),
            Op::Or => N::Bin(B::Or, l, r),
            Op::Question | Op::Colon => return Err(Fail::invalid()),
        };
        let k = self.push(node);
        self.out.push(k);
        Ok(())
    }

    /// Pops one operand and applies a prefix: `+` is no node; a `-` whose operand is the number
    /// right after it is that literal's sign.
    fn prefix(&mut self, un: Option<ir::UnOp>, fold: Option<usize>) -> Result<(), Fail> {
        let a = self.out.pop().ok_or_else(Fail::invalid)?;
        let node = match (un, &mut self.nodes[a]) {
            (None, _) => a,
            (Some(ir::UnOp::Neg), N::Num(lit)) if fold == Some(a) && !self.paren[a] => {
                lit.mantissa = -lit.mantissa;
                a
            }
            (Some(ir::UnOp::Neg), N::Long(l)) if fold == Some(a) && !self.paren[a] => {
                l.negate();
                a
            }
            (Some(op), _) => self.push(N::Un(op, a)),
        };
        self.out.push(node);
        Ok(())
    }

    /// Pops a condition, a then and an else (C grouping: `1?0?4:8:16` = 8).
    fn ternary(&mut self) -> Result<(), Fail> {
        let f = self.out.pop().ok_or_else(Fail::invalid)?;
        let t = self.out.pop().ok_or_else(Fail::invalid)?;
        let c = self.out.pop().ok_or_else(Fail::invalid)?;
        let k = self.push(N::Tern(c, t, f));
        self.out.push(k);
        Ok(())
    }
}
