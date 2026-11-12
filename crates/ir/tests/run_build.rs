//! The builder's run methods and the run owner's typed accessors: builder calls in, the rows
//! read back by handle out. Oracle: a model (the values the test pushed, compared field by
//! field with what the design returns) and the asserts each method's doc states.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 ("what goes in comes back by handle"); `crates/ir/src/
//! build_run.rs` docs; `rewrite/IR-DECISIONS.md` A6, A7.
//!
//! Contents: tests only.

use spicerack_ir::dc_ac::{Dc, DcAxis};
use spicerack_ir::measure::{MeasureParam, MeasureTrigTarg};
use spicerack_ir::plan::{MonteCarlo, Sampler, SweepLoop, Variations};
use spicerack_ir::stats::Correlation;
use spicerack_ir::tran::{StepCap, Tran};
use spicerack_ir::{
    AcPart, Alter, Analysis, AnalysisKind, Builder, DefKind, Design, Edge, Env, Event, ExprId,
    IcScope, ItemKind, ItemRef, List, Loop, LoopEnd, LoopKind, Measure, MeasureKind, NodeSet,
    NodeSetKind, OptionKey, OptionRow, Print, PrintView, ProbeKind, Ref, Role, Save, ScopeId,
    ScopeKind, Span, Statistics, SweepSpec, Target, TargetKind, Vary, VaryDist,
};

// ---- private ----

/// A design with one deck file; the test builds into its top scope.
fn deck() -> (Design, ScopeId) {
    let mut d = Design::new();
    let top = d.builder().file("t.sp", Role::Deck).1;
    (d, top)
}

fn tran(b: &mut Builder<'_>) -> Tran {
    Tran {
        stop: b.num(1e-6),
        record_start: ExprId::NONE,
        print_step: b.num(1e-9),
        max_step: ExprId::NONE,
        initial_step: ExprId::NONE,
        noise_fmax: ExprId::NONE,
        noise_fmin: ExprId::NONE,
        noise_seed: ExprId::NONE,
        segments: List::EMPTY,
        cap: StepCap::NgspiceRule,
        uic: true,
        startup: false,
    }
}

#[test]
fn pools_give_back_what_was_appended() {
    let (mut d, _) = deck();
    let mut b = d.builder();
    let p = [b.probe_v(&["x1", "out"]), b.probe_i(&["v1"])];
    let probes = b.probes(&p);
    let t = [Target::TEMP, b.target(TargetKind::Param, &[], "r", None)];
    let targets = b.targets(&t);
    let sweep = SweepSpec::lin_step(b.num(0.0), b.num(5.0), b.num(0.1));
    let axes_in = [DcAxis {
        target: t[1],
        sweep,
    }];
    let axes = b.dc_axes(&axes_in);
    let spread = b.num(0.1);
    let v = [Vary {
        param: Ref::unresolved(b.name("vth")),
        dist: VaryDist::Gauss,
        percent: false,
        mean: ExprId::NONE,
        spread,
    }];
    let varies = b.varies(&v);
    let cc = b.num(0.5);
    let c = [Correlation {
        a: Ref::unresolved(b.name("vth")),
        b: Ref::unresolved(b.name("tox")),
        cc,
    }];
    let corr = b.correlations(&c);
    assert_eq!(d.probes(probes), &p);
    assert_eq!(d.targets(targets), &t);
    assert_eq!(d.dc_axes(axes), &axes_in);
    assert_eq!(d.varies(varies), &v);
    assert_eq!(d.correlations(corr), &c);
}

#[test]
fn targets_and_probes_hold_their_fields() {
    let (mut d, _) = deck();
    let mut b = d.builder();
    let dev = b.target(TargetKind::DeviceParam, &["x1"], "r1", Some("r"));
    let opt = b.target_option(OptionKey::RelTol, None);
    let private = b.target_option(OptionKey::Private, Some(("vacask", "tran_rmax")));
    let vd = b.probe_vdiff(&["a"], &["b"]);
    let it = b.probe_terminal(&["m1"], 2);
    let pp = b.probe_param(&["m1"], "gm");
    let pw = b.probe_power(&["r1"]);
    let x = b.time();
    let pe = b.probe_expr(x);
    let d = b.design();
    assert_eq!(dev.kind, TargetKind::DeviceParam);
    assert_eq!(d.str(dev.a.name), "r1");
    assert!(dev.a.def.is_none());
    assert_eq!(d.str(dev.b), "r");
    assert_eq!(
        d.names(dev.path)
            .iter()
            .map(|&n| d.str(n))
            .collect::<Vec<_>>(),
        ["x1"]
    );
    assert_eq!(
        (opt.kind, opt.option),
        (TargetKind::Option, OptionKey::RelTol)
    );
    assert_eq!(private.option, OptionKey::Private);
    assert_eq!(
        (d.str(private.a.name), d.str(private.b)),
        ("tran_rmax", "vacask")
    );
    assert_eq!((vd.kind, d.names(vd.b).len()), (ProbeKind::Vdiff, 1));
    assert_eq!((it.kind, it.terminal), (ProbeKind::ITerminal, 2));
    assert_eq!(d.str(d.names(pp.b)[0]), "gm");
    assert_eq!((pw.kind, pw.part), (ProbeKind::Power, AcPart::None));
    assert_eq!((pe.kind, pe.expr), (ProbeKind::Expr, x));
}

