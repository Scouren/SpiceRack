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
//! - `pub struct FeatureSet`, `FeatureSet::EMPTY`, `FeatureSet::ALL`,
//!   `FeatureSet::placed(ScopeKind, &[ItemKind]) -> FeatureSet`, `FeatureSet::with(self, Feature) -> FeatureSet`,
//!   `FeatureSet::union(self, FeatureSet) -> FeatureSet`,
//!   `FeatureSet::minus(self, FeatureSet) -> FeatureSet`,
//!   `FeatureSet::contains(&self, Feature) -> bool`, `FeatureSet::is_empty(&self) -> bool`,
//!   `FeatureSet::iter(&self) -> impl Iterator<Item = Feature>`
//! - `pub fn features_used(&Design) -> FeatureSet`
//! - crate: `pub(crate) fn design_level(&Design, &mut FeatureSet)`,
//!   `pub(crate) fn unplaced_level(&Design, &mut FeatureSet)`, `pub(crate) fn item_features(&Design, ItemRef, ScopeId, &mut ExprWalk, &mut FeatureSet)`,
//!   `pub(crate) fn chain_kinds(&Design, ScopeId) -> [bool; ScopeKind::COUNT]`, `pub(crate) struct ExprWalk` (`ExprWalk::new`,
//!   `ExprWalk::tree`), `pub(crate) fn expr_roots(&Design, &[Handle]) -> Vec<ExprId>`,
//!   `pub(crate) fn roots_with_use(&Design, &[Handle]) -> Vec<(ExprId, ExprUse)>`; re-exported from
//!   `form`: `pub enum Form`, `pub fn root_form(&Design, ExprId, ExprUse) -> Form`

use crate::analysis::AnalysisKind;
use crate::circuit_extra::CircuitExtra;
use crate::class::def_classes;
use crate::design::Design;
use crate::element::ElemKind;
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, Expr, ExprTag, Func, NumForm, UnOp};
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::form::{DYNAMIC, STATIC, body_forms, form_feature, form_of};
pub use crate::form::{Form, root_form};
use crate::id::{DefId, ExprId, ItemRef, ScopeId};
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
    /// A statistical draw's distribution, in an expression of the dynamic form (A28): its
    /// static twin is `Dist`.
    DistDyn(Dist),
}

/// The kernel's features that are fields or placements, not kinds. The circuit and run owners'
/// are `CircuitExtra` and `RunExtra`, in their own files (A7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Extra {
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
    /// A `Subckt` head default that is not a constant: it holds a `Param` read or `$tnom` (A25,
    /// VACASK run c3), or its class is `Temp` (A12 K2, widened by A15: VACASK lets an instance override only a formal whose
    /// default folds to a constant; it was `FormalReadsParam`).
    FormalNotConstant,
    /// A file of role `Deck` whose top scope holds no `Title` item (A12 K2 addendum: ngspice
    /// and VACASK read a deck's line 1 as its title whatever it holds, so an absent title and an
    /// empty one are the same text there).
    DeckWithoutTitle,
    /// A `Subckt` head parameter with no default (A15 K2: an instance must pass it).
    FormalWithoutDefault,
    /// An `If` condition of class `Temp` or `Circuit` (A15 K2: a netlist chosen by a value no
    /// constant fold decides).
    IfConditionDynamic,
    /// A literal of form `Long` anywhere in the design (A18, R37): its digits outlive the
    /// mantissa. A writer that spells them verbatim lists it; another target gets the pass that
    /// writes the correctly rounded shortest double.
    LongLiteral,
    /// A `Param` listed in a `Subckt` body whose value is a constant (no parameter read, no
    /// `$tnom`, class `Constant`): VACASK reads `parameters x=1` there as a formal, ngspice's
    /// `.param` there is none (A25, R48). A structure pass makes it a formal.
    SubcktConstantParam,
    /// A `Param`'s value, or a `Subckt` head default, whose root is a string (A25 item 4: a
    /// value by type; a string in an element's or model's parameter list is `Expr(Str)` alone).
    StringValue,
    /// A `Param`'s value, or a `Subckt` head default, whose root is a list (A25 item 4).
    ListValue,
    /// A `Subckt` head default holding a read bound to a definition outside the subcircuit's
    /// body: read by the lexical rule (R62, A37). A read of a sibling formal is none.
    FormalOuterRead,
    /// A `Subckt` head default holding an unbound read whose name is no sibling formal's: looked
    /// up from each instance's scope (R62, A37, A29).
    FormalUnboundRead,
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
const COUNTS: [usize; 21] = [
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
    Dist::COUNT,
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
        + PLACED
        + Dist::COUNT) as u16;

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
            Feature::DistDyn(m) => (20, m as usize),
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
            20 => Feature::DistDyn(Dist::ALL[i]),
            _ => unreachable!(),
        }
    }
}

