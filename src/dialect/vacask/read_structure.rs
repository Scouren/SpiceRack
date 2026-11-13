//! The structure statements → IR rows: the title, `include` (and `section=`), `section`,
//! `load`, `embed`, `subckt` (nested too) with its `parameters` split into formals and locals,
//! `parameters`, `global`, `ground`, `@if` chains.
//!
//! Rules (`rewrite/DIALECT-VACASK.md` §3.2, each with its evidence there):
//! - a subckt's `parameters` entry whose value folds to a constant at parse time (literals,
//!   builtin constants, functions of them) is a formal; one that reads a name is a local
//!   (RUN c1, t2);
//! - a parameter defined twice in one scope is refused (RUN n3, `R2001`);
//! - `@if` bodies hold instances, models and `@if`s: a `parameters` there is refused (`P2005`);
//! - `ground` names are node `0` everywhere; the statement itself is `Inert` (R15);
//! - a `load` of a library file VACASK ships is a `DeviceLibrary` (A12 C5; its default file
//!   with no path, `tables.rs`); any
//!   other `.osdi` or `.va` a `Load`; a `map=` other than the default `Private`;
//! - an `embed` that a `load` or `include` names is `Private`, else `Inert` (C A20).
//!
//! Cites: VD cir-subckt, cir-subckt-nested, cir-conditional, cir-nodes, cir-loading,
//! input-embed, input-include.
//!
//! Contents: `PLACEMENTS`, `FEATURES`, `CODES`, `REDEFINED`, `IN_BRANCH`, `INCLUDED_TWICE`,
//! `ENCLOSING`, `TOP_ONLY`, and the area
//! functions
//! `title`, `include`, `section`, `subckt`, `if_chain`, `param`, `global`, `ground`, `load`,
//! `embed` (crate-private)

use super::expr_read::{self, Context};
use super::read::{self, Lower};
use super::{codes, families, tables};
use crate::dialect::Reference;
use crate::ir;
use crate::syntax::ast::{Name, Node, Tag};

mod if_chain;
mod load;

pub(super) use if_chain::if_chain;
pub(super) use load::load;

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = families::proven_families(PLACEMENTS)
    .with(ir::Feature::Item(ir::ItemKind::Title))
    .with(ir::Feature::Item(ir::ItemKind::Comment))
    .with(ir::Feature::Item(ir::ItemKind::Include))
    .with(ir::Feature::Item(ir::ItemKind::Global))
    .with(ir::Feature::Item(ir::ItemKind::Load))
    .with(ir::Feature::Item(ir::ItemKind::DeviceLibrary))
    .with(ir::Feature::Item(ir::ItemKind::Opaque))
    .with(ir::Feature::Item(ir::ItemKind::Subckt))
    .with(ir::Feature::Item(ir::ItemKind::Section))
    .with(ir::Feature::Item(ir::ItemKind::If))
    .with(ir::Feature::Item(ir::ItemKind::Param))
    .with(ir::Feature::Extra(ir::Extra::IncludeSection))
    .with(ir::Feature::Extra(ir::Extra::CaseSensitiveNames))
    // a `parameters` value (or a subcircuit's constant one, its formal's default) may be a
    // string (`check_expr` takes `Tag::Str`); a list there is `Private` (G13, `private_expr`)
    .with(ir::Feature::Extra(ir::Extra::StringValue));

