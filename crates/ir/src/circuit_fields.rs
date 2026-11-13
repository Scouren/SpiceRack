//! The circuit rows' fields as data: each detail, side and wave row lists its fields in
//! declaration order with their IR-text key, their kind of value and their default, once. A row
//! in, a walk over its fields out; `handles`, IR-text `dump` and `parse` are written once over
//! that walk instead of once per row type (25 types).
//!
//! The default of a field is what its doc calls absent: `NONE`, an empty list, `NO_ROW`, the
//! enum's first member, `noisy` true, a count of 0. `dump` leaves a field out exactly when it
//! holds its default; `parse` starts every row from `Fields::BLANK`.
//!
//! Cites: `rewrite/IR-IMPL.md` §1 (bodies); `rewrite/IR-KERNEL.md` §9 (canonical text; owners'
//! rows: keyword, fields in declaration order, `NONE` fields left out); `rewrite/IR-DECISIONS.md`
//! A5 (`Handle::Expr` vs `Handle::ExprDyn`).
//!
//! Contents (crate-private): `pub(crate) enum Field`, `pub(crate) enum Side`,
//! `pub(crate) trait Choice` (`Choice::get`, `Choice::set`, `Choice::words`),
//! `pub(crate) trait Fields` (`Fields::BLANK`, `Fields::fields`), its impls for every circuit row,
//! `pub(crate) fn element_map_exprs`, `pub(crate) fn model_map_exprs`,
//! `pub(crate) fn element_map_names`, `pub(crate) fn is_behavioural(&Design, ItemRef) -> bool`

use crate::circuit_text::get_row;
use crate::controlled::{
    Behav, Controlled, Delay, DelayLaw, FreqLaw, FreqTable, FreqUnit, Laplace, LaplaceForm,
    OutKind, Poly, Quantity, TableLaw, TableSource,
};
use crate::design::{Design, DetailRow};
use crate::element::MultLaw;
use crate::element::NO_ROW;
use crate::element::{DetailKind, Element};
use crate::facts::ExprUse;
use crate::id::{ExprId, ExprList, ItemRef, List, NameId, NameList, ParamList};
use crate::item::ItemKind;
use crate::line::Line;
use crate::model::Model;
use crate::model::Polarity;
use crate::passive::{Coupling, IcMode, Parasitics, Passive, TcBase, TcLaw};
use crate::semi::{InitialState, Semi, Switch};
use crate::source::{DcRule, Distof, PortSpec, Source};
use crate::wave::{
    Am, AmModTime, Exp, GivenZero, Pattern, PreDelay, Pulse, PulseLaw, Pwl, PwlJoin, PwlRepeat,
    Sffm, SffmLaw, Sin, TdFrom, TrNoise, TrRandom, TrRandomKind, WaveKind, WaveRef,
};

