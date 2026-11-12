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
//! ref      = name                    bound by `link` after the whole text (exact name, from the
//!                                    item's scope outward); unresolved if nothing matches
//!          | name "?"                deliberately unresolved (def NONE)
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
//! References are bound by `link` after the whole text is read (A4), so a bare name may name a
//! definition that comes later. `name?` is held unresolved through the link; `dump` writes
//! `name#k` only when the lookup would bind another definition than the row's.
//!
//! Contents: `pub fn dump(&Design, FileId, &mut String)`,
//! `pub fn parse(&str, &mut Builder<'_>, &mut Diags)`, `Words` (re-exported from `text_words`), the
//! writers `Ctx`, `write_expr`, `write_value`, `write_exprs`, `write_names`, `write_params`,
//! `write_ref`, `write_name`, `write_string`, `write_literal` (re-exported from `text_expr`); crate:
//! `pub(crate) fn keyword(ItemKind) -> &'static str`

use std::collections::HashMap;

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::expr::ExprTag;
use crate::facts::RowFacts;
use crate::id::{BranchList, DefId, ExprId, FileId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, If, ItemKind, Role};
use crate::item_text;
use crate::link::link;
pub use crate::text_expr::{
    Ctx, write_expr, write_exprs, write_literal, write_name, write_names, write_params, write_ref,
    write_string, write_value,
};
pub use crate::text_words::Words;

/// An `if` whose `elif`/`else` lines are still to come at its depth: its branches and the
/// next one to open. `None` branches: the `if` failed, its other lines are skipped.
struct Chain {
    branches: Option<BranchList>,
    next: usize,
}

/// The keyword of each item kind in IR text (the kernel writes it; a row's `dump` writes what
/// follows).
pub(crate) fn keyword(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Title => "title",
        ItemKind::Comment => "comment",
        ItemKind::Include => "include",
        ItemKind::Global => "global",
        ItemKind::Load => "load",
        ItemKind::Opaque => "opaque",
        ItemKind::Subckt => "subckt",
        ItemKind::Section => "section",
        ItemKind::If => "if",
        ItemKind::Param => "param",
        ItemKind::Func => "func",
        ItemKind::Model => "model",
        ItemKind::DeviceLibrary => "device_library",
        ItemKind::Statistics => "statistics",
        ItemKind::Element => "element",
        ItemKind::Analysis => "analysis",
        ItemKind::Loop => "loop",
        ItemKind::Alter => "alter",
        ItemKind::Option => "option",
        ItemKind::Env => "env",
        ItemKind::NodeSet => "nodeset",
        ItemKind::Save => "save",
        ItemKind::Print => "print",
        ItemKind::Measure => "measure",
    }
}

