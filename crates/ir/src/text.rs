//! The IR's own text, "dialect zero": a design's file in, canonical lines out (`dump`), and
//! lines in, builder calls out (`parse`). The format of reader fixtures written by hand from a
//! manual clause, of `--dump-ir`, and of the Python store.
//!
//! Properties: **canonical** (one spelling per design: `dump` chooses every space, parenthesis,
//! number spelling and field order), **lossless** (`parse(dump(d))` rebuilds tables equal to
//! `d`'s under `Design::eq_ignoring_spans`), **line oriented** (one item per line, a body's items
//! on the lines below it), **indented by scope** (two spaces per level), **writable by hand**
//! (`parse` accepts any spacing between tokens and redundant parentheses; it rebuilds the same
//! rows).
//!
//! # Grammar
//!
//! ```text
//! text     = { line }
//! line     = indent ( block | item | branch ) NL | indent "#" any NL | NL
//! indent   = "  " × depth            (a file's items at depth 1, a body's one deeper)
//! block    = "file" string role      (depth 0; the items below belong to it)
//! role     = "deck" | "library" | "veriloga" | "missing"
//! item     = keyword { " " field }
//! field    = name | ref | "(" [ name { " " name } ] ")" | key "=" value
//! value    = atom | "(" expr ")" | "(" [ name { " " name } ] ")"
//! atom     = number | string | ref | leaf
//! number   = ["-"] digits ["." digits] [ "e" ["-"] digits | scale | "mil" ]
//!            the literal as stored (Addendum A3): the mantissa's digits with `frac` of them
//!            after the point (trailing zeros kept: 0.050000), then the form: `e<exp10>`
//!            (Exponent), a scale letter (Scale), `mil`, or nothing (Plain). Lossless: the
//!            text rebuilds the literal field by field; `1k` and `1e3` stay distinct.
//! scale    = a(−18) f(−15) p(−12) n(−9) u(−6) m(−3) c(−2) k(3) meg(6) g(9) t(12) peta(15)
//!            (lower case, one spelling each)
//! string   = '"' { char | '\"' | '\\' | '\n' } '"'
//! name     = bare | "`" { char | "\`" | "\\" } "`"
//! bare     = [A-Za-z0-9_.$]+ in a name position; [A-Za-z_][A-Za-z0-9_.$]* in an expression
//! ref      = name                    resolved: bound by `Design::lookup` from the item's scope
//!          | name "?"                unresolved (def NONE)
//!          | name "#" digits         bound to that definition (only when lookup binds elsewhere)
//! ```
//!
//! Kernel items (owners' items follow the same shape: keyword, then the row's fields in
//! declaration order, `NONE` fields left out; their keyword tables are in their files):
//!
//! ```text
//! title "text"
//! comment "text"
//! include "path" [section=name]
//! global name { name }
//! load veriloga|osdi "path" [modules=(name …)]      the loaded modules' definitions
//! opaque name unread|private|inert|invalid|script "text"
//! subckt name (port …) { key=value }                 body below
//! section name [library=name]                        body below
//! if (expr)            body below; then at the same depth:  elif (expr)  …  else
//! param name = expr
//! func name(arg, …) = expr
//! ```
//!
//! Expressions, loosest to tightest, every binary level left-associative (`dump` writes the
//! fewest parentheses that rebuild the same tree, one space around a binary operator, one after a
//! comma):
//!
//! ```text
//! expr    = or [ " ? " expr " : " expr ]
//! or      = and { " || " and }          and  = bor { " && " bor }
//! bor     = bxor { " | " bxor }         bxor = band { " ^ " band }
//! band    = eq { " & " eq }             eq   = rel { (" == " | " != ") rel }
//! rel     = shift { (" < " | " <= " | " > " | " >= ") shift }
//! shift   = add { (" << " | " >> ") add }
//! add     = mul { (" + " | " - ") mul }
//! mul     = unary { (" * " | " / " | " // ") unary }    (`//` is IntDiv)
//! unary   = ("-" | "!" | "~") unary | primary
//!           (a "-" glued to a digit where a unary may stand is a negative literal's sign; the
//!           `Neg` of a literal is written "-(…)": `-1.5` and `-(1.5)` are two trees)
//! primary = atom | "(" expr ")" | "[" [ expr { ", " expr } ] "]"
//!         | func "(" args ")"                  snake case of `Func`: pow_abs, round_half_even, …
//!         | "@" ref "(" args ")"               a user function call
//!         | "draw:" dist "(" args ")"          snake case of `Dist`: abs_gauss, …
//!         | "v(" name [", " name] ")" | "i(" name ")"
//! leaf    = "$time" | "$temp" | "$freq" | "$tstep" | "$tstop" | "$tnom"
//! ```
//!
//! # Worked example
//!
//! A deck with a parameter, a subcircuit holding two elements, an instance, a source and an
//! analysis (element and analysis lines are the circuit and run owners'; shown with their
//! intended shape):
//!
//! ```text
//! file "rc.sp" deck
//!   title "rc divider"
//!   param rval = 1k
//!   subckt div (in out gnd) r=rval
//!     element resistor r1 (in out) value=r
//!     element resistor r2 (out gnd) value=(r / 2)
//!   element instance x1 (a b 0) model=div
//!   element vsource v1 (a 0) dc=1.8
//!   analysis op
//! ```
//!
//! `rval` in `r=rval` is bound by lookup to the top-level `param`; `r` in the bodies to the
//! subcircuit's head default; `model=div` to the subcircuit.
//!
//! Cites: `REWRITE-PLAN.md` §3.6, §4.2 (L5); `rewrite/IR-KERNEL.md` §9.
//!
//! The kernel writes `file` lines, indentation and bodies, and dispatches each item line by its
//! keyword (one exhaustive `match` over `ItemKind`) to the row's `RowFacts::dump` / `parse`.
//! A row's `parse` reads its statement through [`Words`], the reader's cursor over one line.
//!
//! References in a row are bound as they are read: a bare name to what `Design::lookup` finds
//! among the definitions pushed so far, so `dump` writes `name#k` for a reference to a definition
//! that comes later in the text. Parameter and user-call reads inside expressions are bound after
//! the whole text is read (`Builder::resolve`), so they may name later definitions bare.
//!
//! Contents: `pub fn dump(&Design, FileId, &mut String)`,
//! `pub fn parse(&str, &mut Builder<'_>, &mut Diags)`, `pub struct Words<'t>`,
//! `Words::next(&mut self) -> Option<&'t str>`, `Words::peek(&self) -> Option<&'t str>`,
//! `Words::at_end(&self) -> bool`, `Words::expect(&mut self, &str, &mut Diags) -> bool`,
//! `Words::key(&mut self) -> Option<&'t str>`,
//! `Words::name(&mut self, &mut Builder<'_>, &mut Diags) -> NameId`,
//! `Words::names(&mut self, &mut Builder<'_>, &mut Diags) -> NameList`,
//! `Words::number(&mut self, &mut Diags) -> Option<Literal>`,
//! `Words::string(&mut self, &mut Diags) -> Option<Cow<'t, str>>`,
//! `Words::reference(&mut self, DefKind, ScopeId, &mut Builder<'_>, &mut Diags) -> Ref`,
//! `Words::value(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ExprId`,
//! `Words::expr(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ExprId`,
//! `Words::exprs(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ExprList`,
//! `Words::params(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ParamList`,
//! `Words::error(&self, &mut Diags)`, `Words::span(&self) -> Span`

