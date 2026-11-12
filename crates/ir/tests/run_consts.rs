//! The run owner's constant facts and value constructors: plain values in, plain values out,
//! no design needed. Each expected value is the one the member's or constructor's doc states
//! (oracle: spec example, the doc read by hand).
//!
//! Cites: `rewrite/IR-IMPL.md` §2; `crates/ir/src/{analysis, plan, option_key, sweep, probe,
//! measure}.rs` docs.
//!
//! Contents: tests only.

use spicerack_ir::{
    AcPart, AnalysisKind, DefKind, Edge, Event, EventKind, ExprId, List, OptionClass, OptionKey,
    Probe, ProbeKind, SweepBounds, SweepEnd, SweepKind, SweepSpec, Target, TargetKind,
};

// ---- private ----

fn e(i: u32) -> ExprId {
    ExprId(i)
}

#[test]
fn has_detail_is_false_only_for_op_and_dc_inc() {
    // AnalysisKind docs: every kind but Op and DcInc names a detail row.
    let without: Vec<AnalysisKind> = AnalysisKind::ALL
        .iter()
        .copied()
        .filter(|k| !k.has_detail())
        .collect();
    assert_eq!(without, vec![AnalysisKind::Op, AnalysisKind::DcInc]);
}

#[test]
fn def_kind_names_what_a_refers_to() {
    // Target::a doc: Param → Param, SubcktParam → Subckt, ModelParam → Model, else no ref.
    let expect = [
        (TargetKind::Temp, None),
        (TargetKind::Param, Some(DefKind::Param)),
        (TargetKind::SubcktParam, Some(DefKind::Subckt)),
        (TargetKind::SourceDc, None),
        (TargetKind::ElementValue, None),
        (TargetKind::DeviceParam, None),
        (TargetKind::ModelParam, Some(DefKind::Model)),
        (TargetKind::Option, None),
    ];
    assert_eq!(expect.len(), TargetKind::COUNT);
    for (k, d) in expect {
        assert_eq!(k.def_kind(), d, "{k:?}");
    }
}

#[test]
fn option_class_follows_the_declaration_groups() {
    // option_key.rs: the four comment groups, and Private is Accuracy.
    use OptionKey as K;
    let accuracy = [
        K::RelTol,
        K::AbsTol,
        K::VnTol,
        K::ChgTol,
        K::TrTol,
        K::PivTol,
        K::PivRel,
        K::MaxOrd,
        K::MethodTrap,
        K::MethodGear,
        K::MethodEuler,
        K::Itl1,
        K::Itl2,
        K::Itl4,
        K::GminSteps,
        K::SrcSteps,
        K::NoOpIter,
        K::MaxStep,
        K::Private,
    ];
    let equations = [
        K::Gmin,
        K::GminDc,
        K::GminTran,
        K::Rshunt,
        K::Gshunt,
        K::Cshunt,
        K::Rseries,
        K::Scale,
        K::DefL,
        K::DefW,
        K::DefAd,
        K::DefAs,
    ];
    let path = [K::Klu, K::Sparse, K::Seed, K::Search, K::Autostop];
    for k in OptionKey::ALL {
        let want = if accuracy.contains(&k) {
            OptionClass::Accuracy
        } else if equations.contains(&k) {
            OptionClass::Equations
        } else if path.contains(&k) {
            OptionClass::Path
        } else {
            OptionClass::Output
        };
        assert_eq!(k.class(), want, "{k:?}");
    }
}

#[test]
fn flags_are_the_keys_documented_as_flags() {
    // Each member whose doc begins "Flag:".
    use OptionKey as K;
    let flags = [
        K::MethodTrap,
        K::MethodGear,
        K::MethodEuler,
        K::NoOpIter,
        K::Klu,
        K::Sparse,
        K::Autostop,
        K::Interp,
        K::Acct,
        K::NoAcct,
        K::List,
        K::NoMod,
        K::NoPage,
        K::Node,
        K::Opts,
        K::Warn,
        K::NoRefValue,
        K::KeepOpInfo,
        K::NoInit,
    ];
    for k in OptionKey::ALL {
        assert_eq!(k.is_flag(), flags.contains(&k), "{k:?}");
    }
}

#[test]
fn sweep_constructors_fill_exactly_their_fields() {
    let (a, b, c) = (e(1), e(2), e(3));
    let lin = SweepSpec::lin_step(a, b, c);
    let want = SweepSpec {
        kind: SweepKind::Lin,
        end: SweepEnd::Grid,
        bounds: SweepBounds::StartStop,
        start: a,
        stop: b,
        step: c,
        points: ExprId::NONE,
        values: List::EMPTY,
    };
    assert_eq!(lin, want);
    assert_eq!(
        SweepSpec::lin_points(a, b, c),
        SweepSpec {
            step: ExprId::NONE,
            points: c,
            ..want
        }
    );
    for (s, kind) in [
        (SweepSpec::dec(a, b, c), SweepKind::Dec),
        (SweepSpec::oct(a, b, c), SweepKind::Oct),
        (SweepSpec::log(a, b, c), SweepKind::Log),
    ] {
        assert_eq!(
            s,
            SweepSpec {
                kind,
                step: ExprId::NONE,
                points: c,
                ..want
            }
        );
    }
    let values = List {
        start: 4,
        len: 3,
        pool: Default::default(),
    };
    assert_eq!(
        SweepSpec::list(values),
        SweepSpec {
            kind: SweepKind::List,
            start: ExprId::NONE,
            stop: ExprId::NONE,
            step: ExprId::NONE,
            values,
            ..want
        }
    );
    assert_eq!(lin.ending(SweepEnd::OnStop).end, SweepEnd::OnStop);
    assert_eq!(lin.ending(SweepEnd::OnStop).start, a);
}

#[test]
fn category_probe_has_no_path_and_with_part_changes_only_the_part() {
    let p = Probe::category(ProbeKind::AllV);
    assert_eq!(p.kind, ProbeKind::AllV);
    assert_eq!(p.part, AcPart::None);
    assert_eq!(p.terminal, u8::MAX);
    assert!(p.a.len == 0 && p.b.len == 0);
    assert_eq!(p.expr, ExprId::NONE);
    let db = p.with_part(AcPart::Db);
    assert_eq!(
        db,
        Probe {
            part: AcPart::Db,
            ..p
        }
    );
}

#[test]
#[should_panic(expected = "category")]
fn category_of_a_path_kind_asserts() {
    let _ = Probe::category(ProbeKind::V);
}

#[test]
fn event_constructors() {
    let at = Event::at(e(7));
    assert_eq!(at.kind, EventKind::At);
    assert_eq!(at.value, e(7));
    assert_eq!((at.count, at.delay), (ExprId::NONE, ExprId::NONE));
    let sig = Probe::category(ProbeKind::OutputNoise);
    let c = Event::cross(sig, Edge::Fall, e(1), e(2));
    assert_eq!(
        c,
        Event {
            kind: EventKind::Cross,
            edge: Edge::Fall,
            last: false,
            signal: sig,
            value: e(1),
            count: e(2),
            delay: ExprId::NONE,
            until: ExprId::NONE,
            skip_initial: false,
        }
    );
    assert_eq!(Event::BOUND.kind, EventKind::Bound);
}

#[test]
fn temp_target_holds_nothing() {
    assert_eq!(Target::TEMP.kind, TargetKind::Temp);
    assert!(Target::TEMP.a.name.is_none() && Target::TEMP.b.is_none());
    assert_eq!(Target::TEMP.path.len, 0);
}
