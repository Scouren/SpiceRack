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
//!   `ItemRow::table_mut`), `pub trait DetailRow` (`DetailRow::rows`, `DetailRow::rows_mut`)
//! - `Design::new() -> Design`, `Design::builder(&mut self) -> Builder<'_>`,
//!   `Design::editor(&mut self) -> Editor<'_>`, `Design::entry(&self) -> FileId`,
//!   `Design::files(&self) -> impl Iterator<Item = FileId>`, `Design::file(&self, FileId) -> File`,
//!   `Design::scope(&self, ScopeId) -> Scope`, `Design::items(&self, ScopeId) -> &[ItemRef]`,
//!   `Design::row<R: ItemRow>(&self, u32) -> R`, `Design::span_of(&self, ItemRef) -> Span`,
//!   `Design::str(&self, NameId) -> &str`, `Design::find_name(&self, &str) -> NameId`,
//!   `Design::names(&self, NameList) -> &[NameId]`, `Design::exprs(&self, ExprList) -> &[ExprId]`,
//!   `Design::params(&self, ParamList) -> &[(NameId, ExprId)]`,
//!   `Design::branches(&self, BranchList) -> &[Branch]`, `Design::text(&self, TextId) -> &str`,
//!   `Design::detail<D: DetailRow>(&self, u32) -> D`,
//!   `Design::expr(&self, ExprId) -> Expr`, `Design::num(&self, NumId) -> Literal`,
//!   `Design::expr_count(&self) -> usize`,
//!   `Design::definitions(&self) -> &[Def]`, `Design::def(&self, DefId) -> Def`,
//!   `Design::eq_ignoring_spans(&self, &Design) -> bool`,
//!   `Design::eq_meaning(&self, &Design) -> bool`, `Design::children(&self, ExprId, &mut Vec<ExprId>)`
//!   (crate)

use std::collections::HashMap;

