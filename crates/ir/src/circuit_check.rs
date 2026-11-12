//! The circuit rows' own invariants: an `Element` or `Model` row in, `X` diagnostics out
//! (`check_element`, `check_model`), and the same rules as a verdict the builder asserts on
//! (`verdict`, `model_ok`). Total over any table contents: an index out of range is a
//! `DetailRow` report, never a panic.
//!
//! Each element gets at most one `TerminalCount`, one `DetailRow` (its detail, side or wave row
//! missing, present on a kind without one, or out of range) and one `FieldDomain` (any field
//! outside the domain its doc states: a required field `NONE`, a pair given half, a list of the
//! wrong length, a law-only field under another law, a master named where the kind takes none).
//! `arg` is the row's index in its kind's table.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9 (check), §7.2; `rewrite/IR-DECISIONS.md` A5
//! (`FieldDomain`); the field docs of `element.rs`, `passive.rs`, `source.rs`,
//! `controlled.rs`, `semi.rs`, `line.rs`, `wave.rs`, `model.rs`.
//!
//! Contents (crate-private): `pub(crate) struct Verdict`, `pub(crate) enum Classes`,
//! `pub(crate) fn verdict`,
//! `pub(crate) fn check_element`, `pub(crate) fn check_model`, `pub(crate) fn model_ok`,
//! `pub(crate) fn detail_len`; it re-exports the tables of `circuit_tables.rs`

use crate::check::Violation;
pub(crate) use crate::circuit_tables::{
    bound_family, builtin_fits, builtin_kind, has_polarity, typed_key, versioned,
};
use crate::circuit_text::get_row;
use crate::class::{ExprClass, def_classes, tree_class};
use crate::controlled::{DelayLaw, OutKind, Quantity, TableLaw};
use crate::design::{Design, DetailRow};
use crate::diag::{Diags, violation_code};
use crate::element::MultLaw;
use crate::element::{DetailKind, ElemKind, Element, NO_ROW};
use crate::expr::Expr;
use crate::id::{DefId, ExprId, ItemRef, NameId};
use crate::model::{Model, ModelFamily, Polarity};
use crate::passive::{IcMode, TcBase, TcLaw};
use crate::source::DcRule;
use crate::wave::{GivenZero, PulseLaw, PwlJoin, PwlRepeat, WaveKind, WaveRef};

/// What is wrong with an element beyond its terminal count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Verdict {
    /// Its detail, side and wave rows exist exactly where the kind and fields say.
    pub(crate) rows: bool,
    /// Every field is in its domain.
    pub(crate) domain: bool,
}

/// Where `verdict` takes the definitions' expression classes from.
#[derive(Clone, Copy)]
pub(crate) enum Classes<'a> {
    /// The builder's assert: no table is computed (it is O(design) after every push), and a
    /// value is assumed to read the circuit, so the rules that depend on a class admit it and
    /// `check` decides them.
    Assume,
    /// `check`: the design's table (`class::def_classes`).
    Table(&'a [ExprClass]),
}

/// The verdict on `e`'s rows and fields (not its terminal count). Total. Under
/// `Classes::Assume` it is O(the element's rows), with no table sized by the design.
pub(crate) fn verdict(design: &Design, e: &Element, classes: Classes<'_>) -> Verdict {
    let mut v = Verdict {
        rows: true,
        domain: true,
    };
    // the master
    let named = e.model.name != NameId::NONE;
    if e.kind.needs_model() && !named
        || e.kind.master_kind().is_none() && e.kind != ElemKind::Unresolved && named
        || e.kind == ElemKind::Unresolved && e.model.def != DefId::NONE
    {
        v.domain = false;
    }
    // ngspice's multiplier law only on an instance that has a multiplier (A15 C1)
    if e.mult_law == MultLaw::NgspiceAppend && (e.kind != ElemKind::Instance || !some(e.mult)) {
        v.domain = false;
    }
    if let Some(family) = bound_family(design, e.model)
        && !builtin_fits(e.kind, family)
    {
        v.domain = false;
    }
    let kind = e.kind.detail();
    if design
        .params(e.params)
        .iter()
        .any(|&(key, _)| typed_key(kind, design.str(key)))
    {
        v.domain = false;
    }
    if kind == DetailKind::None {
        v.rows &= e.detail == NO_ROW;
        return v;
    }
    if e.detail as usize >= detail_len(design, kind) {
        v.rows = false;
        return v;
    }
    detail_verdict(design, e, classes, &mut v);
    v
}

