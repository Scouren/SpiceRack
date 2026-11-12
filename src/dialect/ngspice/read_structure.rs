//! The structure statements → their IR items: line 1 → `Title`; `.end` → framing; `.include`,
//! `.lib file section` → `Include`; `.subckt` … `.ends` (nested too) → `Subckt` and its body;
//! `.lib name` … `.endl` → `Section`; `.if`/`.elseif`/`.else`/`.endif` → `If`; `.param` →
//! `Param`s; `.func` → `Func`; `.global` → `Global`. A card ngspice rejects is an `Opaque` of
//! class `InvalidInSource`, one with no IR rule yet an `Unread`, each with its code.
//!
//! Each function is an area function of `read.rs`'s dispatch (`fn(&mut Lower, Stmt)`): it
//! validates the whole statement first (every expression parsed into a local tree), then pushes,
//! so a refusal leaves no row behind.
//!
//! Cites: ngspice manual §2.3 (title, `.end`), §2.4 (`.subckt`, `params:`), §2.6 (`.global`),
//! §2.7 (`.include`), §2.8 (`.lib`), §2.12 (`.func`), §2.15 (`.if`); ngspice-45 `inpcom.c`
//! (`.end` dropped and reading goes on; `gnd`), numparam (`.param` last wins);
//! `rewrite/INV-semantics.md` 3.01, 11.01, 11.05, 11.07, 14.12; `rewrite/INV-constructs.md`
//! A1, A3, A8, A11, A13, A21, B1–B6, B13, B28, C1, C6.
//!
//! Contents: `pub(super) fn title`, `end`, `include`, `subckt`, `section`, `if_chain`, `param`,
//! `func`, `global`, `card` (each `(lw: &mut Lower<'_, '_>, st: Stmt)`);
//! `pub(super) fn def_name`, `func_parts` (the declare step's readers)

use super::codes;
use super::cst::{self, joined};
use super::declare::Class;
use super::expr_lex;
use super::expr_read::{self, Tree};
use super::read::{Def, Lower, Stmt};
use super::read_outputs;
use super::read_run;
use super::tables::Eval;
use crate::ir;
use crate::syntax::ast::{Ast, NONE, Node, Tag};
use crate::syntax::token::Tokens;

/// A `.subckt` head: name, ports as written, and `(name, value text)` defaults.
struct Head {
    name: String,
    ports: Vec<String>,
    params: Vec<(String, String)>,
}

/// Line 1 of a deck: its text as the title, blank too (every ngspice deck has a title line,
/// INV 14.12); a library's line 1 is the reader's own blank line, and an empty text has no
/// line 1: framing.
pub(super) fn title(lw: &mut Lower<'_, '_>, st: Stmt) {
    let text = lw.toks.text(lw.ast.main_token(st.node));
    let text = text.trim_end_matches('\r');
    if lw.role != ir::Role::Deck || lw.toks.source().is_empty() {
        lw.framing();
        return;
    }
    let span = lw.span(st.node);
    lw.b.title(st.scope, text, span);
}

/// `.end`: ngspice drops it and reads on (INV 11.07); the writer frames a deck with its own.
pub(super) fn end(lw: &mut Lower<'_, '_>, _st: Stmt) {
    lw.framing();
}

/// `.include path` (whole file) or `.lib path section` (one section of it).
pub(super) fn include(lw: &mut Lower<'_, '_>, st: Stmt) {
    if in_branch(lw, st) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::BRANCH_DEF);
    }
    let fields = lw.ast.fields(st.node);
    let want = if lw.ast.tag(st.node) == Tag::LibCall {
        2
    } else {
        1
    };
    if fields.len() != want {
        return malformed(lw, st);
    }
    let path = cst::unquote(&lw.text(Node(fields[0])));
    if path.is_empty() {
        return malformed(lw, st);
    }
    let section = (want == 2).then(|| lw.names.fold(&lw.text(Node(fields[1]))));
    let path = lw.include_path(st, &path);
    let span = lw.span(st.node);
    lw.b.include(st.scope, &path, section.as_deref(), span);
}

