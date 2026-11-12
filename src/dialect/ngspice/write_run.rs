//! Run items → ngspice dot cards: `Analysis` → `.op .dc .ac .tran .noise .tf .sens .pz .disto
//! .sp .four`, each in its positional form; `Option`, `Env`, `NodeSet` → `.options`, `.temp`,
//! `.ic`, `.nodeset` (with AMEND-A12 A14). Items are written in IR order (R25: no writer
//! reorders run items); ngspice has no statistics block.
//!
//! Each function writes one item of a design legal for `FEATURES` (the spine checked it), sees
//! only the IR, and never learns a source. `FEATURES` is `read_run::FEATURES`, the reader's
//! image (L5); what a feature cannot state and ngspice cannot spell is refused by [`admits`]
//! (R21). Codes: `W1300–W1349` (this file).
//!
//! Cites: ngspice-45 `inp2dot.c` (each card's form), `traninit.c:29` (a zero `tmax` is none);
//! `rewrite/DIALECT-NGSPICE.md` §4; `rewrite/RULINGS.md` R21, R24–R26.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn analysis`, `option`, `env`, `node_set`, `statistics`

use super::codes::{self, Row};
use super::expr_write;
use super::number::{self, Grammar};
use super::read_run;
use super::write::Out;
use crate::ir::dc_ac::{Ac, Dc, Disto, Noise, Pz, PzFind, PzTransfer, Sens, SensAc, Sp, Tf};
use crate::ir::tran::{Four, StepCap, Tran};
use crate::ir::{self, AnalysisKind};

/// What the run area spells: exactly what its reader produces (L5).
pub(super) const FEATURES: ir::FeatureSet = read_run::FEATURES;

/// This area's writing codes, `W1300–W1349`.
pub(super) const CODES: &[Row] = &[
    refusal(
        FOUR_HARMONICS,
        "a Fourier analysis with another harmonic count than ngspice's default",
        "ngspice-45 fourier.c:69-70",
        "ngspice takes the count from the nfreqs option, not from the .four card; refused.",
    ),
    refusal(
        CARD_FORM,
        "an analysis field ngspice's card has no position for",
        "ngspice-45 inp2dot.c dot_dc, dot_ac, dot_tran; traninit.c:29",
        "a DC axis over another target or sweep, a frequency sweep by step, or a transient ceiling ngspice would read back otherwise; refused.",
    ),
    refusal(
        ANALYSIS_NAME,
        "a named analysis",
        "ngspice-45 inp2dot.c (no card has a name)",
        "an ngspice analysis card has no name; refused.",
    ),
];

/// A Fourier analysis whose harmonic count is not ngspice's default (DC and 9 harmonics): its
/// count is the deck's `nfreqs` option, which no `.four` card states.
const FOUR_HARMONICS: ir::Code = ir::Code::new(b'W', 1300);
/// An analysis field ngspice's card has no position for: a DC axis other than a source, a
/// resistor or `temp` swept by step; a frequency sweep by step; a transient ceiling without a
/// start, a zero ceiling, or another default ceiling than ngspice's.
const CARD_FORM: ir::Code = ir::Code::new(b'W', 1301);
/// A named analysis: an ngspice card has no name.
const ANALYSIS_NAME: ir::Code = ir::Code::new(b'W', 1302);

