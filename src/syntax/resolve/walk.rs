//! A file's tree → its statements in source order, each with its
//! [`Scope`] and the subcircuits around it: the one walk `declare` and
//! `lookup` share, on a heap stack (rule 7 of the compiler-construction
//! skill), so a file nested 10⁴ deep needs no deeper native stack than a
//! flat one.
//!
//! Cites: ngspice manual §2.5 (`.subckt` nests), §2.15 (`.if`); Spectre
//! Reference 19.1 p.491 (`if`), p.498 (`section`).
//!
//! Contents:
//! - `pub(super) fn walk(ast: &Ast, visit: impl FnMut(u32, Scope, &[u32]))`

use super::Scope;
use crate::syntax::ast::{Ast, NONE, Node, Tag};

/// An open block of [`walk`].
struct Open<'a> {
    items: &'a [u32],
    at: usize,
    scope: Scope,
    /// The scope around the block.
    outer: Scope,
    /// Whether the block is an `if` branch, whose chain's next follows it.
    branch: bool,
    /// Whether the block is a subcircuit.
    subckt: bool,
    node: u32,
}

/// Visits every statement of `ast` in source order (a block's head, then
/// its body, then a chain's next branch), with its [`Scope`] and the
/// subcircuits around it, outermost first. A heap stack, so a file nested
/// 10⁴ deep needs no deeper native stack than a flat one.
pub(super) fn walk(ast: &Ast, mut visit: impl FnMut(u32, Scope, &[u32])) {
    let mut stack = vec![Open {
        items: ast.fields(ast.root()),
        at: 0,
        scope: Scope::Top,
        outer: Scope::Top,
        branch: false,
        subckt: false,
        node: 0,
    }];
    let mut chain: Vec<u32> = Vec::new();
    while let Some(top) = stack.last_mut() {
        if let Some(&n) = top.items.get(top.at) {
            top.at += 1;
            let scope = top.scope;
            visit(n, scope, &chain);
            if let Some(open) = opened(ast, n, scope) {
                if open.subckt {
                    chain.push(n);
                }
                stack.push(open);
            }
            continue;
        }
        let done = stack.pop().expect("the top frame");
        if done.subckt {
            assert_eq!(
                chain.pop(),
                Some(done.node),
                "the chain ends in this subckt"
            );
        }
        if done.branch
            && let Some(next) = ast.next(Node(done.node))
        {
            visit(next.0, done.outer, &chain);
            stack.extend(opened(ast, next.0, done.outer));
        }
    }
    assert!(chain.is_empty(), "every subckt frame popped");
}

// ---- private ----

/// The body statement `n` opens, if any, and the scope inside it.
fn opened<'a>(ast: &'a Ast, n: u32, scope: Scope) -> Option<Open<'a>> {
    let node = Node(n);
    let tag = ast.tag(node);
    if !has_body(tag) || ast.rhs(node) == NONE {
        return None;
    }
    let inner = match tag {
        Tag::Subckt => Scope::Subckt(node),
        Tag::Section => Scope::Section(node),
        Tag::If | Tag::Else => Scope::Branch(node),
        // else: the other bodies are no scope of their own.
        _ => scope,
    };
    Some(Open {
        items: ast.body(node),
        at: 0,
        scope: inner,
        outer: scope,
        branch: matches!(tag, Tag::If | Tag::Else),
        subckt: tag == Tag::Subckt,
        node: n,
    })
}

/// Whether a `tag` statement's `rhs` is a body when it is not `NONE`.
fn has_body(tag: Tag) -> bool {
    match tag {
        Tag::Subckt
        | Tag::Section
        | Tag::Library
        | Tag::If
        | Tag::Else
        | Tag::Control
        | Tag::Region
        | Tag::Data
        | Tag::Func
        | Tag::Model
        | Tag::Analysis
        | Tag::Command
        | Tag::Statistics
        | Tag::Process
        | Tag::Card
        | Tag::Instance => true,
        Tag::Root
        | Tag::Title
        | Tag::End
        | Tag::Verbatim
        | Tag::Include
        | Tag::LibCall
        | Tag::Line
        | Tag::Param
        | Tag::Options
        | Tag::Save
        | Tag::Print
        | Tag::Measure
        | Tag::Ic
        | Tag::Nodeset
        | Tag::Temp
        | Tag::Global
        | Tag::Step
        | Tag::Word
        | Tag::Assign
        | Tag::Group
        | Tag::Seq
        | Tag::Delimited
        | Tag::Num
        | Tag::Ident
        | Tag::Str
        | Tag::Unary
        | Tag::Binary
        | Tag::Ternary
        | Tag::Call
        | Tag::Probe
        | Tag::Index
        | Tag::Paren
        | Tag::Vector
        | Tag::List
        | Tag::Bad => false,
    }
}