/// `.subckt name ports [params:] k=v …` and its body, scheduled after it.
pub(super) fn subckt(lw: &mut Lower<'_, '_>, st: Stmt) {
    if in_branch(lw, st) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::BRANCH_DEF);
    }
    let Some(head) = head(lw, st.node) else {
        return malformed(lw, st);
    };
    if !lw.wins(st, Def::Subckt, &head.name, 0) || !distinct(&head.params) {
        return lw.opaque(st, ir::OpaqueClass::Unread, codes::DUPLICATE_DEF);
    }
    let mut trees = Vec::new();
    for (_, value) in &head.params {
        match parameter(lw, st.node, value) {
            Ok(t) => trees.push(t),
            Err(f) => return lw.opaque(st, f.class, f.code),
        }
    }
    let ports: Vec<String> = head.ports.iter().map(|p| lw.names.node(p)).collect();
    if !distinct_names(&ports) {
        return malformed(lw, st);
    }
    if defaults_cycle(&head.params) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::DEFAULT_CYCLE);
    }
    let name = lw.names.fold(&head.name);
    let mut params = Vec::new();
    let mut marks = vec![lw.pending()];
    for ((key, _), tree) in head.params.iter().zip(&trees) {
        let e = lw.expr(st, tree, st.scope);
        params.push((lw.names.fold(key), e));
        marks.push(lw.pending());
    }
    let mark = marks[0];
    let ports: Vec<&str> = ports.iter().map(String::as_str).collect();
    let params: Vec<(&str, ir::ExprId)> = params.iter().map(|(k, e)| (k.as_str(), *e)).collect();
    let span = lw.span(st.node);
    let (_, body) = lw.b.subckt(st.scope, &name, &ports, &params, span);
    // a head default reads from the subcircuit's body (IR-KERNEL A10), its own name from the
    // enclosing scope (runs outer.cir, selfplus.cir)
    lw.rescope(mark, body);
    for (k, (key, _)) in params.iter().enumerate() {
        lw.rescope_name(marks[k], marks[k + 1], key, st.scope);
    }
    open_body(lw, st, body);
}

/// `.lib name` … `.endl`: a library section and its body.
pub(super) fn section(lw: &mut Lower<'_, '_>, st: Stmt) {
    if in_branch(lw, st) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::BRANCH_DEF);
    }
    let Some(name) = def_name(lw.ast, lw.toks, st.node) else {
        return malformed(lw, st);
    };
    if !lw.wins(st, Def::Section, &name, 0) {
        return lw.opaque(st, ir::OpaqueClass::Unread, codes::DUPLICATE_DEF);
    }
    let name = lw.names.fold(&name);
    let span = lw.span(st.node);
    let (_, body) = lw.b.section(st.scope, &name, None, span);
    open_body(lw, st, body);
}

/// `.if (c)` … `.elseif (c)` … `.else` … `.endif`: one `If`, a branch per clause, each body
/// scheduled in order. A condition is a numparam expression (a single `=` is equality, B28).
pub(super) fn if_chain(lw: &mut Lower<'_, '_>, st: Stmt) {
    if lw.ast.rhs(st.node) == NONE {
        // an `.elseif` with no `.if` open (the parser's P0004): no block
        return malformed(lw, st);
    }
    let mut branches = vec![st.node];
    // Bounded: a chain's branches are distinct nodes.
    for _ in 0..lw.ast.len() {
        match cst::next(lw.ast, *branches.last().expect("one branch")) {
            Some(n) => branches.push(n),
            None => break,
        }
    }
    let mut trees = Vec::new();
    for &br in &branches {
        if lw.ast.tag(br) == Tag::Else {
            trees.push(None);
            continue;
        }
        let Some(&cond) = lw.ast.fields(br).first().filter(|c| **c != NONE) else {
            return malformed(lw, st);
        };
        let text = lw.text(Node(cond));
        if lw.class(st.container, &text, &[]) >= Class::Temp {
            // numparam decides an `.if` once, before the run: no temperature, no circuit
            return lw.opaque(
                st,
                ir::OpaqueClass::InvalidInSource,
                codes::PARAM_READS_CIRCUIT,
            );
        }
        match parameter(lw, st.container, &text) {
            Ok(t) => trees.push(Some(t)),
            Err(f) => return lw.opaque(st, f.class, f.code),
        }
    }
    let conds: Vec<ir::ExprId> = trees
        .iter()
        .map(|t| {
            t.as_ref()
                .map_or(ir::ExprId::NONE, |t| lw.expr(st, t, st.scope))
        })
        .collect();
    let span = lw.span(st.node);
    let (_, list) = lw.b.if_(st.scope, &conds, span);
    let scopes: Vec<ir::ScopeId> =
        lw.b.design()
            .branches(list)
            .iter()
            .map(|b| b.body)
            .collect();
    let closer = cst::closer(lw.ast, st.node);
    if closer.is_none() {
        lw.note(st, codes::UNCLOSED);
    }
    for k in (0..branches.len()).rev() {
        let close = branches.get(k + 1).copied().or(closer);
        let body = cst::body(lw.ast, branches[k]);
        lw.open(scopes[k], body, branches[k], close);
    }
}

