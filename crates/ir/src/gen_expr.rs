//! The generator's expressions: a feature set and a context in, one random expression tree
//! out, built depth first in post-order left to right (a caller's order, so IR text reads it
//! back with the same ids) from an explicit frame stack (C11). A tree's class is decided up
//! front (R14): a dynamic tree starts from a leaf of its class (the temperature, or a probe
//! under `Reads::Circuit`) and may read anything; a constant tree holds neither and reads only
//! constant-class definitions; `Ddt`/`Idt` only in a dynamic tree under `Reads::Circuit`. The
//! class of the tree built is left in `Gen::class`, and rows note it for the definitions they
//! push.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9, A11; `rewrite/RULINGS.md` R14; `crates/ir/src/gen.rs`.
//!
//! Contents: `Gen::expr(&mut self, &mut Builder<'_>, &FeatureSet, Reads) -> ExprId`

use crate::build::Builder;
use crate::class::ExprClass;
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, ExprTag, Func, NumForm, UnOp};
use crate::feature::{Feature, FeatureSet};
use crate::r#gen::{Gen, Reads};
use crate::id::{DefId, ExprId};
use crate::item::DefKind;

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

impl Gen {
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
        // a dynamic tree (class >= Temp, R14) starts from a leaf of its class: the temperature,
        // or under `Reads::Circuit` a probe; a constant tree holds neither and reads only
        // constant-class definitions
        let mut dyn_leaves: Vec<ExprTag> = if reads == Reads::Circuit {
            probes
        } else {
            Vec::new()
        };
        if features.contains(Feature::Expr(ExprTag::Temp)) {
            dyn_leaves.push(ExprTag::Temp);
        }
        let probes = dyn_leaves;
        let dynamic = !self.constant_only && !probes.is_empty() && self.chance(2);
        (self.dynamic, self.reads, self.class) = (dynamic, reads, ExprClass::Constant);
        self.int_literals = features.contains(Feature::Extra(crate::feature::Extra::IntLiteral));
        self.long_literals = features.contains(Feature::Extra(crate::feature::Extra::LongLiteral));
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
                Some((node, _)) if !need_probe => {
                    self.class = self.class.max(self.own_class(&node));
                    build(b, node, Vec::new())
                }
                Some(_) | None => self.leaf(b, features, &probes, dynamic, &mut need_probe),
            };
            // hand the value up, building every frame it completes
            loop {
                let Some(top) = stack.last_mut() else {
                    // the tree's class by the kernel's rule, with this generator's own table
                    let table = &self.def_class;
                    self.class = crate::class::tree_class_with(b.design(), value, &|d| {
                        table.get(d.index()).copied().unwrap_or(ExprClass::Constant)
                    });
                    return value;
                };
                top.kids.push(value);
                if top.kids.len() < top.need {
                    break;
                }
                let frame = stack.pop().expect("the frame just filled");
                self.class = self.class.max(self.own_class(&frame.node));
                value = build(b, frame.node, frame.kids);
            }
        }
    }
}

// ---- private ----

