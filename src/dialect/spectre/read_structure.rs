//! The structure statements → IR rows: the title, `simulator lang=`, a `simulator lang=spice`
//! region, `parameters`, `subckt` … `ends` (its body's `parameters` its formals), `if` …
//! `else`, `real f(real x) { return e; }`, `include`, `global`, `statistics`.
//!
//! Rules (`rewrite/DIALECT-SPECTRE.md` §2, §4, each with its evidence):
//! - `simulator lang=spectre` changes nothing in a Spectre file: `Inert` (MAN: UG p.53);
//! - a `simulator lang=spice` region is SPICE text Spectre reads (its `.measure` runs: RUN-SP):
//!   kept whole as this dialect's text, `Private`;
//! - a `parameters` statement in a subcircuit's body declares that subcircuit's parameters,
//!   which an instance may set (MAN: SR5 p.871; RUN-SP: the `t_s3_*` decks' `rl`);
//! - `if (c) { … } else { … }` holds instances, models and `parameters` (RUN-SP:
//!   `p_conditional`, `p_branch_param`); its condition reads no circuit;
//! - a user function is `real name(real a, …) { return e; }` at the top level, its body
//!   reading no circuit (MAN: SR p.870; RUN-SP `p_func`);
//! - `statistics { process { vary … } mismatch { vary … } }` with `dist=gauss std=`,
//!   `dist=unif N=`, `dist=lnorm std=`, `percent=yes` (MAN: SR19 pp.175-179; RUN-SP: the draw
//!   rules of the `t_s*` decks); `correlate` and `truncate` are not read yet (`Unread`).
//!
//! Contents: `FEATURES`, `title`, `simulator`, `region`, `parameters`, `subckt`, `if_chain`,
//! `func`, `include`, `global`, `statistics`, `defined_here` (crate-private)

use super::codes;
use super::expr_read::{self, Context};
use super::read::{self, Lower};
use crate::dialect::Reference;
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::placed(
    ir::ScopeKind::Subckt,
    &[
        ir::ItemKind::Subckt,
        ir::ItemKind::Model,
        ir::ItemKind::Element,
        ir::ItemKind::If,
        ir::ItemKind::Comment,
        ir::ItemKind::Opaque,
    ],
)
.union(ir::FeatureSet::placed(
    ir::ScopeKind::Branch,
    &[
        ir::ItemKind::Element,
        ir::ItemKind::Model,
        ir::ItemKind::Param,
        ir::ItemKind::If,
        ir::ItemKind::Comment,
        ir::ItemKind::Opaque,
    ],
))
.union(ir::FeatureSet::placed(
    ir::ScopeKind::Loop,
    &[
        ir::ItemKind::Analysis,
        ir::ItemKind::Comment,
        ir::ItemKind::Opaque,
    ],
))
.with(ir::Feature::Item(ir::ItemKind::Title))
.with(ir::Feature::Item(ir::ItemKind::Comment))
.with(ir::Feature::Item(ir::ItemKind::Opaque))
.with(ir::Feature::Item(ir::ItemKind::Include))
.with(ir::Feature::Item(ir::ItemKind::Global))
.with(ir::Feature::Item(ir::ItemKind::Subckt))
.with(ir::Feature::Item(ir::ItemKind::If))
.with(ir::Feature::Item(ir::ItemKind::Param))
.with(ir::Feature::Item(ir::ItemKind::Func))
.with(ir::Feature::Item(ir::ItemKind::Statistics))
.with(ir::Feature::Run(ir::RunExtra::StatsMismatch))
.with(ir::Feature::Run(ir::RunExtra::VaryPercent))
.with(ir::Feature::Run(ir::RunExtra::VaryLogNormal))
.with(ir::Feature::Extra(ir::Extra::CaseSensitiveNames))
.with(ir::Feature::Extra(ir::Extra::FormalNotConstant))
.with(ir::Feature::Extra(ir::Extra::FormalOuterRead));

/// Line 1 of a deck: its `Title` (an included file has none; an empty deck holds no item).
pub(super) fn title(l: &mut Lower<'_, '_, '_>, n: Node) {
    let raw = l.token(n);
    let text = raw.strip_suffix('\r').unwrap_or(raw).to_string();
    let deck = l.b.design().file(l.cst.file).role == ir::Role::Deck && l.cst.shift == 0;
    let only = l.cst.ast.fields(l.cst.ast.root()).len() == 1;
    if deck && !(text.is_empty() && only) {
        let span = l.span(n);
        l.b.title(l.scope, &text, span);
    }
}