/// Where VACASK takes each statement (A15 `Placed`: enclosed at any depth by a scope of that kind).
/// A subcircuit: parameters, instances, models, nested subcircuits, `@if`, `include` (VD
/// cir-subckt; run pl/sub). An `@if` branch: instances, models, nested `@if` (VD
/// cir-conditional; runs pl/br, pl/brs reject `include` of parameters and `subckt`). A section:
/// parameters, subcircuits, models, instances, `@if`, `include`, `load`, `global` (run pl/sec).
/// Comments everywhere, and VACASK's own opaques wherever the reader keeps one. Not run, so left
/// out (a refusal, never a wrong deck): `ground`, `embed` and `section` in a section, a section in
/// a subcircuit or branch.
pub(super) const PLACEMENTS: ir::FeatureSet = ir::FeatureSet::placed(
    ir::ScopeKind::Subckt,
    &[
        ir::ItemKind::Param,
        ir::ItemKind::Subckt,
        ir::ItemKind::Model,
        ir::ItemKind::Element,
        ir::ItemKind::If,
        ir::ItemKind::Include,
        ir::ItemKind::Comment,
        ir::ItemKind::Opaque,
    ],
)
.union(ir::FeatureSet::placed(
    ir::ScopeKind::Branch,
    &[
        ir::ItemKind::Element,
        ir::ItemKind::Model,
        ir::ItemKind::If,
        ir::ItemKind::Comment,
        ir::ItemKind::Opaque,
    ],
))
.union(ir::FeatureSet::placed(
    ir::ScopeKind::Section,
    &[
        ir::ItemKind::Param,
        ir::ItemKind::Subckt,
        ir::ItemKind::Model,
        ir::ItemKind::Element,
        ir::ItemKind::If,
        ir::ItemKind::Include,
        ir::ItemKind::Load,
        ir::ItemKind::DeviceLibrary,
        ir::ItemKind::Global,
        ir::ItemKind::Comment,
        ir::ItemKind::Opaque,
    ],
));

/// A parameter defined twice in one scope.
pub(super) const REDEFINED: ir::Code = ir::Code::new(b'R', 2001);
/// A statement an `@if` branch may not hold (`parameters`, `subckt`).
pub(super) const IN_BRANCH: ir::Code = ir::Code::new(b'P', 2005);
/// A name defined both in a scope and in what its includes bring in, or by two included files
/// or sections (the argument: the first definition's `DefId`).
pub(super) const INCLUDED_TWICE: ir::Code = ir::Code::new(b'P', 2030);
/// A name only an enclosing subcircuit defines.
pub(super) const ENCLOSING: ir::Code = ir::Code::new(b'R', 2003);
/// A statement VACASK allows only at the top level (`load`, `embed`, `global`, `ground`).
pub(super) const TOP_ONLY: ir::Code = ir::Code::new(b'P', 2008);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        REDEFINED,
        ir::Info {
            title: "a name defined twice in one scope",
            cite: "DIALECT-VACASK Appendix A runs n3 (\"Parameter redefinition.\"), dp1 (\"Terminal '1' is not unique.\"); INV-semantics 3.01",
            explain: "VACASK refuses a second definition of a parameter or a subcircuit in one scope, and a subcircuit port named twice; the first parameter is kept, a subcircuit so written is kept as text, and the deck is refused.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        INCLUDED_TWICE,
        ir::Info {
            title: "a name defined twice once includes are expanded",
            cite: "settle b_incsec_top_before, b_incsec_top_after (\"Parameter redefinition.\"), b_redef_model_*, b_redef_sub_* (\"A model/subcircuit with name … already exists.\"), b_redef_sections; VACASK exits 1 on each",
            explain: "VACASK's include is textual: a parameter, model or subcircuit defined in a scope and again in a file or section it includes (or in two included sections) is a redefinition in either order, and VACASK refuses the deck. The diagnostic stands at the second definition in the expanded text; its argument is the first.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        ENCLOSING,
        ir::Info {
            title: "a name only an enclosing subcircuit defines",
            cite: "DIALECT-VACASK Appendix A runs n1, m1; VD cir-hier (Scope)",
            explain: "VACASK looks a name up in the own subcircuit, then the top level, never in an enclosing subcircuit (\"Variable or constant … not defined\", \"Master … not found\"). The reference is left unbound and the deck refused.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        TOP_ONLY,
        ir::Info {
            title: "a top-level statement inside a subcircuit or a branch",
            cite: "VD cir-subckt (Allowed contents), cir-loading, cir-nodes",
            explain: "`load`, `embed`, `global` and `ground` are allowed only at the top level; VACASK rejects them inside a subcircuit.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        IN_BRANCH,
        ir::Info {
            title: "a statement an @if branch may not hold",
            cite: "VD cir-conditional (Allowed contents)",
            explain: "An @if branch holds instances, models and nested @if blocks only; VACASK rejects `parameters` and `subckt` there (runs pl/br, pl/brs).",
            kind: ir::Kind::Refusal,
        },
    ),
];

/// The title line of an entry deck (VD; S 14.12): a `Title` item, an empty line too, except in
/// a file that holds nothing else.
pub(super) fn title(l: &mut Lower<'_, '_, '_>, n: Node) {
    // A CRLF file's line 1 ends in `\r`, which is no part of the title (corpus
    // demo/cmos180n.lib read as a deck: the title row's one line, X0015).
    let raw = l.cst.tokens.text(l.cst.ast.main_token(n));
    let text = raw.strip_suffix('\r').unwrap_or(raw).to_string();
    let deck = l.b.design().file(l.cst.file).role == ir::Role::Deck && l.cst.shift == 0;
    // An empty file is no deck statement at all (law L1: an empty text holds no item).
    let only = l.cst.ast.fields(l.cst.ast.root()).len() == 1;
    if deck && !(text.is_empty() && only) {
        let span = l.span(n);
        l.b.title(l.scope, &text, span);
    }
}

/// `include "path" [section=name]`: an `Include` of the file `src` holds at that path
/// (resolved against the including file), else of the path as written.
pub(super) fn include(l: &mut Lower<'_, '_, '_>, n: Node) {
    // VACASK reads an include in a branch as its text there (run pl/br): valid only for what a
    // branch holds, which the reader cannot see; kept as VACASK's own text.
    if in_branch(l) {
        return read::opaque(l, n, ir::OpaqueClass::Private, Some(codes::PRIVATE));
    }
    let fields = l.cst.ast.fields(n).to_vec();
    let Some(&path) = fields.first() else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(codes::PRIVATE));
    };
    let written = read::field_name(l.cst, path);
    let mut section = None;
    for &f in &fields[1..] {
        match assign(l, f) {
            Some((key, value)) if key == "section" => {
                section = Some(read::field_name(l.cst, value))
            }
            _ => return read::opaque(l, n, ir::OpaqueClass::Private, Some(codes::PRIVATE)),
        }
    }
    let design = l.b.design();
    let from = design.str(design.file(l.cst.file).path).to_string();
    let resolved = Reference::File(written.clone()).resolve(&from);
    let path = match design.find_file(&resolved).is_none() {
        true => written,
        false => resolved,
    };
    let span = l.span(n);
    l.b.include(l.scope, &path, section.as_deref(), span);
}

