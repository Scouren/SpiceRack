//! The element header half of the circuit generator (`circuit_gen.rs`'s child module): its
//! terminals and its master, a definition the kind may name (a builtin model only by its own
//! kind, AMEND-A12 C5) or a fresh unresolved name, in the order `dump` writes them.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator); `rewrite/AMEND-A12.md` C5; `circuit_gen.rs`.
//!
//! Contents: `R::terminals`, `R::reference`, `R::master`, `R::has_visible`, `R::element_names`,
//! `R::element_name`, `R::bound_subckt`, `R::fresh_names`, `R::fitting_model`, `fn takes` (`pub(super)`)

use super::{BUILTINS, NONE_REF, R};
use crate::circuit_check::{bound_family, builtin_fits, builtin_kind};
use crate::circuit_extra::CircuitExtra as X;
use crate::element::ElemKind;
use crate::id::{DefId, List, NameId, NameList, Ref};
use crate::item::{DefKind, ItemKind};
use crate::model::{Model, ModelFamily};

/// Whether an element of `kind` takes a model of `family` (the generator's data, R48 d: a
/// family whose own docs name the kind; the IR itself admits any non-builtin family, C5).
pub(super) fn takes(kind: ElemKind, family: ModelFamily) -> bool {
    use ModelFamily as F;
    if let Some(k) = builtin_kind(family) {
        return k == kind;
    }
    match kind {
        ElemKind::Resistor => matches!(family, F::Resistor | F::ResistorMultiplier),
        ElemKind::Capacitor => matches!(family, F::Capacitor | F::CapacitorMultiplier),
        ElemKind::Inductor => matches!(family, F::Inductor | F::InductorMultiplier),
        ElemKind::Coupling => family == F::CoreJilesAtherton,
        ElemKind::Diode => matches!(
            family,
            F::Diode
                | F::DiodeLevel3
                | F::DiodePspice
                | F::DiodePspiceTemp
                | F::DiodeLtspice
                | F::DiodeIdeal
                | F::DiodeSpectre
                | F::Juncap200
                | F::DiodeCmc
        ),
        ElemKind::Bjt => matches!(
            family,
            F::GummelPoon | F::Vbic | F::Hicum0 | F::Hicum2 | F::Mextram | F::HbtUcsd
        ),
        ElemKind::Mosfet => matches!(
            family,
            F::Mos0
                | F::Mos1
                | F::Mos2
                | F::Mos3
                | F::Mos6
                | F::Mos9
                | F::Bsim1
                | F::Bsim2
                | F::Bsim3
                | F::Bsim4
                | F::BsimSoi
                | F::B3SoiFd
                | F::B3SoiDd
                | F::B3SoiPd
                | F::B4Soi
                | F::Soi3
                | F::Ekv
                | F::Hisim1
                | F::Hisim2
                | F::HisimHv
                | F::Psp103
                | F::BsimBulk
                | F::Vdmos
                | F::VdmosRpi
        ),
        ElemKind::Jfet => matches!(
            family,
            F::JfetShichman | F::JfetParkerSkellern | F::JfetShockley | F::JfetHspice2
        ),
        ElemKind::Mesfet => matches!(family, F::Mesfet | F::Gaas | F::Mesa | F::Hfet1 | F::Hfet2),
        ElemKind::Igbt => family == F::Igbt,
        ElemKind::VSwitch => matches!(
            family,
            F::SwitchHyst | F::SwitchHystSmooth | F::SwitchLogCubic | F::Relay
        ),
        ElemKind::ISwitch => matches!(
            family,
            F::CurrentSwitchHyst | F::CurrentSwitchHystSmooth | F::CurrentSwitchLogCubic
        ),
        ElemKind::LossyLine => family == F::Ltra,
        ElemKind::RcLine => family == F::Urc,
        ElemKind::SingleLossyLine => family == F::Txl,
        ElemKind::CoupledLines => matches!(family, F::Cpl | F::HspiceW),
        ElemKind::CodeModel => family == F::CodeModel,
        ElemKind::ModuleModel => matches!(family, F::Module | F::Other),
        _ => false,
    }
}