impl Gen {
    /// The class a built node adds by itself: a circuit-reading function, or a user function's
    /// noted class (its arguments add theirs as they are built).
    fn own_class(&self, node: &Node) -> ExprClass {
        match node {
            Node::Call(f) if f.reads_circuit() => ExprClass::Circuit,
            Node::User(_, def) if !def.is_none() => self.class_of(*def),
            _ => ExprClass::Constant,
        }
    }

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
        const CONSTANT: [ExprTag; 6] = [
            ExprTag::Num,
            ExprTag::Str,
            ExprTag::Param,
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
                .filter(|t| !matches!(t, ExprTag::Param | ExprTag::Tnom) || self.param_reads)
                .chain(probes.iter().copied().filter(|_| dynamic))
                .collect();
            leaves[self.below(leaves.len() as u32) as usize]
        };
        let own = match tag {
            ExprTag::Temp => ExprClass::Temp,
            ExprTag::V | ExprTag::I | ExprTag::Time | ExprTag::Freq => ExprClass::Circuit,
            _ => ExprClass::Constant,
        };
        self.class = self.class.max(own);
        match tag {
            ExprTag::Num if self.long_literals && self.chance(16) => {
                // A18: 19 to 24 significant digits, in one of the three written forms
                let n = 19 + self.below(6);
                let digits: String = (0..n)
                    .map(|k| char::from(b'0' + (self.below(9) + u32::from(k == 0)) as u8))
                    .collect();
                let (int, dec) = digits.split_at(self.below(4) as usize);
                let sign = if self.chance(4) { "-" } else { "" };
                let significand = format!("{sign}{}.{dec}", if int.is_empty() { "0" } else { int });
                let (exp10, form) = match self.below(3) {
                    0 => (0, NumForm::Plain),
                    1 => (self.below(41) as i16 - 20, NumForm::Exponent),
                    _ => (-6, NumForm::Scale),
                };
                b.long_literal(&significand, exp10, form)
            }
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

    /// Whether a read of `def` keeps the tree within its class: a constant tree reads only
    /// constant definitions, a dynamic one under `Reads::Constant` none of class `Circuit`
    /// (A15 K3: `ProbeContext` follows the class).
    fn may_read(&self, def: DefId) -> bool {
        let class = self.class_of(def);
        match (self.dynamic, self.reads) {
            (false, _) => class == ExprClass::Constant,
            (true, Reads::Constant) => class <= ExprClass::Temp,
            (true, Reads::Circuit) => true,
        }
    }

    /// A parameter read: a visible definition, a formal of the function being generated, or a
    /// fresh name no definition has.
    fn param_read(&mut self, b: &mut Builder<'_>) -> ExprId {
        let def = self.pick_def(b.design(), self.scope, DefKind::Param);
        let readable = !def.is_none() && self.may_read(def);
        match self.below(3) {
            0 if readable => {
                self.class = self.class.max(self.class_of(def));
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
        // A19: a field that may read the circuit counts the dynamic form whatever the class
        let form = dynamic || self.reads == Reads::Circuit || self.dyn_form;
        // a member is allowed in its form, or in both for a function body (A28)
        let both = self.both_forms;
        let allowed = |stat: Feature, dyn_: Feature| {
            if both {
                has(stat) && has(dyn_)
            } else if form {
                has(dyn_)
            } else {
                has(stat)
            }
        };
        let mut nodes: Vec<(Node, usize)> = Vec::new();
        if has(Feature::Expr(ExprTag::Unary)) {
            for op in UnOp::ALL {
                if allowed(Feature::Unary(op), Feature::UnaryDyn(op)) {
                    nodes.push((Node::Unary(op), 1));
                }
            }
        }
        if has(Feature::Expr(ExprTag::Binary)) {
            for op in BinOp::ALL {
                if allowed(Feature::Binary(op), Feature::BinaryDyn(op)) {
                    nodes.push((Node::Binary(op), 2));
                }
            }
        }
        if has(Feature::Expr(ExprTag::Ternary)) {
            nodes.push((Node::Ternary, 3));
        }
        if has(Feature::Expr(ExprTag::Call)) {
            for f in Func::ALL {
                let member = allowed(Feature::Func(f), Feature::FuncDyn(f));
                if member && ((dynamic && self.reads == Reads::Circuit) || !f.reads_circuit()) {
                    nodes.push((Node::Call(f), f.arity().0 as usize));
                }
            }
        }
        if has(Feature::Expr(ExprTag::UserCall)) {
            let def = self.pick_def(b.design(), self.scope, DefKind::Func);
            let def = if !def.is_none() && self.may_read(def) {
                def
            } else {
                DefId::NONE
            };
            let name = if def.is_none() {
                self.fresh()
            } else {
                b.design().str(b.design().def(def).name).to_string()
            };
            // a bound call takes its function's formals, one argument each (CallArity)
            let bound = (!def.is_none())
                .then(|| b.design().def(def))
                .filter(|d| d.item.kind == crate::item::ItemKind::Func);
            let arity = match bound {
                Some(d) => {
                    let f: crate::item::Func = b.design().row(d.item.index);
                    b.design().names(f.args).len()
                }
                None => self.below(3) as usize,
            };
            nodes.push((Node::User(name, def), arity));
        }
        if has(Feature::Expr(ExprTag::Draw)) {
            let dists: Vec<Dist> = Dist::ALL
                .into_iter()
                .filter(|d| allowed(Feature::Dist(*d), Feature::DistDyn(*d)))
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
        // the node's class is added when it is built, not here: a choice may end in a leaf
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

#[cfg(test)]
mod tests {
    use crate::class::{ExprClass, expr_class};
    use crate::design::Design;
    use crate::feature::FeatureSet;
    use crate::r#gen::{Gen, Reads, Size};
    use crate::id::DefId;
    use crate::item::{DefKind, Role};
    use crate::span::Span;

    #[test]
    fn the_tracked_class_is_the_trees() {
        // `Gen::class` is the generated tree's class (oracle: `expr_class`), with nullary user
        // functions of class Circuit to pick: a node chosen and then abandoned for a leaf adds
        // nothing
        let size = Size {
            items: 40,
            scope_depth: 3,
            expr_depth: 4,
            files: 1,
        };
        for seed in 0..300u64 {
            let mut d = Design::new();
            let mut b = d.builder();
            let (_, top) = b.file("t.sp", Role::Deck);
            let mut g = Gen::new(seed, &FeatureSet::ALL, size);
            g.scope = top;
            for k in 0..3 {
                let v = b.v("out", None);
                b.func(top, &format!("h{k}"), &[], v, Span::NONE);
                let def =
                    b.design()
                        .lookup(top, DefKind::Func, b.design().find_name(&format!("h{k}")));
                g.note(def, ExprClass::Circuit);
            }
            let _ = DefId::NONE;
            for _ in 0..20 {
                for reads in [Reads::Circuit, Reads::Constant] {
                    let e = g.expr(&mut b, &FeatureSet::ALL, reads);
                    assert_eq!(g.class, expr_class(b.design(), e), "seed {seed}, {reads:?}");
                }
            }
        }
    }

    #[test]
    fn a_typed_parameter_read_is_wrapped_or_not_drawn() {
        // A33 (oracle: the tree's own rows): under `typed_params` every `Param` row is the
        // operand of a `ToReal` or `ToInt` call the set allows dynamically (`ToInt` missing:
        // only `ToReal`), with a floor of reads drawn; with neither allowed, no `Param` row
        use crate::expr::{Expr, Func};
        use crate::feature::Feature;
        let size = Size {
            items: 40,
            scope_depth: 3,
            expr_depth: 4,
            files: 1,
        };
        let only_real =
            FeatureSet::ALL.minus(FeatureSet::EMPTY.with(Feature::FuncDyn(Func::ToInt)));
        let neither = only_real.minus(FeatureSet::EMPTY.with(Feature::FuncDyn(Func::ToReal)));
        let mut wrapped_reads = 0;
        for (set, wrapper) in [(only_real, Some(Func::ToReal)), (neither, None)] {
            for seed in 0..200u64 {
                let mut d = Design::new();
                let mut b = d.builder();
                let (_, top) = b.file("t.sp", Role::Deck);
                let mut g = Gen::new(seed, &set, size);
                g.scope = top;
                g.typed_params = true;
                for _ in 0..10 {
                    let e = g.expr(&mut b, &set, Reads::Circuit);
                    let des = b.design();
                    let mut nodes = Vec::new();
                    crate::class::tree_nodes(des, e, &mut nodes);
                    let mut operands = Vec::new();
                    for &n in &nodes {
                        if let Expr::Call(f, _) = des.expr(n)
                            && Some(f) == wrapper
                        {
                            des.children(n, &mut operands);
                        }
                    }
                    for &n in &nodes {
                        if let Expr::Param(_) = des.expr(n) {
                            assert!(operands.contains(&n), "seed {seed}: an untyped read");
                            wrapped_reads += 1;
                        }
                    }
                }
            }
        }
        assert!(
            wrapped_reads >= 50,
            "only {wrapped_reads} typed reads drawn"
        );
    }
}
