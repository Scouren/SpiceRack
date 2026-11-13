//! The rewriting each law point states (R92, R95, R96): every variant's `Law::rewrite` against a
//! table written from the rulings, not from `laws_table.rs`: the kind, a stated point's
//! parameter names and the value its absence means on an n- and a p-channel model, a negated
//! point's spellings.
//!
//! Oracle: the rulings' text (R92: F21 `xom`/`xoi` absent 1e4 Å in ngspice-45 and 1e14 Å in the
//! pinned VACASK; F24 `kp` absent 25+10·type against 15+5·type; F25 `tcvth`/`vtotc` negated;
//! R96: F40 and F41 noted; R93a: F43, F44, F45 guarded under ngspice-45's law and noted under
//! the exact one, F42 and F46 noted both ways; R102: the default temperature and nominal
//! temperature stated, 27 °C or 25 °C; every other point none).
//!
//! Contents: tests only.

mod circuit_common;

mod tests {
    use super::circuit_common::model;
    use spicerack_ir::{Law, ModelFamily, NameId, Polarity, Rewrite};

    /// A point's kind as the rulings state it: none, stated (names, absent on n, absent on p),
    /// negated (spellings), noted.
    #[derive(Debug, PartialEq)]
    enum Want {
        None,
        Stated(Vec<(Vec<&'static str>, f64, f64)>),
        Negated(Vec<&'static str>),
        Noted,
        Guarded,
    }

    fn want(law: Law) -> Want {
        use Law as L;
        let xom = |v: f64| Want::Stated(vec![(vec!["xom"], v, v), (vec!["xoi"], v, v)]);
        match law {
            L::DiodeXomAbsent1e4 => xom(1e4),
            L::DiodeXomAbsent1e14 => xom(1e14),
            L::VdmosKpAbsent35 => Want::Stated(vec![(vec!["kp"], 35.0, 15.0)]),
            L::VdmosKpAbsent20 => Want::Stated(vec![(vec!["kp"], 20.0, 10.0)]),
            L::VdmosTcvthLowers | L::VdmosTcvthRaises => Want::Negated(vec!["tcvth", "vtotc"]),
            L::DefaultTemp27 => Want::Stated(vec![(vec!["temp"], 27.0, 27.0)]),
            L::DefaultTemp25 => Want::Stated(vec![(vec!["temp"], 25.0, 25.0)]),
            L::DefaultTnom27 => Want::Stated(vec![(vec!["tnom"], 27.0, 27.0)]),
            L::DefaultTnom25 => Want::Stated(vec![(vec!["tnom"], 25.0, 25.0)]),
            L::DiodeIsrSmallSignalHand
            | L::DiodeIsrSmallSignalExact
            | L::DiodeTtFactorUnclamped
            | L::DiodeTtFactorClamped
            | L::TrigReduceCast
            | L::TrigExact
            | L::ExpExact
            | L::LogZeroError
            | L::DivExact
            | L::AsinEdgeZero
            | L::AsinEdgeFails => Want::Noted,
            L::ExpClamp1e99 | L::ExpClamp1e200 | L::LogZeroMinus1e99 | L::DivGminGuard => {
                Want::Guarded
            }
            L::BcSatAreaTimesAreab
            | L::BcSatAreabOnly
            | L::SubSatLastFactor
            | L::SubSatEmissionNs
            | L::DiodeWlMultTwice
            | L::DiodeWlMultOnce
            | L::DiodeSidewallBottomF1
            | L::DiodeSidewallOwnF1
            | L::VdmosClampByN
            | L::VdmosClampByFc
            | L::MosSmallSignalHand
            | L::MosSmallSignalExact
            | L::Mos6SmallSignalAbsent
            | L::Mos6SmallSignalExact
            | L::BjtIrbSmallSignalFrozen
            | L::BjtIrbSmallSignalExact
            | L::VdmosMultChargesOff
            | L::VdmosMultChargesCopies
            | L::DiodeIsrReverseNone
            | L::DiodeIsrReversePresent => Want::None,
        }
    }

    #[test]
    fn every_law_states_the_rewriting_its_ruling_gives() {
        let n = model(NameId::NONE, ModelFamily::Vdmos, Polarity::N);
        let p = model(NameId::NONE, ModelFamily::Vdmos, Polarity::P);
        for law in Law::ALL {
            let got = match law.rewrite() {
                Rewrite::None => Want::None,
                Rewrite::Noted => Want::Noted,
                Rewrite::Guarded => Want::Guarded,
                Rewrite::Negated(names) => Want::Negated(names.to_vec()),
                Rewrite::Stated(rows) => Want::Stated(
                    rows.iter()
                        .map(|s| (s.names.to_vec(), (s.absent)(&n), (s.absent)(&p)))
                        .collect(),
                ),
            };
            assert_eq!(got, want(law), "{law:?}");
        }
    }
}