/// Appends a refusal for this area's items ngspice cannot spell for a reason no feature states
/// (R21): a card form (`W1300–W1302`), a value that reads the run (`W1005`: numparam evaluates a
/// card's `{…}` once) or a literal ngspice reads back otherwise (`W1003`).
pub(super) fn admits(design: &ir::Design, item: ir::ItemRef, diags: &mut ir::Diags) {
    let span = design.span_of(item);
    let mut refuse = |code| diags.push(code, span, 0);
    if item.kind != ir::ItemKind::Analysis {
        return;
    }
    let a: ir::Analysis = design.row(item.index);
    if !a.name.is_none() {
        refuse(ANALYSIS_NAME);
    }
    let mut values = Vec::new();
    let mut form_ok = true;
    let freq = |s: ir::SweepSpec, values: &mut Vec<ir::ExprId>| {
        values.extend([s.start, s.stop, s.points]);
        s.kind != ir::SweepKind::Lin || s.step.is_none()
    };
    match a.kind {
        AnalysisKind::Dc => {
            let dc: Dc = design.detail(a.detail);
            for axis in design.dc_axes(dc.axes) {
                let t = axis.target;
                let name = (!t.a.name.is_none())
                    .then(|| design.str(t.a.name).as_bytes()[0].to_ascii_lowercase());
                form_ok &= t.path.len == 0
                    && match t.kind {
                        ir::TargetKind::Temp => true,
                        ir::TargetKind::SourceDc => matches!(name, Some(b'v' | b'i')),
                        ir::TargetKind::ElementValue => name == Some(b'r'),
                        _ => false,
                    };
                let s = axis.sweep;
                form_ok &= s.kind == ir::SweepKind::Lin && !s.step.is_none();
                values.extend([s.start, s.stop, s.step]);
            }
        }
        AnalysisKind::Ac => form_ok &= freq(design.detail::<Ac>(a.detail).sweep, &mut values),
        AnalysisKind::Noise => {
            let n: Noise = design.detail(a.detail);
            form_ok &= freq(n.sweep, &mut values);
            values.push(n.points_per_summary);
        }
        AnalysisKind::SensAc => {
            form_ok &= freq(design.detail::<SensAc>(a.detail).sweep, &mut values);
        }
        AnalysisKind::Disto => {
            let d: Disto = design.detail(a.detail);
            form_ok &= freq(d.sweep, &mut values);
            values.push(d.f2_over_f1);
        }
        AnalysisKind::Sp => form_ok &= freq(design.detail::<Sp>(a.detail).sweep, &mut values),
        AnalysisKind::Tran => {
            let t: Tran = design.detail(a.detail);
            let zero_max = !t.max_step.is_none()
                && matches!(design.expr(t.max_step), ir::Expr::Num(n) if design.num(n).mantissa == 0);
            form_ok &= t.cap == StepCap::NgspiceRule
                && (t.max_step.is_none() || !t.record_start.is_none())
                && !zero_max;
            values.extend([t.print_step, t.stop, t.record_start, t.max_step]);
        }
        AnalysisKind::Four => {
            let f: Four = design.detail(a.detail);
            let nine = matches!(design.expr(f.harmonics), ir::Expr::Num(n) if design.num(n).value() == 9.0);
            if !nine {
                refuse(FOUR_HARMONICS);
            }
            values.push(f.fundamental);
        }
        _ => {}
    }
    if !form_ok {
        refuse(CARD_FORM);
    }
    for e in values.into_iter().filter(|e| !e.is_none()) {
        if ir::expr_class(design, e) >= ir::ExprClass::Temp {
            refuse(codes::READS_RUN);
        }
        if !expr_write::literals_spell(design, e) {
            refuse(codes::LITERAL_FORM);
        }
    }
}

