//! The law table's rows (R90, R92, R93a, R94, R94a, R95, R96, R102): a `Law` in, its `Row` out (point,
//! families, member, IR word, rewriting kind), exhaustive over the variants. Split from `laws.rs`
//! at its size limit; each variant's doc there carries its build, run and carriers.
//!
//! Cites: `rewrite/RULINGS.md` R90–R96 (on the `rulings` branch).
//!
//! Contents (crate-private): `Law::row(self) -> Row`

use crate::circuit_extra::CircuitExtra as X;
use crate::laws::{Law, Point, Rewrite, Row, Stated};
use crate::model::{Model, ModelFamily};

const GP: &[ModelFamily] = &[ModelFamily::GummelPoon];
const L3: &[ModelFamily] = &[ModelFamily::DiodeLevel3];
const DIODES: &[ModelFamily] = &[ModelFamily::Diode, ModelFamily::DiodeLevel3];
const VDMOS: &[ModelFamily] = &[ModelFamily::Vdmos];
const MOS239: &[ModelFamily] = &[ModelFamily::Mos2, ModelFamily::Mos3, ModelFamily::Mos9];
const MOS6: &[ModelFamily] = &[ModelFamily::Mos6];
/// R93a's function points: carried by an expression node, no model family.
const NONE: &[ModelFamily] = &[];
const XOM_1E4: &[Stated] = &[
    Stated {
        names: &["xom"],
        absent: |_| 1e4,
    },
    Stated {
        names: &["xoi"],
        absent: |_| 1e4,
    },
];
const XOM_1E14: &[Stated] = &[
    Stated {
        names: &["xom"],
        absent: |_| 1e14,
    },
    Stated {
        names: &["xoi"],
        absent: |_| 1e14,
    },
];
const KP_35: &[Stated] = &[Stated {
    names: &["kp"],
    absent: |m| 25.0 + 10.0 * channel(m),
}];
const KP_20: &[Stated] = &[Stated {
    names: &["kp"],
    absent: |m| 15.0 + 5.0 * channel(m),
}];
/// R102: the run-wide `Env` field each default fills in, °C (a design point: the model is unused).
const TEMP_27: &[Stated] = &[Stated {
    names: &["temp"],
    absent: |_| 27.0,
}];
const TEMP_25: &[Stated] = &[Stated {
    names: &["temp"],
    absent: |_| 25.0,
}];
const TNOM_27: &[Stated] = &[Stated {
    names: &["tnom"],
    absent: |_| 27.0,
}];
const TNOM_25: &[Stated] = &[Stated {
    names: &["tnom"],
    absent: |_| 25.0,
}];
const TCVTH: Rewrite = Rewrite::Negated(&["tcvth", "vtotc"]);