/// `simulator lang=spectre`: `Inert`; any other `simulator` statement outside a region: no rule.
pub(super) fn simulator(l: &mut Lower<'_, '_, '_>, n: Node) {
    let text = l.text(n).split_whitespace().collect::<Vec<_>>().join(" ");
    match text.as_str() {
        "simulator lang=spectre" => {
            read::opaque(l, n, ir::OpaqueClass::Inert, codes::SIMULATOR_SPECTRE)
        }
        _ => read::unread(l, n),
    }
}

/// A `simulator lang=spice` region, its closing `simulator lang=spectre` included: kept whole.
pub(super) fn region(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::opaque(l, n, ir::OpaqueClass::Private, codes::SPICE_REGION);
}

/// `parameters a=e b=e …`: one `Param` per entry, in order; in a subcircuit's body the
/// statement was read as its formals (`subckt`), unless a value could not be read.
pub(super) fn parameters(l: &mut Lower<'_, '_, '_>, n: Node) {
    let Some(entries) = assigns(l, n) else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
    };
    for &(_, value) in &entries {
        if let Err(r) = expr_read::check_expr(l, value, Context::Param) {
            return read::opaque(l, n, r.class, r.code);
        }
    }
    if l.b.design().scope(l.scope).kind == ir::ScopeKind::Subckt {
        return;
    }
    let span = l.span(n);
    for (key, value) in entries {
        if defined_here(l, ir::DefKind::Param, &key) {
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::REDEFINED);
        }
        let e = expr_read::expr(l, value, Context::Param).expect("checked");
        l.b.param(l.scope, &key, e, span);
    }
}

/// `subckt name (ports)` … `ends`: a `Subckt` whose formals are its body's readable
/// `parameters` entries, in order, then the body.
pub(super) fn subckt(l: &mut Lower<'_, '_, '_>, n: Node) {
    let kind = l.b.design().scope(l.scope).kind;
    if !matches!(kind, ir::ScopeKind::File | ir::ScopeKind::Subckt) {
        return read::opaque(l, n, ir::OpaqueClass::Unread, codes::PLACEMENT);
    }
    let fields = l.cst.ast.fields(n).to_vec();
    let Some((&name, ports)) = fields.split_first() else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
    };
    let name = l.field(name);
    let ports: Vec<String> = ports.iter().map(|&p| l.field(p)).collect();
    let mut sorted = ports.clone();
    sorted.sort_unstable();
    if sorted.windows(2).any(|w| w[0] == w[1]) || defined_here(l, ir::DefKind::Subckt, &name) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::REDEFINED);
    }
    let body = l.cst.ast.body(n).to_vec();
    let mut formals: Vec<(String, u32)> = Vec::new();
    for &s in &body {
        let stmt = Node(s);
        if l.cst.ast.tag(stmt) != Tag::Param {
            continue;
        }
        let Some(entries) = assigns(l, stmt) else {
            continue;
        };
        if entries
            .iter()
            .all(|&(_, v)| expr_read::check_expr(l, v, Context::Param).is_ok())
        {
            formals.extend(entries);
        }
    }
    let mut keys: Vec<&str> = formals.iter().map(|(k, _)| k.as_str()).collect();
    keys.sort_unstable();
    if keys.windows(2).any(|w| w[0] == w[1]) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::REDEFINED);
    }
    let first = l.reads.len();
    let values: Vec<(String, ir::ExprId)> = formals
        .into_iter()
        .map(|(k, v)| (k, expr_read::expr(l, v, Context::Param).expect("checked")))
        .collect();
    let ports: Vec<&str> = ports.iter().map(String::as_str).collect();
    let values: Vec<(&str, ir::ExprId)> = values.iter().map(|(k, e)| (k.as_str(), *e)).collect();
    let span = l.span(n);
    let until = closer_start(l, n);
    let (_, scope) = l.b.subckt(l.scope, &name, &ports, &values, span);
    // a default is read from the body: a sibling formal first, then outward
    for read in &mut l.reads[first..] {
        read.1 = scope;
    }
    l.body(scope, &body, until);
}

