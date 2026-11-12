//! ngspice names → the one spelling the IR gets, and an element kind → its letter. ngspice
//! folds case (it lowercases a deck before reading it), so `VDD` and `vdd` are one name: the
//! reader hands the builder the first spelling it saw (REWRITE-PLAN §3.1 principle 7). A node
//! named `gnd` is node `0` (ngspice replaces the whole word on every card: `inpcom.c`
//! `inp_fix_gnd_name`). IR element names carry no letter (IR-KERNEL §6): the writer puts the
//! kind's letter back.
//!
//! In: names as a deck spells them; an `ElemKind`. Out: the folded spelling; the letter.
//!
//! Cites: `rewrite/INV-semantics.md` 3.02, 11.01, 11.03, 11.11; ngspice-45 `inpcom.c`
//! (`inp_casefix`, `inp_fix_gnd_name`); ngspice manual §3.1 (element letters).
//!
//! Contents: `pub(super) struct Folder`, `Folder::fold(&mut self, &str) -> String`,
//! `Folder::node(&mut self, &str) -> String`, `pub(super) fn key(&str) -> String`,
//! `pub(super) const fn letter(ir::ElemKind) -> Option<u8>`

use std::collections::HashMap;

use crate::ir;

/// The first spelling of each name, by its folded key. One per read: names fold across every
/// file of a kit.
#[derive(Debug, Default)]
pub(super) struct Folder {
    first: HashMap<String, String>,
}

impl Folder {
    /// The spelling `name` gets: the first one seen of its case-folded key. Asserts `name` is
    /// non-empty.
    pub(super) fn fold(&mut self, name: &str) -> String {
        assert!(!name.is_empty(), "Folder::fold: a name");
        let spelling = self
            .first
            .entry(key(name))
            .or_insert_with(|| name.to_string());
        debug_assert!(spelling.eq_ignore_ascii_case(name), "one key, one name");
        spelling.clone()
    }

    /// A node's spelling: `gnd` (any case) is `0`, else [`Folder::fold`].
    pub(super) fn node(&mut self, name: &str) -> String {
        if name.eq_ignore_ascii_case("gnd") {
            return "0".to_string();
        }
        self.fold(name)
    }
}

/// The case-folded key ngspice compares a name by (ASCII lowercase, as `inp_casefix`).
pub(super) fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// The letter ngspice reads `kind` from (manual §3.1), or `None` where the letter depends on the
/// detail row (the controlled-source families: `E`/`G`/`F`/`H`/`B` by the output and law) or
/// ngspice has no such element. The S3.1 writer owns the detail-dependent choice.
#[cfg_attr(not(test), expect(dead_code, reason = "S3.1's writers call it"))]
pub(super) const fn letter(kind: ir::ElemKind) -> Option<u8> {
    use ir::ElemKind as K;
    Some(match kind {
        K::Resistor => b'R',
        K::Capacitor | K::ChargeCapacitor => b'C',
        K::Inductor => b'L',
        K::Coupling => b'K',
        K::VSource => b'V',
        K::ISource => b'I',
        K::Vcvs => b'E',
        K::Vccs => b'G',
        K::Cccs => b'F',
        K::Ccvs => b'H',
        K::Diode => b'D',
        K::Bjt => b'Q',
        K::Mosfet => b'M',
        K::Jfet => b'J',
        K::Mesfet => b'Z',
        K::VSwitch => b'S',
        K::ISwitch => b'W',
        K::LosslessLine => b'T',
        K::LossyLine => b'O',
        K::RcLine => b'U',
        K::SingleLossyLine => b'Y',
        K::CoupledLines => b'P',
        K::CodeModel => b'A',
        K::ModuleModel | K::Module => b'N',
        K::Instance | K::Unresolved => b'X',
        K::Poly | K::Table | K::Behav | K::Laplace | K::FreqTable | K::Delay => return None,
        K::FluxInductor | K::Port | K::Igbt => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_spelling_wins_and_gnd_is_ground() {
        // INV 3.02/11.11 (ngspice folds case), 11.01 (gnd is 0 on ngspice, run P5-086).
        let mut f = Folder::default();
        assert_eq!(f.fold("VDD"), "VDD");
        assert_eq!(f.fold("vdd"), "VDD");
        assert_eq!(f.node("Vdd"), "VDD");
        assert_eq!(f.node("GND"), "0");
        assert_eq!(f.node("gnd2"), "gnd2");
    }

    #[test]
    fn letters_are_the_manuals() {
        // ngspice manual §3.1: R C L K V I E G F H D Q M J Z S W T O U Y P A N X.
        assert_eq!(letter(ir::ElemKind::Mosfet), Some(b'M'));
        assert_eq!(letter(ir::ElemKind::Instance), Some(b'X'));
        assert_eq!(letter(ir::ElemKind::Behav), None);
    }
}
