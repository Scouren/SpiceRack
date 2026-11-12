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
//! number   = Rust `{:?}` of the f64 (shortest round trip): 1000.0, 1e-6, -2.5
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
//!   param rval = 1000.0
//!   subckt div (in out gnd) r=rval
//!     element resistor r1 (in out) value=r
//!     element resistor r2 (out gnd) value=(r / 2.0)
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
//! Contents: `pub fn dump(&Design, FileId, &mut String)`,
//! `pub fn parse(&str, &mut Builder<'_>, &mut Diags)`

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::id::FileId;

/// Appends `file`'s block (its `file` line and every item of its scopes, depth first in item
/// order) to `out`. Total over a design that passes `check`. Spans are not written.
// definition phase: ptr_arg fires only because the body is todo!() (it will push to `out`).
#[allow(clippy::ptr_arg)]
pub fn dump(design: &Design, file: FileId, out: &mut String) {
    todo!()
}

/// Reads one or more `file` blocks into `b`. Total: a line it cannot read appends a
/// `diag::TEXT_SYNTAX` row (span: the line's bytes in `text`, file `NONE`) and is skipped; what
/// it can read is still built. References are bound after the whole text is read.
pub fn parse(text: &str, b: &mut Builder<'_>, diags: &mut Diags) {
    todo!()
}
