//! The device laws a design states (R90): which simulator build's law its numbers were written
//! under, where two builds of one family's module disagree. A design's stated set and a circuit
//! row (an element or a model) in, the stated variant's `CircuitExtra` member out at each row
//! that carries the law's point; the set in and out of the IR text's `laws` line; the check that
//! no point is stated twice.
//!
//! Data (the six questions). *What*: a set of [`Law`] variants, each a variant of one law point
//! (a question two builds answer differently); a point's variants exclude each other. *How
//! many*: one set per design; `Law::COUNT` variants (8), one bit each in a `u8` (the
//! compile-time assertion below fails the build when the enum outgrows it). *Who writes*: a
//! reader, once per read, the constant of its simulator build (`Builder::set_laws`); the text's
//! `laws` line; the generator; never a pass. *Who reads*: `features`, once per element or model
//! row (O(the stated variants) ≤ `Law::COUNT`, each predicate O(the row's parameters and its one
//! model)); `check` (`LawConflict`); `dump`. *Hot or cold*: cold, read beside each row's other
//! features. *Lifetime*: the design's.
//!
//! A built design states nothing, carries nothing and goes to any target under the target's law.
//! Ceiling (R90): a design whose models follow two builds of one module cannot say so.
//!
//! Cites: `rewrite/RULINGS.md` R86, R88, R90, R91 (on the `rulings` branch); the runs in
//! `crates/ir/tests/fixtures/bjt_substrate/` and `crates/ir/tests/fixtures/families/diode/`.
//!
//! Contents:
//! - `pub enum Law` (`ALL`, `COUNT`), `Law::member(self) -> CircuitExtra`,
//!   `Law::word(self) -> &'static str`
//! - `pub struct Laws`, `Laws::EMPTY`, `Laws::of(&[Law]) -> Laws`, `Laws::with(self, Law) -> Laws`,
//!   `Laws::contains(self, Law) -> bool`, `Laws::is_empty(self) -> bool`,
//!   `Laws::iter(self) -> impl Iterator<Item = Law>`, `Laws::features(self) -> FeatureSet`
//! - `Design::laws(&self) -> Laws`, `Builder::set_laws(&mut self, Laws)`
//! - crate: `pub(crate) enum Carrier`, `pub(crate) fn carried(&Design, Carrier, &mut impl FnMut(CircuitExtra))`,
//!   `pub(crate) fn conflicts(Laws) -> impl Iterator<Item = u32>`, `pub(crate) fn dump_line(&Design, FileId, &mut String)`,
//!   `pub(crate) fn parse_line(&mut Words, &mut Builder) -> bool`, `pub(crate) fn draw(&mut Gen, &FeatureSet) -> Laws` (gen)

use crate::build::Builder;
use crate::circuit_extra::CircuitExtra as X;
use crate::circuit_text::get_row;
use crate::design::Design;
use crate::element::{ElemKind, Element};
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ExprId, FileId};
use crate::item::ItemKind;
use crate::model::{Model, ModelFamily};
use crate::semi::Semi;
use crate::text_words::Words;

