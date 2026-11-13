//! The device laws a design states (R90): which simulator build's law its numbers were written
//! under, where two builds of one family's module disagree. A design's stated set and a circuit
//! row (an element or a model) in, the stated variant's `CircuitExtra` member out at each row
//! that carries the law's point; the set in and out of the IR text's `laws` line; the check that
//! no point is stated twice.
//!
//! Data (the six questions). *What*: a set of [`Law`] variants, each a variant of one law point
//! (a question two builds answer differently); a point's variants exclude each other. Each row
//! of the table (`Law::row`) gives the variant's point, the families whose models follow it, its
//! member, its IR word and its point's exact rewriting (R92, R96, R93a: none, stated, negated,
//! noted, guarded). *How many*: one set per design; `Law::COUNT` variants (44, two per point, 22
//! points), one bit each in a `u64` (the compile-time assertions below fail the build when the
//! enum outgrows 64 or a point gains a third variant). *Who writes*: a
//! reader, once per read, the constant of its simulator build (`Builder::set_laws`); the text's
//! `laws` line; the generator; never a pass. *Who reads*: `features`, once per element or model
//! row and once per expression node of dynamic form (R93a's function points) (O(the stated variants) ≤ `Law::COUNT`, each predicate O(the row's parameters and its one
//! model)); `check` (`LawConflict`); `dump`. *Hot or cold*: cold, read beside each row's other
//! features. *Lifetime*: the design's.
//!
//! A built design states nothing, carries nothing and goes to any target under the target's law.
//! Ceiling (R90): a design whose models follow two builds of one module cannot say so.
//!
//! Cites: `rewrite/RULINGS.md` R86, R88, R90, R91, R92, R93a, R94, R94a, R95, R95a, R96, R102 (on the `rulings`
//! branch); the
//! runs in `crates/ir/tests/fixtures/bjt_substrate/` and `crates/ir/tests/fixtures/families/`.
//! The carrier predicates are `laws_carry.rs`, the table's rows `laws_table.rs`.
//!
//! Contents:
//! - `pub enum Law` (`ALL`, `COUNT`), `Law::member(self) -> CircuitExtra`,
//!   `Law::word(self) -> &'static str`, `Law::rewrite(self) -> Rewrite`
//! - `pub enum Rewrite`, `pub struct Stated`
//! - `pub struct Laws`, `Laws::EMPTY`, `Laws::of(&[Law]) -> Laws`, `Laws::with(self, Law) -> Laws`,
//!   `Laws::contains(self, Law) -> bool`, `Laws::is_empty(self) -> bool`,
//!   `Laws::iter(self) -> impl Iterator<Item = Law>`, `Laws::features(self) -> FeatureSet`
//! - `Design::laws(&self) -> Laws`, `Builder::set_laws(&mut self, Laws)`
//! - crate: `pub(crate) enum Carrier`, `pub(crate) enum Point`, `pub(crate) struct Row`,
//!   (`Law::row(self) -> Row`: `laws_table.rs`), `pub(crate) fn carried(&Design, Carrier, &mut impl FnMut(CircuitExtra))`,
//!   `pub(crate) fn conflicts(Laws) -> impl Iterator<Item = u32>`, `pub(crate) fn dump_line(&Design, FileId, &mut String)`,
//!   `pub(crate) fn parse_line(&mut Words, &mut Builder) -> bool`, `pub(crate) fn draw(&mut Gen, &FeatureSet) -> Laws` (gen)

