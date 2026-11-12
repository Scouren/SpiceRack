//! The kernel rows' facts (A2): for each row of `item.rs` but `Param` and `Func` (in
//! `item_facts_defs.rs`), the handles it holds, its references (none), its expression roots
//! (A23), the features it adds beyond its kind, its own invariants, and its IR text and
//! generator. The kernel's generic walks reach these through `for_kind!`. Split from `item.rs`
//! so that file keeps its rows and enums.
//!
//! A kernel row's own invariants are `FieldDomain` (a name that must be set, distinct ports,
//! arguments, parameters and global nets, ground never global, `else` only last) and
//! `OpaqueEmpty`; handle ranges and empty names are the kernel's `check`.
//!
//! Cites: `rewrite/IR-KERNEL.md` §5, §7.2 (A2, A4, A5), §9.
//!
//! Contents: `RowFacts` for `Title`, `Comment`, `Include`, `Global`, `Load`, `Opaque` (`Subckt`,
//! `Section`, `If`: `item_facts_scopes.rs`; `Param`, `Func`: `item_facts_defs.rs`); crate:
//! `pub(crate) fn field_domain(&Design, ItemRef, &mut Diags)`,
//! `pub(crate) fn distinct(&[NameId]) -> bool`, `pub(crate) fn reads_dependent(&Design, ExprId) -> bool`,
//! `pub(crate) fn value_type(&Design, ExprId, &mut FeatureSet)`,
//! `pub(crate) fn value_roots(&mut Gen, &FeatureSet, bool)` (gen)

use std::borrow::Cow;

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
    Comment, DefKind, Global, Include, Load, LoadKind, Opaque, OpaqueClass, Role, ScopeKind, Title,
};
use crate::item_text::{class_word, defined, distinct_text, dump_load, modules_of, names};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::{Ctx, write_name, write_string};

impl RowFacts for Title {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Text(self.text));
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn check(&self, design: &Design, at: ItemRef, scope: ScopeId, out: &mut Diags) {
        // A15 K3: one line of text; only in a deck's top (a pass may have moved it)
        let s = design.scope(scope);
        let deck = s.kind == crate::item::ScopeKind::File
            && design.file(s.file).role == crate::item::Role::Deck;
        if !deck || design.text(self.text).contains(['\n', '\r']) {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        write_string(d.text(row.text), out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let s = b.design().scope(scope);
        let deck = s.kind == ScopeKind::File && b.design().file(s.file).role == Role::Deck;
        let Some(text) = w.string(diags) else {
            return ItemRef::NONE;
        };
        if !deck {
            w.error(diags);
            return ItemRef::NONE;
        }
        b.title(scope, &text, w.line_span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        _features: &FeatureSet,
    ) -> ItemRef {
        let text = g.line_word();
        b.title(scope, text, Span::NONE)
    }
}

impl RowFacts for Comment {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Text(self.text));
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        // A15 K3: one line of text
        if design.text(self.text).contains(['\n', '\r']) {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        write_string(d.text(row.text), out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let Some(text) = w.string(diags) else {
            return ItemRef::NONE;
        };
        b.comment(scope, &text, w.line_span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        _features: &FeatureSet,
    ) -> ItemRef {
        let text = g.line_word();
        b.comment(scope, text, Span::NONE)
    }
}

impl RowFacts for Include {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.path), Handle::Name(self.section)]);
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
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

    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        write_string(d.str(row.path), out);
        if !row.section.is_none() {
            out.push_str(" section=");
            write_name(d.str(row.section), out);
        }
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let Some(path) = w.string(diags) else {
            return ItemRef::NONE;
        };
        let section = if w.key() == Some("section") {
            let Some(s) = w.name_text(diags) else {
                return ItemRef::NONE;
            };
            Some(s)
        } else {
            None
        };
        b.include(scope, &path, section.as_deref(), w.line_span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let f = features;
        let path = format!("{}.lib", g.fresh());
        let section =
            (f.contains(Feature::Extra(Extra::IncludeSection)) && g.chance(2)).then(|| g.fresh());
        b.include(scope, &path, section.as_deref(), Span::NONE)
    }
}