/// `.param a=v b=w …`: one `Param` per assignment that wins its scope (the last definition,
/// INV 3.01); a losing one is noted and left out (it changes no number); a card whose every
/// assignment loses is one `Inert` opaque.
pub(super) fn param(lw: &mut Lower<'_, '_>, st: Stmt) {
    if in_branch(lw, st) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::BRANCH_DEF);
    }
    let Some(pairs) = assignments(lw, st.node) else {
        return malformed(lw, st);
    };
    if pairs.is_empty() || !distinct(&pairs) {
        return malformed(lw, st);
    }
    let mut trees = Vec::new();
    for (k, (name, value)) in pairs.iter().enumerate() {
        if !lw.wins(st, Def::Param, name, k) && lw.wins(st, Def::Param, name, usize::MAX) {
            return lw.opaque(st, ir::OpaqueClass::Unread, codes::DUPLICATE_DEF);
        }
        match parameter(lw, st.container, value) {
            Ok(t) => trees.push(t),
            Err(f) => return lw.opaque(st, f.class, f.code),
        }
    }
    let winners: Vec<usize> = (0..pairs.len())
        .filter(|&k| lw.wins(st, Def::Param, &pairs[k].0, k))
        .collect();
    if winners.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::Inert, codes::DUPLICATE_PARAM);
    }
    if winners.len() < pairs.len() {
        lw.note(st, codes::DUPLICATE_PARAM);
    }
    let span = lw.span(st.node);
    for k in winners {
        let value = lw.expr(st, &trees[k], st.scope);
        let name = lw.names.fold(&pairs[k].0);
        lw.b.param(st.scope, &name, value, span);
    }
}

/// `.func name(args) [=] {body}`: a user function; its formals bind in its body.
pub(super) fn func(lw: &mut Lower<'_, '_>, st: Stmt) {
    if in_branch(lw, st) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::BRANCH_DEF);
    }
    let rest = lw.rest(st.node);
    let Some((name, args, body)) = func_parts(&rest) else {
        return malformed(lw, st);
    };
    if !lw.wins(st, Def::Func, &name, 0) {
        return lw.opaque(st, ir::OpaqueClass::Unread, codes::DUPLICATE_DEF);
    }
    let class = lw.class(st.container, body, &args);
    let tree = match expr_read::parse_in(body, eval(class)) {
        Ok(t) => t,
        Err(f) => return lw.opaque(st, f.class, f.code),
    };
    let name = lw.names.fold(&name);
    let args: Vec<String> = args.iter().map(|a| lw.names.fold(a)).collect();
    if !distinct_names(&args) {
        return malformed(lw, st);
    }
    let body = lw.expr(st, &tree, st.scope);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let span = lw.span(st.node);
    lw.b.func(st.scope, &name, &args, body, span);
}

/// `.global n …`: the nets, folded, ground dropped (`gnd` is `0`), each once.
pub(super) fn global(lw: &mut Lower<'_, '_>, st: Stmt) {
    let fields = lw.ast.fields(st.node);
    if fields.is_empty() || fields.iter().any(|&f| lw.ast.tag(Node(f)) != Tag::Word) {
        return malformed(lw, st);
    }
    let mut nets: Vec<String> = Vec::new();
    for &f in fields {
        let net = lw.names.node(&lw.text(Node(f)));
        if net != "0" && !nets.contains(&net) {
            nets.push(net);
        }
    }
    if nets.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::Inert, codes::GLOBAL_GROUND);
    }
    let nets: Vec<&str> = nets.iter().map(String::as_str).collect();
    let span = lw.span(st.node);
    lw.b.global(st.scope, &nets, span);
}

