//! The design: every table of the IR in one owned arena, and the read accessors writers use.
//! Appends come through `Builder` (readers, builders), rewrites through `Editor` (passes); a
//! writer gets `&Design` and nothing else. A `Design` holds no source text, no tokens and no
//! source-dialect tag (REWRITE-PLAN §2.2 rule 1), and is freed whole.
//!
//! Tables (IR-KERNEL §4): the name interner and text pool; files, scopes and their ordered item
//! lists; definitions and their lookup index; expression rows; the four shared pools; one
//! [`Table`] per [`ItemKind`]; the circuit and run owners' detail tables (`CircuitDetails`,
//! `RunDetails`, declared in their files so they add tables without editing this one).
//!
//! Typed access to a row of any kind is generic over [`ItemRow`] (`design.row::<Subckt>(i)`,
//! `builder.push(scope, row, span)`, `editor.set(i, row)`): one accessor per kind, written once.
//!
//! ```ignore
//! for file in design.files() {
//!     for &item in design.items(design.file(file).top) {
//!         match item.kind {
//!             ItemKind::Subckt => { let s: Subckt = design.row(item.index); /* … */ }
//!             // one arm per kind, no `_` arm
//!         }
//!     }
//! }
//! ```
//!
//! Cites: `rewrite/IR-KERNEL.md` §4, C3, C4, C5, C13; `REWRITE-PLAN.md` §3.2.
//!
//! Contents:
//! - `pub struct Design`, `pub trait ItemRow` (`ItemRow::KIND`, `ItemRow::table`,
//!   `ItemRow::table_mut`)
//! - `Design::new() -> Design`, `Design::builder(&mut self) -> Builder<'_>`,
//!   `Design::editor(&mut self) -> Editor<'_>`, `Design::entry(&self) -> FileId`,
//!   `Design::files(&self) -> impl Iterator<Item = FileId>`, `Design::file(&self, FileId) -> File`,
//!   `Design::scope(&self, ScopeId) -> Scope`, `Design::items(&self, ScopeId) -> &[ItemRef]`,
//!   `Design::row<R: ItemRow>(&self, u32) -> R`, `Design::span_of(&self, ItemRef) -> Span`,
//!   `Design::str(&self, NameId) -> &str`, `Design::find_name(&self, &str) -> NameId`,
//!   `Design::names(&self, NameList) -> &[NameId]`, `Design::exprs(&self, ExprList) -> &[ExprId]`,
//!   `Design::params(&self, ParamList) -> &[(NameId, ExprId)]`,
//!   `Design::branches(&self, BranchList) -> &[Branch]`, `Design::text(&self, TextId) -> &str`,
//!   `Design::expr(&self, ExprId) -> Expr`, `Design::expr_count(&self) -> usize`,
//!   `Design::definitions(&self) -> &[Def]`, `Design::def(&self, DefId) -> Def`,
//!   `Design::eq_ignoring_spans(&self, &Design) -> bool`

use std::collections::HashMap;

use crate::analysis::{Analysis, RunDetails};
use crate::build::Builder;
use crate::edit::Editor;
use crate::element::{CircuitDetails, Element};
use crate::expr::{Expr, Exprs};
use crate::facts::RowFacts;
use crate::id::{
    BranchList, DefId, ExprId, ExprList, FileId, ItemRef, NameId, NameList, ParamList, ScopeId,
    TextId,
};
use crate::item::{
    Branch, Comment, DefKind, Func, Global, If, Include, ItemKind, Load, Opaque, Param, Section,
    Subckt, Title,
};
use crate::model::Model;
use crate::names::{Names, Texts};
use crate::option::{Env, NodeSet, OptionRow};
use crate::output::{Measure, Print, Save};
use crate::plan::{Alter, Loop};
use crate::scope::{Def, File, Scope};
use crate::span::Span;
use crate::stats::Statistics;
use crate::table::Table;

/// One design: every file of a deck or kit, as typed rows. Construct with [`Design::new`]; the
/// caller owns it and may reuse it per deck.
#[derive(Clone, Debug)]
pub struct Design {
    pub(crate) names: Names,
    pub(crate) texts: Texts,
    pub(crate) files: Vec<File>,
    pub(crate) scopes: Vec<Scope>,
    /// Per scope, its items in order (C5: passes rewrite these lists).
    pub(crate) scope_items: Vec<Vec<ItemRef>>,
    pub(crate) defs: Vec<Def>,
    /// `(scope, kind, name)` → definition: lookup only, never iterated.
    pub(crate) def_index: HashMap<(ScopeId, DefKind, NameId), DefId>,
    pub(crate) exprs: Exprs,
    pub(crate) name_pool: Vec<NameId>,
    pub(crate) expr_pool: Vec<ExprId>,
    pub(crate) param_pool: Vec<(NameId, ExprId)>,
    pub(crate) branch_pool: Vec<Branch>,
    // one table per ItemKind, in ItemKind order
    pub(crate) titles: Table<Title>,
    pub(crate) comments: Table<Comment>,
    pub(crate) includes: Table<Include>,
    pub(crate) globals: Table<Global>,
    pub(crate) loads: Table<Load>,
    pub(crate) opaques: Table<Opaque>,
    pub(crate) subckts: Table<Subckt>,
    pub(crate) sections: Table<Section>,
    pub(crate) ifs: Table<If>,
    pub(crate) params: Table<Param>,
    pub(crate) funcs: Table<Func>,
    pub(crate) models: Table<Model>,
    pub(crate) statistics: Table<Statistics>,
    pub(crate) elements: Table<Element>,
    pub(crate) analyses: Table<Analysis>,
    pub(crate) loops: Table<Loop>,
    pub(crate) alters: Table<Alter>,
    pub(crate) options: Table<OptionRow>,
    pub(crate) envs: Table<Env>,
    pub(crate) node_sets: Table<NodeSet>,
    pub(crate) saves: Table<Save>,
    pub(crate) prints: Table<Print>,
    pub(crate) measures: Table<Measure>,
    // detail tables, owned by the circuit and run owners
    pub(crate) circuit: CircuitDetails,
    pub(crate) run: RunDetails,
}