/// Appends `e`'s violations (module header).
pub(crate) fn check_element(e: &Element, design: &Design, at: ItemRef, out: &mut Diags) {
    let (lo, hi) = e.kind.terminals();
    let count = e.terminals.len();
    if count < lo as usize || count > hi as usize {
        out.push_at(
            violation_code(Violation::TerminalCount),
            design,
            at,
            at.index,
        );
    }
    let v = verdict(design, e, Classes::Table(def_classes(design)));
    if !v.rows {
        out.push_at(violation_code(Violation::DetailRow), design, at, at.index);
    }
    if !v.domain {
        out.push_at(violation_code(Violation::FieldDomain), design, at, at.index);
    }
}

/// Appends `m`'s `FieldDomain` violation, if any.
pub(crate) fn check_model(m: &Model, design: &Design, at: ItemRef, out: &mut Diags) {
    if !model_ok(design, m) {
        out.push_at(violation_code(Violation::FieldDomain), design, at, at.index);
    }
}

/// Whether a model row's fields are in their domains: `word` exactly for `CodeModel` and
/// `Other`, `module` exactly for `Module`, a polarity exactly for the families that have one.
pub(crate) fn model_ok(design: &Design, m: &Model) -> bool {
    // A19: no key the row types (`level` only on families whose level is a device parameter),
    // a version only on a versioned family (a value that reads the circuit is the kernel's
    // `ProbeContext`: model values are `Handle::Expr` fields)
    let free = matches!(
        m.family,
        ModelFamily::Other | ModelFamily::Module | ModelFamily::CodeModel
    );
    let keys_ok = design.params(m.params).iter().all(|&(key, _)| {
        let key = design.str(key);
        let typed = key.eq_ignore_ascii_case("type")
            || key.eq_ignore_ascii_case("version")
            || key.eq_ignore_ascii_case("level") && m.family != ModelFamily::Mesa;
        free || !typed
    });
    let version_ok = m.version == ExprId::NONE || versioned(m.family);
    let wants_word = matches!(m.family, ModelFamily::CodeModel | ModelFamily::Other);
    let wants_module = m.family == ModelFamily::Module;
    let polar = m.family == ModelFamily::Other || has_polarity(m.family);
    keys_ok
        && version_ok
        && (builtin_kind(m.family).is_none() || m.params.is_empty())
        && (m.word != NameId::NONE) == wants_word
        && (m.module.name != NameId::NONE) == wants_module
        && (m.polarity != Polarity::None || !has_polarity(m.family))
        && (m.polarity == Polarity::None || polar)
}

/// The row count of `kind`'s table (0 for `DetailKind::None`).
pub(crate) fn detail_len(design: &Design, kind: DetailKind) -> usize {
    let c = &design.circuit;
    match kind {
        DetailKind::None => 0,
        DetailKind::Passive => c.passives.len(),
        DetailKind::Coupling => c.couplings.len(),
        DetailKind::Source => c.sources.len(),
        DetailKind::Controlled => c.controlled.len(),
        DetailKind::Poly => c.polys.len(),
        DetailKind::Table => c.tables.len(),
        DetailKind::Behav => c.behavs.len(),
        DetailKind::Laplace => c.laplaces.len(),
        DetailKind::FreqTable => c.freq_tables.len(),
        DetailKind::Delay => c.delays.len(),
        DetailKind::Semi => c.semis.len(),
        DetailKind::Switch => c.switches.len(),
        DetailKind::Line => c.lines.len(),
    }
}