/// Any other dot card: a stray closer (`.ends`, `.endl`, `.endif`, `.endc` with no block) is
/// `InvalidInSource`; `.title` is `Unread` here; `.width` and `.probe` belong to the output
/// area, the rest to the run area (S3.3).
pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt) {
    match lw.keyword(st.node).as_str() {
        ".ends" | ".eom" | ".endl" | ".endif" | ".endc" | ".enddata" | ".end_variation" => {
            malformed(lw, st)
        }
        ".title" => lw.unread(st),
        // `.width` sets the print width of `.print` output: the output area's
        ".width" => read_outputs::card(lw, st),
        // ngspice's lexicon has no `.probe` keyword (it parses as a card): the output area's
        ".probe" => read_outputs::card(lw, st),
        _ => read_run::card(lw, st),
    }
}

/// The defined name of a `.subckt`, `.lib` section or `.func` node, as written.
pub(super) fn def_name(ast: &Ast<'_>, toks: &Tokens<'_>, n: Node) -> Option<String> {
    let first = *ast.fields(n).first()?;
    let text = joined(toks, ast, Node(first));
    let name = match ast.tag(n) {
        Tag::Func => text.split('(').next().unwrap_or("").trim().to_string(),
        _ if ast.tag(Node(first)) == Tag::Word => text,
        _ => return None,
    };
    (!name.is_empty()).then_some(name)
}

/// `name(a, b) [=] body`: the name, the formals and the body text without its wrapper.
pub(super) fn func_parts(text: &str) -> Option<(String, Vec<String>, &str)> {
    let (name, rest) = text.split_once('(')?;
    let name = name.trim();
    let (args, body) = rest.split_once(')')?;
    let args: Vec<String> = if args.trim().is_empty() {
        Vec::new()
    } else {
        args.split(',').map(|a| a.trim().to_string()).collect()
    };
    let ident = |s: &str| {
        s.as_bytes()
            .first()
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
            && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    };
    if !ident(name) || !args.iter().all(|a| ident(a)) {
        return None;
    }
    let body = body.trim();
    let body = body.strip_prefix('=').unwrap_or(body).trim();
    let body = unwrap(body);
    (!body.trim().is_empty()).then(|| (name.to_string(), args, body))
}

// ---- private ----

/// The pieces of a `.subckt` head, or `None` when it is no `name ports [params:] k=v…`.
fn head(lw: &Lower<'_, '_>, n: Node) -> Option<Head> {
    let fields = lw.ast.fields(n);
    let (&first, rest) = fields.split_first()?;
    if lw.ast.tag(Node(first)) != Tag::Word {
        return None;
    }
    let mut h = Head {
        name: lw.text(Node(first)),
        ports: Vec::new(),
        params: Vec::new(),
    };
    let mut in_params = false;
    for &f in rest {
        let f = Node(f);
        match lw.ast.tag(f) {
            Tag::Word if lw.text(f).eq_ignore_ascii_case("params:") && !in_params => {
                in_params = true;
            }
            Tag::Word if !in_params => h.ports.push(lw.text(f)),
            Tag::Assign if lw.ast.rhs(f) != NONE => {
                in_params = true;
                let key = lw.text(Node(lw.ast.lhs(f)));
                let value = lw.text(Node(lw.ast.rhs(f)));
                h.params.push((key, value));
            }
            _ => return None,
        }
    }
    Some(h)
}

/// The `(name, value text)` of each field of a `.param` node, or `None` when a field is no
/// assignment.
fn assignments(lw: &Lower<'_, '_>, n: Node) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    for &f in lw.ast.fields(n) {
        let f = Node(f);
        if lw.ast.tag(f) != Tag::Assign || lw.ast.rhs(f) == NONE {
            return None;
        }
        let key = lw.text(Node(lw.ast.lhs(f)));
        if key.is_empty() || !key.as_bytes()[0].is_ascii_alphabetic() && !key.starts_with('_') {
            return None;
        }
        out.push((key, lw.text(Node(lw.ast.rhs(f)))));
    }
    Some(out)
}