use crate::build::Builder;
use crate::circuit_extra::CircuitExtra as X;
use crate::design::Design;
use crate::element::Element;
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::FileId;
use crate::model::{Model, ModelFamily};
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
    /// bottom junction's F1 (ngspice-45 `dioload.c:431`), whose charge jumps at the knee. Run
    /// (`l3_ramp_0.7`, R91b): here the transient cannot cross the knee ("Timestep too small",
    /// refused), against 8.544073680612274e-03 under `DiodeSidewallOwnF1` (the integral of C dv);
    /// equal below the knee (`l3_ramp_0.3`). Carried by a diode with a perimeter (`pj` given and not literally 0, or
    /// `w` and `l` given on a level-3 or unresolved model) on a `Diode` or `DiodeLevel3` model
    /// whose `cjsw` is given and not literally 0, or on an unresolved model.
    DiodeSidewallBottomF1,
    /// Point R91 F22: the sidewall's own F1 (the VACASK module the flake pins, `diode.va:1112`).
    DiodeSidewallOwnF1,
    /// Point R92 F21, a diode model's metal and poly oxide thicknesses `xom`, `xoi` when absent:
    /// 1e4 Å (ngspice-45 `dio/diosetup.c:262, 265`, 1e-6 m). Run (`families/diode/runs.out`,
    /// `l2_xom_absent`): 2.17e-06 here against 1.0e-12 under `DiodeXomAbsent1e14`; `xom=1e4`
    /// given agrees (`l2_xom_given`). Carried by a `Diode` or `DiodeLevel3` model that gives
    /// `wm`, `lm`, `wp` or `lp` and lacks `xom` or `xoi`. Stated: the rule writes the absent value.
    DiodeXomAbsent1e4,
    /// Point R92 F21: 1e14 Å (the VACASK module the flake pins, `diode.va:190-191`, 1e4 m).
    DiodeXomAbsent1e14,
    /// Point R92 F24, a VDMOS model's `kp` when absent: 25 + 10·type, 35 on an n-channel and 15
    /// on a p-channel (ngspice-45 `vdmos/vdmosset.c:35`). Run (`families/vdmos/runs.out`,
    /// `v1_kp_absent`): -157.5 A here against -90 under `VdmosKpAbsent20`. Carried by a `Vdmos`
    /// model that lacks `kp`. Stated.
    VdmosKpAbsent35,
    /// Point R92 F24: 15 + 5·type, 20 and 10 (the VACASK module the flake pins, `vdmos.va:816`).
    VdmosKpAbsent20,
    /// Point R92 F25, a VDMOS threshold's temperature coefficient: vth0 − type·tcvth·ΔT, which an
    /// n-channel's positive `tcvth` lowers (ngspice-45 `vdmos/vdmostemp.c:50`). Run (`v2_n_*`):
    /// -40.35 A here against -19.28 under `VdmosTcvthRaises`; equal on a p-channel. Carried by an
    /// n-channel `Vdmos` model whose `tcvth` (or `vtotc`) is given and not literally 0. Negated.
    VdmosTcvthLowers,
    /// Point R92 F25: vth0 + tcvth·ΔT (the VACASK module the flake pins, `vdmos.va:938`).
    VdmosTcvthRaises,
    /// Point R92 F26, the VDMOS body diode's junction-potential clamp when fc·vj > 2.5: 2.5/n
    /// (ngspice-45 `vdmos/vdmostemp.c:123`). Run (`v3_vj_6`): 4.709e-03 here against 5.524e-03
    /// under `VdmosClampByFc`; equal at vj = 1. Carried by a `Vdmos` model that gives `cjo` and
    /// whose fc·vj is not provably at most 2.5 (absent `fc` 0.5 and `vj` 0.8 in both builds).
    VdmosClampByN,
    /// Point R92 F26: 2.5/fc (the VACASK module the flake pins, `vdmos.va:985`).
    VdmosClampByFc,
    /// Point R94 F27, the small-signal of MOS levels 2, 3 and 9: ngspice-45's hand-written
    /// derivatives, not its DC law's (`families/mos{2,3,9}/runs.out`: in saturation gm 4.9e-2
    /// and gds 1.7e-2 off the central differences of its own DC currents). Carried by a MOSFET
    /// on a `Mos2`, `Mos3` or `Mos9` model (or an unresolved one) in a design that holds an
    /// analysis linearised about an operating point, or no analysis (`analysis::linearised`).
    MosSmallSignalHand,
    /// Point R94 F27: the DC law's own derivatives (the VACASK modules the flake pins, 1e-9).
    MosSmallSignalExact,
    /// Point R94 F28, the small-signal of MOS level 6: none (ngspice-45 `mos6/mos6init.c:43`,
    /// `.DEVacLoad = NULL`: every AC run stops with a singular matrix). Carried as F27, on a
    /// `Mos6` model.
    Mos6SmallSignalAbsent,
    /// Point R94 F28: the DC law's own derivatives (the VACASK module the flake pins).
    Mos6SmallSignalExact,
    /// Point R94a F29, the small-signal of a Gummel-Poon BJT whose base resistance falls with the
    /// base current: the resistance held at its operating value (ngspice-45;
    /// `families/bjt/runs.out`: in saturation gm 1.47e-2, gds 3.22e-2 off its DC derivatives,
    /// `bjt_rbconst` the control). Carried as F27, by a BJT on a `GummelPoon` model whose `irb`
    /// is given and not literally 0 (or an unresolved model).
    BjtIrbSmallSignalFrozen,
    /// Point R94a F29: the DC law's own derivatives (the VACASK module the flake pins, 5e-9).
    BjtIrbSmallSignalExact,
    /// Point R95/R95a F30, a VDMOS's charges at a multiplier not 1: not m copies of one
    /// (ngspice-45; `families/vdmos_load/runs.out`: small-signal m3/nom 2.63 to 8.15 where DC is
    /// 3; `families/tran/runs.out`: at m = 3, 2.4655 V at 30 ns against its own three copies'
    /// 3.2708 V). Carried by a MOSFET on a `Vdmos` model (or an unresolved one) whose `m` is
    /// given and not literally 1, in a design that holds an analysis using charges (any but an
    /// operating point and a DC sweep) or no analysis (`analysis::charge_design`).
    VdmosMultChargesOff,
    /// Point R95/R95a F30: m copies (the VACASK module the flake pins: small-signal 3 to 1e-9,
    /// the transient its three copies' to 1e-6).
    VdmosMultChargesCopies,
    /// Point R95 F31, a diode's recombination current (`isr`) under reverse bias: none (ngspice-45;
    /// `families/diode_load/runs.out`: 2.020e-12 A here against 3.679e-12 A at -2 V, the control
    /// without `isr` equal). Carried by a diode on a `Diode` or `DiodeLevel3` model whose `isr` is
    /// given and not literally 0 (or an unresolved model), in any design (the bias is the run's).
    DiodeIsrReverseNone,
    /// Point R95 F31: present (the VACASK module the flake pins).
    DiodeIsrReversePresent,
    /// Point R96, a diode's small-signal with `isr`: ngspice-45's conductance 7.7e-5 off the slope
    /// of its own DC current (`diode_load`, nom_fwd at 0.6 V). Carried as F27, by a diode on a model
    /// whose `isr` is given and not literally 0 (or an unresolved model). Noted: under 1e-3.
    DiodeIsrSmallSignalHand,
    /// Point R96: the DC law's own slope (the VACASK module the flake pins, 2e-8).
    DiodeIsrSmallSignalExact,
    /// Point R96 (R91's clamp), a diode's transit-time temperature factor 1 + ttt1·ΔT + ttt2·ΔT²
    /// below 1e-3: taken as it is (ngspice-45; `families/diode/runs.out`, `l4_ttt1_0.01`:
    /// 1.126195826779 here against 1.1261960490808718). Carried by a `Diode` or `DiodeLevel3`
    /// model whose `ttt1` or `ttt2` is given and not literally 0. Noted: 2e-7 apart.
    DiodeTtFactorUnclamped,
    /// Point R96: clamped at 1e-3 (the VACASK module the flake pins, `diode.va`).
    DiodeTtFactorClamped,
    /// Point R93a F42, a behavioural `sin`, `cos`, `tan` of a large argument: reduced as
    /// x − (int)(x/L)·L, the cast saturating past 2^31 periods (ngspice-45's B source; 2.8e-7 to
    /// 1.3e-6 relative off the exact function, no wrong branch). Carried by the function in a
    /// root of dynamic form. Noted both ways (no exact form; the note names the region).
    TrigReduceCast,
    /// Point R93a F42: the exact function (the pinned VACASK build).
    TrigExact,
    /// Point R93a F43, a behavioural `exp`: 1e99 above 227.9559242 (ngspice-45's B source; the
    /// true value at 228 is 1.045e99). Carried by `exp` in a root of dynamic form. Guarded toward
    /// a target with the exact law (`x > 227.9559242 ? 1e99 : exp(x)`).
    ExpClamp1e99,
    /// Point R93a F43: the exact function (VACASK). Noted toward ngspice.
    ExpExact,
    /// Point R93a F44, a behavioural `ln`, `log10` of 0: -1e99 (ngspice-45's B source). Carried by
    /// `ln` or `log10` in a root of dynamic form. Guarded (`x == 0 ? -1e99 : ln(x)`).
    LogZeroMinus1e99,
    /// Point R93a F44: an error (VACASK). Noted toward ngspice.
    LogZeroError,
    /// Point R93a F45, a behavioural division: ±gmin·1e-20 added to the denominator
    /// (ngspice-45's B source; 1/1e-30 is 1 % off, 1/0 is 1e32). Carried by `/` (and a typed
    /// division) in a root of dynamic form. Guarded (the guard a literal from the design's gmin
    /// where that is constant; else the rule refuses).
    DivGminGuard,
    /// Point R93a F45: the exact quotient (VACASK). Noted toward ngspice.
    DivExact,
    /// Point R93a F46, a dynamic `asin(±1)`, `acos(-1)`: converges to 0 (ngspice-45, the
    /// linearization's slope 1e32 cancels the value). Carried by `asin` or `acos` in a root of
    /// dynamic form. Noted both ways.
    AsinEdgeZero,
    /// Point R93a F46: fails loudly (VACASK).
    AsinEdgeFails,
    /// Point R102, the temperature a design with no temperature statement runs at: 27 °C
    /// (ngspice, VACASK, Xyce, LTspice, ESPice). Carried by the design when no `Env` sets a
    /// temperature. Stated: the rule writes `temp` as a run-wide `Env` item.
    DefaultTemp27,
    /// Point R102: 25 °C (HSPICE; `hspice-2`, dialect-hspice 88cd9649).
    DefaultTemp25,
    /// Point R102, the nominal temperature (`tnom`) of a design with no nominal temperature
    /// statement: 27 °C (ngspice, VACASK, Xyce, LTspice, ESPice). Carried by the design when no
    /// `Env` sets `tnom`. Stated: the rule writes `tnom` as a run-wide `Env` item.
    DefaultTnom27,
    /// Point R102: 25 °C (HSPICE).
    DefaultTnom25,
}

