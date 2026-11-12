//! Row facts: what the kernel's generic code (`check`, `features_used`, `text::dump`/`parse`,
//! `gen::generate`, passes that renumber) needs from an item row it does not own. Each row's
//! owner implements [`RowFacts`] in the row's own file; the kernel reaches every kind through one
//! exhaustive `match` over `ItemKind` (no `_` arm), so a new kind fails to compile until it has
//! its facts.
//!
//! Per-kind constant facts an owner's `check` needs (`ElemKind::terminals() -> (u8, u8)`,
//! `ElemKind::has_detail()`, `ElemKind::needs_model()`) are `const fn`s in the owner's file.
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.2 (Addenda A2, A4, A9).
//!
//! Contents: `pub trait RowFacts` (`RowFacts::handles`, `RowFacts::map_refs`,
//! `RowFacts::features`, `RowFacts::check`, `RowFacts::dump`, `RowFacts::parse`,
//! `RowFacts::generate`), `pub enum Handle`,
//! `macro todo_facts` (crate-internal: the definition phase's placeholder impl),
//! `macro for_kind` (crate-internal: the one exhaustive dispatch from an `ItemKind` to its row
//! type)

use crate::build::Builder;
use crate::design::{Design, ItemRow};
use crate::diag::Diags;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{
    BranchList, ExprId, ExprList, ItemRef, NameId, NameList, ParamList, Ref, ScopeId, TextId,
};
use crate::item::DefKind;
use crate::text::Words;
use crate::text_expr::Ctx;

/// What the kernel's generic code needs from an item row. Implemented by the row's owner, in
/// the row's file; the kernel reaches every kind through one exhaustive dispatch.
pub trait RowFacts: ItemRow {
    /// Appends every handle the row and its detail rows hold, in field order (the kernel checks
    /// their ranges and reads the expression features from them).
    fn handles(&self, design: &Design, out: &mut Vec<Handle>);

    /// Calls `f` on every `Ref` the item `at` owns (its header row, its detail rows, its pools),
    /// in a fixed order, with the definition kind each field expects, and stores them back (A4,
    /// A9). An associated function, so a row reaches references outside its header. The linker
    /// binds through it, `check` reads through it, `Builder::bind_ref` addresses its k-th slot,
    /// and a pass that renumbers definitions rewrites through it. Must not change anything but
    /// the references `f` changes.
    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind));

    /// Adds what the row uses beyond `Feature::Item(Self::KIND)`: its element, wave or analysis
    /// kind and its `Extra`, `CircuitExtra` or `RunExtra` features; `at` is the item, `scope` the
    /// scope that lists it (A9). The kernel adds the expression and placement features; features
    /// of the whole design are the owners' `design_features` hooks.
    fn features(&self, design: &Design, at: ItemRef, scope: ScopeId, set: &mut FeatureSet);

    /// Appends the violations of the row's own invariants (counts, a detail row present exactly
    /// when the kind has one, field domains); `at` is the item (for the span), `scope` the scope
    /// that lists it. Handle ranges are the kernel's check.
    fn check(&self, design: &Design, at: ItemRef, scope: ScopeId, out: &mut Diags);

    /// Writes the row's fields as IR text: what follows its keyword, without indentation, line
    /// break or body (the kernel writes those). `ctx` is where it stands (`ctx.item` the item,
    /// `ctx.scope` the scope a reference's bare spelling is looked up from); pass it to the
    /// `text_expr` writers so references come out bare, `name?` or `name#k` by the A4 rule.
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String);

    /// Reads one IR-text statement of this kind (`words` stands after the keyword) and pushes it
    /// into `scope`. On a malformed statement it reports through `words` and still pushes the
    /// best row it can, or returns `ItemRef::NONE` having pushed nothing.
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef;

    /// Pushes one random valid row of this kind that uses only `features` (the kernel only asks
    /// for a kind whose `Feature::Item` is in the set).
    #[cfg(any(test, feature = "gen"))]
    fn generate(g: &mut Gen, scope: ScopeId, b: &mut Builder<'_>, features: &FeatureSet)
    -> ItemRef;
}

/// One handle a row holds, typed by its table. `NONE` handles are listed too: whether `NONE` is
/// allowed is the row's own `check`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// An expression field that must be constant in time: parameters, `Temp` and the A1 leaves
    /// (`TStep`, `TStop`, `Tnom`) may appear; `V`, `I`, `Time`, `Freq`, `Ddt`, `Idt` may not
    /// (`check`: `ProbeContext`; `Gen::expr` with `Reads::Constant`).
    Expr(ExprId),
    /// An expression field that may read the circuit: a behavioural source's expression, a
    /// table's input, a circuit-dependent passive value, a switch's control (`Gen::expr` with
    /// `Reads::Circuit`). Which features it uses is still decided by what it holds (A1).
    ExprDyn(ExprId),
    /// A name.
    Name(NameId),
    /// A reference to a definition.
    Ref(Ref),
    /// A name list.
    Names(NameList),
    /// An expression list.
    Exprs(ExprList),
    /// A parameter list.
    Params(ParamList),
    /// A body scope the row opens.
    Scope(ScopeId),
    /// A text.
    Text(TextId),
    /// An `If`'s branches.
    Branches(BranchList),
}