// ---- private ----

fn some(e: ExprId) -> bool {
    e != ExprId::NONE
}

/// `a` present exactly when `b` is.
fn paired(a: ExprId, b: ExprId) -> bool {
    some(a) == some(b)
}

/// The detail row of `e` (in range) and its side and wave rows.
fn detail_verdict(design: &Design, e: &Element, classes: Classes<'_>, v: &mut Verdict) {
    let reads_circuit = |root: ExprId| {
        root != ExprId::NONE
            && match classes {
                Classes::Assume => true,
                Classes::Table(t) => tree_class(design, root, t) == ExprClass::Circuit,
            }
    };
    let i = e.detail;
    let terms = e.terminals.len();
    let ok = match e.kind.detail() {
        DetailKind::None => true,
        DetailKind::Passive => {
            let p: crate::passive::Passive = design.detail(i);
            let par = side::<crate::passive::Parasitics>(design, p.parasitics, v);
            let resistor = e.kind == ElemKind::Resistor;
            // A15 C2: the 27 °C base and the reciprocal law only where ngspice's behavioural
            // rewrite gives them a meaning (a circuit-valued value with a coefficient)
            let dynamic = (some(p.tc1) || some(p.tc2)) && reads_circuit(p.value);
            // A19: only a resistor or a circuit-valued element has thermal noise to switch off
            (p.noisy || resistor || reads_circuit(p.value))
                && (p.tc_base == TcBase::Tnom || dynamic)
                && (p.tc_law == TcLaw::Direct || dynamic && e.kind == ElemKind::Capacitor)
                && (some(p.value) || e.model.name != NameId::NONE)
                && (some(p.ic) == (p.ic_mode != IcMode::None))
                && !(resistor && some(p.ic))
                && (resistor || !some(p.ac))
                && match par {
                    None => true,
                    Some(_) if resistor => false,
                    Some(q) => e.kind == ElemKind::Capacitor || !some(q.lser) && !some(q.rlshunt),
                }
        }
        DetailKind::Coupling => {
            let c: crate::passive::Coupling = design.detail(i);
            let names = design.names(c.inductors);
            let distinct = names
                .iter()
                .enumerate()
                .all(|(k, n)| !names[..k].contains(n));
            names.len() >= 2 && distinct && some(c.k)
        }
        DetailKind::Source => {
            let s: crate::source::Source = design.detail(i);
            let par = side::<crate::passive::Parasitics>(design, s.parasitics, v);
            let port = side::<crate::source::PortSpec>(design, s.port, v);
            let dis = side::<crate::source::Distof>(design, s.distof, v);
            (s.dc_rule == DcRule::Given) == some(s.dc)
                && (s.dc_rule == DcRule::Given || s.wave != WaveRef::NONE)
                && paired(s.ac_mag, s.ac_phase_deg)
                && paired(s.pac_mag, s.pac_phase_deg)
                && (s.port != NO_ROW) == (e.kind == ElemKind::Port)
                && port.is_none_or(|p| some(p.index) && some(p.z0) && paired(p.power, p.freq))
                && dis.is_none_or(|d| {
                    paired(d.f1_mag, d.f1_phase_deg) && paired(d.f2_mag, d.f2_phase_deg)
                })
                && par.is_none_or(|q| {
                    e.kind == ElemKind::VSource
                        && !some(q.lser)
                        && !some(q.rpar)
                        && !some(q.rlshunt)
                })
                && wave_ok(design, s.wave, v)
        }
        DetailKind::Controlled => {
            let c: crate::controlled::Controlled = design.detail(i);
            let current = matches!(e.kind, ElemKind::Cccs | ElemKind::Ccvs);
            some(c.gain) && (c.sense != NameId::NONE) == current
        }
        DetailKind::Poly => {
            let p: crate::controlled::Poly = design.detail(i);
            let per = if p.control == Quantity::Voltage { 2 } else { 1 };
            p.dimension >= 1
                && p.controls.len() == per * p.dimension as usize
                && !p.coeffs.is_empty()
        }
        DetailKind::Table => {
            let t: crate::controlled::TableSource = design.detail(i);
            some(t.input)
                && t.points.len() >= 2
                && t.points.len().is_multiple_of(2)
                && (t.law == TableLaw::Smoothed || !some(t.delta))
        }
        DetailKind::Behav => {
            let b: crate::controlled::Behav = design.detail(i);
            let want = match e.kind {
                ElemKind::ChargeCapacitor => Some(OutKind::Charge),
                ElemKind::FluxInductor => Some(OutKind::Flux),
                _ => None,
            };
            let tc = some(b.tc1) || some(b.tc2);
            some(b.expr)
                && (b.tc_base == TcBase::Tnom || tc)
                && (b.tc_law == TcLaw::Direct || tc && e.kind == ElemKind::ChargeCapacitor)
                && match want {
                    Some(o) => b.output == o,
                    None => !matches!(b.output, OutKind::Charge | OutKind::Flux),
                }
        }
        DetailKind::Laplace => {
            let l: crate::controlled::Laplace = design.detail(i);
            !l.num.is_empty() && !l.den.is_empty() && some(l.input) == (terms == 2)
        }
        DetailKind::FreqTable => {
            let f: crate::controlled::FreqTable = design.detail(i);
            some(f.input) == (terms == 2) && f.points.len() >= 3 && f.points.len().is_multiple_of(3)
        }
        DetailKind::Delay => {
            let d: crate::controlled::Delay = design.detail(i);
            some(d.td) && some(d.gain) && some(d.samples) == (d.law == DelayLaw::Sampled)
        }
        DetailKind::Semi => {
            let s: crate::semi::Semi = design.detail(i);
            s.ic.is_empty() == (s.ic_mode == IcMode::None)
        }
        DetailKind::Switch => {
            let s: crate::semi::Switch = design.detail(i);
            let current = e.kind == ElemKind::ISwitch;
            (s.sense != NameId::NONE) == current
                && if current {
                    !some(s.control)
                } else {
                    some(s.control) == (terms == 2)
                }
        }
        DetailKind::Line => line_ok(design.detail(i), e.kind, terms),
    };
    v.domain &= ok;
}

