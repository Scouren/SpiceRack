//! IR text of the run rows (loops, alters, state, outputs, measures, statistics; the
//! analyses are `run_text.rs`'s): hand-written text in, the rows the test builds by hand out (`parse`
//! then `eq_ignoring_spans`), and `dump` of those rows is the same text (canonical). Every run
//! item kind and every analysis, loop and measure detail kind appears once. Oracle: spec example
//! (the forms documented in the run files' headers, written here by hand).
//!
//! The built side calls the builder in the order the text reads (ids are compared), then `link`,
//! as `parse` does.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (IR text row); `crates/ir/src/text.rs` grammar; the run files'
//! "IR text" header sections.
//!
//! Contents: tests only.

use spicerack_ir::measure::{
    MeasureDeriv, MeasureFind, MeasureParam, MeasureStat, MeasureTrigTarg, MeasureWhen,
};
use spicerack_ir::plan::{MonteCarlo, Sampler, SweepLoop, TableLoop, Variations};
use spicerack_ir::stats::Correlation;
use spicerack_ir::{
    AcPart, AnalysisKind, Applies, BinOp, Builder, Design, Diags, Edge, Env, Event, ExprId, FileId,
    IcScope, List, Literal, LoopEnd, NodeSet, NodeSetKind, NumForm, OptionKey, OptionRow,
    PrintView, Probe, Ref, Role, ScopeId, Span, Stat, Statistics, SweepSpec, Target, TargetKind,
    Vary, VaryDist, dump, link, parse,
};

const NO: ExprId = ExprId::NONE;

// ---- private ----

/// Parses `text` and compares it with what `build` builds into a deck `t.sp`; then dumps the
/// built design and compares the text.
fn case(text: &str, build: impl FnOnce(&mut Builder<'_>, ScopeId)) {
    let mut parsed = Design::new();
    let mut diags = Diags::new();
    parse(text, &mut parsed.builder(), &mut diags);
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags.rows());
    let mut built = Design::new();
    {
        let mut b = built.builder();
        let top = b.file("t.sp", Role::Deck).1;
        build(&mut b, top);
    }
    link(&mut built, &mut Diags::new());
    let mut out = String::new();
    dump(&built, FileId(0), &mut out);
    assert_eq!(out, text, "dump of the built rows");
    assert!(
        parsed.eq_ignoring_spans(&built),
        "parsed rows differ from the built rows"
    );
}

/// A literal as the grammar spells it: digits, an optional fraction, an optional scale word.
fn lit(text: &str) -> Literal {
    let scales = [
        ("meg", 6),
        ("peta", 15),
        ("a", -18),
        ("f", -15),
        ("p", -12),
        ("n", -9),
        ("u", -6),
        ("m", -3),
        ("c", -2),
        ("k", 3),
        ("g", 9),
        ("t", 12),
    ];
    let digits_end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(text.len());
    let (num, suffix) = text.split_at(digits_end);
    let frac = num.split_once('.').map_or(0, |(_, f)| f.len()) as u8;
    let mantissa: i64 = num.replace('.', "").parse().unwrap();
    let (exp10, form) = match scales.iter().find(|(s, _)| *s == suffix) {
        Some(&(_, e)) => (e, NumForm::Scale),
        None => {
            assert!(suffix.is_empty(), "test literal {text}");
            (0, NumForm::Plain)
        }
    };
    Literal {
        mantissa,
        frac,
        exp10,
        form,
    }
}

fn n(b: &mut Builder<'_>, text: &str) -> ExprId {
    b.literal(lit(text))
}

fn sweep(
    b: &mut Builder<'_>,
    kind: fn(ExprId, ExprId, ExprId) -> SweepSpec,
    s: [&str; 3],
) -> SweepSpec {
    let (x, y, z) = (n(b, s[0]), n(b, s[1]), n(b, s[2]));
    kind(x, y, z)
}