/// One variant of one law point: what a simulator build computes where another build of the
/// same module computes otherwise. Each variant's doc names its point, the build that follows
/// it, the run that shows the difference and the rows that carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Law {
    /// Point R86, a Gummel-Poon model's base-collector saturation current under an area when the
    /// model gives neither `ibe` nor `ibc` (with both, both laws take `ibc` × areab): area ×
    /// areab. ngspice-45 (`bjttemp.c:179, 199`) and the older VACASK module
    /// (/nix/store/g5ial84g…-vacask-unstable-2026, vadng-pre-46, `bjt.va:873, 888`). Run: a
    /// saturated common-emitter stage at area 1.5 reads 4.812462e-02 here, 3.854936e-02 under
    /// `BcSatAreabOnly`; equal at area 1. Carried by a BJT whose area is given and not literally
    /// 1, on a Gummel-Poon model that does not give both `ibe` and `ibc`, or on an unresolved
    /// model.
    BcSatAreaTimesAreab,
    /// Point R86: areab alone (the VACASK module the flake pins,
    /// /nix/store/dhxz6l3i…-vacask-unstable-2026, vadng-pre-48, `bjt.va:917, 933, 940`).
    BcSatAreabOnly,
    /// Point R88, a Gummel-Poon model's substrate saturation current `iss` under `tlev` 0 or 1:
    /// the factor last set before it, `exp(factlog)` or `exp(factlog/nr)` with both `ibe` and
    /// `ibc` (ngspice-45 `bjttemp.c:181-182`; the older VACASK module `bjt.va:875-877`). Runs
    /// (`tests/fixtures/bjt_substrate/runs.out`): at 100 °C with `ns = 2`, -9.58448e-05 A here
    /// against -1.039134e-06 A under `SubSatEmissionNs`. Carried by a Gummel-Poon model that
    /// gives `iss`, whose `tlev` is absent or literally 0 or 1, and whose `ns` is not literally
    /// the last-set factor's exponent (1, or `nr` with both `ibe` and `ibc`; absent is 1, a
    /// parameter read is not literal). No rewriting: `ns` also sets the substrate diode.
    SubSatLastFactor,
    /// Point R88: `exp(factlog/ns)` (the VACASK module the flake pins, `bjt.va:919-922`).
    SubSatEmissionNs,
    /// Point R91 F20, a level-3 diode's area from `w` and `l` under a multiplier `m`: scaled by
    /// `m` twice (ngspice-45: the area from `w`·`l` carries `m`, then `is`·area·`m`;
    /// `dio/diosetup.c:304`, `diotemp.c`). Run (`tests/fixtures/families/diode/runs.out`, `l1_m2`):
    /// -9.94249665686e-06 A here against -4.971248578432379e-06 A under `DiodeWlMultOnce`, equal
    /// at `m = 1` (`l1_m1`). Carried by a diode that gives `w` and `l`, whose `m` is given and
    /// not literally 1, on a `DiodeLevel3` model or an unresolved model.
    DiodeWlMultTwice,
    /// Point R91 F20: `m` once (the VACASK module the flake pins; the older build alike).
    DiodeWlMultOnce,
    /// Point R91 F22, a diode's sidewall depletion charge past its knee (`fcs`·`php`): with the
    /// bottom junction's F1 (ngspice-45 `dioload.c:431`). Run (`l3_ramp_0.7`): 4.435741165180e-03
    /// here against 8.544073680612274e-03 under `DiodeSidewallOwnF1`, equal below the knee
    /// (`l3_ramp_0.3`). Carried by a diode with a perimeter (`pj` given and not literally 0, or
    /// `w` and `l` given on a level-3 or unresolved model) on a `Diode` or `DiodeLevel3` model
    /// whose `cjsw` is given and not literally 0, or on an unresolved model.
    DiodeSidewallBottomF1,
    /// Point R91 F22: the sidewall's own F1 (the VACASK module the flake pins, `diode.va:1112`).
    DiodeSidewallOwnF1,
}

/// A set of stated laws, one bit per [`Law`]. Two variants of one point may stand in the set;
/// `check` refuses that design (`Violation::LawConflict`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Laws {
    bits: u8,
}

/// Which row a law's predicate reads.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Carrier<'a> {
    Element(&'a Element),
    Model(&'a Model),
}

/// A law point: the question its variants answer. `LawConflict`'s `arg` is its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    BcSatArea,
    SubSatTemp,
    DiodeWlMult,
    DiodeSidewallKnee,
}

/// One variant's row: its point, the families whose models follow it, its member, its word.
struct Row {
    point: Point,
    families: &'static [ModelFamily],
    member: X,
    word: &'static str,
}

const GP: &[ModelFamily] = &[ModelFamily::GummelPoon];
const L3: &[ModelFamily] = &[ModelFamily::DiodeLevel3];
const DIODES: &[ModelFamily] = &[ModelFamily::Diode, ModelFamily::DiodeLevel3];

/// Every point, in declaration order.
const POINTS: [Point; 4] = [
    Point::BcSatArea,
    Point::SubSatTemp,
    Point::DiodeWlMult,
    Point::DiodeSidewallKnee,
];

const _: () = assert!(Law::COUNT <= u8::BITS as usize);
const _: () = assert!(size_of::<Laws>() == 1);

