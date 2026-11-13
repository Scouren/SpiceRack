//! The facts of the kernel's two definition rows, `Param` and `Func` (A2): their handles, no
//! references, their roots for passes (A23), their own invariants, IR text and generator. Split
//! from `item_facts.rs` at its size limit; the shared helpers stay there.
//!
//! Cites: `rewrite/IR-KERNEL.md` §5, §7.2 (A2, A4, A9), A23.
//!
//! Contents: `RowFacts` for `Param`, `Func`

use std::borrow::Cow;

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::{Extra, Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::DefId;
use crate::id::{ExprId, ItemRef, ScopeId};
use crate::item::{DefKind, Func, Param, ScopeKind};
#[cfg(any(test, feature = "gen"))]
use crate::item_facts::value_roots;
use crate::item_facts::{distinct, field_domain, reads_dependent};
use crate::item_text::{defined, distinct_text, names};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::{Ctx, write_expr, write_name};

impl RowFacts for Param {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.name), Handle::Expr(self.value)]);
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let mut row: Param = design.row(at.index);
        f(&mut row.value, ExprUse::Constant);
        design.editor().set(at.index, row);
    }

    fn features(&self, design: &Design, _at: ItemRef, scope: ScopeId, set: &mut FeatureSet) {
        // A25: a constant local of a subcircuit (VACASK reads it as a formal)
        let in_body = design.scope(scope).kind == ScopeKind::Subckt;
        let constant = !reads_dependent(design, self.value)
            && crate::class::tree_class(design, self.value, crate::class::def_classes(design))
                == crate::class::ExprClass::Constant;
        if in_body && constant {
            *set = set.with(Feature::Extra(Extra::SubcktConstantParam));
        }
        // A25 item 4: a value by type
        crate::item_facts::value_type(design, self.value, set);
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.name.is_none() || self.value.is_none() {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        write_name(d.str(row.name), out);
        out.push_str(" = ");
        write_expr(d, row.value, ctx, out);
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
        if !w.expect("=", diags) {
            return ItemRef::NONE;
        }
        let value = w.expr(scope, b, diags);
        if value.is_none() {
            return ItemRef::NONE;
        }
        if defined(b, scope, DefKind::Param, &name) {
            w.error(diags);
            return ItemRef::NONE;
        }
        b.param(scope, &name, value, w.line_span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let f = features;
        value_roots(g, f, true);
        let value = g.expr(b, f, Reads::Constant);
        value_roots(g, f, false);
        let class = g.class;
        let name = g.fresh();
        let item = b.param(scope, &name, value, Span::NONE);
        g.note(
            DefId((b.design().definitions_all().len() - 1) as u32),
            class,
        );
        item
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

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let mut row: Func = design.row(at.index);
        f(&mut row.body, ExprUse::Circuit);
        design.editor().set(at.index, row);
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.name.is_none() || self.body.is_none() || !distinct(design.names(self.args)) {
            field_domain(design, at, out);
        }
    }

    /// `name(a, b) = body`, the body written with this function's formal arguments in scope.
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        let (row, d, at) = (self, design, ctx.item);
        write_name(d.str(row.name), out);
        out.push('(');
        names(d, d.names(row.args), ", ", out);
        out.push_str(") = ");
        // the function defines itself in the scope that lists it: one index lookup
        let formals = match d.def_index.get(&(ctx.scope, DefKind::Func, row.name)) {
            Some(&def) if d.def(def).item == at => def,
            Some(_) | None => crate::id::DefId::NONE,
        };
        write_expr(d, row.body, &Ctx { formals, ..*ctx }, out);
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
        if !w.expect("(", diags) {
            return ItemRef::NONE;
        }
        let mut args: Vec<Cow<'_, str>> = Vec::new();
        // bounded by the line: each turn consumes a word
        while w.peek() != Some(")") {
            let Some(a) = w.name_text(diags) else {
                return ItemRef::NONE;
            };
            args.push(a);
            if w.peek() == Some(",") {
                w.next();
            } else if w.peek() != Some(")") {
                w.error(diags);
                return ItemRef::NONE;
            }
        }
        w.next();
        if !w.expect("=", diags) {
            return ItemRef::NONE;
        }
        let body = w.expr(scope, b, diags);
        if body.is_none() {
            return ItemRef::NONE;
        }
        if !distinct_text(&args) || defined(b, scope, DefKind::Func, &name) {
            w.error(diags);
            return ItemRef::NONE;
        }
        let args: Vec<&str> = args.iter().map(|a| a.as_ref()).collect();
        b.func(scope, &name, &args, body, w.line_span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let f = features;
        let args: Vec<String> = (0..g.below(3)).map(|_| g.fresh()).collect();
        g.formals = args.clone();
        // A28: a body counts in its callers' forms, so its operators are those of both
        g.both_forms = true;
        let body = g.expr(b, f, Reads::Circuit);
        g.both_forms = false;
        let class = g.class;
        g.formals.clear();
        let name = g.fresh();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let item = b.func(scope, &name, &args, body, Span::NONE);
        g.note(
            DefId((b.design().definitions_all().len() - 1) as u32),
            class,
        );
        item
    }
}