use crate::analysis::{Analysis, RunDetails};
use crate::build::Builder;
use crate::edit::Editor;
use crate::element::{CircuitDetails, Element};
use crate::expr::{
    BinOp, Dist, DrawPer, DrawWhen, Expr, ExprTag, Exprs, Func as Builtin, Literal, UnOp,
};
use crate::facts::RowFacts;
use crate::id::{
    BranchList, DefId, ExprId, ExprList, FileId, ItemRef, List, NameId, NameList, NumId, ParamList,
    Ref, ScopeId, TextId,
};
use crate::item::{
    Branch, Comment, DefKind, Func, Global, If, Include, ItemKind, Load, Opaque, Param, Role,
    Section, Subckt, Title,
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

/// A row of an owner's table that an item header or another row indexes (an element's or an
/// analysis's detail, a waveform, a side row). Implemented by the owner of the table, in the
/// row's owner file, reaching `design.circuit.<field>` or `design.run.<field>`; the one generic
/// door behind `Design::detail`, `Builder::detail` and `Editor::set_detail` (A6). Detail rows
/// have no spans: a detail's span is its header's.
pub trait DetailRow: Copy + 'static {
    /// The table of this row type.
    fn rows(design: &Design) -> &[Self];

    /// The table of this row type, mutable. Reachable only through `&mut Design`, which
    /// readers and writers never hold (they get `Builder` and `&Design`).
    fn rows_mut(design: &mut Design) -> &mut Vec<Self>;
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
        Design {
            names: Names::new(),
            texts: Texts::new(),
            files: Vec::new(),
            scopes: Vec::new(),
            scope_items: Vec::new(),
            defs: Vec::new(),
            def_index: HashMap::new(),
            exprs: Exprs::default(),
            name_pool: Vec::new(),
            expr_pool: Vec::new(),
            param_pool: Vec::new(),
            branch_pool: Vec::new(),
            titles: Table::new(),
            comments: Table::new(),
            includes: Table::new(),
            globals: Table::new(),
            loads: Table::new(),
            opaques: Table::new(),
            subckts: Table::new(),
            sections: Table::new(),
            ifs: Table::new(),
            params: Table::new(),
            funcs: Table::new(),
            models: Table::new(),
            statistics: Table::new(),
            elements: Table::new(),
            analyses: Table::new(),
            loops: Table::new(),
            alters: Table::new(),
            options: Table::new(),
            envs: Table::new(),
            node_sets: Table::new(),
            saves: Table::new(),
            prints: Table::new(),
            measures: Table::new(),
            circuit: CircuitDetails::default(),
            run: RunDetails::default(),
        }
    }

    /// The append-only door for readers and builders.
    pub fn builder(&mut self) -> Builder<'_> {
        Builder { design: self }
    }

    /// The door for passes: replace rows and item lists, append through a builder.
    pub fn editor(&mut self) -> Editor<'_> {
        Editor { design: self }
    }

    /// The file a run starts from: the first `Deck` file, else file 0. Asserts the design has a
    /// file.
    pub fn entry(&self) -> FileId {
        assert!(!self.files.is_empty(), "a design with a file has an entry");
        let deck = self.files.iter().position(|f| f.role == Role::Deck);
        FileId(deck.unwrap_or(0) as u16)
    }

    /// Every file, in id order.
    pub fn files(&self) -> impl Iterator<Item = FileId> + use<> {
        // A real body: `todo!()` does not satisfy an `impl Iterator` return.
        (0..self.files.len() as u16).map(FileId)
    }

    /// The row of `file`. Asserts it is in range.
    pub fn file(&self, file: FileId) -> File {
        assert!(file.index() < self.files.len(), "a file id in range");
        self.files[file.index()]
    }

    /// The row of `scope`. Asserts it is in range.
    pub fn scope(&self, scope: ScopeId) -> Scope {
        assert!(scope.index() < self.scopes.len(), "a scope id in range");
        self.scopes[scope.index()]
    }

    /// The items of `scope`, in order (order is meaning: card order is matrix and result order).
    pub fn items(&self, scope: ScopeId) -> &[ItemRef] {
        assert!(
            scope.index() < self.scope_items.len(),
            "a scope id in range"
        );
        &self.scope_items[scope.index()]
    }

    /// The row at `index` of `R`'s table. Asserts `index` is in range.
    pub fn row<R: ItemRow>(&self, index: u32) -> R {
        R::table(self).get(index)
    }

    /// The detail row at `index` of `D`'s table. Asserts `index` is in range. The owners' typed
    /// accessors (`design.passive(&e)`, …) add the header-kind check on top of this.
    pub fn detail<D: DetailRow>(&self, index: u32) -> D {
        let rows = D::rows(self);
        assert!((index as usize) < rows.len(), "a detail index in range");
        rows[index as usize]
    }

    /// The span of any item (one exhaustive dispatch over its kind).
    pub fn span_of(&self, item: ItemRef) -> Span {
        for_kind!(item.kind, R => R::table(self).span(item.index))
    }

    /// The bytes of a name. Asserts `name` is not `NONE`.
    pub fn str(&self, name: NameId) -> &str {
        self.names.str(name)
    }

    /// The id of `text` if it is an interned name, else `NONE`. Never interns.
    pub fn find_name(&self, text: &str) -> NameId {
        self.names.get(text)
    }

    /// The names of a list.
    pub fn names(&self, list: NameList) -> &[NameId] {
        slice(&self.name_pool, list.start, list.len)
    }

    /// The expressions of a list.
    pub fn exprs(&self, list: ExprList) -> &[ExprId] {
        slice(&self.expr_pool, list.start, list.len)
    }

    /// The `(name, value)` pairs of a list.
    pub fn params(&self, list: ParamList) -> &[(NameId, ExprId)] {
        slice(&self.param_pool, list.start, list.len)
    }

    /// The branches of an `If`.
    pub fn branches(&self, list: BranchList) -> &[Branch] {
        slice(&self.branch_pool, list.start, list.len)
    }

    /// The bytes of a text (`Opaque`, `Comment`, `Title`).
    pub fn text(&self, text: TextId) -> &str {
        self.texts.str(text)
    }

    /// The expression row `e`, decoded. Asserts `e` is not `NONE`.
    pub fn expr(&self, e: ExprId) -> Expr {
        let i = e.index();
        let x = &self.exprs;
        assert!(i < x.tags.len(), "an expression id in range");
        let (sub, a, b) = (x.subs[i] as usize, x.a[i], x.b[i]);
        let list = List::range(a, b);
        match x.tags[i] {
            ExprTag::Num => Expr::Num(NumId(a)),
            ExprTag::Str => Expr::Str(NameId(a)),
            ExprTag::Param => Expr::Param(Ref {
                name: NameId(a),
                def: DefId(b),
            }),
            ExprTag::Call => Expr::Call(Builtin::ALL[sub], list),
            ExprTag::UserCall => {
                let call = x.calls[a as usize];
                Expr::UserCall(call.func, call.args)
            }
            ExprTag::Unary => Expr::Unary(UnOp::ALL[sub], ExprId(a)),
            ExprTag::Binary => Expr::Binary(BinOp::ALL[sub], ExprId(a), ExprId(b)),
            ExprTag::Ternary => {
                let pair = slice(&self.expr_pool, b, 2);
                Expr::Ternary(ExprId(a), pair[0], pair[1])
            }
            ExprTag::V => Expr::V(NameId(a), NameId(b)),
            ExprTag::I => Expr::I(NameId(a)),
            ExprTag::Time => Expr::Time,
            ExprTag::Temp => Expr::Temp,
            ExprTag::Freq => Expr::Freq,
            ExprTag::TStep => Expr::TStep,
            ExprTag::TStop => Expr::TStop,
            ExprTag::Tnom => Expr::Tnom,
            ExprTag::Draw => Expr::Draw(
                Dist::ALL[sub & 0xF],
                DrawWhen::ALL[(sub >> 4) & 1],
                DrawPer::ALL[(sub >> 5) & 3],
                list,
            ),
            ExprTag::List => Expr::List(list),
        }
    }

    /// The literal of a `Num` expression. Asserts `num` is in range.
    pub fn num(&self, num: NumId) -> Literal {
        assert!(num.index() < self.exprs.nums.len(), "a literal id in range");
        self.exprs.nums[num.index()]
    }

    /// Expression row count; ids `0..expr_count()` are valid.
    pub fn expr_count(&self) -> usize {
        self.exprs.tags.len()
    }

    /// Every definition, in definition order (a kit's model of what it defines).
    pub fn definitions(&self) -> &[Def] {
        &self.defs
    }

    /// The row of `def`. Asserts it is in range.
    pub fn def(&self, def: DefId) -> Def {
        assert!(def.index() < self.defs.len(), "a definition id in range");
        self.defs[def.index()]
    }

    /// Structural equality: every table, pool, list and id equal, literals field by field
    /// (`1k` ≠ `1e3`), spans ignored. The `≅` of law L5 before its case rule.
    pub fn eq_ignoring_spans(&self, other: &Design) -> bool {
        self.same(other, |a, b| a == b)
    }

    /// As [`Design::eq_ignoring_spans`], but two literals are equal when their exact decimal
    /// values are (`Literal::eq_value`: `1k` = `1e3`). For the differential harness
    /// (REWRITE-PLAN §7.3), which compares meanings across dialects.
    pub fn eq_meaning(&self, other: &Design) -> bool {
        self.same(other, Literal::eq_value)
    }
}