/// Appends `file`'s block (its `file` line and every item of its scopes, depth first in item
/// order) to `out`. Total over a design that passes `check`. Spans are not written. Each item
/// line is its row's `RowFacts::dump` (one exhaustive dispatch over `ItemKind`).
pub fn dump(design: &Design, file: FileId, out: &mut String) {
    /// What remains to write: a scope's items from `next` on, or an `elif`/`else` line.
    enum Frame {
        Items {
            scope: ScopeId,
            depth: usize,
            next: usize,
        },
        Branch {
            cond: ExprId,
            scope: ScopeId,
            item: ItemRef,
            depth: usize,
        },
    }
    let f = design.file(file);
    out.push_str("file ");
    write_string(design.str(f.path), out);
    out.push_str(match f.role {
        Role::Deck => " deck\n",
        Role::Library => " library\n",
        Role::VerilogA => " veriloga\n",
        Role::Missing => " missing\n",
    });
    let bodies = design.bodies();
    let mut modules: HashMap<ItemRef, Vec<NameId>> = HashMap::new();
    for def in design
        .definitions()
        .iter()
        .filter(|def| def.kind == DefKind::Module)
    {
        modules.entry(def.item).or_default().push(def.name);
    }
    let mut stack = vec![Frame::Items {
        scope: f.top,
        depth: 1,
        next: 0,
    }];
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Branch {
                cond,
                scope,
                item,
                depth,
            } => {
                indent(depth, out);
                if cond.is_none() {
                    out.push_str("else\n");
                } else {
                    out.push_str("elif (");
                    write_expr(
                        design,
                        cond,
                        &Ctx {
                            scope,
                            item,
                            formals: DefId::NONE,
                        },
                        out,
                    );
                    out.push_str(")\n");
                }
            }
            Frame::Items { scope, depth, next } => {
                let Some(&item) = design.items(scope).get(next) else {
                    continue;
                };
                stack.push(Frame::Items {
                    scope,
                    depth,
                    next: next + 1,
                });
                indent(depth, out);
                out.push_str(keyword(item.kind));
                let fields = out.len();
                out.push(' ');
                let ctx = Ctx {
                    scope,
                    item,
                    formals: DefId::NONE,
                };
                let own = modules.get(&item).map_or(&[][..], |m| m.as_slice());
                write_fields(design, item, &ctx, own, out);
                if out.len() == fields + 1 {
                    out.truncate(fields);
                }
                out.push('\n');
                if item.kind == ItemKind::If {
                    let row: If = design.row(item.index);
                    let branches = design.branches(row.branches);
                    for (k, br) in branches.iter().enumerate().rev() {
                        stack.push(Frame::Items {
                            scope: br.body,
                            depth: depth + 1,
                            next: 0,
                        });
                        if k > 0 {
                            stack.push(Frame::Branch {
                                cond: br.cond,
                                scope,
                                item,
                                depth,
                            });
                        }
                    }
                } else if let Some(scopes) = bodies.get(&item) {
                    for &body in scopes.iter().rev() {
                        stack.push(Frame::Items {
                            scope: body,
                            depth: depth + 1,
                            next: 0,
                        });
                    }
                }
            }
        }
    }
}

/// Reads one or more `file` blocks into `b`. Every span it makes (an item's, a `TEXT_SYNTAX`
/// row's) names the file block its line sits in (`FileId::NONE` for a `file` line itself) and
/// holds byte offsets into `text`, the whole text handed to `parse`, not into its block. Total: a line it cannot read appends a
/// `diag::TEXT_SYNTAX` row (span: the line's bytes in `text`, file `NONE`) and is skipped; what
/// it can read is still built. Each item line goes to its kind's `RowFacts::parse` by its keyword
/// (one exhaustive dispatch over `ItemKind`); then `link` binds every reference, `name?` ones
/// excepted (module header).
pub fn parse(text: &str, b: &mut Builder<'_>, diags: &mut Diags) {
    let lines = line_ranges(text);
    let mut stack: Vec<ScopeId> = Vec::new(); // stack[d - 1]: the scope of a depth-d line
    let mut chains: Vec<Option<Chain>> = Vec::new(); // per depth: an `if` awaiting `elif`/`else`
    let mut skip_deeper: Option<usize> = None;
    for (k, &(start, end)) in lines.iter().enumerate() {
        let Some((depth, at)) = statement_start(text, start, end) else {
            continue;
        };
        if let Some(d) = skip_deeper {
            if depth > d {
                continue;
            }
            skip_deeper = None;
        }
        let before = diags.len();
        // the line's file: the block it sits in (a `file` line itself is in none)
        let file = match stack.first() {
            Some(&top) if depth > 0 => b.design().scope(top).file,
            Some(_) | None => FileId::NONE,
        };
        let mut w = Words {
            text,
            pos: at,
            end,
            line: at,
            file,
        };
        let failed = statement(
            &mut w,
            depth,
            &lines[k + 1..],
            &mut stack,
            &mut chains,
            b,
            diags,
        );
        if failed {
            skip_deeper = Some(depth);
            if diags.len() == before {
                w.error(diags);
            }
        }
        diags
            .rows
            .truncate(before + (diags.len() > before) as usize); // one row per bad line
    }
    link(b.design, diags);
    clear_held(b.design);
}

// ---- private ----