/// One field of a row, borrowed mutably, by the kind of value it holds.
pub(crate) enum Field<'a> {
    /// An expression constant in time (`Handle::Expr`).
    Expr(&'a mut ExprId),
    /// An expression that may read the circuit (`Handle::ExprDyn`).
    Dyn(&'a mut ExprId),
    /// A name.
    Name(&'a mut NameId),
    /// A name list, written `(a b)`.
    Names(&'a mut NameList),
    /// An expression list, written `[e1, e2]`.
    Exprs(&'a mut ExprList),
    /// A small count, written in decimal; default 0.
    Count(&'a mut u8),
    /// A flag whose default is `true`; written `key=false`.
    Flag(&'a mut bool),
    /// A closed enum; default its first member.
    Choice(&'a mut dyn Choice),
    /// A side row index (`NO_ROW` = none), written `key=(fields)`.
    Side(&'a mut u32, Side),
    /// A waveform, written `wave=<kind>(fields)`.
    Wave(&'a mut WaveRef),
}

/// Which side table a `Field::Side` indexes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    /// `Parasitics`.
    Parasitics,
    /// `PortSpec`.
    Port,
    /// `Distof`.
    Distof,
}

/// A closed enum field seen as an index into its IR-text words.
pub(crate) trait Choice {
    /// The member's index in `ALL`.
    fn get(&self) -> usize;
    /// Sets the member at `index` of `ALL`. Asserts `index < COUNT`.
    fn set(&mut self, index: usize);
    /// The IR-text word of every member, in `ALL` order.
    fn words(&self) -> &'static [&'static str];
}

/// A circuit row whose fields can be walked.
pub(crate) trait Fields: Copy {
    /// The row with every field at its default.
    const BLANK: Self;
    /// Calls `f` with each field's key and the field, in declaration order.
    fn fields(&mut self, f: &mut dyn FnMut(&'static str, Field<'_>));
}

macro_rules! choices {
    ($($t:ty: [$($w:literal),* $(,)?];)*) => {
        $(
            impl Choice for $t {
                fn get(&self) -> usize {
                    *self as usize
                }
                fn set(&mut self, index: usize) {
                    *self = <$t>::ALL[index];
                }
                fn words(&self) -> &'static [&'static str] {
                    &[$($w),*]
                }
            }
            const _: () = assert!([$($w),*].len() == <$t>::COUNT);
        )*
    };
}

macro_rules! fields {
    ($($row:ident { $($f:ident: $ctor:ident $([$side:ident])? = $default:expr),* $(,)? })*) => {
        $(
            impl Fields for $row {
                const BLANK: Self = $row { $($f: $default),* };
                fn fields(&mut self, f: &mut dyn FnMut(&'static str, Field<'_>)) {
                    $( f(stringify!($f), fields!(@ $ctor self.$f $(, $side)?)); )*
                }
            }
        )*
    };
    (@ Side $e:expr, $side:ident) => { Field::Side(&mut $e, Side::$side) };
    (@ Choice $e:expr) => { Field::Choice(&mut $e) };
    (@ $ctor:ident $e:expr) => { Field::$ctor(&mut $e) };
}

const N: ExprId = ExprId::NONE;
const NN: NameId = NameId::NONE;
const NL: NameList = List::EMPTY;
const EL: ExprList = List::EMPTY;

choices! {
    IcMode: ["none", "uic_only", "in_op"];
    InitialState: ["unset", "off", "on"];
    Quantity: ["voltage", "current"];
    OutKind: [
        "voltage", "current", "charge", "flux", "resistance", "conductance", "capacitance",
        "capacitance_charge", "inductance", "power",
    ];
    TableLaw: ["exact", "ngspice_rounded", "smoothed"];
    LaplaceForm: ["polynomial", "pole_zero"];
    FreqLaw: ["complex_linear", "db_log_freq"];
    FreqUnit: ["db", "magnitude", "re_im"];
    DelayLaw: ["ideal", "sampled"];
    PreDelay: ["hold_delay_value", "zero"];
    SffmLaw: ["spice3", "ngspice"];
    PwlRepeat: ["never", "from", "period"];
    PwlJoin: ["jump", "ramp", "ramp_width", "extrapolate"];
    AmModTime: ["delay", "carrier_phase"];
    TrRandomKind: ["uniform", "gauss", "exponential", "poisson"];
    Polarity: ["none", "n", "p"];
    PulseLaw: ["spice", "vacask"];
    TdFrom: ["zero", "td1"];
    DcRule: ["given", "wave_at_zero"];
    GivenZero: ["as_given", "spice_default"];
    TcBase: ["tnom", "fixed27"];
    TcLaw: ["direct", "reciprocal"];
    MultLaw: ["parallel", "ngspice_append"];
}

fields! {
    Passive {
        value: Dyn = N, ic: Expr = N, tc1: Expr = N, tc2: Expr = N, tce: Expr = N,
        temp_abs: Expr = N, dtemp: Expr = N, ac: Expr = N, scale: Expr = N,
        parasitics: Side[Parasitics] = NO_ROW, ic_mode: Choice = IcMode::None, noisy: Flag = true,
        tc_base: Choice = TcBase::Tnom, tc_law: Choice = TcLaw::Direct,
    }
    Parasitics { rser: Expr = N, lser: Expr = N, rpar: Expr = N, cpar: Expr = N, rlshunt: Expr = N }
    Coupling { inductors: Names = NL, k: Dyn = N }
    Source {
        dc: Expr = N, ac_mag: Expr = N, ac_phase_deg: Expr = N, tc1: Expr = N, tc2: Expr = N,
        pac_mag: Expr = N, pac_phase_deg: Expr = N, wave: Wave = WaveRef::NONE,
        noise_psd: Exprs = EL, parasitics: Side[Parasitics] = NO_ROW, port: Side[Port] = NO_ROW,
        distof: Side[Distof] = NO_ROW, dc_rule: Choice = DcRule::Given,
    }
    PortSpec { index: Expr = N, z0: Expr = N, noise_temp: Expr = N, power: Expr = N, freq: Expr = N }
    Distof { f1_mag: Expr = N, f1_phase_deg: Expr = N, f2_mag: Expr = N, f2_phase_deg: Expr = N }
    Controlled { gain: Expr = N, sense: Name = NN }
    Poly {
        controls: Names = NL, coeffs: Exprs = EL, dimension: Count = 0,
        output: Choice = Quantity::Voltage, control: Choice = Quantity::Voltage,
    }
    TableSource {
        input: Dyn = N, points: Exprs = EL, delta: Expr = N, law: Choice = TableLaw::Exact,
        output: Choice = Quantity::Voltage,
    }
    Behav {
        expr: Dyn = N, tc1: Expr = N, tc2: Expr = N, temp_abs: Expr = N, dtemp: Expr = N,
        rpar: Expr = N, output: Choice = OutKind::Voltage, tc_base: Choice = TcBase::Tnom,
        tc_law: Choice = TcLaw::Direct,
    }
    Laplace {
        num: Exprs = EL, den: Exprs = EL, input: Dyn = N, output: Choice = Quantity::Voltage,
        form: Choice = LaplaceForm::Polynomial,
    }
    FreqTable {
        input: Dyn = N, points: Exprs = EL, law: Choice = FreqLaw::ComplexLinear,
        unit: Choice = FreqUnit::Db, output: Choice = Quantity::Voltage,
    }
    Delay { td: Expr = N, gain: Expr = N, samples: Expr = N, law: Choice = DelayLaw::Ideal }
    Semi {
        area: Expr = N, ic: Exprs = EL, temp_abs: Expr = N, dtemp: Expr = N,
        ic_mode: Choice = IcMode::None, initial: Choice = InitialState::Unset,
    }
    Switch { sense: Name = NN, control: Dyn = N, initial: Choice = InitialState::Unset }
    Line {
        z0: Expr = N, td: Expr = N, freq: Expr = N, nl: Expr = N, length: Expr = N,
        vel: Expr = N, lumps: Expr = N,
    }
    Pulse {
        v1: Expr = N, v2: Expr = N, delay: Expr = N, rise: Expr = N, fall: Expr = N,
        width: Expr = N, period: Expr = N, cycles: Expr = N, law: Choice = PulseLaw::Spice,
        given_zero: Choice = GivenZero::AsGiven,
    }
    Sin {
        offset: Expr = N, amplitude: Expr = N, freq: Expr = N, delay: Expr = N,
        damping: Expr = N, phase_deg: Expr = N, cycles: Expr = N, amplitude2: Expr = N,
        freq2: Expr = N, pre: Choice = PreDelay::HoldDelayValue, given_zero: Choice = GivenZero::AsGiven,
    }
    Exp {
        v1: Expr = N, v2: Expr = N, td1: Expr = N, tau1: Expr = N, td2: Expr = N, tau2: Expr = N,
        td2_from: Choice = TdFrom::Zero, given_zero: Choice = GivenZero::AsGiven,
    }
    Pwl {
        points: Exprs = EL, delay: Expr = N, repeat_at: Expr = N, repeat_count: Expr = N,
        join_width: Expr = N, file: Name = NN, repeat: Choice = PwlRepeat::Never,
        join: Choice = PwlJoin::Jump,
    }
    Sffm {
        offset: Expr = N, amplitude: Expr = N, carrier_freq: Expr = N, mod_index: Expr = N,
        signal_freq: Expr = N, delay: Expr = N, phase_c_deg: Expr = N, phase_m_deg: Expr = N,
        law: Choice = SffmLaw::Spice3, pre: Choice = PreDelay::HoldDelayValue,
        given_zero: Choice = GivenZero::AsGiven,
    }
    Am {
        offset: Expr = N, amplitude: Expr = N, mod_offset: Expr = N, mod_index: Expr = N,
        mod_freq: Expr = N, carrier_freq: Expr = N, delay: Expr = N, phase_m_deg: Expr = N,
        phase_c_deg: Expr = N, mod_time: Choice = AmModTime::Delay, pre: Choice = PreDelay::HoldDelayValue,
        given_zero: Choice = GivenZero::AsGiven,
    }
    TrNoise {
        na: Expr = N, nt: Expr = N, nalpha: Expr = N, namp: Expr = N, rts_amp: Expr = N,
        rts_capture: Expr = N, rts_emission: Expr = N,
    }
    TrRandom {
        ts: Expr = N, td: Expr = N, p1: Expr = N, p2: Expr = N, kind: Choice = TrRandomKind::Uniform,
        given_zero: Choice = GivenZero::AsGiven,
    }
    Pattern {
        high: Expr = N, low: Expr = N, delay: Expr = N, rise: Expr = N, fall: Expr = N,
        sample: Expr = N, bits: Name = NN, repeats: Expr = N, repeat_start: Expr = N,
    }
}

/// Calls `f` on every expression root of the element at `index`, in `element_handles`' order
/// (`params`' values, `mult`, then the detail row's fields with their side and wave rows in
/// place), and stores back what `f` changed (A23). A `Behav` row's `expr` is reported
/// `ExprUse::Behavioural` (R66); `roots_with_use` reads the same from the handles through
/// `is_behavioural`. Asserts `index` is in range.
pub(crate) fn element_map_exprs(
    design: &mut Design,
    index: u32,
    f: &mut dyn FnMut(&mut ExprId, ExprUse),
) {
    let mut row: Element = design.row(index);
    map_params(design, row.params, f);
    f(&mut row.mult, ExprUse::Constant);
    design.editor().set(index, row);
    match row.kind.detail() {
        DetailKind::None => {}
        DetailKind::Passive => map_row::<Passive>(design, row.detail, f),
        DetailKind::Coupling => map_row::<Coupling>(design, row.detail, f),
        DetailKind::Source => map_row::<Source>(design, row.detail, f),
        DetailKind::Controlled => map_row::<Controlled>(design, row.detail, f),
        DetailKind::Poly => map_row::<Poly>(design, row.detail, f),
        DetailKind::Table => map_row::<TableSource>(design, row.detail, f),
        // R66: the row's one `Dyn` field is its expression, the behavioural evaluator's own
        DetailKind::Behav => map_row::<Behav>(design, row.detail, &mut |e, u| {
            f(
                e,
                if u == ExprUse::Circuit {
                    ExprUse::Behavioural
                } else {
                    u
                },
            )
        }),
        DetailKind::Laplace => map_row::<Laplace>(design, row.detail, f),
        DetailKind::FreqTable => map_row::<FreqTable>(design, row.detail, f),
        DetailKind::Delay => map_row::<Delay>(design, row.detail, f),
        DetailKind::Semi => map_row::<Semi>(design, row.detail, f),
        DetailKind::Switch => map_row::<Switch>(design, row.detail, f),
        DetailKind::Line => map_row::<Line>(design, row.detail, f),
    }
}

/// Calls `f` on the model's expression roots at `index`, in `model_handles`' order (`params`'
/// values, then `version`), and stores back what `f` changed (A23).
pub(crate) fn model_map_exprs(
    design: &mut Design,
    index: u32,
    f: &mut dyn FnMut(&mut ExprId, ExprUse),
) {
    let mut row: Model = design.row(index);
    map_params(design, row.params, f);
    f(&mut row.version, ExprUse::Constant);
    design.editor().set(index, row);
}

/// Calls `f` on each element name the element at `index` uses by name (A32), in field order,
/// each a slice of one stored in place: a coupling's inductors, a current-controlled source's
/// or switch's sensing element, a current-controlled polynomial's controls. A `NONE` name is
/// no use; a voltage-controlled polynomial's controls and every terminal are nets.
pub(crate) fn element_map_names(design: &mut Design, index: u32, f: &mut dyn FnMut(&mut [NameId])) {
    let e: Element = design.row(index);
    let list = match e.kind.detail() {
        DetailKind::Coupling => get_row::<Coupling>(design, e.detail).map(|c| c.inductors),
        DetailKind::Poly => get_row::<Poly>(design, e.detail)
            .filter(|p| p.control == Quantity::Current)
            .map(|p| p.controls),
        DetailKind::Controlled => {
            if let Some(mut c) = get_row::<Controlled>(design, e.detail)
                && c.sense != NameId::NONE
            {
                f(std::slice::from_mut(&mut c.sense));
                design.editor().set_detail(e.detail, c);
            }
            None
        }
        DetailKind::Switch => {
            if let Some(mut s) = get_row::<Switch>(design, e.detail)
                && s.sense != NameId::NONE
            {
                f(std::slice::from_mut(&mut s.sense));
                design.editor().set_detail(e.detail, s);
            }
            None
        }
        DetailKind::None
        | DetailKind::Passive
        | DetailKind::Source
        | DetailKind::Table
        | DetailKind::Behav
        | DetailKind::Laplace
        | DetailKind::FreqTable
        | DetailKind::Delay
        | DetailKind::Semi
        | DetailKind::Line => None,
    };
    if let Some(list) = list {
        let start = list.start as usize;
        let end = start + list.len as usize;
        assert!(end <= design.name_pool.len(), "a name list in range");
        for name in &mut design.name_pool[start..end] {
            if *name != NameId::NONE {
                f(std::slice::from_mut(name));
            }
        }
    }
}

/// Whether `item` is an element whose detail is a `Behav` row (a behavioural source, a
/// charge-defined capacitor, a flux-defined inductor): its one `Handle::ExprDyn` is the
/// behavioural evaluator's expression (R66). `false` for any other item, `ItemRef::NONE`
/// included, and for an element out of range (`check` reports it).
pub(crate) fn is_behavioural(design: &Design, item: ItemRef) -> bool {
    item.kind == ItemKind::Element
        && (item.index as usize) < design.elements.len()
        && design.row::<Element>(item.index).kind.detail() == DetailKind::Behav
}

// ---- private ----

/// Calls `f` on each value of `list`, in place in the pool.
fn map_params(design: &mut Design, list: ParamList, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
    let start = list.start as usize;
    let end = start + list.len as usize;
    assert!(end <= design.param_pool.len(), "a param list in range");
    for p in &mut design.param_pool[start..end] {
        f(&mut p.1, ExprUse::Constant);
    }
}

/// Calls `f` on each element of `list`, in place in the pool.
fn map_list(design: &mut Design, list: ExprList, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
    let start = list.start as usize;
    let end = start + list.len as usize;
    assert!(end <= design.expr_pool.len(), "an expression list in range");
    for e in &mut design.expr_pool[start..end] {
        f(e, ExprUse::Constant);
    }
}

/// Walks the roots of row `index` of `D`'s table (`row_handles`' order) and writes it back.
fn map_row<D: Fields + DetailRow>(
    design: &mut Design,
    index: u32,
    f: &mut dyn FnMut(&mut ExprId, ExprUse),
) {
    let Some(mut row) = get_row::<D>(design, index) else {
        return;
    };
    row.fields(&mut |_, field| match field {
        Field::Expr(e) => f(e, ExprUse::Constant),
        Field::Dyn(e) => f(e, ExprUse::Circuit),
        Field::Exprs(l) => map_list(design, *l, f),
        Field::Name(_) | Field::Names(_) | Field::Count(_) | Field::Flag(_) | Field::Choice(_) => {}
        Field::Side(i, Side::Parasitics) => map_row::<Parasitics>(design, *i, f),
        Field::Side(i, Side::Port) => map_row::<PortSpec>(design, *i, f),
        Field::Side(i, Side::Distof) => map_row::<Distof>(design, *i, f),
        Field::Wave(w) => map_wave(design, *w, f),
    });
    design.editor().set_detail(index, row);
}

/// Walks a waveform row's roots, if it exists.
fn map_wave(design: &mut Design, w: WaveRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
    match w.kind {
        WaveKind::Pulse => map_row::<Pulse>(design, w.index, f),
        WaveKind::Sin => map_row::<Sin>(design, w.index, f),
        WaveKind::Exp => map_row::<Exp>(design, w.index, f),
        WaveKind::Pwl => map_row::<Pwl>(design, w.index, f),
        WaveKind::Sffm => map_row::<Sffm>(design, w.index, f),
        WaveKind::Am => map_row::<Am>(design, w.index, f),
        WaveKind::TrNoise => map_row::<TrNoise>(design, w.index, f),
        WaveKind::TrRandom => map_row::<TrRandom>(design, w.index, f),
        WaveKind::Pattern => map_row::<Pattern>(design, w.index, f),
    }
}