/// A set of stated laws, one bit per [`Law`]. Two variants of one point may stand in the set;
/// `check` refuses that design (`Violation::LawConflict`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Laws {
    bits: u64,
}

/// The exact rewriting a law point has (R92, a closed set): what a rule needs and nothing more.
/// The rule bodies are the passes owner's, one per kind.
#[derive(Clone, Copy, Debug)]
pub enum Rewrite {
    /// None: a carrier of the point is refused across a pair whose laws differ.
    None,
    /// The variant names the value each absent parameter means; the rule writes it on each
    /// carrier, after which nothing carries the point.
    Stated(&'static [Stated]),
    /// A sign convention: the rule negates the parameter (by any of these spellings) on every
    /// carrier and restates the design's variant.
    Negated(&'static [&'static str]),
    /// A difference under the simulators' own relative tolerance (1e-3) everywhere it was
    /// measured (R96): the rule restates the design's variant and leaves one note per carrier
    /// naming the point. R93a widens it to a difference confined to a region the note states,
    /// where no exact form exists (F42, F46; F43–F45 toward ngspice).
    Noted,
    /// The variant's law is an expression around the plain member (R93a: F43's clamp, F44's
    /// zero, F45's guarded denominator): toward a target with the other law, the rule writes
    /// that expression on each carrier and restates the variant, so nothing grows on a round
    /// trip. The rule bodies are the expression group owner's.
    Guarded,
}

