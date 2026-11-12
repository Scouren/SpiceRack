//! Whether an element has an ngspice card that reads back as itself: the element half of
//! `Write::admits` (R21), for what no feature states. The letter of its name is its kind's
//! card (until A13's `NameWithoutLetter` covers every case); `m` only where the device takes
//! one (G, F, a current B, the semiconductors, R/C/L, X); an R/C/L whose value names the circuit
//! or the temperature carries only what ngspice's B-source rewrite keeps; a static R/C/L only
//! the keys its device takes; an instance parameter never one the reader reads as a typed
//! field; a B-source field only what the B-source grammar spells.
//!
//! Contents: `pub(super) fn spellable(&ir::Design, &ir::Element) -> bool`

use super::super::expr_write;
use super::super::tables::Eval;
use super::cards::{names_run, passive_fields};
use crate::ir::{self, ElemKind as K, Expr, ExprId};

/// Whether `e` has an ngspice card that reads back as `e` (R21; the features are checked
/// before).
pub(super) fn spellable(d: &ir::Design, e: &ir::Element) -> bool {
    let name = d.str(e.name).as_bytes();
    let letter = name.first().map(u8::to_ascii_lowercase);
    let none = |x: ExprId| x.is_none();
    let no_params = e.params.is_empty();
    let keys_ok = |typed: &[&str]| {
        d.params(e.params)
            .iter()
            .all(|&(k, _)| !typed.contains(&d.str(k).to_ascii_lowercase().as_str()))
    };
    let want = match e.kind {
        K::Resistor => b'r',
        K::Capacitor | K::ChargeCapacitor => b'c',
        K::Inductor => b'l',
        K::Coupling => b'k',
        K::Vcvs => b'e',
        K::Vccs => b'g',
        K::Cccs => b'f',
        K::Ccvs => b'h',
        K::Poly => {
            let p = d.poly(e);
            match (p.output, p.control) {
                (ir::controlled::Quantity::Voltage, ir::controlled::Quantity::Voltage) => b'e',
                (ir::controlled::Quantity::Current, ir::controlled::Quantity::Voltage) => b'g',
                (ir::controlled::Quantity::Current, ir::controlled::Quantity::Current) => b'f',
                (ir::controlled::Quantity::Voltage, ir::controlled::Quantity::Current) => b'h',
            }
        }
        K::Table => match d.table_source(e).output {
            ir::controlled::Quantity::Voltage => b'e',
            ir::controlled::Quantity::Current => b'g',
        },
        K::Behav => letter
            .filter(|l| matches!(l, b'b' | b'e' | b'g'))
            .unwrap_or(b'b'),
        K::Diode => b'd',
        K::Bjt => b'q',
        K::Mosfet => b'm',
        K::Jfet => b'j',
        K::Mesfet => b'z',
        K::VSwitch => b's',
        K::ISwitch => b'w',
        K::LosslessLine => b't',
        K::LossyLine => b'o',
        K::RcLine => b'u',
        K::SingleLossyLine => b'y',
        K::CoupledLines => b'p',
        K::CodeModel => b'a',
        K::ModuleModel => b'n',
        K::Instance => b'x',
        K::FluxInductor
        | K::VSource
        | K::ISource
        | K::Port
        | K::Laplace
        | K::FreqTable
        | K::Delay
        | K::Igbt
        | K::Module
        | K::Unresolved => return false,
    };
    if letter != Some(want) {
        return false;
    }
    match e.kind {
        K::Resistor | K::Capacitor | K::Inductor => passive_ok(d, e),
        K::ChargeCapacitor => {
            let b = d.behav(e);
            // a charge's tc: from 27 C, dividing the charge (the B rewrite's reciproctc=1)
            let tc = (b.tc1.is_none() && b.tc2.is_none())
                || (b.tc_base == ir::passive::TcBase::Fixed27
                    && b.tc_law == ir::passive::TcLaw::Reciprocal
                    && [b.tc1, b.tc2]
                        .iter()
                        .all(|&t| t.is_none() || tc_spells(d, t)));
            let tc = tc && b.temp_abs.is_none() && b.dtemp.is_none();
            no_params && e.model.name.is_none() && tc && b_ok(d, b.expr)
        }
        K::Coupling => no_params && e.mult.is_none(),
        K::Vcvs | K::Ccvs | K::Poly => no_params && e.mult.is_none(),
        K::Vccs | K::Cccs => no_params,
        K::Table => {
            let t = d.table_source(e);
            let mult_ok = e.mult.is_none() || want == b'g';
            let nums = d.exprs(t.points).iter().all(|&p| is_num(d, p));
            no_params && mult_ok && nums && b_ok(d, t.input)
        }
        K::Behav => {
            let b = d.behav(e);
            let voltage = b.output == ir::controlled::OutKind::Voltage;
            let form = match want {
                b'e' => voltage && none(b.temp_abs) && none(b.dtemp),
                b'g' => !voltage && none(b.temp_abs) && none(b.dtemp),
                _ => true,
            };
            // a B source's tc is measured from 27 C (Fixed27); E/G forms carry none
            let tc = (none(b.tc1) && none(b.tc2))
                || (want == b'b' && b.tc_base == ir::passive::TcBase::Fixed27);
            no_params && form && tc && (!voltage || e.mult.is_none()) && b_ok(d, b.expr)
        }
        K::Diode | K::Bjt | K::Jfet => {
            let s = d.semi(e);
            s.initial != ir::semi::InitialState::On
                && keys_ok(&["area", "ic", "m", "temp", "dtemp"])
        }
        K::Mosfet | K::Mesfet => {
            // a MOS takes no area (no ngspice MOS has the key: InvalidInSource on read)
            let s = d.semi(e);
            let area = e.kind == K::Mesfet || none(s.area);
            area && s.initial != ir::semi::InitialState::On
                && keys_ok(&["area", "ic", "m", "temp", "dtemp"])
        }
        K::VSwitch | K::ISwitch => no_params,
        K::LosslessLine | K::LossyLine | K::RcLine | K::SingleLossyLine | K::CoupledLines => {
            line_ok(d, e)
        }
        K::Instance => instance_ok(d, e),
        K::ModuleModel => e.mult.is_none(),
        K::CodeModel => no_params && e.mult.is_none(),
        _ => false,
    }
}