#[test]
fn analyses_push_a_header_and_a_detail() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let op = b.op(top, Some("op1"), Span::NONE);
    let row = tran(&mut b);
    let tr = b.analysis(top, None, row, Span::NONE);
    let inc = b.dc_inc(top, None, Span::NONE);
    assert_eq!(d.items(top), &[op, tr, inc]);
    let h: Analysis = d.row(op.index);
    assert_eq!(
        (h.kind, d.str(h.name), h.detail),
        (AnalysisKind::Op, "op1", u32::MAX)
    );
    let h: Analysis = d.row(tr.index);
    assert_eq!(
        (h.kind, h.name),
        (AnalysisKind::Tran, spicerack_ir::NameId::NONE)
    );
    assert_eq!(d.detail::<Tran>(h.detail), row);
    let h: Analysis = d.row(inc.index);
    assert_eq!((h.kind, h.detail), (AnalysisKind::DcInc, u32::MAX));
}

#[test]
#[should_panic(expected = "analysis name")]
fn a_second_analysis_with_the_same_name_asserts() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    b.op(top, Some("a"), Span::NONE);
    b.op(top, Some("a"), Span::NONE);
}

#[test]
fn loops_open_a_loop_scope_owned_by_the_item() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let r = b.target(TargetKind::Param, &[], "r", None);
    let sweep = SweepSpec::lin_step(b.num(1e3), b.num(1e4), b.num(1e3));
    let row = SweepLoop { target: r, sweep };
    let (item, body) = b.loop_(top, Some("sw1"), row, LoopEnd::KeepLast, Span::NONE);
    let mc = MonteCarlo {
        runs: b.num(10.0),
        seed: ExprId::NONE,
        samples: List::EMPTY,
        variations: Variations::All,
        sampler: Sampler::Random,
        nominal: false,
    };
    let (inner, inner_body) = b.loop_(body, None, mc, LoopEnd::Restore, Span::NONE);
    let l: Loop = d.row(item.index);
    assert_eq!(
        (l.kind, l.after, l.body),
        (LoopKind::Sweep, LoopEnd::KeepLast, body)
    );
    assert_eq!(d.str(l.name), "sw1");
    assert_eq!(d.detail::<SweepLoop>(l.detail), row);
    let s = d.scope(body);
    assert_eq!((s.kind, s.parent, s.owner), (ScopeKind::Loop, top, item));
    assert_eq!(d.items(body), &[inner]);
    let l: Loop = d.row(inner.index);
    assert_eq!((l.kind, l.body), (LoopKind::MonteCarlo, inner_body));
    assert_eq!(d.detail::<MonteCarlo>(l.detail), mc);
}

#[test]
#[should_panic(expected = "loop depth")]
fn a_ninth_nested_loop_asserts() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let mut scope = top;
    for _ in 0..9 {
        let sweep = SweepSpec::lin_step(b.num(0.0), b.num(1.0), b.num(1.0));
        let row = SweepLoop {
            target: Target::TEMP,
            sweep,
        };
        scope = b.loop_(scope, None, row, LoopEnd::Restore, Span::NONE).1;
    }
}

#[test]
fn state_items_hold_their_fields() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let (v, t85, t27) = (b.num(1e-3), b.num(85.0), b.num(27.0));
    let al = b.alter(top, Target::TEMP, t85, Span::NONE);
    let op = b.option(top, OptionKey::RelTol, v, Span::NONE);
    let fl = b.option(top, OptionKey::Klu, ExprId::NONE, Span::NONE);
    let pr = b.option_private(top, "xyce", "timeint.reltol", v, Span::NONE);
    let en = b.env(top, t85, t27, Span::NONE);
    let ic = b.node_set(
        top,
        NodeSetKind::Ic,
        IcScope::Transient,
        &["x1", "n"],
        v,
        Span::NONE,
    );
    let d = &d;
    assert_eq!(
        d.row::<Alter>(al.index),
        Alter {
            target: Target::TEMP,
            value: t85
        }
    );
    let o: OptionRow = d.row(op.index);
    assert_eq!(
        (o.key, o.value, o.private, o.dialect),
        (
            OptionKey::RelTol,
            v,
            spicerack_ir::NameId::NONE,
            spicerack_ir::NameId::NONE
        )
    );
    assert_eq!(d.row::<OptionRow>(fl.index).value, ExprId::NONE);
    let o: OptionRow = d.row(pr.index);
    assert_eq!(
        (o.key, d.str(o.private), d.str(o.dialect)),
        (OptionKey::Private, "timeint.reltol", "xyce")
    );
    assert_eq!(
        d.row::<Env>(en.index),
        Env {
            temp: t85,
            tnom: t27
        }
    );
    let n: NodeSet = d.row(ic.index);
    assert_eq!(
        (n.kind, n.applies, n.next_only, n.value),
        (NodeSetKind::Ic, IcScope::Transient, false, v)
    );
    assert_eq!(d.names(n.node).len(), 2);
    assert_eq!(n.reference.len, 0);
}

