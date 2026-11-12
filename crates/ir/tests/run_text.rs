//! IR text of the run rows (the analyses; the other run items are `run_text_items.rs`'s):
//! hand-written text in, the rows the test builds by hand out (`parse`
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

use spicerack_ir::dc_ac::{
    Ac, Dc, DcAxis, DcXf, Disto, Net, Noise, NoiseRef, Pz, PzFind, PzInput, PzTransfer, Sens,
    SensAc, Sp, Tf, TimedOp, Xf,
};
use spicerack_ir::periodic::{FreqAxis, Hb, HbTruncate, Pac, Pnoise, Pss, PssMode, Pstb, Pxf, Stb};
use spicerack_ir::tran::{Four, StepCap, StepCeiling, Tran};
use spicerack_ir::{
    Builder, Design, Diags, ExprId, FileId, List, Literal, NameId, NumForm, Role, ScopeId, Span,
    SweepEnd, SweepSpec, Target, TargetKind, dump, link, parse,
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
fn dc_and_small_signal_analyses() {
    let text = "file \"t.sp\" deck
  analysis op name=op1
  analysis dc (source_dc v1) (lin start=0 stop=5 step=1) (temp) (list values=[0, 27]) name=s2 restart
  analysis ac (dec on_stop start=1 stop=1g points=10)
  analysis noise (vdiff (out) (ref)) source=v1 (oct start=1k stop=1meg points=3) points_per_summary=5 source_ac
  analysis tf (i (v2)) source=v1
  analysis dc_xf (v (out))
  analysis xf (v (out)) (log start=1 stop=1k points=4)
  analysis sens (v (out))
  analysis sens_ac (v (out)) (lin start=1 stop=2 points=2)
  analysis pz source voltage both out_p=out out_n=0 source=v1
  analysis sp (lin center_span start=1g stop=2g points=3) ports=(p1 p2) noise
  analysis net (v (out)) source=v1 rin=50 rout=50
  analysis disto (dec start=1 stop=1k points=5) f2_over_f1=1
";
    case(text, |b, top| {
        b.op(top, Some("op1"), Span::NONE);
        let v1 = b.target(TargetKind::SourceDc, &[], "v1", None);
        let s1 = sweep(b, SweepSpec::lin_step, ["0", "5", "1"]);
        let vals = [n(b, "0"), n(b, "27")];
        let s2 = SweepSpec::list(b.exprs(&vals));
        let name = b.name("s2");
        let axes = b.dc_axes(&[
            DcAxis {
                target: v1,
                sweep: s1,
                name: NameId::NONE,
            },
            DcAxis {
                target: Target::TEMP,
                sweep: s2,
                name,
            },
        ]);
        b.analysis(
            top,
            None,
            Dc {
                axes,
                restart: true,
            },
            Span::NONE,
        );
        let s = sweep(b, SweepSpec::dec, ["1", "1g", "10"]).ending(SweepEnd::OnStop);
        b.analysis(top, None, Ac { sweep: s }, Span::NONE);
        let output = b.probe_vdiff(&["out"], &["ref"]);
        let source = b.name("v1");
        let s = sweep(b, SweepSpec::oct, ["1k", "1meg", "3"]);
        let pps = n(b, "5");
        let row = Noise {
            output,
            source,
            sweep: s,
            points_per_summary: pps,
            ac_sweep: false,
            input_ref: NoiseRef::SourceAc,
        };
        b.analysis(top, None, row, Span::NONE);
        let output = b.probe_i(&["v2"]);
        let source = b.name("v1");
        b.analysis(top, None, Tf { output, source }, Span::NONE);
        let output = b.probe_v(&["out"]);
        b.analysis(top, None, DcXf { output }, Span::NONE);
        let output = b.probe_v(&["out"]);
        let s = sweep(b, SweepSpec::log, ["1", "1k", "4"]);
        b.analysis(top, None, Xf { output, sweep: s }, Span::NONE);
        let output = b.probe_v(&["out"]);
        b.analysis(top, None, Sens { output }, Span::NONE);
        let output = b.probe_v(&["out"]);
        let s = sweep(b, SweepSpec::lin_points, ["1", "2", "2"]);
        b.analysis(top, None, SensAc { output, sweep: s }, Span::NONE);
        let (out_p, out_n, source) = (b.name("out"), b.name("0"), b.name("v1"));
        let row = Pz {
            input: PzInput::Source,
            transfer: PzTransfer::Voltage,
            find: PzFind::Both,
            in_p: NameId::NONE,
            in_n: NameId::NONE,
            out_p,
            out_n,
            source,
        };
        b.analysis(top, None, row, Span::NONE);
        let mut s = sweep(b, SweepSpec::lin_points, ["1g", "2g", "3"]);
        s.bounds = spicerack_ir::SweepBounds::CenterSpan;
        let ports = b.names(&["p1", "p2"]);
        b.analysis(
            top,
            None,
            Sp {
                sweep: s,
                ports,
                noise: true,
            },
            Span::NONE,
        );
        let output = b.probe_v(&["out"]);
        let source = b.name("v1");
        let (rin, rout) = (n(b, "50"), n(b, "50"));
        b.analysis(
            top,
            None,
            Net {
                output,
                source,
                rin,
                rout,
            },
            Span::NONE,
        );
        let s = sweep(b, SweepSpec::dec, ["1", "1k", "5"]);
        let f = n(b, "1");
        b.analysis(
            top,
            None,
            Disto {
                sweep: s,
                f2_over_f1: f,
            },
            Span::NONE,
        );
    });
}

#[test]
fn time_domain_and_periodic_analyses() {
    let text = "file \"t.sp\" deck
  analysis tran name=t1 stop=1u print_step=1n segments=[2n, 2u] cap=ngspice_rule uic
  analysis tran stop=2u record_start=1u print_step=1n max_step=2n
  analysis four fundamental=1k harmonics=9 periods=1 (v (out)) (v (in))
  option n_freqs value=10
  analysis four fundamental=2k periods=1 (v (out))
  analysis timed_op times=[1u, 2u]
  analysis hb tones=[1g, 1.1g] harmonics=[5, 5] intermod=7 truncate=diamond
  analysis pss autonomous fundamental=1meg tstab=1u harmonics=10 osc_p=out osc_n=0 sc_iter=50 steady_coeff=1m
  analysis pac (lin start=1 stop=1k points=11) sidebands=3 relharm=1 relative
  analysis pnoise (v (out)) source=v1 (dec start=1 stop=1k points=2) sidebands=2
  analysis pxf (v (out)) (dec start=1 stop=1k points=2)
  analysis pstb probe=vp (dec start=1 stop=1k points=2)
  analysis stb probe=vp (dec start=1 stop=1k points=2)
  analysis dc_inc
";
    case(text, |b, top| {
        let (stop, step) = (n(b, "1u"), n(b, "1n"));
        let seg = [n(b, "2n"), n(b, "2u")];
        let segments = b.exprs(&seg);
        let row = Tran {
            stop,
            record_start: NO,
            print_step: step,
            initial_step: NO,
            noise_fmax: NO,
            noise_fmin: NO,
            noise_seed: NO,
            segments,
            ceiling: StepCeiling::Rule(StepCap::NgspiceRule),
            uic: true,
            startup: false,
        };
        b.analysis(top, Some("t1"), row, Span::NONE);
        let (stop2, rec, step2, cap2) = (n(b, "2u"), n(b, "1u"), n(b, "1n"), n(b, "2n"));
        let given = Tran {
            stop: stop2,
            record_start: rec,
            print_step: step2,
            ceiling: StepCeiling::Given(cap2),
            uic: false,
            segments: List::EMPTY,
            ..row
        };
        b.analysis(top, None, given, Span::NONE);
        let (f, h, p) = (n(b, "1k"), n(b, "9"), n(b, "1"));
        let probes = [b.probe_v(&["out"]), b.probe_v(&["in"])];
        let outputs = b.probes(&probes);
        let row = Four {
            fundamental: f,
            harmonics: h,
            periods: p,
            grid: NO,
            outputs,
        };
        b.analysis(top, None, row, Span::NONE);
        let v = n(b, "10");
        b.option(top, spicerack_ir::OptionKey::NFreqs, v, Span::NONE);
        let (f, p) = (n(b, "2k"), n(b, "1"));
        let o = [b.probe_v(&["out"])];
        let outputs = b.probes(&o);
        let row = Four {
            fundamental: f,
            harmonics: NO,
            periods: p,
            grid: NO,
            outputs,
        };
        b.analysis(top, None, row, Span::NONE);
        let t = [n(b, "1u"), n(b, "2u")];
        let times = b.exprs(&t);
        b.analysis(top, None, TimedOp { times }, Span::NONE);
        let t = [n(b, "1g"), n(b, "1.1g")];
        let tones = b.exprs(&t);
        let h = [n(b, "5"), n(b, "5")];
        let harmonics = b.exprs(&h);
        let intermod = n(b, "7");
        b.analysis(
            top,
            None,
            Hb {
                tones,
                harmonics,
                intermod,
                truncate: HbTruncate::Diamond,
            },
            Span::NONE,
        );
        let (f, t, h) = (n(b, "1meg"), n(b, "1u"), n(b, "10"));
        let (osc_p, osc_n) = (b.name("out"), b.name("0"));
        let (sc_iter, steady_coeff) = (n(b, "50"), n(b, "1m"));
        let row = Pss {
            mode: PssMode::Autonomous,
            uic: false,
            fundamental: f,
            tstab: t,
            harmonics: h,
            points: NO,
            osc_p,
            osc_n,
            sc_iter,
            steady_coeff,
        };
        b.analysis(top, None, row, Span::NONE);
        let s = sweep(b, SweepSpec::lin_points, ["1", "1k", "11"]);
        let (sb, rh) = (n(b, "3"), n(b, "1"));
        b.analysis(
            top,
            None,
            Pac {
                sweep: s,
                sidebands: sb,
                relharm: rh,
                axis: FreqAxis::Relative,
            },
            Span::NONE,
        );
        let output = b.probe_v(&["out"]);
        let source = b.name("v1");
        let s = sweep(b, SweepSpec::dec, ["1", "1k", "2"]);
        let sb = n(b, "2");
        let row = Pnoise {
            output,
            source,
            sweep: s,
            sidebands: sb,
            relharm: NO,
            axis: FreqAxis::Absolute,
        };
        b.analysis(top, None, row, Span::NONE);
        let output = b.probe_v(&["out"]);
        let s = sweep(b, SweepSpec::dec, ["1", "1k", "2"]);
        b.analysis(
            top,
            None,
            Pxf {
                output,
                sweep: s,
                sidebands: NO,
                relharm: NO,
                axis: FreqAxis::Absolute,
            },
            Span::NONE,
        );
        let probe = b.name("vp");
        let s = sweep(b, SweepSpec::dec, ["1", "1k", "2"]);
        b.analysis(
            top,
            None,
            Pstb {
                probe,
                sweep: s,
                sidebands: NO,
                relharm: NO,
                axis: FreqAxis::Absolute,
            },
            Span::NONE,
        );
        let probe = b.name("vp");
        let s = sweep(b, SweepSpec::dec, ["1", "1k", "2"]);
        b.analysis(top, None, Stb { probe, sweep: s }, Span::NONE);
        b.dc_inc(top, None, Span::NONE);
    });
}