impl RowFacts for Global {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Names(self.nets));
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let nets = design.names(self.nets);
        if nets.is_empty() || nets.contains(&NameId::GROUND) || !distinct(nets) {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        names(d, d.names(row.nets), " ", out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let mut nets: Vec<Cow<'_, str>> = Vec::new();
        while !w.at_end() {
            let Some(n) = w.name_text(diags) else {
                return ItemRef::NONE;
            };
            nets.push(n);
        }
        if nets.is_empty() || nets.iter().any(|n| n == "0") || !distinct_text(&nets) {
            w.error(diags);
            return ItemRef::NONE;
        }
        let refs: Vec<&str> = nets.iter().map(|n| n.as_ref()).collect();
        b.global(scope, &refs, w.line_span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        _features: &FeatureSet,
    ) -> ItemRef {
        let nets: Vec<String> = (0..1 + g.below(3)).map(|_| g.fresh()).collect();
        let refs: Vec<&str> = nets.iter().map(String::as_str).collect();
        b.global(scope, &refs, Span::NONE)
    }
}

impl RowFacts for Load {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Name(self.path));
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.path.is_none() {
            field_domain(design, at, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_load(self, design, &modules_of(design, ctx.item), out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let kind = match w.next() {
            Some("veriloga") => LoadKind::VerilogA,
            Some("osdi") => LoadKind::Osdi,
            _ => {
                w.error(diags);
                return ItemRef::NONE;
            }
        };
        let Some(path) = w.string(diags) else {
            return ItemRef::NONE;
        };
        let mut modules = Vec::new();
        if w.key() == Some("modules") {
            let Some(m) = w.names_text(diags) else {
                return ItemRef::NONE;
            };
            modules = m;
        }
        let taken = |b: &Builder<'_>, n: &str| defined(b, scope, DefKind::Module, n);
        if !distinct_text(&modules) || modules.iter().any(|m| taken(b, m)) {
            w.error(diags);
            return ItemRef::NONE;
        }
        let item = b.load(scope, kind, &path, w.line_span());
        for m in &modules {
            let name = b.name(m);
            b.define(DefKind::Module, name, scope, item);
        }
        item
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        _features: &FeatureSet,
    ) -> ItemRef {
        let kind = if g.chance(2) {
            LoadKind::VerilogA
        } else {
            LoadKind::Osdi
        };
        // A25: the extension a simulator decides the kind by
        let extension = match kind {
            LoadKind::VerilogA => "va",
            LoadKind::Osdi => "osdi",
        };
        let path = format!("{}.{extension}", g.fresh());
        let item = b.load(scope, kind, &path, Span::NONE);
        for _ in 0..g.below(3) {
            let module = g.fresh_name(b);
            b.define(DefKind::Module, module, scope, item);
        }
        item
    }
}

impl RowFacts for Opaque {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.dialect), Handle::Text(self.text)]);
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let no_dialect = self.dialect.is_none() || design.str(self.dialect).is_empty();
        if no_dialect || design.text(self.text).is_empty() {
            out.push_at(violation_code(Violation::OpaqueEmpty), design, at, at.index);
        }
    }

    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        let (row, d) = (self, design);
        write_name(d.str(row.dialect), out);
        out.push(' ');
        out.push_str(class_word(row.class));
        out.push(' ');
        write_string(d.text(row.text), out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let w = words;
        let Some(dialect) = w.name_text(diags) else {
            return ItemRef::NONE;
        };
        let class = match w.next() {
            Some("unread") => OpaqueClass::Unread,
            Some("private") => OpaqueClass::Private,
            Some("inert") => OpaqueClass::Inert,
            Some("invalid") => OpaqueClass::InvalidInSource,
            Some("script") => OpaqueClass::Script,
            Some("hint") => OpaqueClass::Hint,
            _ => {
                w.error(diags);
                return ItemRef::NONE;
            }
        };
        let Some(text) = w.string(diags) else {
            return ItemRef::NONE;
        };
        b.opaque(scope, &dialect, class, &text, w.line_span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        _features: &FeatureSet,
    ) -> ItemRef {
        const DIALECTS: [&str; 4] = ["ngspice", "xyce", "spectre", "hspice"];
        let dialect = DIALECTS[g.below(4) as usize];
        let class = OpaqueClass::ALL[g.below(OpaqueClass::COUNT as u32) as usize];
        let text = format!(".{} {}", g.fresh(), g.word());
        b.opaque(scope, dialect, class, &text, Span::NONE)
    }
}

/// Appends `FieldDomain` at `at` (`arg`: the row's index in its table).
pub(crate) fn field_domain(design: &Design, at: ItemRef, out: &mut Diags) {
    out.push_at(violation_code(Violation::FieldDomain), design, at, at.index);
}

/// Whether no name appears twice. O(n²): lists are short (ports, arguments, nets).
pub(crate) fn distinct(names: &[NameId]) -> bool {
    names
        .iter()
        .enumerate()
        .all(|(k, n)| !names[..k].contains(n))
}

/// Whether the tree under `root` reads a parameter or `$tnom` (A12 K2, A25: what makes a head
/// default dependent). Explicit stack (C11); descends only to lower ids, so it ends on any table
/// contents.
pub(crate) fn reads_dependent(design: &Design, root: ExprId) -> bool {
    let mut stack = vec![root];
    let mut kids = Vec::new();
    while let Some(e) = stack.pop() {
        if e.index() >= design.expr_count() {
            continue;
        }
        if let crate::expr::Expr::Param(_) | crate::expr::Expr::Tnom = design.expr(e) {
            return true;
        }
        kids.clear();
        design.children(e, &mut kids);
        stack.extend(kids.iter().copied().filter(|k| *k < e));
    }
    false
}

/// Adds `Extra::StringValue` or `Extra::ListValue` when the value `e` (a `Param`'s, or a
/// `Subckt` head default) is a string or a list at its root (A25 item 4). `NONE`: nothing.
pub(crate) fn value_type(design: &Design, e: ExprId, set: &mut FeatureSet) {
    if e.is_none() || e.index() >= design.expr_count() {
        return;
    }
    match design.expr(e) {
        crate::expr::Expr::Str(_) => *set = set.with(Feature::Extra(Extra::StringValue)),
        crate::expr::Expr::List(_) => *set = set.with(Feature::Extra(Extra::ListValue)),
        _ => {}
    }
}

/// Lets the next expression be a string or a list at its root as the set allows (A25 item 4:
/// a parameter's value, a formal's default), or takes that back.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn value_roots(g: &mut Gen, f: &FeatureSet, on: bool) {
    g.root_str = on && f.contains(Feature::Extra(Extra::StringValue));
    g.root_list = on && f.contains(Feature::Extra(Extra::ListValue));
}

// ---- private ----
