//! Generated circuit rows. Oracles: the checker (tested against hand-broken designs in
//! `circuit_check.rs`): every generated design passes `check` and uses only the requested
//! features; the inverse: `parse(dump(d))` equals `d` (`eq_ignoring_spans`). Per element kind,
//! a set holding only that kind (plus the items and expression features every row needs), and
//! the full set; fixed seeds, printed on failure.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 ("Round trip", "Generator").
//!
//! Contents: tests only.

#![cfg(feature = "gen")]

#[path = "circuit_common.rs"]
mod circuit_common;

#[cfg(test)]
mod tests {

    use super::circuit_common::{codes, full_set};
    use spicerack_ir::r#gen::{Size, generate};
    use spicerack_ir::{
        CircuitExtra, Design, Diags, ElemKind, Feature, FeatureSet, ItemKind, WaveKind, check,
        dump, features_used, parse,
    };

    const SIZE: Size = Size {
        items: 40,
        scope_depth: 2,
        expr_depth: 3,
        files: 1,
    };
    const SEEDS: [u64; 8] = [1, 2, 3, 5, 8, 13, 21, 0x5eed];

    /// Everything but element kinds, wave kinds and circuit extras, plus `kind`.
    fn only_kind(kind: ElemKind) -> FeatureSet {
        let base = full_set();
        let mut set = FeatureSet::EMPTY;
        for f in base.iter() {
            let keep = match f {
                Feature::Elem(k) => k == kind,
                Feature::Wave(w) => w == WaveKind::Pulse,
                Feature::Circuit(_) => false,
                _ => true,
            };
            if keep {
                set = set.with(f);
            }
        }
        set
    }

    fn valid_and_round_trips(seed: u64, set: &FeatureSet) {
        let d = generate(seed, set, SIZE);
        let mut diags = Diags::new();
        check(&d, &mut diags);
        assert!(diags.is_empty(), "seed {seed}: {:?}", codes(&diags));
        let extra = features_used(&d).minus(*set);
        assert!(
            extra.is_empty(),
            "seed {seed}: unrequested {:?}",
            extra.iter().collect::<Vec<_>>()
        );
        let mut text = String::new();
        for file in d.files() {
            dump(&d, file, &mut text);
        }
        let mut back = Design::new();
        let mut pd = Diags::new();
        parse(&text, &mut back.builder(), &mut pd);
        assert!(pd.is_empty(), "seed {seed}: {:?}\n{text}", codes(&pd));
        assert!(back.eq_ignoring_spans(&d), "seed {seed}:\n{text}");
    }

    #[test]
    fn each_element_kind_alone() {
        for kind in ElemKind::ALL {
            let set = only_kind(kind);
            for seed in SEEDS {
                valid_and_round_trips(seed ^ kind as u64, &set);
            }
        }
    }

    #[test]
    fn every_feature_on() {
        let set = full_set();
        for seed in SEEDS {
            valid_and_round_trips(seed, &set);
        }
    }

    #[test]
    fn the_full_set_reaches_every_circuit_kind_and_extra() {
        let set = full_set();
        let mut seen = FeatureSet::EMPTY;
        for seed in 0..400u64 {
            let d = generate(seed, &set, SIZE);
            seen = seen.union(features_used(&d));
        }
        let mut missing = Vec::new();
        for k in ElemKind::ALL {
            if !seen.contains(Feature::Elem(k)) {
                missing.push(format!("{k:?}"));
            }
        }
        for w in WaveKind::ALL {
            if !seen.contains(Feature::Wave(w)) {
                missing.push(format!("{w:?}"));
            }
        }
        for x in CircuitExtra::ALL {
            if !seen.contains(Feature::Circuit(x)) {
                missing.push(format!("{x:?}"));
            }
        }
        assert!(seen.contains(Feature::Item(ItemKind::Model)));
        assert_eq!(missing, Vec::<String>::new());
    }
}