/// `if (c) { … } else if (c) { … } else { … }`: one `If` with a branch per condition.
pub(super) fn if_chain(l: &mut Lower<'_, '_, '_>, n: Node) {
    let ast = &l.cst.ast;
    let mut chain = vec![n];
    // Bounded by the node count: each step moves to a later branch.
    for _ in 0..ast.len() {
        match ast.next(*chain.last().expect("non-empty")) {
            Some(next) => chain.push(next),
            None => break,
        }
    }
    for &branch in &chain {
        if ast.tag(branch) != Tag::If {
            continue;
        }
        let Some(&cond) = ast.fields(branch).first() else {
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
        };
        if let Err(r) = expr_read::check_expr(l, cond, Context::Param) {
            return read::opaque(l, n, r.class, r.code);
        }
    }
    let mut conds = Vec::with_capacity(chain.len());
    for &branch in &chain {
        conds.push(match l.cst.ast.tag(branch) {
            Tag::If => {
                let cond = l.cst.ast.fields(branch)[0];
                expr_read::expr(l, cond, Context::Param).expect("checked")
            }
            _ => ir::ExprId::NONE,
        });
    }
    let span = l.span(n);
    let (_, branches) = l.b.if_(l.scope, &conds, span);
    let scopes: Vec<ir::ScopeId> =
        l.b.design()
            .branches(branches)
            .iter()
            .map(|b| b.body)
            .collect();
    for (k, &branch) in chain.iter().enumerate() {
        let body = l.cst.ast.body(branch).to_vec();
        let until = match chain.get(k + 1) {
            Some(&next) => Some(l.span(next).start),
            None => closer_start(l, branch),
        };
        l.body(scopes[k], &body, until);
    }
}

/// `real name(real a, …) { return e; }` at the top level: a `Func`.
pub(super) fn func(l: &mut Lower<'_, '_, '_>, n: Node) {
    if l.b.design().scope(l.scope).kind != ir::ScopeKind::File {
        return read::opaque(l, n, ir::OpaqueClass::Unread, codes::PLACEMENT);
    }
    let ast = &l.cst.ast;
    let fields = ast.fields(n).to_vec();
    let body = ast.body(n).to_vec();
    let Some((&name, rest)) = fields.split_first() else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
    };
    let typed = rest
        .chunks(2)
        .all(|p| p.len() == 2 && l.field(p[0]) == "real");
    let ret = match body.as_slice() {
        [r] if ast.tag(Node(*r)) == Tag::Card && l.token(Node(*r)) == "return" => {
            ast.fields(Node(*r)).first().copied()
        }
        _ => None,
    };
    let (true, Some(value)) = (typed, ret) else {
        return read::opaque(l, n, ir::OpaqueClass::Unread, codes::FUNC_FORM);
    };
    let name = l.field(name);
    if defined_here(l, ir::DefKind::Func, &name) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::REDEFINED);
    }
    let args: Vec<String> = rest.chunks(2).map(|p| l.field(p[1])).collect();
    let e = match expr_read::expr(l, value, Context::Param) {
        Ok(e) => e,
        Err(r) => return read::opaque(l, n, r.class, r.code),
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let span = l.span(n);
    l.b.func(l.scope, &name, &args, e, span);
}

/// `include "path"`: an `Include` of the file `src` holds at that path (resolved against the
/// including file), else of the path as written; `section=` is not read yet.
pub(super) fn include(l: &mut Lower<'_, '_, '_>, n: Node) {
    let fields = l.cst.ast.fields(n).to_vec();
    let [path] = fields.as_slice() else {
        return read::unread(l, n);
    };
    let written = l.field(*path);
    let design = l.b.design();
    let from = design.str(design.file(l.cst.file).path).to_string();
    let resolved = Reference::File(written.clone()).resolve(&from);
    let path = match design.find_file(&resolved).is_none() {
        true => written,
        false => resolved,
    };
    let span = l.span(n);
    l.b.include(l.scope, &path, None, span);
}

