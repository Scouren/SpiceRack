//! Whether an element has an ngspice card that reads back as itself: the element half of
//! `Write::admits` (R21), for what no feature states. The letter of its name is its kind's
//! card (until A13's `NameWithoutLetter` covers every case); `m` only where the device takes
//! one (G, F, a current B, the semiconductors, R/C/L, X, N); an R/C/L whose value names the circuit
//! or the temperature carries only what ngspice's B-source rewrite keeps; a static R/C/L only
//! the keys its device takes; an instance parameter never one the reader reads as a typed
//! field; a B-source field only what the B-source grammar spells; an instance temperature only
//! where its bound model's family has one (`read_elements::instance_temp`).
//!
//! Contents: `pub(super) enum Why`, `pub(super) fn spellable(&ir::Design, &ir::Element) ->
//! Result<(), Why>` (the refusal's `arg` is the `Why`, so a census can count the branches)

use super::super::expr_write;
use super::super::read_elements;
use super::super::tables::Eval;
use super::cards::{names_run, passive_fields};
use crate::ir::{self, ElemKind as K, Expr, ExprId};

/// Why an element has no ngspice card: the branch of [`spellable`] that refused it, the
/// refusal's `arg` (W1100). Numbered for the census; never renumbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum Why {
    /// A kind ngspice has no card for (a flux inductor, a Laplace, a delay, …).
    NoCard = 1,
    /// The name's first letter is not the kind's card letter.
    Letter = 2,
    /// An R, C or L run as a B source carrying what the rewrite drops (a model, parameters,
    /// fields other than m/tc1/tc2, a tc not from 27 C, a value the B grammar cannot spell).
    PassiveRewrite = 3,
    /// A static R, C or L with an instance parameter its device does not take.
    PassiveKeys = 4,
    /// A C or L with `tce`, `ac` or `noisy=false` (an R's only).
    PassiveROnly = 5,
    /// A static R, C or L whose other field, parameter or mult reads the circuit or time.
    PassiveReadsRun = 6,
    /// An R, C or L with neither a value nor a model.
    PassiveNoValue = 7,
    /// A charge capacitor ngspice's `q=` form does not spell.
    Charge = 8,
    /// Parameters or a multiplicity on a K, E, F, G, H, POLY, table, B, switch or code model
    /// that takes none.
    ParamsOrMult = 9,
    /// A table's points not all numbers, or its input not in the B grammar.
    Table = 10,
    /// A behavioural source the B, E or G form does not spell (tc not from 27 C, temperatures
    /// on E/G, mult on a voltage, an expression outside the B grammar).
    Behav = 11,
    /// An instance temperature on a family that has none.
    SemiTemp = 12,
    /// A semiconductor parameter that reads back as a typed field, or a MOS area.
    SemiKeys = 13,
    /// A line's fields that are not its card's.
    Line = 14,
    /// An X's `m` that reads back as the other of parameter and multiplicity.
    InstanceM = 15,
}

/// Whether `e` has an ngspice card that reads back as `e` (R21; the features are checked
/// before), and if not, why.
pub(super) fn spellable(d: &ir::Design, e: &ir::Element) -> Result<(), Why> {
    let need = |ok: bool, why: Why| if ok { Ok(()) } else { Err(why) };
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
        | K::Unresolved => return Err(Why::NoCard),
    };
    need(letter == Some(want), Why::Letter)?;
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
            need(
                no_params && e.model.name.is_none() && tc && b_ok(d, b.expr),
                Why::Charge,
            )
        }
        K::Coupling | K::Vcvs | K::Ccvs | K::Poly => {
            need(no_params && e.mult.is_none(), Why::ParamsOrMult)
        }
        K::Vccs | K::Cccs => need(no_params, Why::ParamsOrMult),
        K::Table => {
            let t = d.table_source(e);
            let mult_ok = e.mult.is_none() || want == b'g';
            let nums = d.exprs(t.points).iter().all(|&p| is_num(d, p));
            need(no_params && mult_ok, Why::ParamsOrMult)?;
            need(nums && b_ok(d, t.input), Why::Table)
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
            need(no_params, Why::ParamsOrMult)?;
            need(
                form && tc && (!voltage || e.mult.is_none()) && b_ok(d, b.expr),
                Why::Behav,
            )
        }
        K::Diode | K::Bjt | K::Jfet => {
            let s = d.semi(e);
            let (temp, dtemp) = read_elements::instance_temp(d, e);
            need(
                (temp || none(s.temp_abs)) && (dtemp || none(s.dtemp)),
                Why::SemiTemp,
            )?;
            need(
                keys_ok(&["area", "ic", "m", "temp", "dtemp"]),
                Why::SemiKeys,
            )
        }
        K::Mosfet | K::Mesfet => {
            // a MOS takes no area (no ngspice MOS has the key: InvalidInSource on read)
            let s = d.semi(e);
            let (temp, dtemp) = read_elements::instance_temp(d, e);
            need(
                (temp || none(s.temp_abs)) && (dtemp || none(s.dtemp)),
                Why::SemiTemp,
            )?;
            let area = e.kind == K::Mesfet || none(s.area);
            need(
                area && keys_ok(&["area", "ic", "m", "temp", "dtemp"]),
                Why::SemiKeys,
            )
        }
        K::VSwitch | K::ISwitch => need(no_params, Why::ParamsOrMult),
        K::LosslessLine | K::LossyLine | K::RcLine | K::SingleLossyLine | K::CoupledLines => {
            need(line_ok(d, e), Why::Line)
        }
        K::Instance => need(instance_ok(d, e), Why::InstanceM),
        K::ModuleModel => Ok(()),
        K::CodeModel => need(no_params && e.mult.is_none(), Why::ParamsOrMult),
        _ => Err(Why::NoCard),
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
        // a long literal's digits less its leading and trailing zeros (A18)
        Expr::Num(n) if d.long_digits(d.num(n)).is_some() => {
            let (significand, _) = d.long_digits(d.num(n)).expect("a long literal");
            let digits = significand.trim_start_matches(['-', '0', '.']);
            digits
                .trim_end_matches(['0', '.'])
                .bytes()
                .filter(u8::is_ascii_digit)
                .count()
                <= 9
        }
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

/// Whether an R, C or L reads back as itself, and if not, why.
fn passive_ok(d: &ir::Design, e: &ir::Element) -> Result<(), Why> {
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
        let kept = others.iter().all(|f| f.is_none())
            && tc
            && e.model.name.is_none()
            && e.params.is_empty()
            && !reads_run
            && noisy_ok
            && b_ok(d, p.value);
        return if kept {
            Ok(())
        } else {
            Err(Why::PassiveRewrite)
        };
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
    let why = [
        (keys, Why::PassiveKeys),
        (r_only, Why::PassiveROnly),
        (!reads_run, Why::PassiveReadsRun),
        (value, Why::PassiveNoValue),
    ];
    why.iter()
        .find(|(ok, _)| !ok)
        .map_or(Ok(()), |&(_, w)| Err(w))
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
