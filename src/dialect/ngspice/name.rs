//! ngspice names → the one spelling the IR gets. ngspice
//! folds case (it lowercases a deck before reading it), so `VDD` and `vdd` are one name: the
//! reader hands the builder the first spelling it saw (REWRITE-PLAN §3.1 principle 7). A node
//! named `gnd` is node `0` (ngspice replaces the whole word on every card: `inpcom.c`
//! `inp_fix_gnd_name`). An element's IR name is its whole name, letter included (R23).
//!
//! In: names as a deck spells them. Out: the folded spelling.
//!
//! Cites: `rewrite/INV-semantics.md` 3.02, 11.01, 11.03, 11.11; ngspice-45 `inpcom.c`
//! (`inp_casefix`, `inp_fix_gnd_name`).
//!
//! Contents: `pub(super) struct Folder`, `Folder::fold(&mut self, &str) -> String`,
//! `Folder::node(&mut self, &str) -> String`, `pub(super) fn key(&str) -> String`

use std::collections::HashMap;

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
}
