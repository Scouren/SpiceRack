//! A [`Block`] → its elements with every branch's body inlined, as
//! [`Visit`]s in the order a writer writes them; and [`Cond`]'s drop. Both
//! walk the nesting on a heap stack (rule 7 of the compiler-construction
//! skill), so a deck nested 10⁴ deep in `.if`s is written and dropped at
//! a constant call depth.
//!
//! Cites: nothing; the order is [`Block::elements`]'s, each conditional's
//! branches in turn.
//!
//! Contents:
//! - `pub(in crate::syntax::render) enum Visit<'b>`, `struct Walk<'b>`
//! - `Block::walk(&self) -> Walk<'_>`, `Walk::prune(&mut self)`

use super::{Block, Comp, Cond, Elem};

/// One step of a [`Walk`].
#[derive(Clone, Copy)]
pub(in crate::syntax::render) enum Visit<'b> {
    /// An element of the block.
    Comp(&'b Block, Comp),
    /// An `X` card of the block.
    Inst(&'b Block, u32),
    /// A conditional of the block opens; [`Walk::prune`] leaves it out.
    Cond(&'b Block, &'b Cond),
    /// The conditional's branch at this index opens: its body's steps
    /// follow, then its [`Visit::EndBlock`].
    Branch(&'b Cond, usize),
    /// The conditional's last branch is done.
    EndCond,
    /// The block's elements are done (its kept text is the writer's next).
    EndBlock(&'b Block),
}

/// The steps of a block, its own [`Visit::EndBlock`] last.
pub(in crate::syntax::render) struct Walk<'b> {
    stack: Vec<Frame<'b>>,
}

/// A block and the elements left to step, or a conditional and its next
/// branch.
enum Frame<'b> {
    Block(&'b Block, std::vec::IntoIter<Elem>),
    Cond(&'b Cond, usize),
}

impl Block {
    /// The block's steps, depth first: each element in [`Block::elements`]
    /// order, each conditional as its branches' bodies in turn.
    pub(in crate::syntax::render) fn walk(&self) -> Walk<'_> {
        Walk {
            stack: vec![Frame::Block(self, self.elements().into_iter())],
        }
    }
}

impl Walk<'_> {
    /// Leaves out the conditional the last step opened.
    ///
    /// # Panics
    /// When the last step was no [`Visit::Cond`].
    pub(in crate::syntax::render) fn prune(&mut self) {
        let top = self.stack.pop();
        assert!(
            matches!(top, Some(Frame::Cond(_, 0))),
            "prune follows a Visit::Cond"
        );
    }
}

impl<'b> Iterator for Walk<'b> {
    type Item = Visit<'b>;

    fn next(&mut self) -> Option<Visit<'b>> {
        let step = match self.stack.last_mut()? {
            Frame::Block(b, elems) => match elems.next() {
                Some(Elem::Comp(c)) => Visit::Comp(b, c),
                Some(Elem::Inst(n)) => Visit::Inst(b, n),
                Some(Elem::Cond(k)) => {
                    let c = &b.conds[k].1;
                    assert!(!c.branches.is_empty(), "a conditional has a branch");
                    let b = *b;
                    self.stack.push(Frame::Cond(c, 0));
                    Visit::Cond(b, c)
                }
                None => {
                    let b = *b;
                    self.stack.pop();
                    Visit::EndBlock(b)
                }
            },
            Frame::Cond(c, next) => {
                let c = *c;
                match c.branches.get(*next) {
                    Some((_, body)) => {
                        let i = *next;
                        *next += 1;
                        self.stack
                            .push(Frame::Block(body, body.elements().into_iter()));
                        Visit::Branch(c, i)
                    }
                    None => {
                        self.stack.pop();
                        Visit::EndCond
                    }
                }
            }
        };
        Some(step)
    }
}

impl Drop for Cond {
    /// Drops the branches inside on a heap stack: each is emptied of its
    /// conditionals before it drops, so no drop recurses.
    fn drop(&mut self) {
        let mut work: Vec<Block> = self.branches.drain(..).map(|(_, b)| b).collect();
        while let Some(mut b) = work.pop() {
            for (_, mut c) in b.conds.drain(..) {
                work.extend(c.branches.drain(..).map(|(_, b)| b));
            }
        }
    }
}

// ---- private ----

#[cfg(test)]
mod tests {
    use super::*;

    /// A block holding one element, then a two-branch conditional whose
    /// first branch holds a conditional of one empty branch.
    fn nested() -> Block {
        let leaf = Block::default();
        let inner = Cond {
            branches: vec![(7, leaf)],
        };
        let first = Block {
            conds: vec![(0, inner)],
            ..Block::default()
        };
        let outer = Cond {
            branches: vec![(5, first), (6, Block::default())],
        };
        Block {
            comps: vec![Comp::Card(1)],
            conds: vec![(1, outer)],
            ..Block::default()
        }
    }

    #[test]
    fn a_walk_steps_each_branch_body_in_place_and_prune_leaves_one_out() {
        // The order the recursive writers had: element, then the chain's
        // branches in turn, each body before the next head. Checks: 2.
        let b = nested();
        let names = |w: Walk| -> Vec<&str> {
            w.map(|s| match s {
                Visit::Comp(..) => "comp",
                Visit::Inst(..) => "inst",
                Visit::Cond(..) => "cond",
                Visit::Branch(_, 0) => "branch0",
                Visit::Branch(..) => "branch",
                Visit::EndCond => "endcond",
                Visit::EndBlock(_) => "endblock",
            })
            .collect()
        };
        assert_eq!(
            names(b.walk()),
            [
                "comp", "cond", "branch0", "cond", "branch0", "endblock", "endcond", "endblock",
                "branch", "endblock", "endcond", "endblock"
            ]
        );
        let mut w = b.walk();
        let mut kept = Vec::new();
        while let Some(s) = w.next() {
            if let Visit::Cond(..) = s {
                w.prune();
            }
            kept.push(matches!(s, Visit::EndBlock(_)));
        }
        assert_eq!(kept, [false, false, true]);
    }
}