/// An `Analysis` as its card.
pub(super) fn analysis(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let a: ir::Analysis = d.row(item.index);
    assert!(
        a.name.is_none(),
        "ngspice write: a named analysis (admits refuses it)"
    );
    let mut t = String::new();
    match a.kind {
        AnalysisKind::Op => t.push_str(".op"),
        AnalysisKind::Dc => {
            t.push_str(".dc");
            let dc: Dc = d.detail(a.detail);
            for axis in d.dc_axes(dc.axes) {
                t.push(' ');
                match axis.target.kind {
                    ir::TargetKind::Temp => t.push_str("temp"),
                    _ => t.push_str(d.str(axis.target.a.name)),
                }
                for e in [axis.sweep.start, axis.sweep.stop, axis.sweep.step] {
                    value(d, e, &mut t);
                }
            }
        }
        AnalysisKind::Ac => {
            t.push_str(".ac");
            sweep(d, d.detail::<Ac>(a.detail).sweep, &mut t);
        }
        AnalysisKind::Tran => {
            let r: Tran = d.detail(a.detail);
            t.push_str(".tran");
            for e in [r.print_step, r.stop, r.record_start, r.max_step] {
                if !e.is_none() {
                    value(d, e, &mut t);
                }
            }
            if r.uic {
                t.push_str(" uic");
            }
        }
        AnalysisKind::Noise => {
            let n: Noise = d.detail(a.detail);
            t.push_str(".noise");
            probe(d, n.output, &mut t);
            t.push(' ');
            t.push_str(d.str(n.source));
            sweep(d, n.sweep, &mut t);
            if !n.points_per_summary.is_none() {
                value(d, n.points_per_summary, &mut t);
            }
        }
        AnalysisKind::Tf => {
            let f: Tf = d.detail(a.detail);
            t.push_str(".tf");
            probe(d, f.output, &mut t);
            t.push(' ');
            t.push_str(d.str(f.source));
        }
        AnalysisKind::Sens => {
            t.push_str(".sens");
            probe(d, d.detail::<Sens>(a.detail).output, &mut t);
        }
        AnalysisKind::SensAc => {
            let s: SensAc = d.detail(a.detail);
            t.push_str(".sens");
            probe(d, s.output, &mut t);
            t.push_str(" ac");
            sweep(d, s.sweep, &mut t);
        }
        AnalysisKind::Pz => {
            let p: Pz = d.detail(a.detail);
            t.push_str(".pz");
            for n in [p.in_p, p.in_n, p.out_p, p.out_n] {
                t.push(' ');
                t.push_str(d.str(n));
            }
            t.push_str(match p.transfer {
                PzTransfer::Voltage => " vol",
                PzTransfer::Current => " cur",
            });
            t.push_str(match p.find {
                PzFind::Poles => " pol",
                PzFind::Zeros => " zer",
                PzFind::Both => " pz",
            });
        }
        AnalysisKind::Disto => {
            let s: Disto = d.detail(a.detail);
            t.push_str(".disto");
            sweep(d, s.sweep, &mut t);
            if !s.f2_over_f1.is_none() {
                value(d, s.f2_over_f1, &mut t);
            }
        }
        AnalysisKind::Sp => {
            let s: Sp = d.detail(a.detail);
            t.push_str(".sp");
            sweep(d, s.sweep, &mut t);
            if s.noise {
                t.push_str(" 1");
            }
        }
        AnalysisKind::Four => {
            let f: Four = d.detail(a.detail);
            t.push_str(".four");
            value(d, f.fundamental, &mut t);
            for &p in d.probes(f.outputs) {
                probe(d, p, &mut t);
            }
        }
        other @ (AnalysisKind::DcXf
        | AnalysisKind::Xf
        | AnalysisKind::Net
        | AnalysisKind::Hb
        | AnalysisKind::Pss
        | AnalysisKind::Pac
        | AnalysisKind::Pnoise
        | AnalysisKind::Pxf
        | AnalysisKind::Pstb
        | AnalysisKind::Stb
        | AnalysisKind::DcInc
        | AnalysisKind::TimedOp) => {
            unreachable!("ngspice write: FEATURES lacks Analysis({other:?})")
        }
    }
    w.line(&t);
}

/// An `Option` as `.options key[=value]` (`method=trap|gear` for the method flags).
pub(super) fn option(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let o: ir::OptionRow = d.row(item.index);
    let mut t = String::from(".options ");
    match o.key {
        ir::OptionKey::MethodTrap => t.push_str("method=trap"),
        ir::OptionKey::MethodGear => t.push_str("method=gear"),
        key => {
            let (word, _) = read_run::KEYS
                .iter()
                .find(|(_, k)| *k == key)
                .expect("ngspice write: FEATURES has only the keys read_run spells");
            t.push_str(word);
            if !o.value.is_none() {
                t.push('=');
                value(d, o.value, &mut t);
            }
        }
    }
    w.line(&t.replace("= ", "="));
}