/// `global a b …`: a `Global` of the names that are not ground (`0`); only ground: `Inert`.
pub(super) fn global(l: &mut Lower<'_, '_, '_>, n: Node) {
    let names: Vec<String> = l
        .cst
        .ast
        .fields(n)
        .iter()
        .map(|&f| l.field(f))
        .filter(|s| s != "0")
        .collect();
    if names.is_empty() {
        return read::opaque(l, n, ir::OpaqueClass::Inert, codes::GLOBAL_GROUND);
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let span = l.span(n);
    l.b.global(l.scope, &names, span);
}

/// `statistics { process { vary … } mismatch { vary … } }` at a file's top: one `Statistics`.
pub(super) fn statistics(l: &mut Lower<'_, '_, '_>, n: Node) {
    if l.b.design().scope(l.scope).kind != ir::ScopeKind::File {
        return read::opaque(l, n, ir::OpaqueClass::Unread, codes::PLACEMENT);
    }
    let mut lists: [Vec<(String, ir::VaryDist, bool, u32)>; 2] = [Vec::new(), Vec::new()];
    for &s in l.cst.ast.body(n) {
        let block = Node(s);
        let k = match (l.cst.ast.tag(block), l.token(block)) {
            (Tag::Process, "process") => 0,
            (Tag::Process, "mismatch") => 1,
            _ => return read::unread(l, n),
        };
        for &v in l.cst.ast.body(block) {
            match vary(l, Node(v)) {
                Some(row) => lists[k].push(row),
                None => return read::unread(l, n),
            }
        }
    }
    let span = l.span(n);
    let mut varies = [ir::List::EMPTY; 2];
    let mut names = Vec::new();
    for (k, list) in lists.iter().enumerate() {
        let mut rows = Vec::with_capacity(list.len());
        for (param, dist, percent, spread) in list {
            let spread = expr_read::expr(l, *spread, Context::Param).expect("checked");
            let name = l.b.name(param);
            names.push(param.clone());
            rows.push(ir::Vary {
                param: ir::Ref::unresolved(name),
                dist: *dist,
                percent: *percent,
                spread,
            });
        }
        if !rows.is_empty() {
            varies[k] = l.b.varies(&rows);
        }
    }
    let row = ir::Statistics {
        process: varies[0],
        mismatch: varies[1],
        process_correlations: ir::List::EMPTY,
        mismatch_correlations: ir::List::EMPTY,
        truncate: ir::ExprId::NONE,
        process_truncate: ir::ExprId::NONE,
        mismatch_truncate: ir::ExprId::NONE,
    };
    let item = l.b.statistics(l.scope, row, span);
    for (k, name) in names.iter().enumerate() {
        let k = u32::try_from(k).expect("a short block");
        l.refer(item, k, ir::DefKind::Param, name);
    }
}

/// Whether a definition of `kind` named `name` already stands in the current scope.
pub(super) fn defined_here(l: &Lower<'_, '_, '_>, kind: ir::DefKind, name: &str) -> bool {
    let design = l.b.design();
    let id = design.find_name(name);
    if id.is_none() {
        return false;
    }
    let def = design.lookup(l.scope, kind, id);
    !def.is_none() && design.def(def).scope == l.scope
}

// ---- private ----

/// The `key=value` entries of statement `n`'s fields, or `None` when one is no assignment.
fn assigns(l: &Lower<'_, '_, '_>, n: Node) -> Option<Vec<(String, u32)>> {
    let ast = &l.cst.ast;
    ast.fields(n)
        .iter()
        .map(|&f| {
            let f = Node(f);
            (ast.tag(f) == Tag::Assign && ast.tag(Node(ast.lhs(f))) == Tag::Word)
                .then(|| (l.field(ast.lhs(f)), ast.rhs(f)))
        })
        .collect()
}

/// `vary name dist=… std=|N=… [percent=yes]`: the row's parts, the spread's node last; `None`
/// for any other form (a `correlate`, a `truncate`, another key).
fn vary(l: &Lower<'_, '_, '_>, n: Node) -> Option<(String, ir::VaryDist, bool, u32)> {
    let ast = &l.cst.ast;
    if ast.tag(n) != Tag::Card || l.token(n) != "vary" {
        return None;
    }
    let fields = ast.fields(n).to_vec();
    let (&name, rest) = fields.split_first()?;
    if ast.tag(Node(name)) != Tag::Word {
        return None;
    }
    let (mut dist, mut spread, mut percent) = (None, None, false);
    for &f in rest {
        let f = Node(f);
        if ast.tag(f) != Tag::Assign {
            return None;
        }
        let (key, value) = (l.field(ast.lhs(f)), ast.rhs(f));
        match (key.as_str(), l.field(value).as_str()) {
            ("dist", "gauss") => dist = Some((ir::VaryDist::Gauss, "std")),
            ("dist", "unif") => dist = Some((ir::VaryDist::Unif, "N")),
            ("dist", "lnorm") => dist = Some((ir::VaryDist::LogNormal, "std")),
            ("percent", "yes") => percent = true,
            ("percent", "no") => percent = false,
            ("std" | "N", _) => spread = Some((key, value)),
            _ => return None,
        }
    }
    let (dist, key) = dist?;
    let (given, spread) = spread?;
    let ok = given == key && expr_read::check_expr(l, spread, Context::Param).is_ok();
    ok.then(|| (l.field(name), dist, percent, spread))
}

/// The start byte of block `n`'s closer, or `None` when the file ended first.
fn closer_start(l: &Lower<'_, '_, '_>, n: Node) -> Option<u32> {
    l.cst.ast.closer(n).map(|c| {
        let t = l.cst.tokens.start(l.cst.ast.main_token(c));
        t.saturating_sub(l.cst.shift)
    })
}
