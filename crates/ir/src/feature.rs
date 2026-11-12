//! Features: everything a target may be unable to spell, as one closed enum, and the fixed bitset
//! each writer declares (`const FEATURES: FeatureSet`). A design in, the set of features it uses
//! out (`features_used`, one scan); legalization compares that set with the target's.
//!
//! Every kind is a feature (`Item`, `Elem`, `Wave`, `Analysis`, `Expr`, `Func`, `Binary`,
//! `Unary`, `Dist`, a draw's `DrawWhen` and `DrawPer`, each portable `Option` key, A7, and each model `Family`, A15 K5); a
//! function or operator used inside an expression that holds a `V`, `I`, `Time` or `Freq` leaf is its `*Dyn` feature instead (Addendum A1: legality is per context);
//! `Extra` (kernel), `CircuitExtra` (`circuit_extra.rs`) and `RunExtra` (`run_extra.rs`) list
//! the features that are fields, not kinds, each in its owner's file (A7). Feature indices are dense, in that
//! order, so a set is `Feature::COUNT` bits.
//!
//! Cites: `rewrite/IR-KERNEL.md` §8, Addenda A1, A2, A7; `REWRITE-PLAN.md` §3.5, §4.4.
//!
//! Contents:
//! - `pub enum Feature`, `Feature::COUNT`, `Feature::index(self) -> u16`,
//!   `Feature::from_index(u16) -> Feature`
//! - `pub enum Extra`, `Extra::ALL`, `Extra::COUNT`
//! - `pub struct FeatureSet`, `FeatureSet::EMPTY`, `FeatureSet::ALL`, `FeatureSet::with(self, Feature) -> FeatureSet`,
//!   `FeatureSet::union(self, FeatureSet) -> FeatureSet`,
//!   `FeatureSet::minus(self, FeatureSet) -> FeatureSet`,
//!   `FeatureSet::contains(&self, Feature) -> bool`, `FeatureSet::is_empty(&self) -> bool`,
//!   `FeatureSet::iter(&self) -> impl Iterator<Item = Feature>`
//! - `pub fn features_used(&Design) -> FeatureSet`
//! - crate: `pub(crate) fn design_level(&Design, &mut FeatureSet)`, `pub(crate) fn item_features(&Design, ItemRef, ScopeId, &mut ExprWalk, &mut FeatureSet)`,
//!   `pub(crate) fn case_clash(&Design) -> bool`, `pub(crate) struct ExprWalk` (`ExprWalk::new`,
//!   `ExprWalk::tree`), `pub(crate) fn expr_roots(&Design, &[Handle]) -> Vec<ExprId>`

use crate::analysis::AnalysisKind;
use crate::circuit_extra::CircuitExtra;
use crate::class::{ExprClass, def_classes, node_class};
use crate::design::Design;
use crate::element::ElemKind;
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, Expr, ExprTag, Func, NumForm, UnOp};
use crate::facts::{Handle, RowFacts};
use crate::id::{DefId, ExprId, ItemRef, NameId, ScopeId};
use crate::item::{ItemKind, Role, ScopeKind};
use crate::model::ModelFamily;
use crate::option_key::OptionKey;
use crate::run_extra::RunExtra;
use crate::wave::WaveKind;