#[test]
#[should_panic(expected = "option_private")]
fn option_with_the_private_key_asserts() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    b.option(top, OptionKey::Private, ExprId::NONE, Span::NONE);
}

#[test]
#[should_panic(expected = "flag")]
fn a_flag_with_a_value_asserts() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let one = b.num(1.0);
    b.option(top, OptionKey::Klu, one, Span::NONE);
}

#[test]
#[should_panic(expected = "tnom")]
fn env_without_tnom_asserts() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let t = b.num(27.0);
    b.env(top, t, ExprId::NONE, Span::NONE);
}

#[test]
fn outputs_hold_their_probes() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let p = [
        b.probe_v(&["out"]),
        b.probe_i(&["v1"]).with_part(AcPart::Mag),
    ];
    let all = b.save(top, None, &p, Span::NONE);
    let tr = b.save(top, Some(AnalysisKind::Tran), &p[..1], Span::NONE);
    let pr = b.print(top, AnalysisKind::Hb, PrintView::HbTime, &p, Span::NONE);
    let s: Save = d.row(all.index);
    assert!(s.every_analysis);
    assert_eq!(d.probes(s.probes), &p);
    let s: Save = d.row(tr.index);
    assert_eq!((s.every_analysis, s.analysis), (false, AnalysisKind::Tran));
    let q: Print = d.row(pr.index);
    assert_eq!((q.analysis, q.view), (AnalysisKind::Hb, PrintView::HbTime));
    assert_eq!(d.probes(q.probes), &p);
}

#[test]
#[should_panic(expected = "no probe")]
fn a_save_of_nothing_asserts() {
    let (mut d, top) = deck();
    d.builder().save(top, None, &[], Span::NONE);
}

#[test]
fn a_measure_is_a_definition() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let (half, one) = (b.num(0.5), b.num(1.0));
    let vin = b.probe_v(&["in"]);
    let vout = b.probe_v(&["out"]);
    let row = MeasureTrigTarg {
        trig: Event::cross(vin, Edge::Rise, half, one),
        targ: Event::cross(vout, Edge::Rise, half, one),
    };
    let m = b.measure(top, "tdel", AnalysisKind::Tran, row, Span::NONE);
    let x = b.param_ref("tdel");
    let p = b.measure(
        top,
        "twice",
        AnalysisKind::Tran,
        MeasureParam { value: x },
        Span::NONE,
    );
    let h: Measure = d.row(m.index);
    assert_eq!(
        (d.str(h.name), h.analysis, h.kind),
        ("tdel", AnalysisKind::Tran, MeasureKind::TrigTarg)
    );
    assert_eq!(d.detail::<MeasureTrigTarg>(h.detail), row);
    assert_eq!(d.row::<Measure>(p.index).kind, MeasureKind::Param);
    let defs: Vec<(DefKind, &str, ItemRef)> = d
        .definitions()
        .iter()
        .map(|x| (x.kind, d.str(x.name), x.item))
        .collect();
    assert_eq!(
        defs,
        [
            (DefKind::Measure, "tdel", m),
            (DefKind::Measure, "twice", p)
        ]
    );
}

#[test]
fn statistics_and_dc_hold_their_lists() {
    let (mut d, top) = deck();
    let mut b = d.builder();
    let spread = b.num(0.05);
    let v = [Vary {
        param: Ref::unresolved(b.name("vth")),
        dist: VaryDist::LogNormal,
        percent: true,
        mean: ExprId::NONE,
        spread,
    }];
    let process = b.varies(&v);
    let row = Statistics {
        process,
        mismatch: List::EMPTY,
        process_correlations: List::EMPTY,
        mismatch_correlations: List::EMPTY,
        truncate: ExprId::NONE,
        process_truncate: ExprId::NONE,
        mismatch_truncate: ExprId::NONE,
    };
    let st = b.statistics(top, row, Span::NONE);
    let sweep = SweepSpec::dec(b.num(1.0), b.num(1e3), b.num(10.0));
    let src = b.target(TargetKind::SourceDc, &[], "v1", None);
    let axes = b.dc_axes(&[DcAxis { target: src, sweep }]);
    let dc = b.analysis(
        top,
        None,
        Dc {
            axes,
            restart: true,
        },
        Span::NONE,
    );
    assert_eq!(st.kind, ItemKind::Statistics);
    assert_eq!(d.row::<Statistics>(st.index), row);
    assert_eq!(d.varies(row.process), &v);
    let h: Analysis = d.row(dc.index);
    let got = d.detail::<Dc>(h.detail);
    assert!(got.restart);
    assert_eq!(d.dc_axes(got.axes)[0].target, src);
}
