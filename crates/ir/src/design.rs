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
//! ```
//! use spicerack_ir::{Design, Diags, ItemKind, item::Subckt, parse};
//!
//! let mut design = Design::new();
//! let text = "file \"a.sp\" deck\n  subckt div (a b)\n    param k = 1\n";
//! parse(text, &mut design.builder(), &mut Diags::new());
//! let mut ports = 0;
//! for file in design.files() {
//!     for &item in design.items(design.file(file).top) {
//!         // a writer matches every kind with no `_` arm; this example reads one
//!         if item.kind == ItemKind::Subckt {
//!             let s: Subckt = design.row(item.index);
//!             ports += design.names(s.ports).len();
//!         }
//!     }
//! }
//! assert_eq!(ports, 2);
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
//!   `Design::item_handles(&self, ItemRef, &mut Vec<Handle>)`,
//!   `Design::literal_value(&self, Literal) -> f64`, `Design::literal_eq(&self, Literal, Literal) -> bool`,
//!   `Design::long_digits(&self, Literal) -> Option<(&str, NumForm)>`,
//!   `Design::item_refs(&self, ItemRef, &mut Vec<(Ref, DefKind)>)`,
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

use crate::analysis::{Analysis, RunDetails};
use crate::build::Builder;
use crate::class::ExprClass;
use crate::edit::Editor;
use crate::element::{CircuitDetails, Element};
use crate::expr::{
    BinOp, Dist, DrawPer, DrawWhen, Expr, ExprTag, Exprs, Func as Builtin, Literal, NumForm, UnOp,
};
use crate::facts::{Handle, RowFacts};
use crate::id::{
    BranchList, DefId, ExprId, ExprList, FileId, ItemRef, List, NameId, NameList, NumId, ParamList,
    Ref, ScopeId, TextId,
};
use crate::item::{
    Branch, Comment, DefKind, Func, Global, If, Include, ItemKind, Load, Opaque, Param, Section,
    Subckt, Title,
};
use crate::library::DeviceLibrary;
use crate::model::Model;
use crate::names::{FxMap, Names, Texts};
use crate::option::{Env, NodeSet, OptionRow};
use crate::output::{Measure, Print, Save};
use crate::plan::{Alter, Elaborate, Loop};
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
    pub(crate) def_index: FxMap<(ScopeId, DefKind, NameId), DefId>,
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
    pub(crate) device_libraries: Table<DeviceLibrary>,
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
    pub(crate) elaborates: Table<Elaborate>,
    // detail tables, owned by the circuit and run owners
    pub(crate) circuit: CircuitDetails,
    pub(crate) run: RunDetails,
    /// The dynamic class of each definition (`class::def_classes`), computed on demand and
    /// cleared by every `Builder` and `Editor` mutation. Not part of equality.
    pub(crate) classes: std::sync::OnceLock<Vec<ExprClass>>,
    /// The forms each function body counts in (`feature::body_forms`, A28): bit 1 static, bit 2
    /// dynamic, by body expression. Computed on demand and cleared with `classes`.
    pub(crate) forms: std::sync::OnceLock<crate::names::FxMap<ExprId, u8>>,
    /// The first element of each whole name in each scope (`Builder::element_named`), built on
    /// demand, extended by every element push and cleared where `editor` hands out its door and
    /// by each Editor mutation that can move or rename an element. Not part of equality.
    pub(crate) element_names: std::sync::OnceLock<crate::names::FxMap<(ScopeId, NameId), ItemRef>>,
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
    DeviceLibrary => DeviceLibrary, device_libraries;
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
    Elaborate => Elaborate, elaborates;
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
            def_index: FxMap::default(),
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
            device_libraries: Table::new(),
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
            elaborates: Table::new(),
            circuit: CircuitDetails::default(),
            run: RunDetails::default(),
            classes: std::sync::OnceLock::new(),
            forms: std::sync::OnceLock::new(),
            element_names: std::sync::OnceLock::new(),
        }
    }

    /// The append-only door for readers and builders.
    pub fn builder(&mut self) -> Builder<'_> {
        self.derived_changed();
        Builder { design: self }
    }

    /// The door for passes: replace rows and item lists, append through a builder.
    pub fn editor(&mut self) -> Editor<'_> {
        self.derived_changed();
        self.element_names.take();
        Editor { design: self }
    }

    /// The file a run starts from: file 0, whatever its role; `FileId::NONE` for a design with
    /// no file (a caller that needs a file asserts). One rule, no heuristic: a reader creates
    /// the entry first (the `Read` contract), IR text's first `file` block is file 0, and a
    /// builder decides its own order.
    pub fn entry(&self) -> FileId {
        if self.files.is_empty() {
            FileId::NONE
        } else {
            FileId(0)
        }
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

    /// The correctly rounded double of any literal of this design, `Long` included (A18): the
    /// generic caller's `Literal::value`.
    pub fn literal_value(&self, l: Literal) -> f64 {
        match self.long_digits(l) {
            Some(_) => crate::literal::long_value(self.long_text(l), l.exp10),
            None => l.value(),
        }
    }

    /// Whether two literals of this design state the same exact decimal number, `Long` included
    /// (A18): the generic caller's `Literal::eq_value`.
    pub fn literal_eq(&self, a: Literal, b: Literal) -> bool {
        literal_eq_across(self, a, self, b)
    }

    /// A `Long` literal's significand as written and the form its power was written in;
    /// `None` for a short literal. A writer spells the digits, then `exp10` in that form.
    pub fn long_digits(&self, l: Literal) -> Option<(&str, NumForm)> {
        (l.form == NumForm::Long).then(|| (self.long_text(l), NumForm::ALL[l.frac as usize]))
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

    /// Appends every handle `item`'s row and detail rows hold, in field order (one exhaustive
    /// dispatch to `RowFacts::handles`). Asserts `item` is in range.
    pub fn item_handles(&self, item: ItemRef, out: &mut Vec<Handle>) {
        let rows = for_kind!(item.kind, R => R::table(self).len());
        assert!((item.index as usize) < rows, "an item in range");
        for_kind!(item.kind, R => self.row::<R>(item.index).handles(self, out))
    }

    /// Appends every `Ref` `item` owns (header, detail rows, pools) with the kind its field
    /// expects, in `Editor::map_refs`'s order, changing nothing (one exhaustive dispatch over
    /// `ItemKind` to `RowFacts::refs`). Asserts `item` is in range.
    pub fn item_refs(&self, item: ItemRef, out: &mut Vec<(Ref, DefKind)>) {
        let rows = for_kind!(item.kind, R => R::table(self).len());
        assert!((item.index as usize) < rows, "an item in range");
        for_kind!(item.kind, R => self.row::<R>(item.index).refs(self, item, out))
    }

    /// Structural equality: every table, pool, list and id equal, literals field by field
    /// (`1k` ≠ `1e3`, forms included), spans ignored. Exact by design (A15 K4). The `≅` of law
    /// L5 before its case rule.
    pub fn eq_ignoring_spans(&self, other: &Design) -> bool {
        self.exprs.long == other.exprs.long && self.same(other, |_, a, _, b| a == b)
    }

    /// As [`Design::eq_ignoring_spans`], but two real literals are equal when their exact
    /// decimal values are, whatever their forms (`Literal::eq_value`: `1k` = `1e3` = `1000`), and
    /// an `Int` literal equals only an `Int` of the same value (A15 K4, R28). For the
    /// differential harness (REWRITE-PLAN §7.3), which compares meanings across dialects.
    pub fn eq_meaning(&self, other: &Design) -> bool {
        self.same(other, literal_eq_across)
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
    fn same(&self, o: &Design, num_eq: fn(&Design, Literal, &Design, Literal) -> bool) -> bool {
        let (x, y) = (&self.exprs, &o.exprs);
        let nums = x.nums.len() == y.nums.len()
            && x.nums
                .iter()
                .zip(&y.nums)
                .all(|(a, b)| num_eq(self, *a, o, *b));
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

impl Design {
    /// Forgets what is derived from the expressions and definitions (the class and form caches):
    /// every door and every mutation that can change a class calls it.
    pub(crate) fn derived_changed(&mut self) {
        self.classes.take();
        self.forms.take();
    }

    /// A `Long` literal's significand. Asserts the form and the pool index.
    fn long_text(&self, l: Literal) -> &str {
        assert!(l.form == NumForm::Long, "a Long literal");
        &self.exprs.long[l.mantissa as usize]
    }
}

/// Whether `a` of `da` and `b` of `db` state the same exact decimal number: the short rule
/// between short literals; digits and power after stripping zeros when either is `Long` (never
/// the doubles: two distinct decimals may round to one); an `Int` equals only an `Int`.
fn literal_eq_across(da: &Design, a: Literal, db: &Design, b: Literal) -> bool {
    let long = |l: Literal| l.form == NumForm::Long;
    if !long(a) && !long(b) {
        return a.eq_value(b);
    }
    if a.form == NumForm::Int || b.form == NumForm::Int {
        return false;
    }
    let normal = |d: &Design, l: Literal| match long(l) {
        true => crate::literal::long_normal(d.long_text(l), l.exp10),
        false => crate::literal::short_normal(l),
    };
    normal(da, a) == normal(db, b)
}

/// `pool[start..start + len]`, asserting the range lies in the pool.
fn slice<T>(pool: &[T], start: u32, len: u32) -> &[T] {
    let (start, end) = (start as usize, start as usize + len as usize);
    assert!(end <= pool.len(), "a list's range lies in its pool");
    &pool[start..end]
}
