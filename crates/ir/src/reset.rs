//! The restore statement (A27, A28; R49): from here, what a script changed holds the design's
//! own value again. A reader's ngspice `reset` or VACASK `clear saves` in, one `Reset` row
//! out; never copies of the cards' values (R13 b).
//!
//! Cites: `rewrite/AMEND-A12.md` A27, A28; `rewrite/RULINGS.md` R49; `ng-control`'s
//! `S3-CONTROL.md` S1; `vc-run`'s runs sv1, sv2 (VACASK 36be560).
//!
//! IR text: `reset <class> …`, the classes in declaration order: `reset values options temp
//! tnom saves`, `reset saves`.
//!
//! Contents: `pub struct Reset`, `pub enum Restorable` (`ALL`, `COUNT`), `pub struct ResetSet`
//! (`EMPTY`, `ALL`, `ResetSet::of(&[Restorable]) -> Self`,
//! `ResetSet::contains(self, Restorable) -> bool`, `ResetSet::is_empty(self) -> bool`,
//! `ResetSet::iter(self) -> impl Iterator<Item = Restorable>`); `RowFacts` for `Reset`

use crate::analysis_facts::report;
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{Handle, RowFacts};
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ItemRef, Ref, ScopeId};
use crate::item::DefKind;
use crate::run_extra::RunExtra;
use crate::run_words::{member, put_word, word};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::Ctx;

/// From here, every item of a class in `restores` that took effect earlier in run order stops
/// holding, and the design's own value holds again: what the `WholeRun` items and the cards
/// give, else the simulator's default. "Earlier" is run order: listed order with includes in
/// place, and inside a loop body every earlier pass's items too (ngspice's `reset` in a
/// `foreach` body is global). Items listed after the reset hold as usual. No class restores
/// parameters: no simulator does (ngspice's `reset` keeps every `alterparam`), so an `Alter`
/// of `Param` or `SubcktParam` holds across every reset. `restores` is never empty (checked).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reset {
    /// The classes restored.
    pub restores: ResetSet,
}

/// A class of script changes a `Reset` undoes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Restorable {
    /// `Alter`s of a source's DC value, an element's value, a device's or a model's
    /// parameter (ngspice `alter`, `altermod`).
    Values,
    /// `FromHere` option rows (ngspice script `option`/`set` of a simulator option).
    Options,
    /// The temperature of `FromHere` `Env` rows (ngspice script `set temp`).
    Temp,
    /// The nominal temperature of `FromHere` `Env` rows (ngspice script `option tnom=`).
    Tnom,
    /// `FromHere` saves (ngspice script `save`; VACASK `save`, whose `clear saves` restores
    /// this class alone: the analyses after it keep their default set until the next save).
    Saves,
}

/// A set of `Restorable` classes, one bit each in declaration order; no other bit is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ResetSet(u8);

const _: () = assert!(size_of::<Reset>() == 1);
const _: () = assert!(Restorable::COUNT <= 8);

impl Restorable {
    /// Every class, in declaration order.
    pub const ALL: [Restorable; 5] = [
        Restorable::Values,
        Restorable::Options,
        Restorable::Temp,
        Restorable::Tnom,
        Restorable::Saves,
    ];
    /// Class count.
    pub const COUNT: usize = Self::ALL.len();
}

impl ResetSet {
    /// No class.
    pub const EMPTY: ResetSet = ResetSet(0);
    /// Every class (ngspice's `reset`).
    pub const ALL: ResetSet = ResetSet((1u8 << Restorable::COUNT) - 1);

    /// The set of `classes` (repeats are one member).
    pub const fn of(classes: &[Restorable]) -> Self {
        let mut bits = 0u8;
        let mut k = 0;
        while k < classes.len() {
            bits |= 1 << classes[k] as u8;
            k += 1;
        }
        ResetSet(bits)
    }

    /// Whether `class` is a member.
    pub const fn contains(self, class: Restorable) -> bool {
        self.0 & (1 << class as u8) != 0
    }

    /// Whether the set has no member.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The members, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = Restorable> {
        Restorable::ALL
            .into_iter()
            .filter(move |&c| self.contains(c))
    }
}

impl RowFacts for Reset {
    fn handles(&self, _design: &Design, _out: &mut Vec<Handle>) {
        // no handle field
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let _ = (design, at, f); // no reference
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        for c in self.restores.iter() {
            *set = set.with(Feature::Run(class_feature(c)));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.restores.is_empty() {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, _design: &Design, _ctx: &Ctx, out: &mut String) {
        let at = out.len();
        for c in self.restores.iter() {
            put_word(out, &word(c));
        }
        crate::run_words::trim_lead(out, at);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let mut classes = Vec::new();
        // one class per pass; the bound allows each twice (repeats are one member)
        for _ in 0..2 * Restorable::COUNT {
            let Some(c) = words.peek().and_then(|w| member(&Restorable::ALL, w)) else {
                break;
            };
            words.next();
            classes.push(c);
        }
        if classes.is_empty() {
            words.error(diags);
            return ItemRef::NONE;
        }
        let restores = ResetSet::of(&classes);
        b.push(scope, Reset { restores }, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !crate::state_order::run_append_ok(b.design(), scope, features) {
            return ItemRef::NONE; // no `RunInterleaved` outside the set (A22)
        }
        let allowed: Vec<Restorable> = Restorable::ALL
            .into_iter()
            .filter(|&c| features.contains(Feature::Run(class_feature(c))))
            .collect();
        if allowed.is_empty() {
            return ItemRef::NONE;
        }
        // each allowed class half the time, at least one
        let mut picked: Vec<Restorable> = allowed.iter().copied().filter(|_| g.chance(2)).collect();
        if picked.is_empty() {
            picked.push(allowed[g.below(allowed.len() as u32) as usize]);
        }
        let restores = ResetSet::of(&picked);
        b.push(scope, Reset { restores }, Span::NONE)
    }
}

// ---- private ----

/// The feature of each class.
const fn class_feature(c: Restorable) -> RunExtra {
    match c {
        Restorable::Values => RunExtra::ResetValues,
        Restorable::Options => RunExtra::ResetOptions,
        Restorable::Temp => RunExtra::ResetTemp,
        Restorable::Tnom => RunExtra::ResetTnom,
        Restorable::Saves => RunExtra::ResetSaves,
    }
}