/// A side row index: `None` for `NO_ROW`, the row when in range; out of range marks `rows`.
fn side<D: DetailRow>(design: &Design, index: u32, v: &mut Verdict) -> Option<D> {
    if index == NO_ROW {
        return None;
    }
    let row = get_row::<D>(design, index);
    v.rows &= row.is_some();
    row
}

fn line_ok(l: crate::line::Line, kind: ElemKind, terms: usize) -> bool {
    let only = |fields: [ExprId; 7], allowed: [bool; 7]| {
        fields.iter().zip(allowed).all(|(&f, a)| a || !some(f))
    };
    let all = [l.z0, l.td, l.freq, l.nl, l.length, l.vel, l.lumps];
    match kind {
        ElemKind::LosslessLine => {
            let forms = [
                some(l.td),
                some(l.freq) && some(l.nl),
                some(l.length) && some(l.vel),
            ];
            some(l.z0)
                && forms.iter().filter(|&&f| f).count() == 1
                && paired(l.freq, l.nl)
                && paired(l.length, l.vel)
                && !some(l.lumps)
        }
        ElemKind::RcLine => only(all, [false, false, false, false, true, false, true]),
        ElemKind::CoupledLines => {
            terms.is_multiple_of(2) && only(all, [false, false, false, false, true, false, false])
        }
        _ => only(all, [false, false, false, false, true, false, false]),
    }
}