impl Extra {
    /// Every member, in declaration order.
    pub const ALL: [Extra; Extra::FormalUnboundRead as usize + 1] = [
        Extra::IncludeSection,
        Extra::SectionLibrary,
        Extra::CaseSensitiveNames,
        Extra::IntLiteral,
        Extra::EnclosingRef,
        Extra::FormalNotConstant,
        Extra::DeckWithoutTitle,
        Extra::FormalWithoutDefault,
        Extra::IfConditionDynamic,
        Extra::LongLiteral,
        Extra::SubcktConstantParam,
        Extra::StringValue,
        Extra::ListValue,
        Extra::FormalOuterRead,
        Extra::FormalUnboundRead,
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
    // two spellings of one name in one namespace (at the item: `check_legal`)
    if !crate::case::case_clashes(design).is_empty() {
        set = set.with(Feature::Extra(Extra::CaseSensitiveNames));
    }
    design_level(design, &mut set);
    set
}

/// The features of the whole design, no item's: each owner's `design_features` hook (A9).
/// `check_legal` reports the run owner's at their items (`analysis::feature_items`) and the rest
/// (`unplaced_level`) at `Span::NONE`.
pub(crate) fn design_level(design: &Design, set: &mut FeatureSet) {
    unplaced_level(design, set);
    crate::analysis::design_features(design, set);
}

/// The design-level features that name no item: a deck without a title, the circuit owner's
/// hook.
pub(crate) fn unplaced_level(design: &Design, set: &mut FeatureSet) {
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
}

/// The features of one reachable item in `scope`: its kind, its row's own (`RowFacts::features`),
/// its placement (`Placed`, A15 K1), and its expressions'.
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
    // A15 K1: every scope kind on the chain but a file's top
    let chain = chain_kinds(design, scope);
    for s in ScopeKind::ALL {
        if s != ScopeKind::File && chain[s as usize] {
            *set = set.with(Feature::Placed(s, item.kind));
        }
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
    for (root, use_) in roots_with_use(design, &handles) {
        let nodes = walk.tree(design, root);
        // the one form decision (A26): the field's use and the tree's class (R14, A19)
        let class = crate::class::tree_class(design, root, def_classes(design));
        // A28 (R51): a function body counts in its callers' forms, every other root in its own
        let forms = if item.kind == ItemKind::Func {
            let bits = body_forms(design).get(&root).copied().unwrap_or(0);
            [bits & STATIC != 0 || bits == 0, bits & DYNAMIC != 0]
        } else {
            let dynamic = form_of(use_, class) == Form::Dynamic;
            [!dynamic, dynamic]
        };
        for &e in nodes {
            *set = set.with(Feature::Expr(design.expr(e).tag()));
            if let Expr::Num(n) = design.expr(e) {
                match design.num(n).form {
                    NumForm::Int => *set = set.with(Feature::Extra(Extra::IntLiteral)),
                    NumForm::Long => *set = set.with(Feature::Extra(Extra::LongLiteral)),
                    NumForm::Plain | NumForm::Exponent | NumForm::Scale | NumForm::Mil => {}
                }
            }
            if let Expr::Draw(_, when, per, _) = design.expr(e) {
                *set = set
                    .with(Feature::DrawWhen(when))
                    .with(Feature::DrawPer(per));
            }
            for dynamic in [false, true] {
                if let Some(f) = forms[dynamic as usize]
                    .then(|| form_feature(design.expr(e), dynamic))
                    .flatten()
                {
                    *set = set.with(f);
                }
            }
        }
    }
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
    roots_with_use(design, handles)
        .into_iter()
        .map(|(e, _)| e)
        .collect()
}

/// The expression roots `handles` lists with how each field reads, in `RowFacts::map_exprs`'s
/// order (A23): `Circuit` for `ExprDyn`, `Constant` for `Expr`, `Exprs` elements and `Params`
/// values. A list outside its pool lists nothing (`check` reports it).
pub(crate) fn roots_with_use(design: &Design, handles: &[Handle]) -> Vec<(ExprId, ExprUse)> {
    let mut roots = Vec::new();
    for h in handles {
        match *h {
            Handle::Expr(e) => roots.push((e, ExprUse::Constant)),
            Handle::ExprDyn(e) => roots.push((e, ExprUse::Circuit)),
            Handle::Exprs(l) if l.start as usize + l.len() <= design.expr_pool.len() => {
                roots.extend(design.exprs(l).iter().map(|&e| (e, ExprUse::Constant)))
            }
            Handle::Params(l) if l.start as usize + l.len() <= design.param_pool.len() => {
                roots.extend(design.params(l).iter().map(|p| (p.1, ExprUse::Constant)))
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

/// The scope kinds on `scope`'s chain (it and every ancestor), by `ScopeKind` index. Bounded by
/// the scope count (`check` reports a cycle); an out-of-range scope ends the chain.
pub(crate) fn chain_kinds(design: &Design, scope: ScopeId) -> [bool; ScopeKind::COUNT] {
    let mut kinds = [false; ScopeKind::COUNT];
    let mut at = scope;
    for _ in 0..=design.scopes.len() {
        if at.is_none() || at.index() >= design.scopes.len() {
            break;
        }
        let s = design.scopes[at.index()];
        kinds[s.kind as usize] = true;
        at = s.parent;
    }
    kinds
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
