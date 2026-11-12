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
//! `Gen::expr(&mut self, &mut Builder<'_>, &FeatureSet, Reads) -> ExprId`,
//! `Gen::pick_def(&mut self, &Design, ScopeId, DefKind) -> DefId`,
//! `Gen::budget(&self) -> u32`, `pub enum Reads`; crate: `Gen::fresh(&mut self) -> String`,
//! `Gen::word(&mut self) -> &'static str`

use crate::build::Builder;
use crate::design::Design;
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, ExprTag, Func, Literal, NumForm, UnOp};
use crate::facts::RowFacts;
use crate::feature::{Extra, Feature, FeatureSet};
use crate::id::{DefId, ExprId, ItemRef, NameId, ScopeId};
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

/// An operator node waiting for its operands.
enum Node {
    Unary(UnOp),
    Binary(BinOp),
    Ternary,
    Call(Func),
    User(String, DefId),
    Draw(Dist, DrawWhen, DrawPer),
    List,
}

/// One open operator of the expression being generated.
struct Frame {
    node: Node,
    kids: Vec<ExprId>,
    need: usize,
}

/// A random design under `features`, at most `size`. A feature outside the set never appears;
/// a feature in it may not (the set is an upper bound).
pub fn generate(seed: u64, features: &FeatureSet, size: Size) -> Design {
    let mut design = Design::new();
    let mut g = Gen {
        state: seed,
        size,
        items_left: size.items,
        fresh: 0,
        scope: ScopeId::NONE,
        formals: Vec::new(),
        by_kind: vec![Vec::new(); DefKind::COUNT],
    };
    let mut b = design.builder();
    let files = 1 + g.below(size.files.max(1) as u32);
    for k in 0..files {
        let role = if k == 0 { Role::Deck } else { Role::Library };
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

    /// A random expression of depth at most `size.expr_depth`, using only `features` and only
    /// leaves `reads` allows. Parameter reads name definitions visible where it is used, or are
    /// left unresolved.
    pub fn expr(&mut self, b: &mut Builder<'_>, features: &FeatureSet, reads: Reads) -> ExprId {
        assert!(
            features.contains(Feature::Expr(ExprTag::Num)),
            "an expression needs literals to end in"
        );
        let probes: Vec<ExprTag> = [ExprTag::V, ExprTag::I, ExprTag::Time, ExprTag::Freq]
            .into_iter()
            .filter(|t| features.contains(Feature::Expr(*t)))
            .collect();
        let dynamic = reads == Reads::Circuit && !probes.is_empty() && self.chance(2);
        let mut need_probe = dynamic;
        let mut stack: Vec<Frame> = Vec::new();
        // depth-first, building each row after its children, left to right: a caller's order
        loop {
            let depth = stack.len();
            let node = if depth + 1 >= self.size.expr_depth as usize || self.chance(3) {
                None
            } else {
                self.operator(b, features, dynamic)
            };
            let mut value = match node {
                Some((node, need)) if need > 0 => {
                    stack.push(Frame {
                        node,
                        kids: Vec::new(),
                        need,
                    });
                    continue;
                }
                // a tree that still owes its probe takes a leaf where an empty node would end it
                Some((node, _)) if !need_probe => build(b, node, Vec::new()),
                Some(_) | None => self.leaf(b, features, &probes, dynamic, &mut need_probe),
            };
            // hand the value up, building every frame it completes
            loop {
                let Some(top) = stack.last_mut() else {
                    return value;
                };
                top.kids.push(value);
                if top.kids.len() < top.need {
                    break;
                }
                let frame = stack.pop().expect("the frame just filled");
                value = build(b, frame.node, frame.kids);
            }
        }
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
            if chain.contains(&design.def(def).scope) {
                return def;
            }
        }
        DefId::NONE
    }

    /// A fresh name as text, not interned (a row's generator passes it to the push, which
    /// interns it in a builder caller's order).
    pub(crate) fn fresh(&mut self) -> String {
        self.fresh += 1;
        format!("g{}", self.fresh - 1)
    }

    /// A word for a string value or a text (`WORDS`).
    pub(crate) fn word(&mut self) -> &'static str {
        WORDS[self.below(WORDS.len() as u32) as usize]
    }

    /// Items still allowed; a row generator that opens a scope fills its body only while this is
    /// positive.
    pub fn budget(&self) -> u32 {
        self.items_left
    }
}

// ---- private ----