#[test]
fn loops_alters_and_state() {
    let text = "file \"t.sp\" deck
  env temp=85 tnom=27
  env temp=27 whole_run
  option rel_tol value=1m
  option klu
  option private timeint.reltol xyce value=1m whole_run
  loop sweep keep_last name=sw1 (param r) (lin start=1k stop=10k step=1k)
    alter (device_param r1 r path=(x1)) value=2k
    analysis op
  loop table (param a) (temp) values=[1, 27, 2, 85]
  loop monte_carlo runs=100 seed=7 samples=[1, 3] all latin_hypercube nominal
  alter (option private tran_rmax vacask) value=10
  alter (option rel_tol) value=1m
  nodeset ic transient node=(x1 n) value=1
  nodeset nodeset every_op next_only node=(a) reference=(b) value=0.5 whole_run
  nodeset device every_op node=(c1) value=0
  elaborate (div amp)
";
    case(text, |b, top| {
        let (t, tn) = (n(b, "85"), n(b, "27"));
        b.env(top, t, tn, Span::NONE);
        let t = n(b, "27");
        b.push(
            top,
            Env {
                temp: t,
                tnom: NO,
                holds: Applies::WholeRun,
            },
            Span::NONE,
        );
        let v = n(b, "1m");
        b.option(top, OptionKey::RelTol, v, Span::NONE);
        b.option(top, OptionKey::Klu, NO, Span::NONE);
        let value = n(b, "1m");
        let (private, dialect) = (b.name("timeint.reltol"), b.name("xyce"));
        let row = OptionRow {
            key: OptionKey::Private,
            private,
            dialect,
            value,
            holds: Applies::WholeRun,
        };
        b.push(top, row, Span::NONE);
        b.name("sw1"); // the text names the loop before its target
        let r = b.target(TargetKind::Param, &[], "r", None);
        let s = sweep(b, SweepSpec::lin_step, ["1k", "10k", "1k"]);
        let (_, body) = b.loop_(
            top,
            Some("sw1"),
            SweepLoop {
                target: r,
                sweep: s,
            },
            LoopEnd::KeepLast,
            Span::NONE,
        );
        let dev = b.target(TargetKind::DeviceParam, &["x1"], "r1", Some("r"));
        let v = n(b, "2k");
        b.alter(body, dev, v, Span::NONE);
        b.op(body, None, Span::NONE);
        let a = b.target(TargetKind::Param, &[], "a", None);
        let columns = b.targets(&[a, Target::TEMP]);
        let vals = [n(b, "1"), n(b, "27"), n(b, "2"), n(b, "85")];
        let values = b.exprs(&vals);
        b.loop_(
            top,
            None,
            TableLoop { columns, values },
            LoopEnd::Restore,
            Span::NONE,
        );
        let (runs, seed) = (n(b, "100"), n(b, "7"));
        let s = [n(b, "1"), n(b, "3")];
        let samples = b.exprs(&s);
        let mc = MonteCarlo {
            runs,
            seed,
            samples,
            variations: Variations::All,
            sampler: Sampler::LatinHypercube,
            nominal: true,
        };
        b.loop_(top, None, mc, LoopEnd::Restore, Span::NONE);
        let opt = b.target_option(OptionKey::Private, Some(("vacask", "tran_rmax")));
        let v = n(b, "10");
        b.alter(top, opt, v, Span::NONE);
        let opt = b.target_option(OptionKey::RelTol, None);
        let v = n(b, "1m");
        b.alter(top, opt, v, Span::NONE);
        let v = n(b, "1");
        b.node_set(
            top,
            NodeSetKind::Ic,
            IcScope::Transient,
            &["x1", "n"],
            v,
            Span::NONE,
        );
        let node = b.names(&["a"]);
        let reference = b.names(&["b"]);
        let value = n(b, "0.5");
        let row = NodeSet {
            kind: NodeSetKind::Nodeset,
            applies: IcScope::EveryOp,
            next_only: true,
            node,
            reference,
            value,
            holds: Applies::WholeRun,
        };
        b.push(top, row, Span::NONE);
        let v = n(b, "0");
        b.node_set(
            top,
            NodeSetKind::Device,
            IcScope::EveryOp,
            &["c1"],
            v,
            Span::NONE,
        );
        b.elaborate(top, &["div", "amp"], Span::NONE);
    });
}