/// One parameter a stated variant fills in: its spellings in the family's own names (ngspice's
/// and the pinned VACASK module's, which agree on every name below) and the value its absence
/// means under the variant, as a function of the model's row. For a design point (R102's
/// default temperatures) the name is the run-wide `Env` field and the model is unused.
#[derive(Clone, Copy, Debug)]
pub struct Stated {
    /// The parameter's spellings; a model carries the point when it gives none of them.
    pub names: &'static [&'static str],
    /// The value an absent parameter means on `model`, in the parameter's deck unit.
    pub absent: fn(&Model) -> f64,
}

/// Which row a law's predicate reads.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Carrier<'a> {
    Element(&'a Element),
    Model(&'a Model),
    /// An expression node in a root of dynamic form (R93a's function points).
    Node(crate::expr::Expr),
    /// The design as a whole (R102's default temperatures).
    Design,
}

/// A law point: the question its variants answer. `LawConflict`'s `arg` is its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Point {
    BcSatArea,
    SubSatTemp,
    DiodeWlMult,
    DiodeSidewallKnee,
    DiodeXom,
    VdmosKp,
    VdmosTcvth,
    VdmosClamp,
    MosSmallSignal,
    Mos6SmallSignal,
    BjtIrbSmallSignal,
    VdmosMultCharges,
    DiodeIsrReverse,
    DiodeIsrSmallSignal,
    DiodeTtFactor,
    TrigReduce,
    ExpClamp,
    LogZero,
    DivGuard,
    AsinEdge,
    DefaultTemp,
    DefaultTnom,
}

