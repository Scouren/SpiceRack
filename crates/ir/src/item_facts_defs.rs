//! The facts of the kernel's two definition rows, `Param` and `Func` (A2): their handles, no
//! references, their roots for passes (A23), their own invariants, IR text and generator. Split
//! from `item_facts.rs` at its size limit; the shared helpers stay there.
//!
//! Cites: `rewrite/IR-KERNEL.md` §5, §7.2 (A2, A4, A9), A23.
//!
//! Contents: `RowFacts` for `Param`, `Func`

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::{Extra, Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ExprId, ItemRef, Ref, ScopeId};
use crate::item::{DefKind, Func, Param, ScopeKind};
use crate::item_facts::{distinct, field_domain, reads_dependent};
use crate::item_text;
use crate::text::Words;
use crate::text_expr::Ctx;

impl RowFacts for Param {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.name), Handle::Expr(self.value)]);
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let mut row: Param = design.row(at.index);
        f(&mut row.value, ExprUse::Constant);
        design.editor().set(at.index, row);
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
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

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let mut row: Func = design.row(at.index);
        f(&mut row.body, ExprUse::Circuit);
        design.editor().set(at.index, row);
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
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