/// A feature: one kind, one context-qualified operator, or one field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    /// An item kind.
    Item(ItemKind),
    /// An element kind.
    Elem(ElemKind),
    /// A waveform kind.
    Wave(WaveKind),
    /// An analysis kind.
    Analysis(AnalysisKind),
    /// An expression tag (`UserCall`, `Str`, `List`, the `TStep`/`TStop`/`Tnom` leaves, …).
    Expr(ExprTag),
    /// A function, in an expression that does not read the circuit.
    Func(Func),
    /// A function, in an expression that reads the circuit.
    FuncDyn(Func),
    /// A binary operator, in an expression that does not read the circuit.
    Binary(BinOp),
    /// A binary operator, in an expression that reads the circuit.
    BinaryDyn(BinOp),
    /// A unary operator, in an expression that does not read the circuit.
    Unary(UnOp),
    /// A unary operator, in an expression that reads the circuit.
    UnaryDyn(UnOp),
    /// A statistical draw's distribution.
    Dist(Dist),
    /// When a draw samples (Addendum A2).
    DrawWhen(DrawWhen),
    /// How often a draw samples (Addendum A2).
    DrawPer(DrawPer),
    /// A kernel field-level feature.
    Extra(Extra),
    /// A circuit owner's field-level feature (`circuit_extra.rs`).
    Circuit(CircuitExtra),
    /// A run owner's field-level feature (`run_extra.rs`).
    Run(RunExtra),
    /// An option key (A7). `OptionKey::Private` is one feature: a writer that has it spells
    /// only its own dialect's private options, and that dialect check is the writer's, not the
    /// feature set's.
    Option(OptionKey),
    /// A model of an equation family (AMEND-A12 A15 K5, R32): "this simulator has no such
    /// model" is a feature a target lacks, as `Elem`, `Wave` and `Analysis` are. Set by the
    /// circuit owner's `Model` row.
    Family(ModelFamily),
    /// An item of the kind inside a scope of the kind at any depth: some scope of that kind is
    /// on the item's scope chain (A15 K1, R27). A subcircuit in an `If` branch in a subcircuit
    /// is `Placed(Branch, Subckt)` and `Placed(Subckt, Subckt)`. `File` is never the scope kind:
    /// every target lets an item stand in a file's top.
    Placed(ScopeKind, ItemKind),
}

/// The kernel's features that are fields or placements, not kinds. The circuit and run owners'
/// are `CircuitExtra` and `RunExtra`, in their own files (A7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Extra {
    /// A `Subckt` inside a subcircuit body.
    NestedSubckt,
    /// A `Model` inside a subcircuit body.
    LocalModel,
    /// A `Func` inside a subcircuit body.
    LocalFunc,
    /// A `Param` inside an `If` branch.
    ParamInBranch,
    /// A `Load` inside an `If` branch.
    LoadInBranch,
    /// An `Include` of one section (`section != NONE`): a library call.
    IncludeSection,
    /// A `Section` wrapped in a library (`library != NONE`).
    SectionLibrary,
    /// Two names of the design that differ only in ASCII case: a case-folding target would
    /// merge them.
    CaseSensitiveNames,
    /// A literal of form `Int` anywhere in the design (A12 K1: typed arithmetic).
    IntLiteral,
    /// An item holds a reference bound to a definition in the body of a subcircuit enclosing
    /// the item's own subcircuit: neither its own subcircuit nor a file's top (A12 K2: VACASK
    /// resolves in the own scope, then the top, never an enclosing subcircuit).
    EnclosingRef,
    /// A `Subckt` head default holds a `Param` read (A12 K2: VACASK lets an instance override
    /// only a formal whose default folds to a constant).
    FormalReadsParam,
    /// A file of role `Deck` whose top scope holds no `Title` item (A12 K2 addendum: ngspice
    /// and VACASK read a deck's line 1 as its title whatever it holds, so an absent title and an
    /// empty one are the same text there).
    DeckWithoutTitle,
    /// A `Subckt` head parameter with no default (A15 K2: an instance must pass it).
    FormalWithoutDefault,
    /// An `If` condition of class `Temp` or `Circuit` (A15 K2: a netlist chosen by a value no
    /// constant fold decides).
    IfConditionDynamic,
}

/// A fixed set of features: one bit per `Feature::index`. `Copy`, built in `const` context
/// (`FeatureSet::EMPTY.with(f).with(g)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FeatureSet {
    words: [u64; WORDS],
}

/// A walker over expression trees that visits each row once per walk (shared subtrees are not
/// re-walked: a DAG's paths can be exponential) and descends only to lower ids that exist, so it
/// terminates on any table contents. Explicit stack (C11).
pub(crate) struct ExprWalk {
    stamp: Vec<u32>,
    epoch: u32,
    nodes: Vec<ExprId>,
    stack: Vec<ExprId>,
    kids: Vec<ExprId>,
}

const WORDS: usize = (Feature::COUNT as usize).div_ceil(64);
/// The member count of each `Feature` group, in declaration order.
const COUNTS: [usize; 20] = [
    ItemKind::COUNT,
    ElemKind::COUNT,
    WaveKind::COUNT,
    AnalysisKind::COUNT,
    ExprTag::COUNT,
    Func::COUNT,
    Func::COUNT,
    BinOp::COUNT,
    BinOp::COUNT,
    UnOp::COUNT,
    UnOp::COUNT,
    Dist::COUNT,
    DrawWhen::COUNT,
    DrawPer::COUNT,
    Extra::COUNT,
    CircuitExtra::COUNT,
    RunExtra::COUNT,
    OptionKey::COUNT,
    ModelFamily::COUNT,
    PLACED,
];
/// The `Placed` group: every scope kind but `File`, by every item kind.
const PLACED: usize = (ScopeKind::COUNT - 1) * ItemKind::COUNT;