/// One variant's row: its point, the families whose models follow it, its member, its word,
/// its rewriting.
pub(crate) struct Row {
    pub(crate) point: Point,
    pub(crate) families: &'static [ModelFamily],
    pub(crate) member: X,
    pub(crate) word: &'static str,
    pub(crate) rewrite: Rewrite,
}

/// Every point, in declaration order.
const POINTS: [Point; 22] = [
    Point::BcSatArea,
    Point::SubSatTemp,
    Point::DiodeWlMult,
    Point::DiodeSidewallKnee,
    Point::DiodeXom,
    Point::VdmosKp,
    Point::VdmosTcvth,
    Point::VdmosClamp,
    Point::MosSmallSignal,
    Point::Mos6SmallSignal,
    Point::BjtIrbSmallSignal,
    Point::VdmosMultCharges,
    Point::DiodeIsrReverse,
    Point::DiodeIsrSmallSignal,
    Point::DiodeTtFactor,
    Point::TrigReduce,
    Point::ExpClamp,
    Point::LogZero,
    Point::DivGuard,
    Point::AsinEdge,
    Point::DefaultTemp,
    Point::DefaultTnom,
];

// 40 variants: a `u64`, one bit each (the build fails here when the enum outgrows it)
const _: () = assert!(Law::COUNT <= u64::BITS as usize);
const _: () = assert!(size_of::<Laws>() == 8);
const _: () = assert!(Law::COUNT == 2 * POINTS.len());

impl Law {
    /// Every variant, in declaration order (its bit in [`Laws`]).
    pub const ALL: [Law; 44] = [
        Law::BcSatAreaTimesAreab,
        Law::BcSatAreabOnly,
        Law::SubSatLastFactor,
        Law::SubSatEmissionNs,
        Law::DiodeWlMultTwice,
        Law::DiodeWlMultOnce,
        Law::DiodeSidewallBottomF1,
        Law::DiodeSidewallOwnF1,
        Law::DiodeXomAbsent1e4,
        Law::DiodeXomAbsent1e14,
        Law::VdmosKpAbsent35,
        Law::VdmosKpAbsent20,
        Law::VdmosTcvthLowers,
        Law::VdmosTcvthRaises,
        Law::VdmosClampByN,
        Law::VdmosClampByFc,
        Law::MosSmallSignalHand,
        Law::MosSmallSignalExact,
        Law::Mos6SmallSignalAbsent,
        Law::Mos6SmallSignalExact,
        Law::BjtIrbSmallSignalFrozen,
        Law::BjtIrbSmallSignalExact,
        Law::VdmosMultChargesOff,
        Law::VdmosMultChargesCopies,
        Law::DiodeIsrReverseNone,
        Law::DiodeIsrReversePresent,
        Law::DiodeIsrSmallSignalHand,
        Law::DiodeIsrSmallSignalExact,
        Law::DiodeTtFactorUnclamped,
        Law::DiodeTtFactorClamped,
        Law::TrigReduceCast,
        Law::TrigExact,
        Law::ExpClamp1e99,
        Law::ExpExact,
        Law::LogZeroMinus1e99,
        Law::LogZeroError,
        Law::DivGminGuard,
        Law::DivExact,
        Law::AsinEdgeZero,
        Law::AsinEdgeFails,
        Law::DefaultTemp27,
        Law::DefaultTemp25,
        Law::DefaultTnom27,
        Law::DefaultTnom25,
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

    /// Its point's exact rewriting under this variant (R92).
    pub const fn rewrite(self) -> Rewrite {
        self.row().rewrite
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
            bits: self.bits | 1 << law as u64,
        }
    }

    /// Whether `law` is stated.
    pub const fn contains(self, law: Law) -> bool {
        self.bits & 1 << law as u64 != 0
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
/// variants × the row's parameters and its one model's); the small-signal points read the
/// design's analyses once per design (a cache).
pub(crate) fn carried(design: &Design, row: Carrier<'_>, add: &mut impl FnMut(X)) {
    let laws = design.laws();
    if laws.is_empty() {
        return;
    }
    for law in laws.iter() {
        let r = law.row();
        if crate::laws_carry::carries(design, &r, row) {
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
