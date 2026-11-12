//! The kernel rows' generators: for each of the 11 rows of `item.rs`, one random valid row,
//! pushed in a builder caller's order (expressions first, then the item with its names) so the
//! IR text reads it back with the same ids. `generate` (`gen.rs`) chooses the kind and fills
//! the bodies; the rows' `RowFacts::generate` delegate here. Built only for tests and under the
//! `gen` feature.
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.2 (A2), §9; `crates/ir/src/gen.rs`.
//!
//! Contents: crate: `gen_title`, `gen_comment`, `gen_include`, `gen_global`, `gen_load`,
//! `gen_opaque`, `gen_subckt`, `gen_section`, `gen_if`, `gen_param`, `gen_func` (each `(&mut
//! Gen, ScopeId, &mut Builder<'_>, &FeatureSet) -> ItemRef`)

use crate::build::Builder;
use crate::expr::ExprTag;
use crate::feature::{Extra, Feature, FeatureSet};
use crate::r#gen::{Gen, Reads};
use crate::id::{ExprId, ItemRef, ScopeId};
use crate::item::{DefKind, LoadKind, OpaqueClass};
use crate::span::Span;

pub(crate) fn gen_title(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    _: &FeatureSet,
) -> ItemRef {
    let text = g.word();
    b.title(scope, text, Span::NONE)
}

pub(crate) fn gen_comment(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    _: &FeatureSet,
) -> ItemRef {
    let text = g.word();
    b.comment(scope, text, Span::NONE)
}

pub(crate) fn gen_include(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    f: &FeatureSet,
) -> ItemRef {
    let path = format!("{}.lib", g.fresh());
    let section =
        (f.contains(Feature::Extra(Extra::IncludeSection)) && g.chance(2)).then(|| g.fresh());
    b.include(scope, &path, section.as_deref(), Span::NONE)
}

pub(crate) fn gen_global(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    _: &FeatureSet,
) -> ItemRef {
    let nets: Vec<String> = (0..1 + g.below(3)).map(|_| g.fresh()).collect();
    let refs: Vec<&str> = nets.iter().map(String::as_str).collect();
    b.global(scope, &refs, Span::NONE)
}

pub(crate) fn gen_load(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    _: &FeatureSet,
) -> ItemRef {
    let kind = if g.chance(2) {
        LoadKind::VerilogA
    } else {
        LoadKind::Osdi
    };
    let path = format!("{}.osdi", g.fresh());
    let item = b.load(scope, kind, &path, Span::NONE);
    for _ in 0..g.below(3) {
        let module = g.fresh_name(b);
        b.define(DefKind::Module, module, scope, item);
    }
    item
}

pub(crate) fn gen_opaque(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    _: &FeatureSet,
) -> ItemRef {
    const DIALECTS: [&str; 4] = ["ngspice", "xyce", "spectre", "hspice"];
    let dialect = DIALECTS[g.below(4) as usize];
    let class = OpaqueClass::ALL[g.below(OpaqueClass::COUNT as u32) as usize];
    let text = format!(".{} {}", g.fresh(), g.word());
    b.opaque(scope, dialect, class, &text, Span::NONE)
}

pub(crate) fn gen_subckt(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    f: &FeatureSet,
) -> ItemRef {
    let defaults: Vec<ExprId> = (0..g.below(3))
        .map(|_| {
            let value = f.contains(Feature::Expr(ExprTag::Num)) && g.chance(2);
            if value {
                g.expr(b, f, Reads::Constant)
            } else {
                ExprId::NONE
            }
        })
        .collect();
    let name = g.fresh();
    let ports: Vec<String> = (0..g.below(4)).map(|_| g.fresh()).collect();
    let keys: Vec<String> = defaults.iter().map(|_| g.fresh()).collect();
    let ports: Vec<&str> = ports.iter().map(String::as_str).collect();
    let params: Vec<(&str, ExprId)> = keys.iter().map(String::as_str).zip(defaults).collect();
    b.subckt(scope, &name, &ports, &params, Span::NONE).0
}

pub(crate) fn gen_section(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    f: &FeatureSet,
) -> ItemRef {
    let name = g.fresh();
    let library =
        (f.contains(Feature::Extra(Extra::SectionLibrary)) && g.chance(2)).then(|| g.fresh());
    b.section(scope, &name, library.as_deref(), Span::NONE).0
}

pub(crate) fn gen_if(g: &mut Gen, scope: ScopeId, b: &mut Builder<'_>, f: &FeatureSet) -> ItemRef {
    let mut conds: Vec<ExprId> = (0..1 + g.below(2))
        .map(|_| g.expr(b, f, Reads::Constant))
        .collect();
    if g.chance(2) {
        conds.push(ExprId::NONE);
    }
    b.if_(scope, &conds, Span::NONE).0
}

pub(crate) fn gen_param(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    f: &FeatureSet,
) -> ItemRef {
    let value = g.expr(b, f, Reads::Constant);
    let name = g.fresh();
    b.param(scope, &name, value, Span::NONE)
}

pub(crate) fn gen_func(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    f: &FeatureSet,
) -> ItemRef {
    let args: Vec<String> = (0..g.below(3)).map(|_| g.fresh()).collect();
    g.formals = args.clone();
    let body = g.expr(b, f, Reads::Circuit);
    g.formals.clear();
    let name = g.fresh();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    b.func(scope, &name, &args, body, Span::NONE)
}
