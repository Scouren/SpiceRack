//! The kernel rows' facts (A2): for each of the 11 rows of `item.rs`, the handles it holds, its
//! references (none), the features it adds beyond its kind, its own invariants, and its IR text
//! and generator. The kernel's generic walks reach these through `for_kind!`. Split from
//! `item.rs` so that file keeps its rows and enums.
//!
//! A kernel row's own invariants are `FieldDomain` (a name that must be set, distinct ports,
//! arguments, parameters and global nets, ground never global, `else` only last) and
//! `OpaqueEmpty`; handle ranges and empty names are the kernel's `check`.
//!
//! Cites: `rewrite/IR-KERNEL.md` §5, §7.2 (A2, A4, A5), §9.
//!
//! Contents: `RowFacts` for `Title`, `Comment`, `Include`, `Global`, `Load`, `Opaque`, `Subckt`,
//! `Section`, `If`, `Param`, `Func`

use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::{Diags, violation_code};
use crate::facts::{Handle, RowFacts};
use crate::feature::{Extra, Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ExprId, ItemRef, NameId, Ref, ScopeId};
use crate::item::{
    Comment, DefKind, Func, Global, If, Include, Load, Opaque, Param, Section, Subckt, Title,
};
use crate::item_text;
use crate::text::Words;
use crate::text_expr::Ctx;

impl RowFacts for Title {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Text(self.text));
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _out: &mut Diags) {
        // no invariant beyond the handles
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_title(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_title(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_title(g, scope, b, features)
    }
}

impl RowFacts for Comment {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Text(self.text));
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _out: &mut Diags) {
        // no invariant beyond the handles
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_comment(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_comment(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_comment(g, scope, b, features)
    }
}

impl RowFacts for Include {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.path), Handle::Name(self.section)]);
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        if !self.section.is_none() {
            *set = set.with(Feature::Extra(Extra::IncludeSection));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.path.is_none() {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_include(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_include(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_include(g, scope, b, features)
    }
}

impl RowFacts for Global {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Names(self.nets));
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let nets = design.names(self.nets);
        if nets.is_empty() || nets.contains(&NameId::GROUND) || !distinct(nets) {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_global(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_global(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_global(g, scope, b, features)
    }
}

impl RowFacts for Load {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Name(self.path));
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.path.is_none() {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_load(self, design, &item_text::modules_of(design, ctx.item), out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_load(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_load(g, scope, b, features)
    }
}

impl RowFacts for Opaque {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.dialect), Handle::Text(self.text)]);
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let no_dialect = self.dialect.is_none() || design.str(self.dialect).is_empty();
        if no_dialect || design.text(self.text).is_empty() {
            out.push(
                violation_code(Violation::OpaqueEmpty),
                design.span_of(at),
                at.index,
            );
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_opaque(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_opaque(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_opaque(g, scope, b, features)
    }
}

impl RowFacts for Subckt {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Name(self.name),
            Handle::Names(self.ports),
            Handle::Params(self.params),
            Handle::Scope(self.body),
        ]);
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        // A12 K2: a head default that reads a parameter
        let defaults = design
            .params(self.params)
            .iter()
            .map(|p| p.1)
            .filter(|e| !e.is_none());
        if defaults.into_iter().any(|e| reads_param(design, e)) {
            *set = set.with(Feature::Extra(Extra::FormalReadsParam));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let ports = design.names(self.ports);
        let params: Vec<NameId> = design.params(self.params).iter().map(|p| p.0).collect();
        if self.name.is_none() || !distinct(ports) || !distinct(&params) {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_subckt(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_subckt(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_subckt(g, scope, b, features)
    }
}

impl RowFacts for Section {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Name(self.name),
            Handle::Name(self.library),
            Handle::Scope(self.body),
        ]);
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        if !self.library.is_none() {
            *set = set.with(Feature::Extra(Extra::SectionLibrary));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.name.is_none() {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_section(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_section(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_section(g, scope, b, features)
    }
}

impl RowFacts for If {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Branches(self.branches));
        for branch in design.branches(self.branches) {
            out.push(Handle::Scope(branch.body));
            if !branch.cond.is_none() {
                out.push(Handle::Expr(branch.cond));
            }
        }
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let branches = design.branches(self.branches);
        let else_early = branches.iter().rev().skip(1).any(|b| b.cond.is_none());
        if branches.is_empty() || else_early {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_if(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_if(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_if(g, scope, b, features)
    }
}

impl RowFacts for Param {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.name), Handle::Expr(self.value)]);
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.name.is_none() || self.value.is_none() {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_param(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_param(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_param(g, scope, b, features)
    }
}

impl RowFacts for Func {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        // a function may be called from an expression that reads the circuit
        out.extend([
            Handle::Name(self.name),
            Handle::Names(self.args),
            Handle::ExprDyn(self.body),
        ]);
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // no feature beyond the kind
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.name.is_none() || self.body.is_none() || !distinct(design.names(self.args)) {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        item_text::dump_func(self, design, ctx.item, ctx, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        item_text::parse_func(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::gen_rows::gen_func(g, scope, b, features)
    }
}

// ---- private ----

/// Appends `FieldDomain` at `at` (`arg`: the row's index in its table).
fn field_domain(design: &Design, at: ItemRef, out: &mut Diags) {
    out.push(
        violation_code(Violation::FieldDomain),
        design.span_of(at),
        at.index,
    );
}

/// Whether the tree under `root` holds a `Param` read. Explicit stack (C11); descends only to
/// lower ids, so it ends on any table contents.
fn reads_param(design: &Design, root: ExprId) -> bool {
    let mut stack = vec![root];
    let mut kids = Vec::new();
    while let Some(e) = stack.pop() {
        if e.index() >= design.expr_count() {
            continue;
        }
        if let crate::expr::Expr::Param(_) = design.expr(e) {
            return true;
        }
        kids.clear();
        design.children(e, &mut kids);
        stack.extend(kids.iter().copied().filter(|k| *k < e));
    }
    false
}

/// Whether no name appears twice. O(n²): lists are short (ports, arguments, nets).
fn distinct(names: &[NameId]) -> bool {
    names
        .iter()
        .enumerate()
        .all(|(k, n)| !names[..k].contains(n))
}