/// A `.param`, head default or `.if` value read in `block`: its `{…}` or `'…'` wrapper dropped,
/// read by the evaluator its class picks (R14: one that reads `temper`, directly or through a
/// definition, is the B-source parser's); one that reads the circuit is ngspice's error.
fn parameter(lw: &Lower<'_, '_>, block: Node, value: &str) -> Result<Tree, expr_read::Fail> {
    let text = unwrap(value);
    let class = lw.class(block, text, &[]);
    if class == Class::Circuit {
        return Err(expr_read::Fail {
            code: codes::PARAM_READS_CIRCUIT,
            class: ir::OpaqueClass::InvalidInSource,
        });
    }
    expr_read::parse_in(text, eval(class))
}

/// The evaluator of a class: the B-source parser from `Temp` up (R14).
fn eval(class: Class) -> Eval {
    if class >= Class::Temp {
        Eval::Behavioural
    } else {
        Eval::Numparam
    }
}

/// `text` without one `{…}` or `'…'` that wraps all of it.
fn unwrap(text: &str) -> &str {
    let t = text.trim();
    if let Some(inner) = t.strip_prefix('\'').and_then(|r| r.strip_suffix('\''))
        && !inner.contains('\'')
    {
        return inner;
    }
    if let Some(inner) = t.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        let mut depth = 0i32;
        let wraps = inner.bytes().all(|c| {
            depth += i32::from(c == b'{') - i32::from(c == b'}');
            depth >= 0
        });
        if wraps && depth == 0 {
            return inner;
        }
    }
    t
}

/// Whether the statement stands in an `.if` branch.
fn in_branch(lw: &Lower<'_, '_>, st: Stmt) -> bool {
    matches!(lw.ast.tag(st.container), Tag::If | Tag::Else)
}

/// Schedules a block's body into `body`, with a note when the file ends before its closer.
fn open_body(lw: &mut Lower<'_, '_>, st: Stmt, body: ir::ScopeId) {
    let closer = cst::closer(lw.ast, st.node);
    if closer.is_none() {
        lw.note(st, codes::UNCLOSED);
    }
    let items = cst::body(lw.ast, st.node);
    lw.open(body, items, st.node, closer);
}

/// `st` as `InvalidInSource` with `P1007`.
fn malformed(lw: &mut Lower<'_, '_>, st: Stmt) {
    lw.opaque(st, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
}

/// Whether the pairs' names are distinct under ngspice's case folding.
fn distinct(pairs: &[(String, String)]) -> bool {
    let names: Vec<String> = pairs.iter().map(|(n, _)| n.to_ascii_lowercase()).collect();
    distinct_names(&names)
}

/// Whether the head defaults read each other in a cycle (a default's read of its own name is
/// the enclosing scope's, no edge): ngspice stops ("a level depth greater 1000 for dependent
/// parameters", run mutual.cir).
fn defaults_cycle(params: &[(String, String)]) -> bool {
    let n = params.len();
    let reads: Vec<Vec<usize>> = params
        .iter()
        .enumerate()
        .map(|(k, (_, value))| {
            let leaves = expr_lex::leaves(value);
            (0..n)
                .filter(|&j| j != k)
                .filter(|&j| {
                    leaves
                        .names
                        .iter()
                        .any(|(m, call)| !call && m.eq_ignore_ascii_case(&params[j].0))
                })
                .collect()
        })
        .collect();
    // Kahn: what no default reads first; a cycle is what is left. Bounded: each default once.
    let mut indegree = vec![0usize; n];
    for r in &reads {
        for &j in r {
            indegree[j] += 1;
        }
    }
    let mut ready: Vec<usize> = (0..n).filter(|&k| indegree[k] == 0).collect();
    let mut done = 0;
    while let Some(k) = ready.pop() {
        done += 1;
        for &j in &reads[k] {
            indegree[j] -= 1;
            if indegree[j] == 0 {
                ready.push(j);
            }
        }
    }
    debug_assert!(done <= n, "each default is taken once");
    done < n
}

/// Whether the names are distinct under ngspice's case folding.
fn distinct_names(names: &[String]) -> bool {
    names
        .iter()
        .enumerate()
        .all(|(k, n)| !names[..k].iter().any(|m| m.eq_ignore_ascii_case(n)))
}