/// Reads one statement (the line's keyword already ahead of `w`). Returns whether it failed;
/// a failed statement's deeper lines are skipped.
fn statement(
    w: &mut Words<'_>,
    depth: usize,
    rest: &[(usize, usize)],
    stack: &mut Vec<ScopeId>,
    chains: &mut Vec<Option<Chain>>,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> bool {
    let Some(kw) = w.next() else { return false };
    if depth == 0 {
        return !file_line(w, kw, stack, chains, b, diags);
    }
    if depth > stack.len() {
        return true;
    }
    stack.truncate(depth);
    chains.truncate(depth);
    let scope = stack[depth - 1];
    match kw {
        "elif" | "else" => {
            let Some(chain) = chains[depth - 1].as_mut() else {
                return true;
            };
            let Some(list) = chain.branches else {
                return false;
            }; // a failed `if`: skip quietly
            let Some(branch) = b.design().branches(list).get(chain.next).copied() else {
                return true;
            };
            chain.next += 1;
            stack.push(branch.body);
            chains.push(None);
            false
        }
        "if" => {
            chains[depth - 1] = None;
            let Some(conds) = if_conditions(w, depth, rest, scope, b, diags) else {
                chains[depth - 1] = Some(Chain {
                    branches: None,
                    next: 0,
                });
                return true;
            };
            let (_, list) = b.if_(scope, &conds, w.line_span());
            chains[depth - 1] = Some(Chain {
                branches: Some(list),
                next: 1,
            });
            stack.push(b.design().branches(list)[0].body);
            chains.push(None);
            false
        }
        _ => {
            chains[depth - 1] = None;
            let Some(kind) = ItemKind::ALL
                .into_iter()
                .find(|k| keyword(*k) == kw && *k != ItemKind::If)
            else {
                return true;
            };
            let scopes_before = b.design().scopes.len();
            let item = for_kind!(kind, R => R::parse(w, scope, b, diags));
            if item == ItemRef::NONE {
                return true;
            }
            if !w.at_end() {
                w.error(diags);
            }
            let d = b.design();
            let body = (scopes_before..d.scopes.len())
                .map(|s| ScopeId(s as u32))
                .find(|s| d.scope(*s).owner == item);
            if let Some(body) = body {
                stack.push(body);
                chains.push(None);
            }
            false
        }
    }
}

/// `file "path" role`: opens a file block. Returns whether it succeeded.
fn file_line(
    w: &mut Words<'_>,
    kw: &str,
    stack: &mut Vec<ScopeId>,
    chains: &mut Vec<Option<Chain>>,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> bool {
    stack.clear();
    chains.clear();
    if kw != "file" {
        return false;
    }
    let Some(path) = w.string(diags) else {
        return false;
    };
    let role = match w.next() {
        Some("deck") => Role::Deck,
        Some("library") => Role::Library,
        Some("veriloga") => Role::VerilogA,
        Some("missing") => Role::Missing,
        _ => return false,
    };
    if !w.at_end() || !b.design().find_file(&path).is_none() {
        return false;
    }
    let (_, top) = b.file(&path, role);
    stack.push(top);
    chains.push(None);
    true
}

/// The conditions of an `if` chain: this line's `(cond)`, then each following `elif (cond)` and
/// a final `else` at the same depth (deeper lines between them are bodies). Built before any
/// body, as a builder caller builds them. `None` (one row reported) if any is malformed.
fn if_conditions(
    w: &mut Words<'_>,
    depth: usize,
    rest: &[(usize, usize)],
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<Vec<ExprId>> {
    let first = w.value(scope, b, diags);
    if first.is_none() || !w.at_end() {
        return None;
    }
    let mut conds = vec![first];
    for &(start, end) in rest {
        let Some((d, at)) = statement_start(w.text, start, end) else {
            continue;
        };
        if d > depth {
            continue;
        }
        let mut line = Words {
            text: w.text,
            pos: at,
            end,
            line: at,
            file: w.file,
        };
        match (d == depth).then(|| line.next()).flatten() {
            Some("elif") => {
                let cond = line.value(scope, b, diags);
                if cond.is_none() || !line.at_end() {
                    return None;
                }
                conds.push(cond);
            }
            Some("else") => {
                if !line.at_end() {
                    line.error(diags);
                    return None;
                }
                conds.push(ExprId::NONE);
                break;
            }
            _ => break,
        }
    }
    Some(conds)
}

/// Writes the fields of `item`'s row (after its keyword): kernel rows with their context
/// (`item_text`), the owners' rows through their `RowFacts::dump`.
fn write_fields(d: &Design, item: ItemRef, ctx: &Ctx, modules: &[NameId], out: &mut String) {
    fn owner<R: RowFacts>(d: &Design, ctx: &Ctx, out: &mut String) {
        d.row::<R>(ctx.item.index).dump(d, ctx, out);
    }
    let i = item.index;
    match item.kind {
        ItemKind::Title => item_text::dump_title(&d.row(i), d, item, ctx, out),
        ItemKind::Comment => item_text::dump_comment(&d.row(i), d, item, ctx, out),
        ItemKind::Include => item_text::dump_include(&d.row(i), d, item, ctx, out),
        ItemKind::Global => item_text::dump_global(&d.row(i), d, item, ctx, out),
        ItemKind::Load => item_text::dump_load(&d.row(i), d, modules, out),
        ItemKind::Opaque => item_text::dump_opaque(&d.row(i), d, item, ctx, out),
        ItemKind::Subckt => item_text::dump_subckt(&d.row(i), d, item, ctx, out),
        ItemKind::Section => item_text::dump_section(&d.row(i), d, item, ctx, out),
        ItemKind::If => item_text::dump_if(&d.row(i), d, item, ctx, out),
        ItemKind::Param => item_text::dump_param(&d.row(i), d, item, ctx, out),
        ItemKind::Func => item_text::dump_func(&d.row(i), d, item, ctx, out),
        ItemKind::Model => owner::<crate::model::Model>(d, ctx, out),
        ItemKind::DeviceLibrary => owner::<crate::library::DeviceLibrary>(d, ctx, out),
        ItemKind::Statistics => owner::<crate::stats::Statistics>(d, ctx, out),
        ItemKind::Element => owner::<crate::element::Element>(d, ctx, out),
        ItemKind::Analysis => owner::<crate::analysis::Analysis>(d, ctx, out),
        ItemKind::Loop => owner::<crate::plan::Loop>(d, ctx, out),
        ItemKind::Alter => owner::<crate::plan::Alter>(d, ctx, out),
        ItemKind::Option => owner::<crate::option::OptionRow>(d, ctx, out),
        ItemKind::Env => owner::<crate::option::Env>(d, ctx, out),
        ItemKind::NodeSet => owner::<crate::option::NodeSet>(d, ctx, out),
        ItemKind::Save => owner::<crate::output::Save>(d, ctx, out),
        ItemKind::Print => owner::<crate::output::Print>(d, ctx, out),
        ItemKind::Measure => owner::<crate::output::Measure>(d, ctx, out),
    }
}

/// After the link: a reference held as `name?` becomes unresolved (`DefId::HELD` never
/// survives `parse`).
fn clear_held(d: &mut Design) {
    let x = &mut d.exprs;
    for (i, tag) in x.tags.iter().enumerate() {
        if *tag == ExprTag::Param && x.b[i] == DefId::HELD.0 {
            x.b[i] = DefId::NONE.0;
        }
    }
    for call in &mut x.calls {
        if call.func.def == DefId::HELD {
            call.func.def = DefId::NONE;
        }
    }
    for (item, _) in d.reachable() {
        d.editor().map_refs(item, &mut |r, _| {
            if r.def == DefId::HELD {
                r.def = DefId::NONE;
            }
        });
    }
}

/// The `[start, end)` byte range of each line (without its line break).
fn line_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (k, c) in text.bytes().enumerate() {
        if c == b'\n' {
            lines.push((start, k));
            start = k + 1;
        }
    }
    if start < text.len() {
        lines.push((start, text.len()));
    }
    lines
}

/// A statement line's depth and first byte; `None` for a blank or `#` comment line. An odd
/// indentation is read at the depth below it (and reported by the line's own reading).
fn statement_start(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let s = &text.as_bytes()[start..end];
    let n = s.iter().take_while(|c| **c == b' ').count();
    let rest = &s[n..];
    if rest
        .iter()
        .all(|c| *c == b' ' || *c == b'\t' || *c == b'\r')
        || rest[0] == b'#'
    {
        return None;
    }
    Some((n / 2, start + n))
}

fn indent(depth: usize, out: &mut String) {
    out.extend(std::iter::repeat_n(' ', 2 * depth));
}