#[test]
fn outputs_measures_and_statistics() {
    let text = "file \"t.sp\" deck
  save (v (out)) (i mag (v1))
  save analysis=tran (all_v) (i_terminal terminal=2 (m1)) (param (m1) (gm)) (power (r1)) (expr expr=(v(a) * 2))
  print hb hb_time (v (out))
  measure tdel tran trig_targ (cross rise (v (in)) value=0.5 count=1) (cross fall last (v (out)) value=0.5 delay=1n)
  measure tw tran when (cross cross (v (a)) value=(v(b)) count=2)
  measure f1 tran find (v (out)) (at value=1u)
  measure d1 tran deriv (v (out)) (at value=1u)
  measure a1 tran stat avg (v (out)) (bound) (at value=1u)
  measure p1 tran param value=(tdel * 2)
  statistics process (vth gauss spread=1m) (tox unif percent mean=1n spread=10) mismatch (vth log_normal spread=1m) process_correlate (vth tox cc=0.5) truncate=4
";
    case(text, |b, top| {
        let p = [
            b.probe_v(&["out"]),
            b.probe_i(&["v1"]).with_part(AcPart::Mag),
        ];
        b.save(top, None, &p, Span::NONE);
        let all = Probe::category(spicerack_ir::ProbeKind::AllV);
        let it = b.probe_terminal(&["m1"], 2);
        let pp = b.probe_param(&["m1"], "gm");
        let pw = b.probe_power(&["r1"]);
        let va = b.v("a", None);
        let two = n(b, "2");
        let x = b.binary(BinOp::Mul, va, two);
        let pe = b.probe_expr(x);
        b.save(
            top,
            Some(AnalysisKind::Tran),
            &[all, it, pp, pw, pe],
            Span::NONE,
        );
        let o = b.probe_v(&["out"]);
        b.print(top, AnalysisKind::Hb, PrintView::HbTime, &[o], Span::NONE);
        b.name("tdel"); // the text names the measure before its probes
        let vin = b.probe_v(&["in"]);
        let (h, one) = (n(b, "0.5"), n(b, "1"));
        let trig = Event::cross(vin, Edge::Rise, h, one);
        let vout = b.probe_v(&["out"]);
        let (h, dl) = (n(b, "0.5"), n(b, "1n"));
        let targ = Event {
            last: true,
            delay: dl,
            ..Event::cross(vout, Edge::Fall, h, NO)
        };
        b.measure(
            top,
            "tdel",
            AnalysisKind::Tran,
            MeasureTrigTarg { trig, targ },
            Span::NONE,
        );
        b.name("tw");
        let a = b.probe_v(&["a"]);
        let vb = b.v("b", None);
        let two = n(b, "2");
        let at = Event::cross(a, Edge::Cross, vb, two);
        b.measure(
            top,
            "tw",
            AnalysisKind::Tran,
            MeasureWhen { at },
            Span::NONE,
        );
        let find = b.probe_v(&["out"]);
        let t = n(b, "1u");
        b.measure(
            top,
            "f1",
            AnalysisKind::Tran,
            MeasureFind {
                find,
                at: Event::at(t),
            },
            Span::NONE,
        );
        let signal = b.probe_v(&["out"]);
        let t = n(b, "1u");
        b.measure(
            top,
            "d1",
            AnalysisKind::Tran,
            MeasureDeriv {
                signal,
                at: Event::at(t),
            },
            Span::NONE,
        );
        let signal = b.probe_v(&["out"]);
        let t = n(b, "1u");
        let row = MeasureStat {
            stat: Stat::Avg,
            signal,
            from: Event::BOUND,
            to: Event::at(t),
        };
        b.measure(top, "a1", AnalysisKind::Tran, row, Span::NONE);
        let td = b.param_ref("tdel");
        let two = n(b, "2");
        let value = b.binary(BinOp::Mul, td, two);
        b.measure(
            top,
            "p1",
            AnalysisKind::Tran,
            MeasureParam { value },
            Span::NONE,
        );
        let vth = Ref::unresolved(b.name("vth"));
        let s = n(b, "1m");
        let v1 = Vary {
            param: vth,
            dist: VaryDist::Gauss,
            percent: false,
            mean: NO,
            spread: s,
        };
        let tox = Ref::unresolved(b.name("tox"));
        let (mean, s) = (n(b, "1n"), n(b, "10"));
        let v2 = Vary {
            param: tox,
            dist: VaryDist::Unif,
            percent: true,
            mean,
            spread: s,
        };
        let process = b.varies(&[v1, v2]);
        let s = n(b, "1m");
        let v3 = Vary {
            param: vth,
            dist: VaryDist::LogNormal,
            percent: false,
            mean: NO,
            spread: s,
        };
        let mismatch = b.varies(&[v3]);
        let cc = n(b, "0.5");
        let process_correlations = b.correlations(&[Correlation { a: vth, b: tox, cc }]);
        let truncate = n(b, "4");
        let row = Statistics {
            process,
            mismatch,
            process_correlations,
            mismatch_correlations: List::EMPTY,
            truncate,
            process_truncate: NO,
            mismatch_truncate: NO,
        };
        b.statistics(top, row, Span::NONE);
    });
}

#[test]
fn references_in_detail_rows_and_pools_are_linked() {
    // A9: the linker reaches a loop's target, a DC axis's target, a table's columns and a
    // statistics block's variations and correlations, so each binds to the `param` it names.
    let text = "file \"t.sp\" deck
  param r = 1
  param s = 2
  loop sweep (param r) (list values=[1])
    analysis dc (param s) (lin start=0 stop=1 step=1)
  loop table (param r) (param s) values=[1, 2]
  statistics process (r gauss spread=1) (s gauss spread=1) process_correlate (r s cc=0.5)
  subckt c1 ()
  elaborate (c1)
";
    let mut d = spicerack_ir::Design::new();
    let mut diags = Diags::new();
    parse(text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let mut bound = 0;
    let mut unbound = Vec::new();
    for item in (0..4).flat_map(|k| d.items(spicerack_ir::ScopeId(k)).to_vec()) {
        let item = &item;
        let mut refs = Vec::new();
        let mut copy = d.clone();
        copy.editor().map_refs(*item, &mut |r, _| refs.push(*r));
        for r in refs {
            if r.def.is_none() {
                unbound.push(r)
            } else {
                bound += 1
            }
        }
    }
    assert!(unbound.is_empty(), "{unbound:?}");
    assert_eq!(
        bound, 9,
        "sweep 1, dc axis 1, table columns 2, varies 2, correlation ends 2, elaborate 1"
    );
    let mut out = String::new();
    dump(&d, FileId(0), &mut out);
    assert_eq!(out, text);
}
