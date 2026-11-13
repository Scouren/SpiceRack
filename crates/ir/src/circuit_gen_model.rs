//! The model half of the circuit generator (`circuit_gen.rs`'s child module): which families a
//! model may take here, and one pushed model row of a given family list, in the order `dump`
//! writes it; each pushed model is noted for the element kinds that take its family (R48 d).
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator); `rewrite/AMEND-A12.md` C4, C5, K5;
//! `rewrite/RULINGS.md` R48.
//!
//! Contents: `R::model_families`, `R::push_model` (`pub(super)`)

use super::{NONE_REF, R, head};
use crate::circuit_check::{builtin_kind, has_polarity, versioned};
use crate::circuit_extra::CircuitExtra as X;
use crate::circuit_tables::{VERSIONS, needs_version};
use crate::element::ElemKind;
use crate::feature::Feature;
use crate::r#gen::Params;
use crate::id::{ExprId, ItemRef, List, NameId};
use crate::item::DefKind;
use crate::model::{BcSatArea, Model, ModelFamily, Polarity};
use crate::span::Span;

impl R<'_, '_> {
    /// The families a model may take here: without `ModelWithoutLibrary` only one some library
    /// of the design implements, and only one the set has (A15 K5). Decided before anything
    /// is interned, so a refusal leaves no orphan name.
    pub(super) fn model_families(&self) -> Vec<ModelFamily> {
        let families: Vec<ModelFamily> = if self.has(X::ModelWithoutLibrary) {
            ModelFamily::ALL.to_vec()
        } else {
            let libraries = &self.b.design().device_libraries.rows;
            libraries.iter().map(|l| l.family).collect()
        };
        families
            .into_iter()
            .filter(|&f| self.f.contains(Feature::Family(f)))
            .collect()
    }

    /// Pushes one random model of one of `families` (not empty) into the row's scope.
    pub(super) fn push_model(&mut self, families: &[ModelFamily]) -> ItemRef {
        assert!(!families.is_empty(), "a family to draw");
        let r = self;
        let family = families[r.g.below(families.len() as u32) as usize];
        // a bin is `<group>.<n>` of a fresh group (A15 C4): the name is interned first, the group
        // where `dump` writes `bin_of`
        let group = r.maybe(X::ModelBin).then(|| r.g.fresh());
        let name = match &group {
            Some(base) => {
                let text = format!("{base}.1");
                r.b.name(&text)
            }
            None => r.g.fresh_name(r.b),
        };
        let polarity = if has_polarity(family) {
            if r.g.chance(2) {
                Polarity::N
            } else {
                Polarity::P
            }
        } else if family == ModelFamily::Other {
            Polarity::ALL[r.g.below(3) as usize]
        } else {
            Polarity::None
        };
        let mut pairs: Vec<(String, ExprId)> = Vec::new();
        // values first, then keys: the order `Words::params` creates them in
        // A28 item 5: `TypedOnly` draws no untyped parameter (elements never draw one)
        let count = if builtin_kind(family).is_some() || r.g.params == Params::TypedOnly {
            0
        } else {
            r.g.below(3)
        };
        // a parameter list's value may be a string or a list (A25 item 4)
        let values: Vec<ExprId> = (0..count)
            .map(|_| {
                (r.g.root_str, r.g.root_list) = (true, true);
                let value = r.ex();
                (r.g.root_str, r.g.root_list) = (false, false);
                value
            })
            .collect();
        for value in values {
            let key = r.g.fresh_name(r.b);
            pairs.push((r.b.design().str(key).to_owned(), value));
        }
        let params = if pairs.is_empty() {
            List::EMPTY
        } else {
            let borrowed: Vec<(&str, ExprId)> =
                pairs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            r.b.params(&borrowed)
        };
        let bin_of = match &group {
            Some(base) => r.b.name(base),
            None => NameId::NONE,
        };
        let ako = if r.maybe(X::ModelAko) {
            r.reference(DefKind::Model)
        } else {
            NONE_REF
        };
        let module = if family == ModelFamily::Module {
            r.reference(DefKind::Module)
        } else {
            NONE_REF
        };
        // a versioned standard states its version, from the vocabulary where one is known
        let version = if needs_version(family) {
            match VERSIONS.iter().find(|v| v.0 == family) {
                Some((_, list)) => {
                    let k = r.g.below(list.len() as u32) as usize;
                    r.b.string(list[k])
                }
                None => {
                    let word = r.g.fresh();
                    r.b.string(&word)
                }
            }
        } else if versioned(family) && r.g.chance(4) {
            r.ex()
        } else {
            ExprId::NONE
        };
        let word = if matches!(family, ModelFamily::CodeModel | ModelFamily::Other) {
            r.g.fresh_name(r.b)
        } else {
            NameId::NONE
        };
        // R86: a Gummel-Poon model's base-collector law by which member the set holds
        let areab_only = family == ModelFamily::GummelPoon
            && r.has(X::BjtBcSatAreabOnly)
            && (!r.has(X::BjtBcSatAreaTimesAreab) || r.g.chance(2));
        let bc_sat_area = if areab_only {
            BcSatArea::AreabOnly
        } else {
            BcSatArea::AreaTimesAreab
        };
        let row = Model {
            name,
            params,
            bin_of,
            ako,
            module,
            version,
            word,
            family,
            polarity,
            bc_sat_area,
        };
        let item = r.b.model(r.scope, row, Span::NONE);
        // R48 (d): the elements that take its family may name it
        let def = r.b.design().lookup(r.scope, DefKind::Model, name);
        assert!(!def.is_none(), "a pushed model is defined");
        for kind in ElemKind::ALL {
            if head::takes(kind, family) {
                r.g.models_for[kind as usize].push(def);
            }
        }
        item
    }
}
