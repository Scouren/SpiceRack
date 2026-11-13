//! The kernel rows' facts for the rows that open a scope (A2): `Subckt`, `Section` and `If`,
//! each with its handles, expression roots (A23), features, own invariants, IR text and
//! generator in its `RowFacts` impl. Split from `item_facts.rs` by kind.
//!
//! Cites: `rewrite/IR-KERNEL.md` §5, §7.2 (A2, A4, A5), §9; `crates/ir/src/text.rs` (grammar).
//!
//! Contents: `RowFacts` for `Subckt`, `Section`, `If`

use std::borrow::Cow;

use crate::build::Builder;
#[cfg(any(test, feature = "gen"))]
use crate::class::ExprClass;
use crate::design::Design;
use crate::diag::Diags;
#[cfg(any(test, feature = "gen"))]
use crate::expr::ExprTag;
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::{Extra, Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::DefId;
use crate::id::{ExprId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, If, Section, Subckt};
#[cfg(any(test, feature = "gen"))]
use crate::item_facts::value_roots;
use crate::item_facts::{distinct, field_domain, reads_dependent, value_type};
use crate::item_text::{defined, distinct_text, names};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::{Ctx, write_expr, write_name, write_value};

impl RowFacts for Subckt {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Name(self.name),
            Handle::Names(self.ports),
            Handle::Params(self.params),
            Handle::Scope(self.body),
        ]);
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        // the head defaults, in list order
        let row: Subckt = design.row(at.index);
        let start = row.params.start as usize;
        let end = (start + row.params.len as usize).min(design.param_pool.len());
        for p in &mut design.param_pool[start.min(end)..end] {
            f(&mut p.1, ExprUse::Constant);
        }
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        // A12 K2, widened by A15: a head default that is not a constant
        let table = crate::class::def_classes(design);
        let not_constant = design.params(self.params).iter().any(|p| {
            !p.1.is_none()
                && (reads_dependent(design, p.1)
                    || crate::class::tree_class(design, p.1, table)
                        >= crate::class::ExprClass::Temp)
        });
        if not_constant {
            *set = set.with(Feature::Extra(Extra::FormalNotConstant));
        }
        // A15 K2: a formal an instance must pass
        if design.params(self.params).iter().any(|p| p.1.is_none()) {
            *set = set.with(Feature::Extra(Extra::FormalWithoutDefault));
        }
        // A25 item 4: a default by type
        for p in design.params(self.params) {
            value_type(design, p.1, set);
        }
        // R62/A37: a default's reads by rule, a sibling formal's aside
        let formals: Vec<NameId> = design.params(self.params).iter().map(|p| p.0).collect();
        let mut nodes = Vec::new();
        for p in design.params(self.params) {
            nodes.clear();
            crate::class::tree_nodes(design, p.1, &mut nodes);
            for &e in &nodes {
                let crate::expr::Expr::Param(r) = design.expr(e) else {
                    continue;
                };
                let rule = if !r.is_resolved() {
                    (!formals.contains(&r.name)).then_some(Extra::FormalUnboundRead)
                } else {
                    let outside = r
                        .def
                        .index_checked(design.defs.len())
                        .is_some_and(|k| design.defs[k].scope != self.body);
                    outside.then_some(Extra::FormalOuterRead)
                };
                if let Some(x) = rule {
                    *set = set.with(Feature::Extra(x));
                }
            }
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
        let (row, d) = (self, design);
        write_name(d.str(row.name), out);
        out.push_str(" (");
        names(d, d.names(row.ports), " ", out);
        out.push(')');
        // A10: the head defaults are read from the body, where the sibling parameters are defined
        let body = Ctx {
            scope: row.body,
            ..*ctx
        };
        for &(name, value) in d.params(row.params) {
            out.push(' ');
            write_name(d.str(name), out);
            if !value.is_none() {
                out.push('=');
                write_value(d, value, &body, out);
            }
        }
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let Some(name) = w.name_text(diags) else {
            return ItemRef::NONE;
        };
        let Some(ports) = w.names_text(diags) else {
            return ItemRef::NONE;
        };
        let mut params: Vec<(Cow<'_, str>, ExprId)> = Vec::new();
        while !w.at_end() {
            if let Some(key) = w.key() {
                let value = w.value(scope, b, diags);
                if value.is_none() {
                    return ItemRef::NONE;
                }
                params.push((Cow::Borrowed(key), value));
            } else {
                let Some(key) = w.name_text(diags) else {
                    return ItemRef::NONE;
                };
                params.push((key, ExprId::NONE));
            }
        }
        let keys: Vec<Cow<'_, str>> = params.iter().map(|p| p.0.clone()).collect();
        if !distinct_text(&ports)
            || !distinct_text(&keys)
            || defined(b, scope, DefKind::Subckt, &name)
        {
            w.error(diags);
            return ItemRef::NONE;
        }
        let ports: Vec<&str> = ports.iter().map(|p| p.as_ref()).collect();
        let params: Vec<(&str, ExprId)> = params.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        b.subckt(scope, &name, &ports, &params, w.line_span()).0
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let f = features;
        let mut classes = Vec::new();
        // a head default is a constant (no parameter read, class Constant) unless the set has
        // FormalNotConstant (A12 K2, A15)
        let free = f.contains(Feature::Extra(Extra::FormalNotConstant));
        g.param_reads = free;
        // R62/A37: a read bound outside the subcircuit, or left unbound, only as the set allows
        g.bound_reads = f.contains(Feature::Extra(Extra::FormalOuterRead));
        g.unbound_reads = f.contains(Feature::Extra(Extra::FormalUnboundRead));
        g.constant_only = !free;
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
                    value_roots(g, f, true);
                    let e = g.expr(b, f, Reads::Constant);
                    value_roots(g, f, false);
                    e
                } else {
                    ExprId::NONE
                };
                classes.push(if value { g.class } else { ExprClass::Constant });
                e
            })
            .collect();
        g.param_reads = true;
        g.constant_only = false;
        (g.bound_reads, g.unbound_reads) = (true, true);
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
}

