//! The design generator `Gen(F)`: a seed, a feature set and a size in, a random design out that
//! passes `check` and uses only features in the set (`features_used(d) ⊆ F`). The input of the
//! dialect laws L4 (writer totality) and L5 (round trip), and of the IR text law
//! `parse(dump(x)) == x`. Built only for tests and under the `gen` feature.
//!
//! Deterministic: the same seed, set and size give the same design (its own PRNG, no
//! dependency). Shrinking is by size: a failing seed is replayed with smaller sizes.
//!
//! Cites: `REWRITE-PLAN.md` §4.2 (`Gen(F)`); `rewrite/IR-KERNEL.md` §9.
//!
//! `generate` picks kinds whose `Feature::Item` is in the set and calls each row's
//! `RowFacts::generate` (one exhaustive dispatch over `ItemKind`) with a [`Gen`], the state every
//! row's generator draws from.
//!
//! Contents: `pub struct Size`, `pub fn generate(u64, &FeatureSet, Size) -> Design`,
//! `pub struct Gen`, `Gen::below(&mut self, u32) -> u32`, `Gen::chance(&mut self, u32) -> bool`,
//! `Gen::literal(&mut self) -> Literal`, `Gen::fresh_name(&mut self, &mut Builder<'_>) -> NameId`,
//! `Gen::expr` (in `gen_expr.rs`),
//! `Gen::pick_def(&mut self, &Design, ScopeId, DefKind) -> DefId`,
//! `Gen::budget(&self) -> u32`, `pub enum Reads`; crate: `Gen::new(u64, &FeatureSet, Size) -> Gen`, `pub(crate) fn placeable(&Design, ScopeId, ItemKind, &FeatureSet) -> bool`, `Gen::fresh(&mut self) -> String`,
//! `Gen::word(&mut self) -> &'static str`, `Gen::line_word(&mut self) -> &'static str`, `Gen::class_of(&self, DefId) -> ExprClass`,
//! `Gen::note(&mut self, DefId, ExprClass)`

use crate::build::Builder;
use crate::class::ExprClass;
use crate::design::Design;
use crate::expr::{ExprTag, Literal, NumForm};
use crate::facts::RowFacts;
use crate::feature::{Extra, Feature, FeatureSet};
use crate::id::{DefId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, ItemKind, Role, ScopeKind};

/// How big a generated design is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Most items over all scopes.
    pub items: u32,
    /// Deepest scope nesting below a file's top (subcircuits, sections, branches, loops).
    pub scope_depth: u8,
    /// Deepest expression tree.
    pub expr_depth: u8,
    /// Most files.
    pub files: u16,
}

/// The generator's state: its PRNG, what is left of the size budget, and a counter for fresh
/// names. Deterministic given the seed.
#[derive(Clone, Debug)]
pub struct Gen {
    /// PRNG state (splitmix64).
    pub(crate) state: u64,
    /// The size being generated to.
    pub(crate) size: Size,
    /// Items still allowed.
    pub(crate) items_left: u32,
    /// Fresh names made so far (`g0`, `g1`, …).
    pub(crate) fresh: u32,
    /// The scope the row being generated goes into (set by `generate` before each row): where
    /// `expr` looks for definitions to read.
    pub(crate) scope: ScopeId,
    /// The formal arguments of the function whose body is being generated (`Func`'s generator
    /// sets them around its `expr` call), which `expr` may read.
    pub(crate) formals: Vec<String>,
    /// The definitions of each kind seen so far, in definition order (`pick_def`'s index,
    /// caught up from the design's table on each call: definitions are only appended).
    pub(crate) by_kind: Vec<Vec<DefId>>,
    /// The dynamic class of each definition generated so far, by `DefId` (`Constant` when not
    /// noted): a constant-class tree reads only constant-class definitions (R14).
    pub(crate) def_class: Vec<ExprClass>,
    /// The class of the tree being generated (after `expr` returns: of the tree it built), and
    /// what that tree may read.
    pub(crate) class: ExprClass,
    pub(crate) dynamic: bool,
    pub(crate) reads: Reads,
    /// Whether `literal` may make an `Int` literal (the set has `Extra::IntLiteral`; set by
    /// `expr` from its feature set).
    pub(crate) int_literals: bool,
    /// Whether a literal may be `Long` (the set has `Extra::LongLiteral`; set by `expr`).
    pub(crate) long_literals: bool,
    /// Whether a read may bind a definition in an enclosing subcircuit's body (the set has
    /// `Extra::EnclosingRef`), and whether the expression being built may read a parameter
    /// (cleared around a head default when the set lacks `Extra::FormalNotConstant`).
    pub(crate) enclosing_ok: bool,
    pub(crate) param_reads: bool,
    /// Whether a fresh name may differ from an earlier one only by ASCII case (the set has
    /// `Extra::CaseSensitiveNames`).
    pub(crate) case_ok: bool,
    /// Whether the expression being built must be of class `Constant` (cleared around an `If`
    /// condition when the set lacks `Extra::IfConditionDynamic`).
    pub(crate) constant_only: bool,
    /// Whether the expression being built lies in a `Handle::ExprDyn` field generated with
    /// `Reads::Constant` (a measure's threshold): its operators take the dynamic form (A19).
    pub(crate) dyn_form: bool,
}