impl Feature {
    /// Feature count; indices are `0..COUNT`.
    pub const COUNT: u16 = (ItemKind::COUNT
        + ElemKind::COUNT
        + WaveKind::COUNT
        + AnalysisKind::COUNT
        + ExprTag::COUNT
        + 2 * Func::COUNT
        + 2 * BinOp::COUNT
        + 2 * UnOp::COUNT
        + Dist::COUNT
        + DrawWhen::COUNT
        + DrawPer::COUNT
        + Extra::COUNT
        + CircuitExtra::COUNT
        + RunExtra::COUNT
        + OptionKey::COUNT
        + ModelFamily::COUNT
        + PLACED) as u16;

    /// The dense index: the variant's offset (the counts of the variants before it, in
    /// declaration order) plus the member's position in its `ALL`.
    pub const fn index(self) -> u16 {
        let (group, member) = match self {
            Feature::Item(m) => (0, m as usize),
            Feature::Elem(m) => (1, m as usize),
            Feature::Wave(m) => (2, m as usize),
            Feature::Analysis(m) => (3, m as usize),
            Feature::Expr(m) => (4, m as usize),
            Feature::Func(m) => (5, m as usize),
            Feature::FuncDyn(m) => (6, m as usize),
            Feature::Binary(m) => (7, m as usize),
            Feature::BinaryDyn(m) => (8, m as usize),
            Feature::Unary(m) => (9, m as usize),
            Feature::UnaryDyn(m) => (10, m as usize),
            Feature::Dist(m) => (11, m as usize),
            Feature::DrawWhen(m) => (12, m as usize),
            Feature::DrawPer(m) => (13, m as usize),
            Feature::Extra(m) => (14, m as usize),
            Feature::Circuit(m) => (15, m as usize),
            Feature::Run(m) => (16, m as usize),
            Feature::Option(m) => (17, m as usize),
            Feature::Family(m) => (18, m as usize),
            Feature::Placed(s, k) => {
                assert!(
                    !matches!(s, ScopeKind::File),
                    "a file's top is no placement"
                );
                (19, (s as usize - 1) * ItemKind::COUNT + k as usize)
            }
        };
        assert!(
            member < COUNTS[group],
            "a member's position lies in its group"
        );
        (offset(group) + member) as u16
    }

    /// The feature at `index`. Asserts `index < COUNT`.
    pub const fn from_index(index: u16) -> Feature {
        assert!(
            index < Feature::COUNT,
            "a feature index below Feature::COUNT"
        );
        let (mut group, mut i) = (0, index as usize);
        while i >= COUNTS[group] {
            i -= COUNTS[group];
            group += 1;
        }
        match group {
            0 => Feature::Item(ItemKind::ALL[i]),
            1 => Feature::Elem(ElemKind::ALL[i]),
            2 => Feature::Wave(WaveKind::ALL[i]),
            3 => Feature::Analysis(AnalysisKind::ALL[i]),
            4 => Feature::Expr(ExprTag::ALL[i]),
            5 => Feature::Func(Func::ALL[i]),
            6 => Feature::FuncDyn(Func::ALL[i]),
            7 => Feature::Binary(BinOp::ALL[i]),
            8 => Feature::BinaryDyn(BinOp::ALL[i]),
            9 => Feature::Unary(UnOp::ALL[i]),
            10 => Feature::UnaryDyn(UnOp::ALL[i]),
            11 => Feature::Dist(Dist::ALL[i]),
            12 => Feature::DrawWhen(DrawWhen::ALL[i]),
            13 => Feature::DrawPer(DrawPer::ALL[i]),
            14 => Feature::Extra(Extra::ALL[i]),
            15 => Feature::Circuit(CircuitExtra::ALL[i]),
            16 => Feature::Run(RunExtra::ALL[i]),
            17 => Feature::Option(OptionKey::ALL[i]),
            18 => Feature::Family(ModelFamily::ALL[i]),
            19 => Feature::Placed(
                ScopeKind::ALL[1 + i / ItemKind::COUNT],
                ItemKind::ALL[i % ItemKind::COUNT],
            ),
            _ => unreachable!(),
        }
    }
}

