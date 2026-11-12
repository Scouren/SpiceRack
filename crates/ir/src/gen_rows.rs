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
use crate::class::ExprClass;
use crate::expr::ExprTag;
use crate::feature::{Extra, Feature, FeatureSet};
use crate::r#gen::{Gen, Reads};
use crate::id::{DefId, ExprId, ItemRef, ScopeId};
use crate::item::{DefKind, LoadKind, OpaqueClass};
use crate::span::Span;

pub(crate) fn gen_title(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    _: &FeatureSet,
) -> ItemRef {
    let text = g.line_word();
    b.title(scope, text, Span::NONE)
}

pub(crate) fn gen_comment(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    _: &FeatureSet,
) -> ItemRef {
    let text = g.line_word();
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
    let mut classes = Vec::new();
    // a head default reads a parameter only when the set has FormalNotConstant (A12 K2)
    g.param_reads = f.contains(Feature::Extra(Extra::FormalNotConstant));
    // no formal can be made when it may neither go without a default nor have one
    let formals = if f.contains(Feature::Expr(ExprTag::Num))
        || f.contains(Feature::Extra(Extra::FormalWithoutDefault))
    {
        g.below(3)
    } else {
        0
    };
    let defaults: Vec<ExprId> = (0..formals)
        .map(|_| {
            // a formal has its default unless the set has FormalWithoutDefault (A15 K2)
            let value = f.contains(Feature::Expr(ExprTag::Num))
                && (!f.contains(Feature::Extra(Extra::FormalWithoutDefault)) || g.chance(2));
            let e = if value {
                g.expr(b, f, Reads::Constant)
            } else {
                ExprId::NONE
            };
            classes.push(if value { g.class } else { ExprClass::Constant });
            e
        })
        .collect();
    g.param_reads = true;
    let name = g.fresh();
    let ports: Vec<String> = (0..g.below(4)).map(|_| g.fresh()).collect();
    let keys: Vec<String> = defaults.iter().map(|_| g.fresh()).collect();
    let ports: Vec<&str> = ports.iter().map(String::as_str).collect();
    let params: Vec<(&str, ExprId)> = keys.iter().map(String::as_str).zip(defaults).collect();
    let item = b.subckt(scope, &name, &ports, &params, Span::NONE).0;
    // the head defaults are the last definitions, in order
    let last = b.design().definitions().len();
    for (k, class) in classes.into_iter().enumerate() {
        g.note(DefId((last - params.len() + k) as u32), class);
    }
    item
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
    // a condition is of class Constant unless the set has IfConditionDynamic (A15 K2)
    g.constant_only = !f.contains(Feature::Extra(Extra::IfConditionDynamic));
    let mut conds: Vec<ExprId> = (0..1 + g.below(2))
        .map(|_| g.expr(b, f, Reads::Constant))
        .collect();
    g.constant_only = false;
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
    let class = g.class;
    let name = g.fresh();
    let item = b.param(scope, &name, value, Span::NONE);
    g.note(DefId((b.design().definitions().len() - 1) as u32), class);
    item
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
    let class = g.class;
    g.formals.clear();
    let name = g.fresh();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let item = b.func(scope, &name, &args, body, Span::NONE);
    g.note(DefId((b.design().definitions().len() - 1) as u32), class);
    item
}