/// Whether a generated expression may read the circuit (`V`, `I`, `Time`, `Freq`, `Ddt`,
/// `Idt`): chosen by the field's handle kind, as `check`'s `ProbeContext` decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Reads {
    /// A field the row lists as `Handle::Expr`: constants, parameters, `Temp` and the A1 leaves
    /// only.
    Constant,
    /// A field the row lists as `Handle::ExprDyn` (a behavioural source, a charge, a table's
    /// input, a switch's control).
    Circuit,
}

/// The powers of ten a `Scale` literal may have (A3).
const SCALE_POWERS: [i16; 12] = [-18, -15, -12, -9, -6, -3, -2, 3, 6, 9, 12, 15];

/// The words generated strings and texts are drawn from: lower case (no case clash), with the
/// characters IR text escapes.
const WORDS: [&str; 8] = [
    "4.8",
    "a",
    "two words",
    "q\"uote",
    "back\\slash",
    "",
    "line\nbreak",
    "x.y",
];

/// A random design under `features`, at most `size`. A feature outside the set never appears;
/// a feature in it may not (the set is an upper bound).
pub fn generate(seed: u64, features: &FeatureSet, size: Size) -> Design {
    let mut design = Design::new();
    let mut g = Gen::new(seed, features, size);
    let mut b = design.builder();
    let files = 1 + g.below(size.files.max(1) as u32);
    for k in 0..files {
        // a deck holds a title unless the set has DeckWithoutTitle (A12 K2 addendum); a set
        // with neither generates libraries only
        let deck_ok = features.contains(Feature::Item(ItemKind::Title))
            || features.contains(Feature::Extra(Extra::DeckWithoutTitle));
        let role = if k == 0 && deck_ok {
            Role::Deck
        } else {
            Role::Library
        };
        let (_, top) = b.file(&format!("f{k}.sp"), role);
        fill(&mut g, &mut b, top, features);
    }
    design
}