/// A source's waveform: `NONE`, or a row in range whose required fields are present.
fn wave_ok(design: &Design, w: WaveRef, v: &mut Verdict) -> bool {
    use crate::wave::{Am, Exp, Pattern, Pulse, Pwl, Sffm, Sin, TrNoise, TrRandom};
    if w == WaveRef::NONE {
        return true;
    }
    let all = |e: &[ExprId]| e.iter().all(|&x| some(x));
    // A17: `req` always present; `def` present under `AsGiven`, may be omitted under
    // `SpiceDefault` (IR-CIRCUIT §3's table)
    let fits = |rule: GivenZero, req: &[ExprId], def: &[ExprId]| {
        all(req) && (rule == GivenZero::SpiceDefault || all(def))
    };
    match w.kind {
        WaveKind::Pulse => side::<Pulse>(design, w.index, v).is_none_or(|p| {
            fits(
                p.given_zero,
                &[p.v1, p.v2],
                &[p.delay, p.rise, p.fall, p.width, p.period],
            ) && !(p.law == PulseLaw::Vacask && nonpositive_literal(design, p.rise))
        }),
        WaveKind::Sin => side::<Sin>(design, w.index, v).is_none_or(|s| {
            fits(
                s.given_zero,
                &[s.offset, s.amplitude],
                &[s.freq, s.delay, s.damping, s.phase_deg],
            ) && paired(s.amplitude2, s.freq2)
        }),
        WaveKind::Exp => side::<Exp>(design, w.index, v)
            .is_none_or(|x| fits(x.given_zero, &[x.v1, x.v2], &[x.td1, x.tau1, x.td2, x.tau2])),
        WaveKind::Pwl => side::<Pwl>(design, w.index, v).is_none_or(|p| {
            let never = p.repeat == PwlRepeat::Never;
            p.points.len().is_multiple_of(2)
                && (!p.points.is_empty() || p.file != NameId::NONE)
                && never != some(p.repeat_at)
                && (!never || p.join == PwlJoin::Jump && !some(p.repeat_count))
                && some(p.join_width) == (p.join == PwlJoin::RampWidth)
        }),
        WaveKind::Sffm => side::<Sffm>(design, w.index, v).is_none_or(|s| {
            let def = [
                s.carrier_freq,
                s.mod_index,
                s.signal_freq,
                s.delay,
                s.phase_c_deg,
                s.phase_m_deg,
            ];
            fits(s.given_zero, &[s.offset, s.amplitude], &def)
        }),
        WaveKind::Am => side::<Am>(design, w.index, v).is_none_or(|a| {
            let def = [
                a.mod_index,
                a.mod_freq,
                a.carrier_freq,
                a.delay,
                a.phase_m_deg,
                a.phase_c_deg,
            ];
            fits(a.given_zero, &[a.offset, a.amplitude, a.mod_offset], &def)
        }),
        WaveKind::TrNoise => side::<TrNoise>(design, w.index, v).is_none_or(|t| all(&[t.na, t.nt])),
        WaveKind::TrRandom => side::<TrRandom>(design, w.index, v)
            .is_none_or(|t| fits(t.given_zero, &[t.ts], &[t.td, t.p1, t.p2])),
        WaveKind::Pattern => side::<Pattern>(design, w.index, v).is_none_or(|p| {
            all(&[
                p.high,
                p.low,
                p.delay,
                p.rise,
                p.fall,
                p.sample,
                p.repeat_start,
            ]) && p.bits != NameId::NONE
        }),
    }
}

/// Whether `e` is a literal at or below zero (VACASK's pulse needs `rise > 0`; an expression's
/// sign is not known before evaluation, so only a literal is judged here).
fn nonpositive_literal(design: &Design, e: ExprId) -> bool {
    match design.expr(e) {
        Expr::Num(k) => design.literal_value(design.num(k)) <= 0.0,
        _ => false,
    }
}