impl R<'_, '_> {
    pub(super) fn terminals(
        &mut self,
        kind: ElemKind,
        two: bool,
        ports: Option<usize>,
    ) -> NameList {
        let (lo, hi) = kind.terminals();
        let count = match kind {
            // R61/A36: an instance of a subcircuit has its ports' count
            _ if ports.is_some() => ports.unwrap_or(0) as u32,
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
            | ElemKind::Ccvs
            | ElemKind::LosslessLine => {
                if !self.has(X::BuiltinMaster) || bare && !self.g.chance(4) {
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
                let mutual = self.has(X::BuiltinMaster) && self.has_visible(ModelFamily::Mutual);
                // without a mutual model and with no bare element allowed, `can` left a core
                let core = self.has(X::CouplingCore) && (!mutual && !bare || self.g.chance(2));
                if core {
                    self.fitting_reference(kind, DefKind::Model)
                } else if mutual && (!bare || self.g.chance(2)) {
                    self.visible_model(ModelFamily::Mutual)
                        .expect("`has_visible` found a mutual model")
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
    /// else a fresh unresolved name. A model is one of a family the kind takes (`takes`), seven
    /// times in eight when one is visible (R48 d: an absent kit stays legal, a small share).
    fn fitting_reference(&mut self, kind: ElemKind, dk: DefKind) -> Ref {
        if dk == DefKind::Model {
            return match self.fitting_model(kind) {
                Some(r) if self.g.below(8) != 0 => r,
                _ => Ref::unresolved(self.g.fresh_name(self.b)),
            };
        }
        let r = self.reference(dk);
        match bound_family(self.b.design(), r) {
            Some(f)
                if !builtin_fits(kind, f)
                    || builtin_kind(f).is_some() && !self.has(X::BuiltinMaster) =>
            {
                Ref::unresolved(self.g.fresh_name(self.b))
            }
            _ => r,
        }
    }

    /// A visible model of a family `kind` takes (`takes`; a builtin one only with
    /// `BuiltinMaster`), drawn as `Gen::pick_def` draws, or none found.
    pub(super) fn fitting_model(&mut self, kind: ElemKind) -> Option<Ref> {
        let design = self.b.design();
        let def = self.g.pick_model_for(design, self.scope, kind);
        let builtin = bound_family(
            design,
            Ref {
                name: NameId::NONE,
                def,
            },
        )
        .and_then(builtin_kind);
        if def.is_none() || builtin.is_some() && !self.has(X::BuiltinMaster) {
            return None;
        }
        let design = self.b.design();
        Some(Ref {
            name: design.def(def).name,
            def,
        })
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

impl R<'_, '_> {
    /// `count` element names, each half the time an element already listed in this scope (A32:
    /// real references for the rename properties) and otherwise fresh; distinct.
    pub(super) fn element_names(&mut self, count: u32) -> NameList {
        let mut ids: Vec<NameId> = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name = match self.existing_element() {
                Some(n) if !ids.contains(&n) => n,
                _ => self.g.fresh_name(self.b),
            };
            ids.push(name);
        }
        if ids.is_empty() {
            List::EMPTY
        } else {
            self.b.name_ids(&ids)
        }
    }

    /// Half the time, the name of an element listed in this scope: up to four draws of a random
    /// item of the scope, O(1) whatever its size.
    fn existing_element(&mut self) -> Option<NameId> {
        if !self.g.chance(2) {
            return None;
        }
        let items = self.b.design().items(self.scope);
        if items.is_empty() {
            return None;
        }
        // bounded: four draws
        for _ in 0..4 {
            let item = items[self.g.below(items.len() as u32) as usize];
            if item.kind == ItemKind::Element {
                return Some(
                    self.b
                        .design()
                        .row::<crate::element::Element>(item.index)
                        .name,
                );
            }
        }
        None
    }

    /// The name of an element of this scope half the time, else a fresh one.
    pub(super) fn element_name(&mut self) -> NameId {
        match self.existing_element() {
            Some(n) => n,
            None => self.g.fresh_name(self.b),
        }
    }

    /// A visible subcircuit an instance is bound to, or none (no name is interned).
    pub(super) fn bound_subckt(&mut self) -> Option<Ref> {
        let def = self
            .g
            .pick_def(self.b.design(), self.scope, DefKind::Subckt);
        (!def.is_none()).then(|| Ref {
            name: self.b.design().def(def).name,
            def,
        })
    }

    pub(super) fn fresh_names(&mut self, count: u32) -> NameList {
        let ids: Vec<NameId> = (0..count).map(|_| self.g.fresh_name(self.b)).collect();
        if ids.is_empty() {
            List::EMPTY
        } else {
            self.b.name_ids(&ids)
        }
    }
}