// ---- private ----

/// Whether the expression is a number.
fn is_num(d: &ir::Design, e: ExprId) -> bool {
    matches!(d.expr(e), Expr::Num(_))
}

/// Whether a B-source field spells in the B-source grammar.
fn b_ok(d: &ir::Design, e: ExprId) -> bool {
    expr_write::spells_in(d, e, Eval::Behavioural)
}

/// Whether a tc number survives `eval_tc`'s `%15.8e` (at most 9 significant digits).
fn tc_spells(d: &ir::Design, e: ExprId) -> bool {
    match d.expr(e) {
        Expr::Num(n) => {
            let mut m = d.num(n).mantissa.unsigned_abs();
            // Bounded: at most 19 decimal digits.
            while m != 0 && m.is_multiple_of(10) {
                m /= 10;
            }
            m < 1_000_000_000
        }
        _ => true,
    }
}

/// Whether an R, C or L reads back as itself.
fn passive_ok(d: &ir::Design, e: &ir::Element) -> bool {
    let p = d.passive(e);
    let r = e.kind == K::Resistor;
    let fields = passive_fields(&p);
    let reads_run = fields.iter().any(|&f| names_run(d, f))
        || d.params(e.params).iter().any(|&(_, v)| names_run(d, v))
        || names_run(d, e.mult);
    if names_run(d, p.value) {
        // the B-source rewrite keeps the value, m, tc1 and tc2 (from 27 C; reciprocal on a C)
        // and (on an R) the noise switch
        let noisy_ok = r || !p.noisy;
        let law = if e.kind == K::Capacitor {
            ir::passive::TcLaw::Reciprocal
        } else {
            ir::passive::TcLaw::Direct
        };
        let tc = (p.tc1.is_none() && p.tc2.is_none())
            || (p.tc_base == ir::passive::TcBase::Fixed27
                && p.tc_law == law
                && [p.tc1, p.tc2]
                    .iter()
                    .all(|&t| t.is_none() || tc_spells(d, t)));
        let others = [p.ic, p.tce, p.temp_abs, p.dtemp, p.ac, p.scale];
        return others.iter().all(|f| f.is_none())
            && tc
            && e.model.name.is_none()
            && e.params.is_empty()
            && !reads_run
            && noisy_ok
            && b_ok(d, p.value);
    }
    let allowed: &[&str] = match e.kind {
        K::Resistor | K::Capacitor => &["l", "w", "bv_max"],
        _ => &["nt"],
    };
    let keys = d
        .params(e.params)
        .iter()
        .all(|&(k, _)| allowed.contains(&d.str(k).to_ascii_lowercase().as_str()));
    let r_only = r || (p.tce.is_none() && p.ac.is_none() && p.noisy);
    let value = !p.value.is_none() || !e.model.name.is_none();
    keys && r_only && !reads_run && value
}

/// Whether a line's fields are the ones its card has.
fn line_ok(d: &ir::Design, e: &ir::Element) -> bool {
    let l = d.line(e);
    let none = |x: ExprId| x.is_none();
    let base = e.params.is_empty() && e.mult.is_none() && none(l.vel);
    base && match e.kind {
        K::LosslessLine => {
            !none(l.z0)
                && none(l.length)
                && none(l.lumps)
                && (none(l.td) != none(l.freq))
                && (none(l.freq) == none(l.nl))
        }
        K::RcLine => none(l.z0) && none(l.td) && none(l.freq) && none(l.nl),
        K::SingleLossyLine | K::CoupledLines => {
            none(l.z0) && none(l.td) && none(l.freq) && none(l.nl) && none(l.lumps)
        }
        _ => {
            none(l.z0)
                && none(l.td)
                && none(l.freq)
                && none(l.nl)
                && none(l.lumps)
                && none(l.length)
        }
    }
}

/// Whether an X reads back with its `m`: a parameter `m` only when the subcircuit declares it,
/// a multiplicity only when it does not.
fn instance_ok(d: &ir::Design, e: &ir::Element) -> bool {
    let declares = if e.model.def.is_none() {
        None
    } else {
        let def = d.def(e.model.def);
        let sub: ir::item::Subckt = d.row(def.item.index);
        Some(
            d.params(sub.params)
                .iter()
                .any(|&(k, _)| d.str(k).eq_ignore_ascii_case("m")),
        )
    };
    let has_m = d
        .params(e.params)
        .iter()
        .any(|&(k, _)| d.str(k).eq_ignore_ascii_case("m"));
    match declares {
        Some(true) => e.mult.is_none(),
        Some(false) => !has_m,
        None => !has_m,
    }
}