/// A row type that is an item: it names its kind and its table. Implemented here for every
/// [`ItemKind`]'s row, and nowhere else.
pub trait ItemRow: Copy + 'static {
    /// The kind whose table holds rows of this type.
    const KIND: ItemKind;

    /// The table of this kind.
    fn table(design: &Design) -> &Table<Self>;

    /// The table of this kind, mutable. Reachable only through `&mut Design`, which readers and
    /// writers never hold (they get `Builder` and `&Design`).
    fn table_mut(design: &mut Design) -> &mut Table<Self>;
}

macro_rules! item_rows {
    ($($row:ty => $kind:ident, $field:ident;)*) => {
        $(
            impl ItemRow for $row {
                const KIND: ItemKind = ItemKind::$kind;
                fn table(design: &Design) -> &Table<Self> {
                    &design.$field
                }
                fn table_mut(design: &mut Design) -> &mut Table<Self> {
                    &mut design.$field
                }
            }
            // every item row has its facts (A2): the kernel's dispatch over `ItemKind` relies on it
            const _: () = {
                fn has_facts<R: RowFacts>() {}
                let _ = has_facts::<$row>;
            };
        )*
    };
}

item_rows! {
    Title => Title, titles;
    Comment => Comment, comments;
    Include => Include, includes;
    Global => Global, globals;
    Load => Load, loads;
    Opaque => Opaque, opaques;
    Subckt => Subckt, subckts;
    Section => Section, sections;
    If => If, ifs;
    Param => Param, params;
    Func => Func, funcs;
    Model => Model, models;
    Statistics => Statistics, statistics;
    Element => Element, elements;
    Analysis => Analysis, analyses;
    Loop => Loop, loops;
    Alter => Alter, alters;
    OptionRow => Option, options;
    Env => Env, envs;
    NodeSet => NodeSet, node_sets;
    Save => Save, saves;
    Print => Print, prints;
    Measure => Measure, measures;
}

impl Design {
    /// An empty design: no file, the name `0` interned as [`NameId::GROUND`].
    pub fn new() -> Design {
        todo!()
    }

    /// The append-only door for readers and builders.
    pub fn builder(&mut self) -> Builder<'_> {
        todo!()
    }

    /// The door for passes: replace rows and item lists, append through a builder.
    pub fn editor(&mut self) -> Editor<'_> {
        todo!()
    }

    /// The file a run starts from: the first `Deck` file, else file 0. Asserts the design has a
    /// file.
    pub fn entry(&self) -> FileId {
        todo!()
    }

    /// Every file, in id order.
    pub fn files(&self) -> impl Iterator<Item = FileId> + use<> {
        // A real body: `todo!()` does not satisfy an `impl Iterator` return.
        (0..self.files.len() as u16).map(FileId)
    }

    /// The row of `file`. Asserts it is in range.
    pub fn file(&self, file: FileId) -> File {
        todo!()
    }

    /// The row of `scope`. Asserts it is in range.
    pub fn scope(&self, scope: ScopeId) -> Scope {
        todo!()
    }

    /// The items of `scope`, in order (order is meaning: card order is matrix and result order).
    pub fn items(&self, scope: ScopeId) -> &[ItemRef] {
        todo!()
    }

    /// The row at `index` of `R`'s table. Asserts `index` is in range.
    pub fn row<R: ItemRow>(&self, index: u32) -> R {
        todo!()
    }

    /// The span of any item (one exhaustive dispatch over its kind).
    pub fn span_of(&self, item: ItemRef) -> Span {
        todo!()
    }

    /// The bytes of a name. Asserts `name` is not `NONE`.
    pub fn str(&self, name: NameId) -> &str {
        todo!()
    }

    /// The id of `text` if it is an interned name, else `NONE`. Never interns.
    pub fn find_name(&self, text: &str) -> NameId {
        todo!()
    }

    /// The names of a list.
    pub fn names(&self, list: NameList) -> &[NameId] {
        todo!()
    }

    /// The expressions of a list.
    pub fn exprs(&self, list: ExprList) -> &[ExprId] {
        todo!()
    }

    /// The `(name, value)` pairs of a list.
    pub fn params(&self, list: ParamList) -> &[(NameId, ExprId)] {
        todo!()
    }

    /// The branches of an `If`.
    pub fn branches(&self, list: BranchList) -> &[Branch] {
        todo!()
    }

    /// The bytes of a text (`Opaque`, `Comment`, `Title`).
    pub fn text(&self, text: TextId) -> &str {
        todo!()
    }

    /// The expression row `e`, decoded. Asserts `e` is not `NONE`.
    pub fn expr(&self, e: ExprId) -> Expr {
        todo!()
    }

    /// Expression row count; ids `0..expr_count()` are valid.
    pub fn expr_count(&self) -> usize {
        todo!()
    }

    /// Every definition, in definition order (a kit's model of what it defines).
    pub fn definitions(&self) -> &[Def] {
        todo!()
    }

    /// The row of `def`. Asserts it is in range.
    pub fn def(&self, def: DefId) -> Def {
        todo!()
    }

    /// Structural equality: every table, pool, list and id equal, spans ignored. The `≅` of law
    /// L5 before its case rule.
    pub fn eq_ignoring_spans(&self, other: &Design) -> bool {
        todo!()
    }
}

impl Default for Design {
    fn default() -> Self {
        Self::new()
    }
}
