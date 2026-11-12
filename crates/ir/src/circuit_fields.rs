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
//! `pub(crate) trait Fields` (`Fields::BLANK`, `Fields::fields`), its impls for every circuit row

use crate::controlled::{
    Behav, Controlled, Delay, DelayLaw, FreqLaw, FreqTable, FreqUnit, Laplace, LaplaceForm,
    OutKind, Poly, Quantity, TableLaw, TableSource,
};
use crate::element::NO_ROW;
use crate::id::{ExprId, ExprList, List, NameId, NameList};
use crate::line::Line;
use crate::model::Polarity;
use crate::passive::{Coupling, IcMode, Parasitics, Passive};
use crate::semi::{InitialState, Semi, Switch};
use crate::source::{DcRule, Distof, PortSpec, Source};
use crate::wave::{
    Am, AmModTime, Exp, Pattern, PreDelay, Pulse, PulseLaw, Pwl, PwlJoin, PwlRepeat, Sffm, SffmLaw,
    Sin, TdFrom, TrNoise, TrRandom, TrRandomKind, WaveRef,
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
    PreDelay: ["offset", "hold_delay_value", "zero"];
    SffmLaw: ["spice3", "ngspice"];
    PwlRepeat: ["never", "from", "period"];
    PwlJoin: ["jump", "ramp", "ramp_width", "extrapolate"];
    AmModTime: ["delay", "carrier_phase"];
    TrRandomKind: ["uniform", "gauss", "exponential", "poisson"];
    Polarity: ["none", "n", "p"];
    PulseLaw: ["spice", "vacask"];
    TdFrom: ["zero", "td1"];
    DcRule: ["given", "wave_at_zero"];
}

fields! {
    Passive {
        value: Dyn = N, ic: Expr = N, tc1: Expr = N, tc2: Expr = N, tce: Expr = N,
        temp_abs: Expr = N, dtemp: Expr = N, ac: Expr = N, scale: Expr = N,
        parasitics: Side[Parasitics] = NO_ROW, ic_mode: Choice = IcMode::None, noisy: Flag = true,
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
        rpar: Expr = N, output: Choice = OutKind::Voltage,
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
    }
    Sin {
        offset: Expr = N, amplitude: Expr = N, freq: Expr = N, delay: Expr = N,
        damping: Expr = N, phase_deg: Expr = N, cycles: Expr = N, amplitude2: Expr = N,
        freq2: Expr = N, pre: Choice = PreDelay::Offset,
    }
    Exp {
        v1: Expr = N, v2: Expr = N, td1: Expr = N, tau1: Expr = N, td2: Expr = N, tau2: Expr = N,
        td2_from: Choice = TdFrom::Zero,
    }
    Pwl {
        points: Exprs = EL, delay: Expr = N, repeat_at: Expr = N, repeat_count: Expr = N,
        join_width: Expr = N, file: Name = NN, repeat: Choice = PwlRepeat::Never,
        join: Choice = PwlJoin::Jump,
    }
    Sffm {
        offset: Expr = N, amplitude: Expr = N, carrier_freq: Expr = N, mod_index: Expr = N,
        signal_freq: Expr = N, delay: Expr = N, phase_c_deg: Expr = N, phase_m_deg: Expr = N,
        law: Choice = SffmLaw::Spice3, pre: Choice = PreDelay::Offset,
    }
    Am {
        offset: Expr = N, amplitude: Expr = N, mod_offset: Expr = N, mod_index: Expr = N,
        mod_freq: Expr = N, carrier_freq: Expr = N, delay: Expr = N, phase_m_deg: Expr = N,
        phase_c_deg: Expr = N, mod_time: Choice = AmModTime::Delay, pre: Choice = PreDelay::Offset,
    }
    TrNoise {
        na: Expr = N, nt: Expr = N, nalpha: Expr = N, namp: Expr = N, rts_amp: Expr = N,
        rts_capture: Expr = N, rts_emission: Expr = N,
    }
    TrRandom { ts: Expr = N, td: Expr = N, p1: Expr = N, p2: Expr = N, kind: Choice = TrRandomKind::Uniform }
    Pattern {
        high: Expr = N, low: Expr = N, delay: Expr = N, rise: Expr = N, fall: Expr = N,
        sample: Expr = N, bits: Name = NN, repeats: Expr = N, repeat_start: Expr = N,
    }
}