impl Extra {
    /// Every member, in declaration order.
    pub const ALL: [Extra; Extra::IfConditionDynamic as usize + 1] = [
        Extra::NestedSubckt,
        Extra::LocalModel,
        Extra::LocalFunc,
        Extra::ParamInBranch,
        Extra::LoadInBranch,
        Extra::IncludeSection,
        Extra::SectionLibrary,
        Extra::CaseSensitiveNames,
        Extra::IntLiteral,
        Extra::EnclosingRef,
        Extra::FormalReadsParam,
        Extra::DeckWithoutTitle,
        Extra::FormalWithoutDefault,
        Extra::IfConditionDynamic,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl FeatureSet {
    /// The empty set.
    pub const EMPTY: FeatureSet = FeatureSet { words: [0; WORDS] };

    /// Every feature: all `Feature::COUNT` bits (the IR-text writer's set: IR text spells
    /// everything).
    pub const ALL: FeatureSet = FeatureSet::first(Feature::COUNT as usize);

    /// The set of the first `n` feature indices.
    const fn first(n: usize) -> FeatureSet {
        let mut words = [0u64; WORDS];
        let mut i = 0;
        while i < n {
            words[i / 64] |= 1 << (i % 64);
            i += 1;
        }
        FeatureSet { words }
    }

    /// The placements of each of `kinds` inside a scope of kind `scope`: `Placed(scope, k)` for
    /// every `k`. Asserts `scope` is not `File` (a file's top is no placement).
    pub const fn placed(scope: ScopeKind, kinds: &[ItemKind]) -> FeatureSet {
        let mut set = FeatureSet::EMPTY;
        let mut i = 0;
        while i < kinds.len() {
            set = set.with(Feature::Placed(scope, kinds[i]));
            i += 1;
        }
        set
    }

    /// This set and `f`.
    pub const fn with(self, f: Feature) -> FeatureSet {
        let i = f.index() as usize;
        let mut words = self.words;
        words[i / 64] |= 1 << (i % 64);
        FeatureSet { words }
    }

    /// Both sets.
    pub const fn union(self, other: FeatureSet) -> FeatureSet {
        let mut words = self.words;
        let mut k = 0;
        while k < WORDS {
            words[k] |= other.words[k];
            k += 1;
        }
        FeatureSet { words }
    }

    /// This set without `other`'s members (what a design uses that a target lacks:
    /// `used.minus(target)`).
    pub const fn minus(self, other: FeatureSet) -> FeatureSet {
        let mut words = self.words;
        let mut k = 0;
        while k < WORDS {
            words[k] &= !other.words[k];
            k += 1;
        }
        FeatureSet { words }
    }

    /// Whether `f` is in the set.
    pub const fn contains(&self, f: Feature) -> bool {
        let i = f.index() as usize;
        self.words[i / 64] >> (i % 64) & 1 == 1
    }

    /// Whether the set has no member.
    pub const fn is_empty(&self) -> bool {
        let mut k = 0;
        while k < WORDS {
            if self.words[k] != 0 {
                return false;
            }
            k += 1;
        }
        true
    }

    /// The members, in index order.
    pub fn iter(&self) -> impl Iterator<Item = Feature> + use<> {
        // A real body: `todo!()` does not satisfy an `impl Iterator` return.
        let set = *self;
        (0..Feature::COUNT)
            .filter(move |&i| set.words[i as usize / 64] >> (i % 64) & 1 == 1)
            .map(Feature::from_index)
    }
}

/// Every feature the design uses, in one scan over its reachable items: per item, its
/// `Feature::Item`, its row's `RowFacts::features` (one exhaustive dispatch over `ItemKind`), and
/// the expression features of the expressions in its `RowFacts::handles`; then the features of the
/// whole design: the case clash and the circuit and run owners' `design_features` hooks, each
/// called once.
/// An expression's functions and operators count as `*Dyn` when its class is at least `Temp`
/// (`class::expr_class`: it, or a parameter or function it reads, holds the temperature or reads
/// the circuit; R14, A11).
pub fn features_used(design: &Design) -> FeatureSet {
    let mut set = FeatureSet::EMPTY;
    let mut walk = ExprWalk::new(design);
    for (item, scope) in design.reachable() {
        item_features(design, item, scope, &mut walk, &mut set);
    }
    design_level(design, &mut set);
    set
}

/// The features of the whole design, no item's: the case clash and each owner's
/// `design_features` hook (A9). `check_legal` reports these at `Span::NONE`.
pub(crate) fn design_level(design: &Design, set: &mut FeatureSet) {
    if case_clash(design) {
        *set = set.with(Feature::Extra(Extra::CaseSensitiveNames));
    }
    let untitled = |f| {
        let file = design.file(f);
        file.role == Role::Deck
            && !design
                .items(file.top)
                .iter()
                .any(|i| i.kind == ItemKind::Title)
    };
    if design.files().any(untitled) {
        *set = set.with(Feature::Extra(Extra::DeckWithoutTitle));
    }
    crate::element::design_features(design, set);
    crate::analysis::design_features(design, set);
}

/// The features of one reachable item in `scope`: its kind, its row's own (`RowFacts::features`),
/// its placement (`Extra`s that depend on the enclosing scopes), and its expressions'.
pub(crate) fn item_features(
    design: &Design,
    item: ItemRef,
    scope: ScopeId,
    walk: &mut ExprWalk,
    set: &mut FeatureSet,
) {
    *set = set.with(Feature::Item(item.kind));
    let mut handles = Vec::new();
    for_kind!(item.kind, R => {
        let row: R = design.row(item.index);
        row.features(design, item, scope, set);
        row.handles(design, &mut handles);
    });
    let (in_subckt, in_branch) = enclosing(design, scope);
    let placed = match item.kind {
        ItemKind::Subckt if in_subckt => Some(Extra::NestedSubckt),
        ItemKind::Model if in_subckt => Some(Extra::LocalModel),
        ItemKind::Func if in_subckt => Some(Extra::LocalFunc),
        ItemKind::Param if in_branch => Some(Extra::ParamInBranch),
        ItemKind::Load if in_branch => Some(Extra::LoadInBranch),
        ItemKind::Title
        | ItemKind::Comment
        | ItemKind::Include
        | ItemKind::Global
        | ItemKind::Load
        | ItemKind::Opaque
        | ItemKind::Subckt
        | ItemKind::Section
        | ItemKind::If
        | ItemKind::Param
        | ItemKind::Func
        | ItemKind::Model
        | ItemKind::DeviceLibrary
        | ItemKind::Statistics
        | ItemKind::Element
        | ItemKind::Analysis
        | ItemKind::Loop
        | ItemKind::Alter
        | ItemKind::Option
        | ItemKind::Env
        | ItemKind::NodeSet
        | ItemKind::Save
        | ItemKind::Print
        | ItemKind::Measure
        | ItemKind::Elaborate => None,
    };
    if let Some(extra) = placed {
        *set = set.with(Feature::Extra(extra));
    }
    // A12 K2: a resolved reference (in the row or in its expressions) into an enclosing
    // subcircuit's body
    let mut refs: Vec<DefId> = handles
        .iter()
        .filter_map(|h| match h {
            Handle::Ref(r) => Some(r.def),
            _ => None,
        })
        .collect();
    for root in expr_roots(design, &handles) {
        for &e in walk.tree(design, root) {
            if let Expr::Param(r) | Expr::UserCall(r, _) = design.expr(e) {
                refs.push(r.def);
            }
        }
    }
    let enclosing = refs.iter().any(|&d| {
        d.index_checked(design.defs.len())
            .is_some_and(|k| design.in_enclosing_subckt(scope, design.defs[k].scope))
    });
    if enclosing {
        *set = set.with(Feature::Extra(Extra::EnclosingRef));
    }
    for root in expr_roots(design, &handles) {
        let nodes = walk.tree(design, root);
        // the Dyn features are class >= Temp (R14, A11): transitive through reads
        let table = def_classes(design);
        let dynamic = nodes
            .iter()
            .any(|&e| node_class(design, e, table) >= ExprClass::Temp);
        for &e in nodes {
            *set = set.with(Feature::Expr(design.expr(e).tag()));
            if let Expr::Num(n) = design.expr(e)
                && design.num(n).form == NumForm::Int
            {
                *set = set.with(Feature::Extra(Extra::IntLiteral));
            }
            let f = match design.expr(e) {
                Expr::Call(func, _) if dynamic => Feature::FuncDyn(func),
                Expr::Call(func, _) => Feature::Func(func),
                Expr::Binary(op, ..) if dynamic => Feature::BinaryDyn(op),
                Expr::Binary(op, ..) => Feature::Binary(op),
                Expr::Unary(op, _) if dynamic => Feature::UnaryDyn(op),
                Expr::Unary(op, _) => Feature::Unary(op),
                Expr::Draw(dist, when, per, _) => {
                    *set = set
                        .with(Feature::DrawWhen(when))
                        .with(Feature::DrawPer(per));
                    Feature::Dist(dist)
                }
                Expr::Num(_)
                | Expr::Str(_)
                | Expr::Param(_)
                | Expr::UserCall(..)
                | Expr::Ternary(..)
                | Expr::V(..)
                | Expr::I(_)
                | Expr::Time
                | Expr::Temp
                | Expr::Freq
                | Expr::TStep
                | Expr::TStop
                | Expr::Tnom
                | Expr::List(_) => continue,
            };
            *set = set.with(f);
        }
    }
}

/// Whether two interned names differ only in ASCII case (a case-folding target merges them).
pub(crate) fn case_clash(design: &Design) -> bool {
    let mut seen = std::collections::HashSet::new();
    (0..design.names.len())
        .any(|k| !seen.insert(design.names.str(NameId(k as u32)).to_ascii_lowercase()))
}

impl ExprWalk {
    /// A walker sized for `design`'s expression rows.
    pub(crate) fn new(design: &Design) -> ExprWalk {
        ExprWalk {
            stamp: vec![0; design.expr_count()],
            epoch: 0,
            nodes: Vec::new(),
            stack: Vec::new(),
            kids: Vec::new(),
        }
    }

    /// Every row of the tree under `root` (each once), root first. An out-of-range root yields
    /// nothing.
    pub(crate) fn tree(&mut self, design: &Design, root: ExprId) -> &[ExprId] {
        self.epoch += 1;
        self.nodes.clear();
        self.stack.clear();
        if root.is_none() || root.index() >= self.stamp.len() {
            return &self.nodes;
        }
        self.stack.push(root);
        while let Some(e) = self.stack.pop() {
            if self.stamp[e.index()] == self.epoch {
                continue;
            }
            self.stamp[e.index()] = self.epoch;
            self.nodes.push(e);
            self.kids.clear();
            design.children(e, &mut self.kids);
            self.stack
                .extend(self.kids.iter().copied().filter(|k| *k < e));
        }
        &self.nodes
    }
}

/// The expression roots among a row's handles: value fields, list elements, parameter values.
/// Out-of-range lists are skipped (`check` reports them).
pub(crate) fn expr_roots(design: &Design, handles: &[Handle]) -> Vec<ExprId> {
    let mut roots = Vec::new();
    for h in handles {
        match *h {
            Handle::Expr(e) | Handle::ExprDyn(e) => roots.push(e),
            Handle::Exprs(l) if l.start as usize + l.len() <= design.expr_pool.len() => {
                roots.extend_from_slice(design.exprs(l))
            }
            Handle::Params(l) if l.start as usize + l.len() <= design.param_pool.len() => {
                roots.extend(design.params(l).iter().map(|p| p.1))
            }
            Handle::Exprs(_)
            | Handle::Params(_)
            | Handle::Name(_)
            | Handle::Ref(_)
            | Handle::Names(_)
            | Handle::Scope(_)
            | Handle::Text(_)
            | Handle::Branches(_) => {}
        }
    }
    roots
}

// ---- private ----

/// The index of a group's first feature: the member counts of the groups before it.
const fn offset(group: usize) -> usize {
    let (mut sum, mut g) = (0, 0);
    while g < group {
        sum += COUNTS[g];
        g += 1;
    }
    sum
}

/// Whether `scope` lies inside a subcircuit body, and inside an `If` branch (any ancestor).
fn enclosing(design: &Design, scope: ScopeId) -> (bool, bool) {
    let (mut sub, mut branch, mut at) = (false, false, scope);
    // bounded by the scope count; `check` reports a cycle
    for _ in 0..=design.scopes.len() {
        if at.is_none() || at.index() >= design.scopes.len() {
            break;
        }
        let s = design.scopes[at.index()];
        sub |= s.kind == ScopeKind::Subckt;
        branch |= s.kind == ScopeKind::Branch;
        at = s.parent;
    }
    (sub, branch)
}