impl Gen {
    /// A uniform integer in `0..n`. Asserts `n > 0`.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0, "a range of at least one value");
        // splitmix64
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) % n as u64) as u32
    }

    /// True with probability `1 / n`. Asserts `n > 0`.
    pub fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }

    /// A random valid literal: every `NumForm`, magnitudes `1e-15..1e9`, either sign, zero,
    /// trailing zeros in `frac`, so every writer's number spelling and the literal round trip are
    /// exercised.
    pub fn literal(&mut self) -> Literal {
        if self.int_literals && self.chance(4) {
            let n = self.below(2001) as i64 - 1000;
            return Literal {
                mantissa: n,
                frac: 0,
                exp10: 0,
                form: NumForm::Int,
            };
        }
        let sign = if self.chance(4) { -1 } else { 1 };
        let mantissa = sign * (1 + self.below(999_999)) as i64;
        let frac = self.below(4) as u8;
        let l = match self.below(5) {
            0 => Literal::ZERO,
            1 => Literal {
                mantissa,
                frac,
                exp10: 0,
                form: NumForm::Plain,
            },
            2 => Literal {
                mantissa,
                frac,
                exp10: self.below(41) as i16 - 20,
                form: NumForm::Exponent,
            },
            3 => {
                let exp10 = SCALE_POWERS[self.below(SCALE_POWERS.len() as u32) as usize];
                Literal {
                    mantissa,
                    frac,
                    exp10,
                    form: NumForm::Scale,
                }
            }
            _ => Literal {
                mantissa,
                frac,
                exp10: 0,
                form: NumForm::Mil,
            },
        };
        debug_assert!(l.is_valid(), "a generated literal is valid");
        l
    }

    /// A name never used before in this design (`g` and a counter), interned.
    pub fn fresh_name(&mut self, b: &mut Builder<'_>) -> NameId {
        let name = self.fresh();
        b.name(&name)
    }

    /// An existing definition of `kind` visible from `scope`, chosen at random (up to eight
    /// draws among that kind's definitions), or `NONE` when the draws find none visible.
    pub fn pick_def(&mut self, design: &Design, scope: ScopeId, kind: DefKind) -> DefId {
        let seen: usize = self.by_kind.iter().map(Vec::len).sum();
        for (k, def) in design.definitions().iter().enumerate().skip(seen) {
            self.by_kind[def.kind as usize].push(DefId(k as u32));
        }
        let mut chain = Vec::new();
        let mut at = scope;
        // bounded by the scope count: each step moves to a parent
        for _ in 0..=design.scopes.len() {
            if at.is_none() {
                break;
            }
            chain.push(at);
            at = design.scope(at).parent;
        }
        // a few random draws among this kind's definitions, kept if visible: O(depth) per read
        let n = self.by_kind[kind as usize].len() as u32;
        for _ in 0..if n == 0 { 0 } else { 8 } {
            let k = self.below(n) as usize;
            let def = self.by_kind[kind as usize][k];
            let at = design.def(def).scope;
            let enclosing = !self.enclosing_ok && design.in_enclosing_subckt(scope, at);
            if chain.contains(&at) && !enclosing {
                return def;
            }
        }
        DefId::NONE
    }

    /// A fresh name as text, not interned (a row's generator passes it to the push, which
    /// interns it in a builder caller's order).
    pub(crate) fn fresh(&mut self) -> String {
        self.fresh += 1;
        // the case twin of the previous call's index: each index has at most one, so the name
        // is still unique as written
        if self.case_ok && self.fresh > 1 && self.chance(16) {
            return format!("G{}", self.fresh - 2);
        }
        format!("g{}", self.fresh - 1)
    }

    /// The class noted for a generated definition (`Constant` when none was).
    pub(crate) fn class_of(&self, def: DefId) -> ExprClass {
        self.def_class
            .get(def.index())
            .copied()
            .unwrap_or(ExprClass::Constant)
    }

    /// Notes the class of a definition just pushed.
    pub(crate) fn note(&mut self, def: DefId, class: ExprClass) {
        if self.def_class.len() <= def.index() {
            self.def_class.resize(def.index() + 1, ExprClass::Constant);
        }
        self.def_class[def.index()] = class;
    }

    /// A generator at `seed` for `features` and `size`, with no scope chosen yet (`generate`'s
    /// start; crate tests drive `expr` from it).
    pub(crate) fn new(seed: u64, features: &FeatureSet, size: Size) -> Gen {
        Gen {
            state: seed,
            size,
            items_left: size.items,
            fresh: 0,
            scope: ScopeId::NONE,
            formals: Vec::new(),
            by_kind: vec![Vec::new(); DefKind::COUNT],
            def_class: Vec::new(),
            class: ExprClass::Constant,
            dynamic: false,
            reads: Reads::Constant,
            int_literals: false,
            long_literals: false,
            enclosing_ok: features.contains(Feature::Extra(Extra::EnclosingRef)),
            param_reads: true,
            case_ok: features.contains(Feature::Extra(Extra::CaseSensitiveNames)),
            constant_only: false,
            dyn_form: false,
        }
    }

    /// A word for a string value or a text (`WORDS`).
    pub(crate) fn word(&mut self) -> &'static str {
        WORDS[self.below(WORDS.len() as u32) as usize]
    }

    /// A word with no line break, for a title or a comment (A15 K3).
    pub(crate) fn line_word(&mut self) -> &'static str {
        let lines: Vec<&'static str> = WORDS.into_iter().filter(|w| !w.contains('\n')).collect();
        lines[self.below(lines.len() as u32) as usize]
    }

    /// Items still allowed; a row generator that opens a scope fills its body only while this is
    /// positive.
    pub fn budget(&self) -> u32 {
        self.items_left
    }
}

