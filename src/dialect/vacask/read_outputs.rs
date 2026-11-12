//! `save` directives → one `Save` row per `save` line, and an analysis's `ic=`/`nodeset=` lists
//! → `NodeSet` rows for that analysis only.
//!
//! Rules (each with its evidence):
//! - `save` is read from its tokens (the CST drops a probe's parentheses): `default` is every
//!   node voltage and every branch current (run e5: an inductor's flow is in the default set),
//!   `v(node)`, `i(instance)`, `p(instance, quantity)` (the quantity as spelled, G-OPQUANT);
//!   hierarchical names are quoted `'x1:n'` and split at `:` (VD cmd-save);
//! - `full` (every unknown, collapsed device-internal nodes included) and the analysis-own
//!   directives (`dv`, `di`, `tf`, `zin`, `yin`, `n`, `nc`, `opdefault`, `opfull`) have no
//!   probe kind: the line is Private (P2308, R43);
//! - a list `{"a", 1, "b", "c", 0.5}` holds a node and its value, or two nodes and their
//!   difference (VD cmd-analysis-op); its rows apply to the next analysis only (`next_only`,
//!   run e1); an `ic` is held in the transient's operating point, a nodeset serves every solve.
//!
//! Known gap (A22): `save v(b)` governs the operating point, its sweeps and the transient, not
//! the ac (run e5); until `Save` states that set, the row is for every analysis (the `save`
//! reader fixture is marked).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.4 (save, ic, nodeset); VD cmd-save, cmd-analysis-op.
//!
//! Contents: `FEATURES`, `CODES`, `SAVE_FORM`, `NodeEntry`, `save`, `node_list`, `node_sets`
//! (crate-private)

use super::read::{self, Lower};
use super::read_run::REJECTED;
use crate::ir;
use crate::syntax::ast::{Node, Tag};
use crate::syntax::token::Tag as Tok;

/// What this area's rows use: the reader's image (law L5).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Item(ir::ItemKind::Save))
    .with(ir::Feature::Item(ir::ItemKind::NodeSet))
    .with(ir::Feature::Run(ir::RunExtra::SaveAnalysisSet))
    .with(ir::Feature::Run(ir::RunExtra::SaveFromHere))
    .with(ir::Feature::Run(ir::RunExtra::SaveMidRun))
    .with(ir::Feature::Run(ir::RunExtra::ProbeAllV))
    .with(ir::Feature::Run(ir::RunExtra::ProbeAllBranchI))
    .with(ir::Feature::Run(ir::RunExtra::ProbeI))
    .with(ir::Feature::Run(ir::RunExtra::ProbeParam))
    .with(ir::Feature::Run(ir::RunExtra::ProbeVdiff))
    .with(ir::Feature::Run(ir::RunExtra::ProbeHierarchical))
    .with(ir::Feature::Run(ir::RunExtra::IcNextOnly))
    .with(ir::Feature::Run(ir::RunExtra::IcDifferential))
    .with(ir::Feature::Run(ir::RunExtra::NodeSetHierarchical));

/// A save directive no probe kind states (`full`, an analysis's own directive).
pub(super) const SAVE_FORM: ir::Code = ir::Code::new(b'P', 2308);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[(
    SAVE_FORM,
    ir::Info {
        title: "a save directive no probe kind states, kept as its text",
        cite: "VD cmd-save, cmd-analysis-*; run e5; R43",
        explain: "`full` saves every unknown, collapsed device-internal nodes included; `dv`, `di`, `tf`, `zin`, `yin`, `n`, `nc`, `opdefault`, `opfull` are one analysis's own outputs. The line is kept as text.",
        kind: ir::Kind::Note,
    },
)];

/// One entry of an `ic=`/`nodeset=` list: a node (a path), the second node of a difference
/// (empty for ground), and the value's CST node.
#[derive(Clone, Debug)]
pub(super) struct NodeEntry {
    /// `Ic` or `Nodeset`.
    pub(super) kind: ir::NodeSetKind,
    /// The node's path.
    pub(super) node: Vec<String>,
    /// The second node's path, or empty.
    pub(super) reference: Vec<String>,
    /// The value's CST node.
    pub(super) value: u32,
}

/// A directive's probe, by name.
enum Probe {
    AllV,
    AllBranchI,
    V(Vec<String>),
    I(Vec<String>),
    Param(Vec<String>, String),
}