impl Law {
    /// Every variant, in declaration order (its bit in [`Laws`]).
    pub const ALL: [Law; 8] = [
        Law::BcSatAreaTimesAreab,
        Law::BcSatAreabOnly,
        Law::SubSatLastFactor,
        Law::SubSatEmissionNs,
        Law::DiodeWlMultTwice,
        Law::DiodeWlMultOnce,
        Law::DiodeSidewallBottomF1,
        Law::DiodeSidewallOwnF1,
    ];
    /// Variant count.
    pub const COUNT: usize = Self::ALL.len();

    /// The feature a carrier of this variant reports; a target that follows the variant holds
    /// it in its `FEATURES`.
    pub const fn member(self) -> X {
        self.row().member
    }

    /// Its spelling on the IR text's `laws` line.
    pub const fn word(self) -> &'static str {
        self.row().word
    }
}

impl Laws {
    /// No law stated: what a built design states.
    pub const EMPTY: Laws = Laws { bits: 0 };

    /// The set of `laws` (a reader's constant: `const LAWS: Laws = Laws::of(&[…])`).
    pub const fn of(laws: &[Law]) -> Laws {
        let mut set = Laws::EMPTY;
        let mut k = 0;
        while k < laws.len() {
            set = set.with(laws[k]);
            k += 1;
        }
        set
    }

    /// The set with `law` added.
    pub const fn with(self, law: Law) -> Laws {
        Laws {
            bits: self.bits | 1 << law as u8,
        }
    }

    /// Whether `law` is stated.
    pub const fn contains(self, law: Law) -> bool {
        self.bits & 1 << law as u8 != 0
    }

    /// Whether nothing is stated.
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// The stated variants, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = Law> {
        Law::ALL.into_iter().filter(move |&l| self.contains(l))
    }

    /// The members of the stated variants: what a dialect whose reader states `self` adds to its
    /// `FEATURES` (law L5: the reader's image).
    pub const fn features(self) -> FeatureSet {
        let mut set = FeatureSet::EMPTY;
        let mut k = 0;
        while k < Law::COUNT {
            if self.contains(Law::ALL[k]) {
                set = set.with(Feature::Circuit(Law::ALL[k].member()));
            }
            k += 1;
        }
        set
    }
}

impl Design {
    /// The laws this design states (R90); [`Laws::EMPTY`] unless its reader set them.
    pub fn laws(&self) -> Laws {
        self.circuit.laws
    }
}

impl Builder<'_> {
    /// States the laws of the simulator build the design was written for, replacing any set
    /// before. A reader calls it once per read with its constant. A truncate restores the set
    /// its mark saw.
    pub fn set_laws(&mut self, laws: Laws) {
        self.design.circuit.laws = laws;
        debug_assert!(self.design.laws() == laws);
    }
}

/// Reports, for each stated variant whose point `row` carries, the variant's member. O(stated
/// variants × the row's parameters and its one model's).
pub(crate) fn carried(design: &Design, row: Carrier<'_>, add: &mut impl FnMut(X)) {
    let laws = design.laws();
    if laws.is_empty() {
        return;
    }
    for law in laws.iter() {
        let r = law.row();
        let on = match (r.point, row) {
            (Point::BcSatArea, Carrier::Element(e)) => bc_sat_area(design, e, r.families),
            (Point::DiodeWlMult, Carrier::Element(e)) => diode_wl_mult(design, e, r.families),
            (Point::DiodeSidewallKnee, Carrier::Element(e)) => {
                diode_sidewall(design, e, r.families)
            }
            (Point::SubSatTemp, Carrier::Model(m)) => sub_sat_temp(design, m, r.families),
            (
                Point::BcSatArea | Point::DiodeWlMult | Point::DiodeSidewallKnee,
                Carrier::Model(_),
            )
            | (Point::SubSatTemp, Carrier::Element(_)) => false,
        };
        if on {
            add(r.member);
        }
    }
}

/// The index of each point `laws` states more than one variant of, once each.
pub(crate) fn conflicts(laws: Laws) -> impl Iterator<Item = u32> {
    let mut seen = [0u8; POINTS.len()];
    for law in laws.iter() {
        seen[law.row().point as usize] += 1;
    }
    (0..seen.len() as u32).filter(move |&p| seen[p as usize] > 1)
}

