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
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, ExprTag, Func, UnOp};
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
}

// ---- private ----

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
                .filter(|t| *t != ExprTag::Param || self.param_reads)
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
        let readable =
            !def.is_none() && (self.dynamic || self.class_of(def) == ExprClass::Constant);
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
                if has(member) && ((dynamic && self.reads == Reads::Circuit) || !f.reads_circuit())
                {
                    nodes.push((Node::Call(f), f.arity().0 as usize));
                }
            }
        }
        if has(Feature::Expr(ExprTag::UserCall)) {
            let def = self.pick_def(b.design(), self.scope, DefKind::Func);
            let def =
                if !def.is_none() && (self.dynamic || self.class_of(def) == ExprClass::Constant) {
                    def
                } else {
                    DefId::NONE
                };
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
        let chosen = nodes.swap_remove(k);
        let own = match &chosen.0 {
            Node::Call(f) if f.reads_circuit() => ExprClass::Circuit,
            Node::User(_, def) if !def.is_none() => self.class_of(*def),
            _ => ExprClass::Constant,
        };
        self.class = self.class.max(own);
        Some(chosen)
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