/// A `save` line.
pub(super) fn save(l: &mut Lower<'_, '_, '_>, n: Node) {
    let Some(directives) = directives(l, n) else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
    };
    let mut probes = Vec::with_capacity(directives.len());
    for (word, args) in &directives {
        let path = |s: &String| -> Vec<String> { s.split(':').map(str::to_string).collect() };
        let plain = args.iter().all(|a| !a.contains('('));
        match (word.as_str(), args.as_slice()) {
            ("default", []) => probes.extend([Probe::AllV, Probe::AllBranchI]),
            ("v", [a]) if plain => probes.push(Probe::V(path(a))),
            ("i", [a]) if plain => probes.push(Probe::I(path(a))),
            ("p", [a, q]) if plain => probes.push(Probe::Param(path(a), q.clone())),
            _ => return read::opaque(l, n, ir::OpaqueClass::Private, Some(SAVE_FORM)),
        }
    }
    let probes: Vec<ir::Probe> = probes
        .iter()
        .map(|p| {
            fn s(v: &[String]) -> Vec<&str> {
                v.iter().map(String::as_str).collect()
            }
            match p {
                Probe::AllV => ir::Probe::category(ir::ProbeKind::AllV),
                Probe::AllBranchI => ir::Probe::category(ir::ProbeKind::AllBranchI),
                Probe::V(a) => l.b.probe_v(&s(a)),
                Probe::I(a) => l.b.probe_i(&s(a)),
                Probe::Param(a, q) => l.b.probe_param(&s(a), q),
            }
        })
        .collect();
    // Run e5: `v()`, `i()`, `p()` are the op and tran directives; `default` is also ac's and
    // dcinc's own (their whole set), so a line of `default` and node or branch probes governs
    // those too (VD cmd-analysis-ac, -dcinc save tables).
    use ir::AnalysisKind as K;
    let own =
        directives.iter().any(|(w, _)| w == "default") && !directives.iter().any(|(w, _)| w == "p");
    let analyses = match own {
        true => ir::AnalysisKinds::of(&[K::Op, K::Dc, K::Tran, K::Ac, K::DcInc]),
        false => ir::AnalysisKinds::of(&[K::Op, K::Dc, K::Tran]),
    };
    let span = l.stmt;
    // Runs sv1, sv2: a save holds from where it stands (A27).
    l.b.save(l.scope, analyses, ir::Applies::FromHere, &probes, span);
}

/// The entries of list node `v` for `kind`, or `None` when it is no list of (string, value)
/// or (string, string, value) groups.
pub(super) fn node_list(
    l: &Lower<'_, '_, '_>,
    kind: ir::NodeSetKind,
    v: u32,
) -> Option<Vec<NodeEntry>> {
    let ast = &l.cst.ast;
    if ast.tag(Node(v)) != Tag::List {
        return None;
    }
    let items = ast.list(ast.rhs(Node(v)) + 1).to_vec();
    let name = |i: u32| {
        (ast.tag(Node(i)) == Tag::Str).then(|| {
            read::field_name(l.cst, i)
                .split(':')
                .map(str::to_string)
                .collect::<Vec<String>>()
        })
    };
    let mut out = Vec::new();
    let mut k = 0;
    // Bounded: each pass takes two or three items.
    while k < items.len() {
        let node = name(items[k])?;
        let (reference, value) = match items.get(k + 1).and_then(|&i| name(i)) {
            Some(r) => (r, *items.get(k + 2)?),
            None => (Vec::new(), *items.get(k + 1)?),
        };
        if name(value).is_some() {
            return None;
        }
        k += 2 + usize::from(!reference.is_empty());
        out.push(NodeEntry {
            kind,
            node,
            reference,
            value,
        });
    }
    (!out.is_empty()).then_some(out)
}

/// The `NodeSet` rows of `entries` (values checked), into the current scope, for the next
/// analysis only.
pub(super) fn node_sets(l: &mut Lower<'_, '_, '_>, entries: &[NodeEntry]) {
    use super::expr_read::{self, Context};
    let span = l.stmt;
    for e in entries {
        let value = expr_read::expr(l, e.value, Context::Param).expect("checked");
        let path = |b: &mut ir::Builder<'_>, p: &[String]| match p.is_empty() {
            true => ir::List::EMPTY,
            false => b.names(&p.iter().map(String::as_str).collect::<Vec<_>>()),
        };
        let node = path(l.b, &e.node);
        let reference = path(l.b, &e.reference);
        let applies = match e.kind {
            ir::NodeSetKind::Ic => ir::IcScope::Transient,
            _ => ir::IcScope::EveryOp,
        };
        let row = ir::NodeSet {
            kind: e.kind,
            applies,
            next_only: true,
            node,
            reference,
            value,
            holds: ir::Applies::FromHere,
        };
        l.b.push(l.scope, row, span);
    }
}

// ---- private ----

/// The directives of `save` statement `n` from its tokens: `word` or `word(arg[, arg])`, each
/// argument one name token (quotes off); `None` when malformed or empty.
fn directives(l: &Lower<'_, '_, '_>, n: Node) -> Option<Vec<(String, Vec<String>)>> {
    let tokens = &l.cst.tokens;
    let tags = tokens.tags();
    let first = l.cst.ast.main_token(n) as usize + 1;
    let end = (first..tags.len())
        .find(|&i| tags[i] == Tok::EndOfStatement)
        .unwrap_or(tags.len());
    let text = |i: usize| tokens.text(i as u32);
    let mut out = Vec::new();
    let mut i = first;
    // Bounded: each pass moves past at least one token.
    while i < end {
        let word = text(i).to_string();
        if matches!(word.as_str(), "(" | ")" | ",") {
            return None;
        }
        i += 1;
        let mut args = Vec::new();
        if i < end && text(i) == "(" {
            i += 1;
            loop {
                if i >= end {
                    return None;
                }
                args.push(super::name::unquote(text(i)));
                i += 1;
                match (i < end).then(|| text(i)) {
                    Some(",") => i += 1,
                    Some(")") => {
                        i += 1;
                        break;
                    }
                    _ => return None,
                }
            }
        }
        out.push((word, args));
    }
    (!out.is_empty()).then_some(out)
}