/// Fills `top` and every body its items open, depth first (each body right after its item: the
/// order of `text::dump`), choosing kinds the set and the placement allow.
fn fill(g: &mut Gen, b: &mut Builder<'_>, top: ScopeId, features: &FeatureSet) {
    let deck = b.design().file(b.design().scope(top).file).role == Role::Deck;
    if deck && features.contains(Feature::Item(ItemKind::Title)) && g.items_left > 0 && g.chance(2)
    {
        let text = g.word();
        b.title(top, text, crate::span::Span::NONE);
        g.items_left -= 1;
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

/// The kinds a row in `scope` may have: in the set, allowed where it sits (placement `Extra`s,
/// sections only in a file's top, scopes only below the depth cap), and with what its fields
/// need (literals for any expression). Never `Title` (`fill` places it first in a deck).
fn candidates(
    d: &Design,
    scope: ScopeId,
    depth: usize,
    cap: usize,
    features: &FeatureSet,
) -> Vec<ItemKind> {
    let (mut in_subckt, mut in_branch, mut at) = (false, false, scope);
    for _ in 0..=d.scopes.len() {
        if at.is_none() {
            break;
        }
        let s = d.scope(at);
        in_subckt |= s.kind == ScopeKind::Subckt;
        in_branch |= s.kind == ScopeKind::Branch;
        at = s.parent;
    }
    let has = |f| features.contains(f);
    let file_top = d.scope(scope).kind == ScopeKind::File;
    let exprs = has(Feature::Expr(ExprTag::Num));
    let opens = depth < cap;
    ItemKind::ALL
        .into_iter()
        .filter(|&k| has(Feature::Item(k)))
        .filter(|&k| match k {
            ItemKind::Title => false,
            ItemKind::Subckt => opens && (!in_subckt || has(Feature::Extra(Extra::NestedSubckt))),
            ItemKind::Section => opens && file_top,
            ItemKind::If => opens && exprs,
            ItemKind::Loop => opens,
            ItemKind::Func => exprs && (!in_subckt || has(Feature::Extra(Extra::LocalFunc))),
            ItemKind::Param => exprs && (!in_branch || has(Feature::Extra(Extra::ParamInBranch))),
            ItemKind::Load => !in_branch || has(Feature::Extra(Extra::LoadInBranch)),
            ItemKind::Model => !in_subckt || has(Feature::Extra(Extra::LocalModel)),
            ItemKind::Comment
            | ItemKind::Include
            | ItemKind::Global
            | ItemKind::Opaque
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
            | ItemKind::Elaborate => true,
        })
        .collect()
}

impl Gen {
    /// A leaf the set allows: constants, parameter reads and the constant leaves; in a
    /// circuit-reading tree also probes, its first leaf always one.
    fn leaf(
        &mut self,
        b: &mut Builder<'_>,
        features: &FeatureSet,
        probes: &[ExprTag],
        dynamic: bool,
        need_probe: &mut bool,
    ) -> ExprId {
        const CONSTANT: [ExprTag; 7] = [
            ExprTag::Num,
            ExprTag::Str,
            ExprTag::Param,
            ExprTag::Temp,
            ExprTag::TStep,
            ExprTag::TStop,
            ExprTag::Tnom,
        ];
        let tag = if std::mem::take(need_probe) {
            probes[self.below(probes.len() as u32) as usize]
        } else {
            let leaves: Vec<ExprTag> = CONSTANT
                .into_iter()
                .filter(|t| features.contains(Feature::Expr(*t)))
                .chain(probes.iter().copied().filter(|_| dynamic))
                .collect();
            leaves[self.below(leaves.len() as u32) as usize]
        };
        match tag {
            ExprTag::Num => {
                let l = self.literal();
                b.literal(l)
            }
            ExprTag::Str => {
                let w = self.word();
                b.string(w)
            }
            ExprTag::Param => self.param_read(b),
            ExprTag::Temp => b.temp(),
            ExprTag::TStep => b.tstep(),
            ExprTag::TStop => b.tstop(),
            ExprTag::Tnom => b.tnom(),
            ExprTag::V => {
                let (net, reference) = (self.fresh(), self.fresh());
                b.v(&net, self.chance(2).then_some(reference.as_str()))
            }
            ExprTag::I => {
                let e = self.fresh();
                b.i(&e)
            }
            ExprTag::Time => b.time(),
            ExprTag::Freq => b.freq(),
            ExprTag::Call
            | ExprTag::UserCall
            | ExprTag::Unary
            | ExprTag::Binary
            | ExprTag::Ternary
            | ExprTag::Draw
            | ExprTag::List => unreachable!("not a leaf"),
        }
    }

    /// A parameter read: a visible definition, a formal of the function being generated, or a
    /// fresh name no definition has.
    fn param_read(&mut self, b: &mut Builder<'_>) -> ExprId {
        let def = self.pick_def(b.design(), self.scope, DefKind::Param);
        match self.below(3) {
            0 if !def.is_none() => {
                let name = b.design().str(b.design().def(def).name).to_string();
                let e = b.param_ref(&name);
                b.resolve(e, def);
                e
            }
            1 if !self.formals.is_empty() => {
                let k = self.below(self.formals.len() as u32) as usize;
                let name = self.formals[k].clone();
                b.param_ref(&name)
            }
            _ => {
                let name = self.fresh();
                b.param_ref(&name)
            }
        }
    }

    /// An operator the set allows and the operand count it takes (`None`: none allowed).
    fn operator(
        &mut self,
        b: &Builder<'_>,
        features: &FeatureSet,
        dynamic: bool,
    ) -> Option<(Node, usize)> {
        let has = |f| features.contains(f);
        let mut nodes: Vec<(Node, usize)> = Vec::new();
        if has(Feature::Expr(ExprTag::Unary)) {
            for op in UnOp::ALL {
                if has(if dynamic {
                    Feature::UnaryDyn(op)
                } else {
                    Feature::Unary(op)
                }) {
                    nodes.push((Node::Unary(op), 1));
                }
            }
        }
        if has(Feature::Expr(ExprTag::Binary)) {
            for op in BinOp::ALL {
                if has(if dynamic {
                    Feature::BinaryDyn(op)
                } else {
                    Feature::Binary(op)
                }) {
                    nodes.push((Node::Binary(op), 2));
                }
            }
        }
        if has(Feature::Expr(ExprTag::Ternary)) {
            nodes.push((Node::Ternary, 3));
        }
        if has(Feature::Expr(ExprTag::Call)) {
            for f in Func::ALL {
                let member = if dynamic {
                    Feature::FuncDyn(f)
                } else {
                    Feature::Func(f)
                };
                if has(member) && (dynamic || !f.reads_circuit()) {
                    nodes.push((Node::Call(f), f.arity().0 as usize));
                }
            }
        }
        if has(Feature::Expr(ExprTag::UserCall)) {
            let def = self.pick_def(b.design(), self.scope, DefKind::Func);
            let name = if def.is_none() {
                self.fresh()
            } else {
                b.design().str(b.design().def(def).name).to_string()
            };
            nodes.push((Node::User(name, def), self.below(3) as usize));
        }
        if has(Feature::Expr(ExprTag::Draw)) {
            let dists: Vec<Dist> = Dist::ALL
                .into_iter()
                .filter(|d| has(Feature::Dist(*d)))
                .collect();
            let whens: Vec<DrawWhen> = DrawWhen::ALL
                .into_iter()
                .filter(|w| has(Feature::DrawWhen(*w)))
                .collect();
            let pers: Vec<DrawPer> = DrawPer::ALL
                .into_iter()
                .filter(|p| has(Feature::DrawPer(*p)))
                .collect();
            if !dists.is_empty() && !whens.is_empty() && !pers.is_empty() {
                let d = dists[self.below(dists.len() as u32) as usize];
                let w = whens[self.below(whens.len() as u32) as usize];
                let p = pers[self.below(pers.len() as u32) as usize];
                nodes.push((Node::Draw(d, w, p), d.arity().0 as usize));
            }
        }
        if has(Feature::Expr(ExprTag::List)) {
            nodes.push((Node::List, self.below(4) as usize));
        }
        if nodes.is_empty() {
            return None;
        }
        let k = self.below(nodes.len() as u32) as usize;
        Some(nodes.swap_remove(k))
    }
}

/// Builds an operator row from its operands (in order).
fn build(b: &mut Builder<'_>, node: Node, kids: Vec<ExprId>) -> ExprId {
    match node {
        Node::Unary(op) => b.unary(op, kids[0]),
        Node::Binary(op) => b.binary(op, kids[0], kids[1]),
        Node::Ternary => b.ternary(kids[0], kids[1], kids[2]),
        Node::Call(f) => b.call(f, &kids),
        Node::User(name, def) => {
            let e = b.user_call(&name, &kids);
            if !def.is_none() {
                b.resolve(e, def);
            }
            e
        }
        Node::Draw(d, w, p) => b.draw(d, w, p, &kids),
        Node::List => b.list(&kids),
    }
}