/// `section name` … `endsection`: a `Section` and its body.
pub(super) fn section(l: &mut Lower<'_, '_, '_>, n: Node) {
    if l.b.design().scope(l.scope).kind != ir::ScopeKind::File {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(TOP_ONLY));
    }
    let ast = &l.cst.ast;
    let Some(&name) = ast.fields(n).first() else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(codes::PRIVATE));
    };
    let name = read::field_name(l.cst, name);
    if defined_here(l, ir::DefKind::Section, &name) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REDEFINED));
    }
    let (body, until) = (ast.body(n).to_vec(), closer_start(l, n));
    let span = l.span(n);
    let (_, scope) = l.b.section(l.scope, &name, None, span);
    l.body(scope, &body, until);
}

/// `subckt name (ports)` … `ends`: a `Subckt` whose head holds the constant `parameters`
/// entries of its body (in order), then the body with the other entries as local `Param`s.
pub(super) fn subckt(l: &mut Lower<'_, '_, '_>, n: Node) {
    if in_branch(l) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(IN_BRANCH));
    }
    let ast = &l.cst.ast;
    let fields = ast.fields(n).to_vec();
    let Some((&name, ports)) = fields.split_first() else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(codes::PRIVATE));
    };
    let name = read::field_name(l.cst, name);
    let ports: Vec<String> = ports.iter().map(|&p| read::field_name(l.cst, p)).collect();
    let mut sorted = ports.clone();
    sorted.sort_unstable();
    // A port named twice: "Terminal '1' is not unique." (run dp1).
    let repeated = sorted.windows(2).any(|w| w[0] == w[1]);
    if repeated || defined_here(l, ir::DefKind::Subckt, &name) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REDEFINED));
    }
    let body = ast.body(n).to_vec();
    let mut formals: Vec<(String, ir::ExprId)> = Vec::new();
    for &s in &body {
        let stmt = Node(s);
        if l.cst.ast.tag(stmt) != Tag::Param {
            continue;
        }
        let entries: Vec<(u32, String, u32)> = l
            .cst
            .ast
            .fields(stmt)
            .iter()
            .filter_map(|&f| assign(l, f).map(|(k, v)| (f, k, v)))
            .collect();
        let whole = entries.len() == l.cst.ast.fields(stmt).len();
        let checked = entries
            .iter()
            .all(|&(_, _, v)| expr_read::check_expr(l, v, Context::Param).is_ok());
        if !whole || !checked {
            continue;
        }
        for (f, key, value) in entries {
            if !is_constant(l, value) {
                continue;
            }
            l.taken.insert(f);
            if formals.iter().any(|(k, _)| *k == key) {
                let span = l.span(stmt);
                l.diags.push(REDEFINED, span, 0);
                continue;
            }
            let e = expr_read::expr(l, value, Context::Param).expect("checked");
            formals.push((key, e));
        }
    }
    let ports: Vec<&str> = ports.iter().map(String::as_str).collect();
    let formals: Vec<(&str, ir::ExprId)> = formals.iter().map(|(k, e)| (k.as_str(), *e)).collect();
    let span = l.span(n);
    let until = closer_start(l, n);
    let (_, scope) = l.b.subckt(l.scope, &name, &ports, &formals, span);
    l.body(scope, &body, until);
}