use std::borrow::Cow;

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
#[cfg(doc)]
use crate::diag::TEXT_SYNTAX;
use crate::expr::Literal;
use crate::id::{ExprId, ExprList, FileId, NameId, NameList, ParamList, Ref, ScopeId};
use crate::item::DefKind;
use crate::span::Span;

/// The IR text reader's cursor over one statement line (after its indentation). A word is a
/// maximal run of non-blank bytes outside a string or backtick name, or one of `( ) [ ] , =`.
/// Every reading method that fails appends a [`TEXT_SYNTAX`] row at the current word, consumes
/// nothing more than the bad word, and returns its failure value (`None`, `NONE`, an empty list).
#[derive(Debug)]
pub struct Words<'t> {
    /// The whole text (spans are offsets into it).
    pub(crate) text: &'t str,
    /// Byte offset of the next word.
    pub(crate) pos: usize,
    /// Byte offset of the end of this line.
    pub(crate) end: usize,
    /// Parameter and user-call reads to bind once the whole text is read: (expression, scope).
    pub(crate) pending: Vec<(ExprId, ScopeId)>,
}

/// Appends `file`'s block (its `file` line and every item of its scopes, depth first in item
/// order) to `out`. Total over a design that passes `check`. Spans are not written. Each item
/// line is its row's `RowFacts::dump` (one exhaustive dispatch over `ItemKind`).
// definition phase: ptr_arg fires only because the body is todo!() (it will push to `out`).
#[allow(clippy::ptr_arg)]
pub fn dump(design: &Design, file: FileId, out: &mut String) {
    todo!()
}