/// An `Env` (until A14 lands: unreachable, `FEATURES` lacks `Item(Env)`).
pub(super) fn env(_w: &mut Out<'_>, _item: ir::ItemRef) {
    unreachable!("ngspice write: FEATURES lacks Item(Env) until A14 lands")
}

/// A `NodeSet` as `.ic v(path)=value`, `.nodeset v(path)=value` or `.nodeset all=value`.
pub(super) fn node_set(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let n: ir::NodeSet = d.row(item.index);
    let mut t = String::from(match n.kind {
        ir::NodeSetKind::Ic => ".ic",
        ir::NodeSetKind::Nodeset => ".nodeset",
        ir::NodeSetKind::Device => unreachable!("ngspice write: FEATURES lacks IcDevice"),
    });
    if n.node.len == 0 {
        t.push_str(" all");
    } else {
        t.push_str(" v(");
        path(d, n.node, &mut t);
        t.push(')');
    }
    t.push('=');
    value(d, n.value, &mut t);
    w.line(&t.replace("= ", "="));
}

/// A `Statistics` block: ngspice has none (`FEATURES` lacks it; a citation row).
pub(super) fn statistics(_w: &mut Out<'_>, _item: ir::ItemRef) {
    unreachable!("ngspice write: FEATURES lacks Item(Statistics): ngspice has no such block")
}

// ---- private ----

/// A refusal row of this area.
const fn refusal(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
    (
        code,
        ir::Info {
            title,
            cite,
            explain,
            kind: ir::Kind::Refusal,
        },
    )
}

/// ` v`: a literal bare, any other expression in `{…}`.
fn value(d: &ir::Design, e: ir::ExprId, out: &mut String) {
    out.push(' ');
    if let ir::Expr::Num(n) = d.expr(e) {
        number::spell(d.num(n), Grammar::Inp, out);
        return;
    }
    out.push('{');
    expr_write::expr(d, e, out);
    out.push('}');
}

/// ` v(path)`, ` v(a,b)` or ` i(path)`: a path's names joined by `.` (ngspice's flattened
/// name). Asserts a kind `FEATURES` has.
fn probe(d: &ir::Design, p: ir::Probe, out: &mut String) {
    assert_eq!(
        p.part,
        ir::AcPart::None,
        "ngspice write: a probe part FEATURES lacks"
    );
    let join = |list: ir::NameList, out: &mut String| path(d, list, out);
    match p.kind {
        ir::ProbeKind::V => {
            out.push_str(" v(");
            join(p.a, out);
        }
        ir::ProbeKind::Vdiff => {
            out.push_str(" v(");
            join(p.a, out);
            out.push(',');
            join(p.b, out);
        }
        ir::ProbeKind::I => {
            out.push_str(" i(");
            join(p.a, out);
        }
        other => unreachable!("ngspice write: FEATURES lacks the probe {other:?} on a card"),
    }
    out.push(')');
}

/// A path's names joined by `.`.
fn path(d: &ir::Design, list: ir::NameList, out: &mut String) {
    for (k, &n) in d.names(list).iter().enumerate() {
        if k > 0 {
            out.push('.');
        }
        out.push_str(d.str(n));
    }
}

/// ` dec|oct|lin np fstart fstop`. Asserts a sweep by count (`admits` refuses one by step).
fn sweep(d: &ir::Design, s: ir::SweepSpec, out: &mut String) {
    out.push_str(match s.kind {
        ir::SweepKind::Dec => " dec",
        ir::SweepKind::Oct => " oct",
        ir::SweepKind::Lin => " lin",
        other => unreachable!("ngspice write: FEATURES lacks the {other:?} sweep"),
    });
    assert!(!s.points.is_none(), "ngspice write: a sweep by count");
    for e in [s.points, s.start, s.stop] {
        value(d, e, out);
    }
}