/// Appends the `laws` line (`laws <word> …`) when `file` is the entry; nothing when the design
/// states none.
pub(crate) fn dump_line(design: &Design, file: FileId, out: &mut String) {
    let laws = design.laws();
    if laws.is_empty() || file != design.entry() {
        return;
    }
    out.push_str("laws");
    for law in laws.iter() {
        out.push(' ');
        out.push_str(law.word());
    }
    out.push('\n');
}

/// Reads the words of a `laws` line (its keyword already read) and adds them to the stated set.
/// Returns false, stating nothing, on an empty line or a word that names no law.
pub(crate) fn parse_line(w: &mut Words<'_>, b: &mut Builder<'_>) -> bool {
    let mut read = Laws::EMPTY;
    // bounded: each pass consumes one word of the line
    while let Some(word) = w.next() {
        let Some(law) = Law::ALL.into_iter().find(|l| l.word() == word) else {
            return false;
        };
        read = read.with(law);
    }
    if read.is_empty() {
        return false;
    }
    let all = Laws {
        bits: b.design().laws().bits | read.bits,
    };
    b.set_laws(all);
    true
}

/// A drawn set: per point, one of the variants whose member `features` holds, or none (a coin
/// among them and none). So a carrier reports only a member of the set.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn draw(g: &mut Gen, features: &FeatureSet) -> Laws {
    let mut set = Laws::EMPTY;
    for point in POINTS {
        let held: Vec<Law> = Law::ALL
            .into_iter()
            .filter(|l| l.row().point == point)
            .filter(|l| features.contains(Feature::Circuit(l.member())))
            .collect();
        let k = g.below(held.len() as u32 + 1) as usize;
        if let Some(&law) = held.get(k) {
            set = set.with(law);
        }
    }
    assert!(
        conflicts(set).next().is_none(),
        "a drawn set states each point once"
    );
    set
}

// ---- private ----

impl Law {
    /// The table: exhaustive over the variants.
    const fn row(self) -> Row {
        let (point, families, member, word) = match self {
            Law::BcSatAreaTimesAreab => (
                Point::BcSatArea,
                GP,
                X::BjtBcSatAreaTimesAreab,
                "bc_sat_area_times_areab",
            ),
            Law::BcSatAreabOnly => (
                Point::BcSatArea,
                GP,
                X::BjtBcSatAreabOnly,
                "bc_sat_areab_only",
            ),
            Law::SubSatLastFactor => (
                Point::SubSatTemp,
                GP,
                X::ModelSubSatLastFactor,
                "sub_sat_last_factor",
            ),
            Law::SubSatEmissionNs => (
                Point::SubSatTemp,
                GP,
                X::ModelSubSatEmissionNs,
                "sub_sat_emission_ns",
            ),
            Law::DiodeWlMultTwice => (
                Point::DiodeWlMult,
                L3,
                X::DiodeWlMultTwice,
                "diode_wl_mult_twice",
            ),
            Law::DiodeWlMultOnce => (
                Point::DiodeWlMult,
                L3,
                X::DiodeWlMultOnce,
                "diode_wl_mult_once",
            ),
            Law::DiodeSidewallBottomF1 => (
                Point::DiodeSidewallKnee,
                DIODES,
                X::DiodeSidewallBottomF1,
                "diode_sidewall_bottom_f1",
            ),
            Law::DiodeSidewallOwnF1 => (
                Point::DiodeSidewallKnee,
                DIODES,
                X::DiodeSidewallOwnF1,
                "diode_sidewall_own_f1",
            ),
        };
        Row {
            point,
            families,
            member,
            word,
        }
    }
}

/// The model an element names: `Ok(None)` when unresolved (its law unknown: a carrier),
/// `Ok(Some(m))` for a model row, `Err(())` when it names another kind of definition.
fn model_of(design: &Design, e: &Element) -> Result<Option<Model>, ()> {
    let Some(d) = e
        .model
        .def
        .index_checked(design.defs.len())
        .map(|k| design.defs[k])
    else {
        return Ok(None);
    };
    if d.item.kind != ItemKind::Model {
        return Err(());
    }
    Ok(Some(design.row(d.item.index)))
}