/// Reads one or more `file` blocks into `b`. Total: a line it cannot read appends a
/// `diag::TEXT_SYNTAX` row (span: the line's bytes in `text`, file `NONE`) and is skipped; what
/// it can read is still built. Each item line goes to its kind's `RowFacts::parse` by its keyword
/// (one exhaustive dispatch over `ItemKind`); expression references are bound after the whole
/// text is read, row references as they are read (module header).
pub fn parse(text: &str, b: &mut Builder<'_>, diags: &mut Diags) {
    todo!()
}

impl<'t> Words<'t> {
    /// The next word, consumed; `None` at the end of the line.
    #[allow(clippy::should_implement_trait)] // a cursor over one line, not an `Iterator`
    pub fn next(&mut self) -> Option<&'t str> {
        todo!()
    }

    /// The next word, not consumed.
    pub fn peek(&self) -> Option<&'t str> {
        todo!()
    }

    /// Whether the line has no word left.
    pub fn at_end(&self) -> bool {
        todo!()
    }

    /// Consumes `word` if it is next; else reports and returns false.
    pub fn expect(&mut self, word: &str, diags: &mut Diags) -> bool {
        todo!()
    }

    /// If the next words are `key =`, consumes them and returns the key; else `None`, consuming
    /// nothing (fields are optional, so this never reports).
    pub fn key(&mut self) -> Option<&'t str> {
        todo!()
    }

    /// A name (bare in a name position, or backtick-quoted), interned.
    pub fn name(&mut self, b: &mut Builder<'_>, diags: &mut Diags) -> NameId {
        todo!()
    }

    /// A parenthesized, blank-separated name list: `(a b c)`, possibly `()`.
    pub fn names(&mut self, b: &mut Builder<'_>, diags: &mut Diags) -> NameList {
        todo!()
    }

    /// A number in the grammar's spelling, as its literal (no floating arithmetic).
    pub fn number(&mut self, diags: &mut Diags) -> Option<Literal> {
        todo!()
    }

    /// A double-quoted string, unescaped: borrowed from the text when it has no escape, owned
    /// when it has.
    pub fn string(&mut self, diags: &mut Diags) -> Option<Cow<'t, str>> {
        todo!()
    }

    /// A reference to a definition of `kind`: `name` (bound by `Design::lookup` from `scope`
    /// among the definitions pushed so far; reports when none), `name?` (unresolved) or
    /// `name#k` (bound to definition `k`, checked by `check` once the text is read).
    pub fn reference(
        &mut self,
        kind: DefKind,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Ref {
        todo!()
    }

    /// A field value: an atom, or a parenthesized expression. Names read in it are bound from
    /// `scope` after the whole text.
    pub fn value(&mut self, scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ExprId {
        todo!()
    }

    /// An expression to the end of the line or the next unmatched `)`/`,`/`]` (the `param` and
    /// `func` right-hand sides). Explicit operator stack: no recursion on nesting depth (C11).
    pub fn expr(&mut self, scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ExprId {
        todo!()
    }

    /// A bracketed expression list: `[e1, e2]`, possibly `[]`.
    pub fn exprs(&mut self, scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ExprList {
        todo!()
    }

    /// A parenthesized `key=value` list: `(w=1e-6 l=(2 * lmin))`, possibly `()`.
    pub fn params(&mut self, scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ParamList {
        todo!()
    }

    /// Appends a [`TEXT_SYNTAX`] row at the current word.
    pub fn error(&self, diags: &mut Diags) {
        todo!()
    }

    /// The current word's bytes (file `NONE`: the text is not a design file).
    pub fn span(&self) -> Span {
        todo!()
    }
}