impl Law {
    /// The table: exhaustive over the variants.
    pub(crate) const fn row(self) -> Row {
        use Point as P;
        use Rewrite::None as N;
        let (point, families, member, word, rewrite) = match self {
            Law::BcSatAreaTimesAreab => (
                P::BcSatArea,
                GP,
                X::BjtBcSatAreaTimesAreab,
                "bc_sat_area_times_areab",
                N,
            ),
            Law::BcSatAreabOnly => (
                P::BcSatArea,
                GP,
                X::BjtBcSatAreabOnly,
                "bc_sat_areab_only",
                N,
            ),
            Law::SubSatLastFactor => (
                P::SubSatTemp,
                GP,
                X::ModelSubSatLastFactor,
                "sub_sat_last_factor",
                N,
            ),
            Law::SubSatEmissionNs => (
                P::SubSatTemp,
                GP,
                X::ModelSubSatEmissionNs,
                "sub_sat_emission_ns",
                N,
            ),
            Law::DiodeWlMultTwice => (
                P::DiodeWlMult,
                L3,
                X::DiodeWlMultTwice,
                "diode_wl_mult_twice",
                N,
            ),
            Law::DiodeWlMultOnce => (
                P::DiodeWlMult,
                L3,
                X::DiodeWlMultOnce,
                "diode_wl_mult_once",
                N,
            ),
            Law::DiodeSidewallBottomF1 => (
                P::DiodeSidewallKnee,
                DIODES,
                X::DiodeSidewallBottomF1,
                "diode_sidewall_bottom_f1",
                N,
            ),
            Law::DiodeSidewallOwnF1 => (
                P::DiodeSidewallKnee,
                DIODES,
                X::DiodeSidewallOwnF1,
                "diode_sidewall_own_f1",
                N,
            ),
            Law::DiodeXomAbsent1e4 => (
                P::DiodeXom,
                DIODES,
                X::DiodeXomAbsent1e4,
                "diode_xom_absent_1e4",
                Rewrite::Stated(XOM_1E4),
            ),
            Law::DiodeXomAbsent1e14 => (
                P::DiodeXom,
                DIODES,
                X::DiodeXomAbsent1e14,
                "diode_xom_absent_1e14",
                Rewrite::Stated(XOM_1E14),
            ),
            Law::VdmosKpAbsent35 => (
                P::VdmosKp,
                VDMOS,
                X::VdmosKpAbsent35,
                "vdmos_kp_absent_35",
                Rewrite::Stated(KP_35),
            ),
            Law::VdmosKpAbsent20 => (
                P::VdmosKp,
                VDMOS,
                X::VdmosKpAbsent20,
                "vdmos_kp_absent_20",
                Rewrite::Stated(KP_20),
            ),
            Law::VdmosTcvthLowers => (
                P::VdmosTcvth,
                VDMOS,
                X::VdmosTcvthLowers,
                "vdmos_tcvth_lowers",
                TCVTH,
            ),
            Law::VdmosTcvthRaises => (
                P::VdmosTcvth,
                VDMOS,
                X::VdmosTcvthRaises,
                "vdmos_tcvth_raises",
                TCVTH,
            ),
            Law::VdmosClampByN => (
                P::VdmosClamp,
                VDMOS,
                X::VdmosClampByN,
                "vdmos_clamp_by_n",
                N,
            ),
            Law::VdmosClampByFc => (
                P::VdmosClamp,
                VDMOS,
                X::VdmosClampByFc,
                "vdmos_clamp_by_fc",
                N,
            ),
            Law::MosSmallSignalHand => (
                P::MosSmallSignal,
                MOS239,
                X::MosSmallSignalHand,
                "mos_small_signal_hand",
                N,
            ),
            Law::MosSmallSignalExact => (
                P::MosSmallSignal,
                MOS239,
                X::MosSmallSignalExact,
                "mos_small_signal_exact",
                N,
            ),
            Law::Mos6SmallSignalAbsent => (
                P::Mos6SmallSignal,
                MOS6,
                X::Mos6SmallSignalAbsent,
                "mos6_small_signal_absent",
                N,
            ),
            Law::Mos6SmallSignalExact => (
                P::Mos6SmallSignal,
                MOS6,
                X::Mos6SmallSignalExact,
                "mos6_small_signal_exact",
                N,
            ),
            Law::BjtIrbSmallSignalFrozen => (
                P::BjtIrbSmallSignal,
                GP,
                X::BjtIrbSmallSignalFrozen,
                "bjt_irb_small_signal_frozen",
                N,
            ),
            Law::BjtIrbSmallSignalExact => (
                P::BjtIrbSmallSignal,
                GP,
                X::BjtIrbSmallSignalExact,
                "bjt_irb_small_signal_exact",
                N,
            ),
            Law::VdmosMultChargesOff => (
                P::VdmosMultCharges,
                VDMOS,
                X::VdmosMultChargesOff,
                "vdmos_mult_charges_off",
                N,
            ),
            Law::VdmosMultChargesCopies => (
                P::VdmosMultCharges,
                VDMOS,
                X::VdmosMultChargesCopies,
                "vdmos_mult_charges_copies",
                N,
            ),
            Law::DiodeIsrReverseNone => (
                P::DiodeIsrReverse,
                DIODES,
                X::DiodeIsrReverseNone,
                "diode_isr_reverse_none",
                N,
            ),
            Law::DiodeIsrReversePresent => (
                P::DiodeIsrReverse,
                DIODES,
                X::DiodeIsrReversePresent,
                "diode_isr_reverse_present",
                N,
            ),
            Law::DiodeIsrSmallSignalHand => (
                P::DiodeIsrSmallSignal,
                DIODES,
                X::DiodeIsrSmallSignalHand,
                "diode_isr_small_signal_hand",
                Rewrite::Noted,
            ),
            Law::DiodeIsrSmallSignalExact => (
                P::DiodeIsrSmallSignal,
                DIODES,
                X::DiodeIsrSmallSignalExact,
                "diode_isr_small_signal_exact",
                Rewrite::Noted,
            ),
            Law::DiodeTtFactorUnclamped => (
                P::DiodeTtFactor,
                DIODES,
                X::DiodeTtFactorUnclamped,
                "diode_tt_factor_unclamped",
                Rewrite::Noted,
            ),
            Law::DiodeTtFactorClamped => (
                P::DiodeTtFactor,
                DIODES,
                X::DiodeTtFactorClamped,
                "diode_tt_factor_clamped",
                Rewrite::Noted,
            ),
            Law::TrigReduceCast => (
                P::TrigReduce,
                NONE,
                X::TrigReduceCast,
                "trig_reduce_cast",
                Rewrite::Noted,
            ),
            Law::TrigExact => (
                P::TrigReduce,
                NONE,
                X::TrigExact,
                "trig_exact",
                Rewrite::Noted,
            ),
            Law::ExpClamp1e99 => (
                P::ExpClamp,
                NONE,
                X::ExpClamp1e99,
                "exp_clamp_1e99",
                Rewrite::Guarded,
            ),
            Law::ExpExact => (P::ExpClamp, NONE, X::ExpExact, "exp_exact", Rewrite::Noted),
            Law::ExpLin230 => (
                P::ExpClamp,
                NONE,
                X::ExpLin230,
                "exp_lin_230",
                Rewrite::Guarded,
            ),
            Law::LogZeroMinus1e99 => (
                P::LogZero,
                NONE,
                X::LogZeroMinus1e99,
                "log_zero_minus_1e99",
                Rewrite::Guarded,
            ),
            Law::LogZeroError => (
                P::LogZero,
                NONE,
                X::LogZeroError,
                "log_zero_error",
                Rewrite::Noted,
            ),
            Law::DivGminGuard => (
                P::DivGuard,
                NONE,
                X::DivGminGuard,
                "div_gmin_guard",
                Rewrite::Guarded,
            ),
            Law::DivExact => (P::DivGuard, NONE, X::DivExact, "div_exact", Rewrite::Noted),
            Law::AsinEdgeZero => (
                P::AsinEdge,
                NONE,
                X::AsinEdgeZero,
                "asin_edge_zero",
                Rewrite::Noted,
            ),
            Law::AsinEdgeFails => (
                P::AsinEdge,
                NONE,
                X::AsinEdgeFails,
                "asin_edge_fails",
                Rewrite::Noted,
            ),
            Law::DefaultTemp27 => (
                P::DefaultTemp,
                NONE,
                X::DefaultTemp27,
                "default_temp_27",
                Rewrite::Stated(TEMP_27),
            ),
            Law::DefaultTemp25 => (
                P::DefaultTemp,
                NONE,
                X::DefaultTemp25,
                "default_temp_25",
                Rewrite::Stated(TEMP_25),
            ),
            Law::DefaultTnom27 => (
                P::DefaultTnom,
                NONE,
                X::DefaultTnom27,
                "default_tnom_27",
                Rewrite::Stated(TNOM_27),
            ),
            Law::DefaultTnom25 => (
                P::DefaultTnom,
                NONE,
                X::DefaultTnom25,
                "default_tnom_25",
                Rewrite::Stated(TNOM_25),
            ),
        };
        Row {
            point,
            families,
            member,
            word,
            rewrite,
        }
    }
}

// ---- private ----

/// A VDMOS model's type: 1 for an n-channel, -1 for a p-channel (`Polarity::None` reads as n,
/// the family's default).
fn channel(m: &Model) -> f64 {
    match m.polarity {
        crate::model::Polarity::P => -1.0,
        crate::model::Polarity::N | crate::model::Polarity::None => 1.0,
    }
}