/// The definition phase's `RowFacts` for a row: every method `todo!()`. The implementation
/// phase replaces each `todo_facts!(Row);` line with the row's own `impl RowFacts`.
#[allow(unused_macros)] // its last users are the owners' placeholder rows; removed at M5
macro_rules! todo_facts {
    ($row:ty) => {
        impl $crate::facts::RowFacts for $row {
            fn handles(&self, design: &$crate::Design, out: &mut Vec<$crate::facts::Handle>) {
                todo!()
            }
            fn map_refs(
                design: &mut $crate::Design,
                at: $crate::ItemRef,
                f: &mut dyn FnMut(&mut $crate::Ref, $crate::DefKind),
            ) {
                todo!()
            }
            fn features(
                &self,
                design: &$crate::Design,
                at: $crate::ItemRef,
                scope: $crate::ScopeId,
                set: &mut $crate::FeatureSet,
            ) {
                todo!()
            }
            fn check(
                &self,
                design: &$crate::Design,
                at: $crate::ItemRef,
                scope: $crate::ScopeId,
                out: &mut $crate::Diags,
            ) {
                todo!()
            }
            #[allow(clippy::ptr_arg)] // definition phase: the body will push to `out`
            fn dump(
                &self,
                design: &$crate::Design,
                ctx: &$crate::text_expr::Ctx,
                out: &mut String,
            ) {
                todo!()
            }
            fn parse(
                words: &mut $crate::text::Words<'_>,
                scope: $crate::ScopeId,
                b: &mut $crate::Builder<'_>,
                diags: &mut $crate::Diags,
            ) -> $crate::ItemRef {
                todo!()
            }
            #[cfg(any(test, feature = "gen"))]
            fn generate(
                g: &mut $crate::r#gen::Gen,
                scope: $crate::ScopeId,
                b: &mut $crate::Builder<'_>,
                features: &$crate::FeatureSet,
            ) -> $crate::ItemRef {
                todo!()
            }
        }
    };
}

/// The one exhaustive dispatch from an `ItemKind` to its row type: `for_kind!(kind, R => body)`
/// evaluates `body` with `R` naming the kind's row type (`ItemRow + RowFacts`). Every generic
/// walk of the kernel (`span_of`, `map_refs`, `check`, `features_used`, `dump`, `parse`,
/// `generate`) goes through it, so a new kind is one arm here.
macro_rules! for_kind {
    ($kind:expr, $R:ident => $body:expr) => {
        match $kind {
            $crate::ItemKind::Title => {
                type $R = $crate::item::Title;
                $body
            }
            $crate::ItemKind::Comment => {
                type $R = $crate::item::Comment;
                $body
            }
            $crate::ItemKind::Include => {
                type $R = $crate::item::Include;
                $body
            }
            $crate::ItemKind::Global => {
                type $R = $crate::item::Global;
                $body
            }
            $crate::ItemKind::Load => {
                type $R = $crate::item::Load;
                $body
            }
            $crate::ItemKind::Opaque => {
                type $R = $crate::item::Opaque;
                $body
            }
            $crate::ItemKind::Subckt => {
                type $R = $crate::item::Subckt;
                $body
            }
            $crate::ItemKind::Section => {
                type $R = $crate::item::Section;
                $body
            }
            $crate::ItemKind::If => {
                type $R = $crate::item::If;
                $body
            }
            $crate::ItemKind::Param => {
                type $R = $crate::item::Param;
                $body
            }
            $crate::ItemKind::Func => {
                type $R = $crate::item::Func;
                $body
            }
            $crate::ItemKind::Model => {
                type $R = $crate::model::Model;
                $body
            }
            $crate::ItemKind::DeviceLibrary => {
                type $R = $crate::library::DeviceLibrary;
                $body
            }
            $crate::ItemKind::Statistics => {
                type $R = $crate::stats::Statistics;
                $body
            }
            $crate::ItemKind::Element => {
                type $R = $crate::element::Element;
                $body
            }
            $crate::ItemKind::Analysis => {
                type $R = $crate::analysis::Analysis;
                $body
            }
            $crate::ItemKind::Loop => {
                type $R = $crate::plan::Loop;
                $body
            }
            $crate::ItemKind::Alter => {
                type $R = $crate::plan::Alter;
                $body
            }
            $crate::ItemKind::Option => {
                type $R = $crate::option::OptionRow;
                $body
            }
            $crate::ItemKind::Env => {
                type $R = $crate::option::Env;
                $body
            }
            $crate::ItemKind::NodeSet => {
                type $R = $crate::option::NodeSet;
                $body
            }
            $crate::ItemKind::Save => {
                type $R = $crate::output::Save;
                $body
            }
            $crate::ItemKind::Print => {
                type $R = $crate::output::Print;
                $body
            }
            $crate::ItemKind::Measure => {
                type $R = $crate::output::Measure;
                $body
            }
        }
    };
}