/// Whether an item of `kind` may be listed in `scope` under `features`: `Placed(S, kind)` is in
/// the set for every scope kind S on `scope`'s chain but a file's top (A15 K1). For a row's own
/// generator that pushes into a body it opened.
pub(crate) fn placeable(d: &Design, scope: ScopeId, kind: ItemKind, features: &FeatureSet) -> bool {
    let chain = crate::feature::chain_kinds(d, scope);
    ScopeKind::ALL.into_iter().all(|s| {
        s == ScopeKind::File || !chain[s as usize] || features.contains(Feature::Placed(s, kind))
    })
}

// ---- private ----

/// Fills `top` and every body its items open, depth first (each body right after its item: the
/// order of `text::dump`), choosing kinds the set and the placement allow.
fn fill(g: &mut Gen, b: &mut Builder<'_>, top: ScopeId, features: &FeatureSet) {
    let deck = b.design().file(b.design().scope(top).file).role == Role::Deck;
    let untitled_ok = features.contains(Feature::Extra(Extra::DeckWithoutTitle));
    let titled = features.contains(Feature::Item(ItemKind::Title))
        && (!untitled_ok || (g.items_left > 0 && g.chance(2)));
    if deck && titled {
        let text = g.line_word();
        b.title(top, text, crate::span::Span::NONE);
        g.items_left = g.items_left.saturating_sub(1); // a required title may pass the budget
    }
    // the top takes what the budget leaves; each body a few items
    let first = g.items_left;
    let mut stack: Vec<(ScopeId, usize, u32)> = vec![(top, 0, first)];
    while let Some((scope, depth, left)) = stack.pop() {
        if left == 0 || g.items_left == 0 {
            continue;
        }
        let kinds = candidates(
            b.design(),
            scope,
            depth,
            g.size.scope_depth as usize,
            features,
        );
        if kinds.is_empty() {
            continue;
        }
        stack.push((scope, depth, left - 1));
        let kind = kinds[g.below(kinds.len() as u32) as usize];
        g.scope = scope;
        let scopes_before = b.design().scopes.len();
        let item: ItemRef = for_kind!(kind, R => R::generate(g, scope, b, features));
        g.items_left -= 1;
        let d = b.design();
        let bodies: Vec<ScopeId> = (scopes_before..d.scopes.len())
            .map(|s| ScopeId(s as u32))
            .filter(|s| d.scope(*s).owner == item)
            .collect();
        for &body in bodies.iter().rev() {
            let n = g.below(4);
            stack.push((body, depth + 1, n));
        }
    }
}

/// The kinds a row in `scope` may have: in the set, allowed where it sits (`placeable`,
/// sections only in a file's top, scopes only below the depth cap), and with what its fields
/// need (literals for any expression). Never `Title` (`fill` places it first in a deck).
fn candidates(
    d: &Design,
    scope: ScopeId,
    depth: usize,
    cap: usize,
    features: &FeatureSet,
) -> Vec<ItemKind> {
    let has = |f| features.contains(f);
    let exprs = has(Feature::Expr(ExprTag::Num));
    let opens = depth < cap;
    ItemKind::ALL
        .into_iter()
        .filter(|&k| has(Feature::Item(k)) && placeable(d, scope, k, features))
        .filter(|&k| match k {
            ItemKind::Title => false,
            ItemKind::Subckt => opens,
            ItemKind::Section => opens,
            ItemKind::If => opens && exprs,
            ItemKind::Loop => opens,
            ItemKind::Func => exprs,
            // A25: a constant local of a subcircuit only when the set has it
            ItemKind::Param => {
                exprs
                    && (d.scope(scope).kind != ScopeKind::Subckt
                        || has(Feature::Extra(Extra::SubcktConstantParam)))
            }
            ItemKind::Comment
            | ItemKind::Load
            | ItemKind::Model
            | ItemKind::Include
            | ItemKind::Global
            | ItemKind::Opaque
            | ItemKind::DeviceLibrary
            | ItemKind::Statistics
            | ItemKind::Element
            | ItemKind::Analysis
            | ItemKind::Alter
            | ItemKind::Option
            | ItemKind::Env
            | ItemKind::NodeSet
            | ItemKind::Save
            | ItemKind::Print
            | ItemKind::Measure
            | ItemKind::Elaborate
            | ItemKind::Reset => true,
        })
        .collect()
}
