//! Source, controlled-source and coupling elements → their VACASK instance lines, the inverse
//! of `read_sources`: `name (nodes) master`, then `type=` and the wave's parameters in VD's
//! order, `mag`, `phase`, `$mfactor` (sources), `ctlinst`, `gain` (controlled), `k`, `ind1`,
//! `ind2` (mutual). A field holding exactly the literal the reader fills for an absent
//! parameter (`read_sources::DEFAULTS`) is left out, so the line reads back as the same row.
//! A wave's `Am.mod_offset` other than the literal 1 the reader fills has no VACASK spelling:
//! `admits` refuses it (W2201). `tline_ideal`: `z0`, then `td` or `f` and `nl` (R39).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD dev-builtin-src, -vcvs, -cccs, -mutual.
//!
//! Contents: `AM_OFFSET`, `PARAMS` (their catalogue rows are in `read_sources::CODES`, the area's one table),
//! `admits`, `source` (crate-private)

use super::expr_write::write_expr;
use super::number;
use super::read_sources::default_of;
use super::write::Writer;
use crate::ir;
use crate::ir::wave as w;

/// An `Am` whose `mod_offset` is not the literal 1 VACASK's AM means.
pub(super) const AM_OFFSET: ir::Code = ir::Code::new(b'W', 2201);
/// A source-like element with instance parameters.
pub(super) const PARAMS: ir::Code = ir::Code::new(b'W', 2203);

/// One source-like `Element` row as its instance line.
pub(super) fn source(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let e: ir::Element = d.row(at.index);
    w.head(&e);
    match e.kind {
        ir::ElemKind::VSource | ir::ElemKind::ISource => independent(w, d.source(&e)),
        ir::ElemKind::Vcvs | ir::ElemKind::Vccs | ir::ElemKind::Cccs | ir::ElemKind::Ccvs => {
            let c = d.controlled(&e);
            if c.sense != ir::NameId::NONE {
                w.out.push_str(" ctlinst=");
                string(d.str(c.sense), w.out);
            }
            param(w, "gain", c.gain);
        }
        ir::ElemKind::Coupling => {
            let c = d.coupling(&e);
            param(w, "k", c.k);
            let inductors = d.names(c.inductors);
            assert!(inductors.len() == 2, "FEATURES lacks CouplingMany");
            for (key, &ind) in ["ind1", "ind2"].iter().zip(inductors) {
                w.out.push_str(&format!(" {key}="));
                string(d.str(ind), w.out);
            }
        }
        ir::ElemKind::LosslessLine => {
            let line = d.line(&e);
            param(w, "z0", line.z0);
            match line.td.is_none() {
                true => {
                    param(w, "f", line.freq);
                    param(w, "nl", line.nl);
                }
                false => {
                    w.out.push_str(" td=");
                    write_expr(d, line.td, w.out);
                }
            }
        }
        k => unreachable!("FEATURES lacks {k:?}: legalize refused it"),
    }
    if !e.mult.is_none() {
        w.out.push_str(" $mfactor=");
        write_expr(d, e.mult, w.out);
    }
    w.out.push('\n');
}

/// Appends a refusal for each row of this area VACASK cannot spell for a reason no feature
/// states (R21): an `Am` whose `mod_offset` is not the literal 1 (`AM_OFFSET`; A31 makes it
/// a feature), a source-like element with instance parameters or a `mutual` with a multiplicity (`PARAMS`: VACASK's
/// builtins take only their own parameters, and `mutual` no `$mfactor`).
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    use ir::ElemKind as K;
    for (at, _) in design.reachable() {
        if at.kind != ir::ItemKind::Element {
            continue;
        }
        let e: ir::Element = design.row(at.index);
        let source_like = matches!(
            e.kind,
            K::VSource
                | K::ISource
                | K::Vcvs
                | K::Vccs
                | K::Cccs
                | K::Ccvs
                | K::Coupling
                | K::LosslessLine
        );
        if source_like && !e.params.is_empty() {
            diags.push_at(PARAMS, design, at, 0);
        }
        if e.kind == K::Coupling && !e.mult.is_none() {
            // `MultOther` covers lines (VACASK's tline_ideal takes `$mfactor`), not `mutual`.
            diags.push_at(PARAMS, design, at, 0);
        }
        if !matches!(e.kind, K::VSource | K::ISource) {
            continue;
        }
        let s = design.source(&e);
        if s.wave.is_none() {
            continue;
        }
        match design.wave(s.wave) {
            ir::Wave::Am(am) if !is_default(design, am.mod_offset, "1.") => {
                diags.push_at(AM_OFFSET, design, at, 0);
            }
            _ => {}
        }
    }
}

// ---- private ----

/// `type=…`, the wave's parameters, `dc`, `mag`, `phase`.
fn independent(w: &mut Writer<'_, '_>, s: ir::source::Source) {
    let d = w.design;
    match (s.wave.is_none(), s.dc_rule) {
        (true, _) => param(w, "dc", s.dc),
        (false, ir::source::DcRule::WaveAtZero) => wave(w, d.wave(s.wave), true),
        (false, ir::source::DcRule::Given) => {
            // R42: the run writer adds the `type` alters (`fold::given`).
            w.out.push_str(" type=\"dc\" dc=");
            write_expr(d, s.dc, w.out);
            wave(w, d.wave(s.wave), false);
        }
    }
    if !s.ac_mag.is_none() {
        let both = is_default(d, s.ac_mag, "0.") && is_default(d, s.ac_phase_deg, "0.");
        match both {
            true => w.out.push_str(" mag=0"),
            false => {
                param(w, "mag", s.ac_mag);
                param(w, "phase", s.ac_phase_deg);
            }
        }
    }
}