impl RowFacts for Section {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Name(self.name),
            Handle::Name(self.library),
            Handle::Scope(self.body),
        ]);
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

    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        write_name(d.str(row.name), out);
        if !row.library.is_none() {
            out.push_str(" library=");
            write_name(d.str(row.library), out);
        }
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let Some(name) = w.name_text(diags) else {
            return ItemRef::NONE;
        };
        let mut library = None;
        if w.key() == Some("library") {
            let Some(l) = w.name_text(diags) else {
                return ItemRef::NONE;
            };
            library = Some(l);
        }
        if defined(b, scope, DefKind::Section, &name) {
            w.error(diags);
            return ItemRef::NONE;
        }
        b.section(scope, &name, library.as_deref(), w.line_span()).0
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let f = features;
        let name = g.fresh();
        let library =
            (f.contains(Feature::Extra(Extra::SectionLibrary)) && g.chance(2)).then(|| g.fresh());
        b.section(scope, &name, library.as_deref(), Span::NONE).0
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

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        // each branch's condition, the `else` (`NONE`) not listed, as `handles`
        let row: If = design.row(at.index);
        let start = row.branches.start as usize;
        let end = (start + row.branches.len as usize).min(design.branch_pool.len());
        for branch in &mut design.branch_pool[start.min(end)..end] {
            if !branch.cond.is_none() {
                f(&mut branch.cond, ExprUse::Constant);
            }
        }
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        // A15 K2: a condition no constant fold decides
        let table = crate::class::def_classes(design);
        let dynamic = design.branches(self.branches).iter().any(|br| {
            crate::class::tree_class(design, br.cond, table) >= crate::class::ExprClass::Temp
        });
        if dynamic {
            *set = set.with(Feature::Extra(Extra::IfConditionDynamic));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let branches = design.branches(self.branches);
        let else_early = branches.iter().rev().skip(1).any(|b| b.cond.is_none());
        if branches.is_empty() || else_early {
            field_domain(design, at, out);
        }
    }

    /// `(cond)` of the first branch; the kernel writes the `elif`/`else` lines.
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        let first = d.branches(row.branches)[0];
        out.push('(');
        write_expr(d, first.cond, ctx, out);
        out.push(')');
    }

    /// One branch: `(cond)`. The kernel's `text::parse` reads an `if` with its `elif`/`else` lines
    /// itself; this is the row's own one-line form.
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let cond = w.value(scope, b, diags);
        if cond.is_none() {
            return ItemRef::NONE;
        }
        b.if_(scope, &[cond], w.line_span()).0
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let f = features;
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
}