/// `parameters a=e b=e …` outside a subcircuit's head: one `Param` per entry, in order.
pub(super) fn param(l: &mut Lower<'_, '_, '_>, n: Node) {
    if in_branch(l) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(IN_BRANCH));
    }
    let fields = l.cst.ast.fields(n).to_vec();
    let mut entries = Vec::with_capacity(fields.len());
    for &f in &fields {
        let Some((key, value)) = assign(l, f) else {
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(codes::PRIVATE));
        };
        if !l.taken.contains(&f) {
            entries.push((key, value));
        }
    }
    for &(_, value) in &entries {
        if let Err(r) = expr_read::check_expr(l, value, Context::Param) {
            return read::opaque(l, n, r.class, Some(r.code));
        }
    }
    let span = l.span(n);
    for (key, value) in entries {
        if defined_here(l, ir::DefKind::Param, &key) {
            l.diags.push(REDEFINED, span, 0);
            continue;
        }
        let e = expr_read::expr(l, value, Context::Param).expect("checked");
        l.b.param(l.scope, &key, e, span);
    }
}

/// `global a b …` / `global (a b …)`: a `Global` of the names that are not ground; only ground
/// names: `Inert`.
pub(super) fn global(l: &mut Lower<'_, '_, '_>, n: Node) {
    if !top_level(l) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(TOP_ONLY));
    }
    let names: Vec<String> = node_names(l, n)
        .into_iter()
        .filter(|x| x != super::name::GROUND && !l.grounds.contains(x))
        .collect();
    if names.is_empty() {
        return read::opaque(l, n, ir::OpaqueClass::Inert, None);
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let span = l.span(n);
    l.b.global(l.scope, &names, span);
}

/// `ground a b …`: `Inert` (its names became `0` everywhere through `Lower::grounds`).
pub(super) fn ground(l: &mut Lower<'_, '_, '_>, n: Node) {
    match top_level(l) {
        true => read::opaque(l, n, ir::OpaqueClass::Inert, None),
        false => read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(TOP_ONLY)),
    }
}

/// `embed "file" <<<MARK … >>>MARK`.
pub(super) fn embed(l: &mut Lower<'_, '_, '_>, n: Node) {
    if !top_level(l) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(TOP_ONLY));
    }
    let named = l
        .cst
        .ast
        .fields(n)
        .first()
        .is_some_and(|&f| l.named_files.contains(&read::field_name(l.cst, f)));
    match named {
        true => read::opaque(l, n, ir::OpaqueClass::Private, Some(codes::PRIVATE)),
        false => read::opaque(l, n, ir::OpaqueClass::Inert, None),
    }
}

// ---- private ----