impl Design {
    /// Appends the child ids of `e` to `out` (operands, arguments, list elements), in order.
    /// Total over any table contents: reads the raw columns and skips a list or a pair that does
    /// not lie in its pool (`check` reports those), so the kernel's walks never panic on a
    /// broken design. Does not check that a child is below `e`; the walks do.
    pub(crate) fn children(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let x = &self.exprs;
        let i = e.index();
        if i >= x.tags.len() {
            return;
        }
        let (a, b) = (x.a[i], x.b[i]);
        let pool = &self.expr_pool;
        let list = |start: u32, len: u32, out: &mut Vec<ExprId>| {
            let end = start as usize + len as usize;
            if end <= pool.len() {
                out.extend_from_slice(&pool[start as usize..end]);
            }
        };
        match x.tags[i] {
            ExprTag::Call | ExprTag::Draw | ExprTag::List => list(a, b, out),
            ExprTag::UserCall => {
                if let Some(call) = x.calls.get(a as usize) {
                    list(call.args.start, call.args.len, out);
                }
            }
            ExprTag::Unary => out.push(ExprId(a)),
            ExprTag::Binary => out.extend([ExprId(a), ExprId(b)]),
            ExprTag::Ternary => {
                out.push(ExprId(a));
                list(b, 2, out);
            }
            ExprTag::Num
            | ExprTag::Str
            | ExprTag::Param
            | ExprTag::V
            | ExprTag::I
            | ExprTag::Time
            | ExprTag::Temp
            | ExprTag::Freq
            | ExprTag::TStep
            | ExprTag::TStop
            | ExprTag::Tnom => {}
        }
    }
}

impl Default for Design {
    fn default() -> Self {
        Self::new()
    }
}

// ---- private ----

impl Design {
    /// Every table equal, spans ignored, literals compared by `num_eq`.
    fn same(&self, o: &Design, num_eq: fn(Literal, Literal) -> bool) -> bool {
        let (x, y) = (&self.exprs, &o.exprs);
        let nums =
            x.nums.len() == y.nums.len() && x.nums.iter().zip(&y.nums).all(|(a, b)| num_eq(*a, *b));
        let structure = self.names.bytes == o.names.bytes
            && self.names.ends == o.names.ends
            && self.texts == o.texts
            && self.files == o.files
            && self.scopes == o.scopes
            && self.scope_items == o.scope_items
            && self.defs == o.defs;
        let exprs =
            x.tags == y.tags && x.subs == y.subs && x.a == y.a && x.b == y.b && x.calls == y.calls;
        let pools = self.name_pool == o.name_pool
            && self.expr_pool == o.expr_pool
            && self.param_pool == o.param_pool
            && self.branch_pool == o.branch_pool;
        let details = self.circuit == o.circuit && self.run == o.run;
        nums && structure && exprs && pools && details && self.same_rows(o)
    }

    /// Every item table's rows equal (not their spans).
    fn same_rows(&self, o: &Design) -> bool {
        ItemKind::ALL
            .iter()
            .all(|&kind| for_kind!(kind, R => R::table(self).rows() == R::table(o).rows()))
    }
}

/// `pool[start..start + len]`, asserting the range lies in the pool.
fn slice<T>(pool: &[T], start: u32, len: u32) -> &[T] {
    let (start, end) = (start as usize, start as usize + len as usize);
    assert!(end <= pool.len(), "a list's range lies in its pool");
    &pool[start..end]
}
