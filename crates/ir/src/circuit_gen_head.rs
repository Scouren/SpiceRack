//! The element header half of the circuit generator (`circuit_gen.rs`'s child module): its
//! terminals and its master, a definition the kind may name (a builtin model only by its own
//! kind, AMEND-A12 C5) or a fresh unresolved name, in the order `dump` writes them.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator); `rewrite/AMEND-A12.md` C5; `circuit_gen.rs`.
//!
//! Contents: `R::terminals`, `R::reference`, `R::master`, `R::has_visible` (`pub(super)`)

use super::{BUILTINS, NONE_REF, R};
use crate::circuit_check::{bound_family, builtin_fits};
use crate::circuit_extra::CircuitExtra as X;
use crate::element::ElemKind;
use crate::id::{DefId, List, NameId, NameList, Ref};
use crate::item::DefKind;
use crate::model::{Model, ModelFamily};

impl R<'_, '_> {
    pub(super) fn terminals(&mut self, kind: ElemKind, two: bool) -> NameList {
        let (lo, hi) = kind.terminals();
        let count = match kind {
            ElemKind::Laplace | ElemKind::FreqTable | ElemKind::VSwitch => {
                if two {
                    2
                } else {
                    4
                }
            }
            ElemKind::Diode => 2 + u32::from(self.maybe(X::DiodeThermal)),
            ElemKind::CoupledLines => 4 + 2 * self.g.below(3),
            _ => lo as u32 + self.g.below((hi - lo).min(3) as u32 + 1),
        };
        let ids: Vec<NameId> = (0..count)
            .map(|_| {
                if self.g.chance(4) {
                    NameId::GROUND
                } else {
                    self.g.fresh_name(self.b)
                }
            })
            .collect();
        if ids.is_empty() {
            List::EMPTY
        } else {
            self.b.name_ids(&ids)
        }
    }

    /// An existing definition of `kind` visible here, or a fresh unresolved name.
    pub(super) fn reference(&mut self, kind: DefKind) -> Ref {
        let def = self.g.pick_def(self.b.design(), self.scope, kind);
        if def == DefId::NONE {
            Ref::unresolved(self.g.fresh_name(self.b))
        } else {
            Ref {
                name: self.b.design().def(def).name,
                def,
            }
        }
    }

    pub(super) fn master(&mut self, kind: ElemKind) -> Ref {
        let bare = self.has(X::ElementWithoutModel);
        match kind {
            ElemKind::Unresolved => Ref::unresolved(self.g.fresh_name(self.b)),
            ElemKind::VSource
            | ElemKind::ISource
            | ElemKind::Vcvs
            | ElemKind::Vccs
            | ElemKind::Cccs
            | ElemKind::Ccvs => {
                if bare && !self.g.chance(4) {
                    return NONE_REF;
                }
                let family = BUILTINS
                    .iter()
                    .find(|b| b.1 == kind)
                    .expect("a builtin per kind")
                    .0;
                match self.visible_model(family) {
                    Some(r) => r,
                    None => Ref::unresolved(self.g.fresh_name(self.b)),
                }
            }
            ElemKind::Coupling => {
                if self.maybe(X::CouplingCore) {
                    self.fitting_reference(kind, DefKind::Model)
                } else if !bare {
                    self.visible_model(ModelFamily::Mutual)
                        .expect("`can` found a mutual model")
                } else {
                    NONE_REF
                }
            }
            ElemKind::Resistor | ElemKind::Capacitor | ElemKind::Inductor => {
                if !bare || self.g.chance(4) {
                    self.fitting_reference(kind, DefKind::Model)
                } else {
                    NONE_REF
                }
            }
            _ => match kind.master_kind() {
                Some(dk) if kind.needs_model() => self.fitting_reference(kind, dk),
                _ => NONE_REF,
            },
        }
    }

    /// A visible definition of `dk` that `kind` may name (a builtin model only by its kind),
    /// else a fresh unresolved name.
    fn fitting_reference(&mut self, kind: ElemKind, dk: DefKind) -> Ref {
        let r = self.reference(dk);
        match bound_family(self.b.design(), r) {
            Some(f) if !builtin_fits(kind, f) => Ref::unresolved(self.g.fresh_name(self.b)),
            _ => r,
        }
    }

    /// A model of `family` that lookup from this scope binds, chosen uniformly, or none.
    fn visible_model(&mut self, family: ModelFamily) -> Option<Ref> {
        let design = self.b.design();
        let found: Vec<Ref> = design
            .definitions()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == DefKind::Model)
            .filter(|(_, d)| design.row::<Model>(d.item.index).family == family)
            .map(|(k, d)| Ref {
                name: d.name,
                def: DefId(k as u32),
            })
            .filter(|r| design.lookup(self.scope, DefKind::Model, r.name) == r.def)
            .collect();
        if found.is_empty() {
            return None;
        }
        Some(found[self.g.below(found.len() as u32) as usize])
    }

    /// Whether a model of `family` is visible here (as `visible_model`, without drawing).
    pub(super) fn has_visible(&self, family: ModelFamily) -> bool {
        let design = self.b.design();
        design.definitions().iter().enumerate().any(|(k, d)| {
            d.kind == DefKind::Model
                && design.row::<Model>(d.item.index).family == family
                && design.lookup(self.scope, DefKind::Model, d.name) == DefId(k as u32)
        })
    }
}