/// A parameter's value by key, `None` when not given.
fn param(design: &Design, list: crate::id::ParamList, key: &str) -> Option<ExprId> {
    design
        .params(list)
        .iter()
        .find(|p| design.str(p.0) == key)
        .map(|p| p.1)
}

/// Whether `e` is given and not literally `v` (a parameter read is not literal).
fn given_not(design: &Design, e: Option<ExprId>, v: f64) -> bool {
    e.is_some_and(|e| literal(design, e) != Some(v))
}

/// R86's carrier: a BJT with an area not literally 1 on a model of `families` that does not
/// give both `ibe` and `ibc`, or on an unresolved model.
fn bc_sat_area(design: &Design, e: &Element, families: &[ModelFamily]) -> bool {
    if e.kind != ElemKind::Bjt {
        return false;
    }
    let Some(s) = get_row::<Semi>(design, e.detail) else {
        return false;
    };
    if s.area.is_none() || crate::run_words::is_one(design, s.area) {
        return false;
    }
    match model_of(design, e) {
        Ok(None) => true,
        Ok(Some(m)) => {
            let given = |k: &str| param(design, m.params, k).is_some();
            families.contains(&m.family) && !(given("ibe") && given("ibc"))
        }
        Err(()) => false,
    }
}

/// R91 F20's carrier: a diode giving `w` and `l` with `m` given and not literally 1, on a model
/// of `families` or an unresolved one.
fn diode_wl_mult(design: &Design, e: &Element, families: &[ModelFamily]) -> bool {
    let wl = param(design, e.params, "w").is_some() && param(design, e.params, "l").is_some();
    if e.kind != ElemKind::Diode || !wl {
        return false;
    }
    if e.mult.is_none() || crate::run_words::is_one(design, e.mult) {
        return false;
    }
    match model_of(design, e) {
        Ok(None) => true,
        Ok(Some(m)) => families.contains(&m.family),
        Err(()) => false,
    }
}

/// R91 F22's carrier: a diode with a perimeter on a model of `families` whose `cjsw` is given
/// and not literally 0, or on an unresolved model.
fn diode_sidewall(design: &Design, e: &Element, families: &[ModelFamily]) -> bool {
    if e.kind != ElemKind::Diode {
        return false;
    }
    let pj = given_not(design, param(design, e.params, "pj"), 0.0);
    let wl = param(design, e.params, "w").is_some() && param(design, e.params, "l").is_some();
    match model_of(design, e) {
        Ok(None) => pj || wl,
        Ok(Some(m)) => {
            let perimeter = pj || wl && m.family == ModelFamily::DiodeLevel3;
            perimeter
                && families.contains(&m.family)
                && given_not(design, param(design, m.params, "cjsw"), 0.0)
        }
        Err(()) => false,
    }
}

/// R88's carrier: a model of `families` giving `iss`, `tlev` absent or literally 0 or 1, and
/// `ns` not literally the last-set factor's exponent.
fn sub_sat_temp(design: &Design, m: &Model, families: &[ModelFamily]) -> bool {
    if !families.contains(&m.family) {
        return false;
    }
    let get = |key: &str| param(design, m.params, key);
    let tlev_ok =
        get("tlev").is_none_or(|e| literal(design, e).is_some_and(|v| v == 0.0 || v == 1.0));
    if get("iss").is_none() || !tlev_ok {
        return false;
    }
    // the exponent the last-set factor uses: nr with both currents, else 1; absent means 1
    let both = get("ibe").is_some() && get("ibc").is_some();
    let other = if both {
        get("nr").map_or(Some(1.0), |e| literal(design, e))
    } else {
        Some(1.0)
    };
    let ns = get("ns").map_or(Some(1.0), |e| literal(design, e));
    !matches!((ns, other), (Some(a), Some(b)) if a == b)
}

/// The value of a literal number; `None` for a parameter read or any other expression.
fn literal(design: &Design, e: ExprId) -> Option<f64> {
    match design.expr(e) {
        crate::expr::Expr::Num(k) => Some(design.literal_value(design.num(k))),
        _ => None,
    }
}