/// `type="<t>"` (when `typed`) and the wave's parameters in VD's order.
fn wave(w: &mut Writer<'_, '_>, wave: ir::Wave, typed: bool) {
    let (typ, fields): (&str, Vec<(&str, ir::ExprId)>) = match wave {
        ir::Wave::Sin(s) => (
            "sine",
            vec![
                ("sinedc", s.offset),
                ("ampl", s.amplitude),
                ("freq", s.freq),
                ("tdphase", s.phase_deg),
                ("delay", s.delay),
                ("theta", s.damping),
            ],
        ),
        ir::Wave::Pulse(p) => (
            "pulse",
            vec![
                ("val0", p.v1),
                ("val1", p.v2),
                ("delay", p.delay),
                ("rise", p.rise),
                ("fall", p.fall),
                ("width", p.width),
                ("period", p.period),
            ],
        ),
        ir::Wave::Exp(x) => (
            "exp",
            vec![
                ("val0", x.v1),
                ("val1", x.v2),
                ("delay", x.td1),
                ("td2", x.td2),
                ("tau1", x.tau1),
                ("tau2", x.tau2),
            ],
        ),
        ir::Wave::Am(a) => (
            "am",
            am_fm(
                a.offset,
                a.amplitude,
                a.carrier_freq,
                a.phase_c_deg,
                a.delay,
                a.mod_freq,
                a.phase_m_deg,
                a.mod_index,
            ),
        ),
        ir::Wave::Sffm(f) => (
            "fm",
            am_fm(
                f.offset,
                f.amplitude,
                f.carrier_freq,
                f.phase_c_deg,
                f.delay,
                f.signal_freq,
                f.phase_m_deg,
                f.mod_index,
            ),
        ),
        ir::Wave::Pwl(p) => return pwl(w, p, typed),
        other => unreachable!("FEATURES lacks {other:?}: legalize refused it"),
    };
    if typed {
        w.out.push_str(&format!(" type=\"{typ}\""));
    }
    for (key, e) in fields {
        param(w, key, e);
    }
}

/// The AM and FM parameters in VD's order.
#[expect(
    clippy::too_many_arguments,
    reason = "one per VACASK parameter, in VD's order"
)]
fn am_fm(
    sinedc: ir::ExprId,
    ampl: ir::ExprId,
    freq: ir::ExprId,
    tdphase: ir::ExprId,
    delay: ir::ExprId,
    modfreq: ir::ExprId,
    modphase: ir::ExprId,
    modindex: ir::ExprId,
) -> Vec<(&'static str, ir::ExprId)> {
    vec![
        ("sinedc", sinedc),
        ("ampl", ampl),
        ("freq", freq),
        ("tdphase", tdphase),
        ("delay", delay),
        ("modfreq", modfreq),
        ("modphase", modphase),
        ("modindex", modindex),
    ]
}

/// `type="pwl"` (when `typed`), `wave=[…]` or `file="…"`, `delay`, `period`.
fn pwl(w: &mut Writer<'_, '_>, p: w::Pwl, typed: bool) {
    let d = w.design;
    if typed {
        w.out.push_str(" type=\"pwl\"");
    }
    if p.file != ir::NameId::NONE {
        w.out.push_str(" file=");
        string(d.str(p.file), w.out);
    } else {
        w.out.push_str(" wave=[");
        for (k, &e) in d.exprs(p.points).iter().enumerate() {
            if k > 0 {
                w.out.push_str(", ");
            }
            write_expr(d, e, w.out);
        }
        w.out.push(']');
    }
    if !p.delay.is_none() {
        w.out.push_str(" delay=");
        write_expr(d, p.delay, w.out);
    }
    if p.repeat == w::PwlRepeat::Period {
        assert!(
            p.join == w::PwlJoin::Extrapolate,
            "FEATURES lacks every other join"
        );
        w.out.push_str(" period=");
        write_expr(d, p.repeat_at, w.out);
    }
}

/// ` key=value`, unless `value` is the literal the reader fills for an absent `key`.
fn param(w: &mut Writer<'_, '_>, key: &str, value: ir::ExprId) {
    let d = w.design;
    if default_of(key).is_some_and(|text| is_default(d, value, text)) {
        return;
    }
    w.out.push(' ');
    w.out.push_str(key);
    w.out.push('=');
    write_expr(d, value, w.out);
}

/// Whether `e` is exactly (field by field) the literal `text` lexes to.
fn is_default(design: &ir::Design, e: ir::ExprId, text: &str) -> bool {
    let want = number::lex(text).expect("a default literal").literal;
    matches!(design.expr(e), ir::Expr::Num(n) if design.num(n) == want)
}

/// `"text"`, `"` and `\` escaped.
fn string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
}