/// `key = value` field `f`: the key's name and the value's node.
fn assign(l: &Lower<'_, '_, '_>, f: u32) -> Option<(String, u32)> {
    let ast = &l.cst.ast;
    let n = Node(f);
    (ast.tag(n) == Tag::Assign).then(|| (read::field_name(l.cst, ast.lhs(n)), ast.rhs(n)))
}

/// Whether the value at `node` folds to a constant at parse time: no name in it but a builtin
/// constant (run c1).
fn is_constant(l: &Lower<'_, '_, '_>, node: u32) -> bool {
    let ast = &l.cst.ast;
    let mut todo = vec![node];
    // Bounded: each node of the (acyclic) expression tree is taken once.
    while let Some(x) = todo.pop() {
        let n = Node(x);
        match ast.tag(n) {
            Tag::Ident | Tag::Word => {
                let name = match ast.tag(n) {
                    Tag::Word => ast.name(Name(ast.rhs(n))),
                    _ => ast.name(Name(ast.lhs(n))),
                };
                if !tables::CONSTANTS.iter().any(|(c, _)| *c == name) {
                    return false;
                }
            }
            Tag::Unary | Tag::Paren => todo.push(ast.lhs(n)),
            Tag::Binary => todo.extend([ast.lhs(n), ast.rhs(n)]),
            Tag::Ternary => {
                let at = ast.rhs(n);
                todo.extend([ast.lhs(n), ast.extra(at), ast.extra(at + 1)]);
            }
            Tag::Call | Tag::Probe => {
                let at = ast.rhs(n);
                todo.extend((0..ast.extra(at + 1)).map(|k| ast.extra(at + 2 + k)));
            }
            _ => {}
        }
    }
    true
}

/// Whether `kind` `name` is defined in the current scope itself.
fn defined_here(l: &mut Lower<'_, '_, '_>, kind: ir::DefKind, name: &str) -> bool {
    let id = l.b.name(name);
    let design = l.b.design();
    let def = design.lookup(l.scope, kind, id);
    !def.is_none() && design.def(def).scope == l.scope
}

/// The start of block `n`'s closer, when it has one.
fn closer_start(l: &Lower<'_, '_, '_>, n: Node) -> Option<u32> {
    l.cst.ast.closer(n).map(|c| l.start(c))
}

/// Whether the current scope is a file's top or a section at it (where VACASK allows `load`,
/// `embed`, `global`, `ground`).
fn top_level(l: &Lower<'_, '_, '_>) -> bool {
    let design = l.b.design();
    let s = design.scope(l.scope);
    match s.kind {
        ir::ScopeKind::File => true,
        ir::ScopeKind::Section => design.scope(s.parent).kind == ir::ScopeKind::File,
        _ => false,
    }
}

/// Whether the current scope is in an `@if` branch, at any depth.
fn in_branch(l: &Lower<'_, '_, '_>) -> bool {
    let design = l.b.design();
    let mut at = l.scope;
    // Bounded: each step moves to a parent, and the scope tree is acyclic.
    for _ in 0..1 << 16 {
        if at.is_none() {
            return false;
        }
        let s = design.scope(at);
        if s.kind == ir::ScopeKind::Branch {
            return true;
        }
        at = s.parent;
    }
    false
}

/// The names of statement `n`'s fields, groups opened (`global (a b)`).
fn node_names(l: &Lower<'_, '_, '_>, n: Node) -> Vec<String> {
    let ast = &l.cst.ast;
    let mut out = Vec::new();
    let mut todo: Vec<u32> = ast.fields(n).iter().rev().copied().collect();
    // Bounded: each node is taken once.
    while let Some(f) = todo.pop() {
        let x = Node(f);
        match ast.tag(x) {
            Tag::Word | Tag::Ident | Tag::Str => out.push(read::field_name(l.cst, f)),
            Tag::Paren => todo.push(ast.lhs(x)),
            Tag::Group | Tag::Vector | Tag::List => {
                let at = ast.rhs(x);
                let count = ast.extra(at + 1);
                todo.extend((0..count).rev().map(|k| ast.extra(at + 2 + k)));
            }
            _ => {}
        }
    }
    out
}
