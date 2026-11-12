//! Controlled sources' typed fields: linear (VCVS, VCCS, CCCS, CCVS), polynomial, table,
//! behavioural (including the charge- and flux-defined elements), Laplace, frequency-table and
//! delay sources, with the law enums that say which formula each one means. Builder pushes in,
//! rows out through `design.controlled(&e)`, `design.poly(&e)`, … (each asserting the kind).
//!
//! Each row stays the construct it is: a `Poly` stays a polynomial, a `TableSource` a table;
//! expanding either into an expression is a pass (REWRITE-PLAN §3.1 principle 2), not a
//! reader's choice. Sign rule, every kind: positive output current flows from the first
//! terminal through the source to the second (INV-semantics 5.24).
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §2; `rewrite/INV-constructs.md` D5, D16–D23, D60–D63, D76,
//! D78, D80, D83, D90–D93, D101, D102, D106, D116, D122, D124, D130;
//! `rewrite/INV-semantics.md` 5.24–5.31, 6.16, 6.31.
//!
//! Contents:
//! - `pub enum Quantity`, `pub enum OutKind`, `pub enum TableLaw`, `pub enum LaplaceForm`,
//!   `pub enum FreqLaw`, `pub enum FreqUnit`, `pub enum DelayLaw`; on each: `ALL`, `COUNT`
//! - `pub struct Controlled`, `pub struct Poly`, `pub struct TableSource`, `pub struct Behav`,
//!   `pub struct Laplace`, `pub struct FreqTable`, `pub struct Delay`
//! - `Design::controlled(&self, &Element) -> Controlled`, `Design::poly(&self, &Element) ->
//!   Poly`, `Design::table_source(&self, &Element) -> TableSource`,
//!   `Design::behav(&self, &Element) -> Behav`, `Design::laplace(&self, &Element) -> Laplace`,
//!   `Design::freq_table(&self, &Element) -> FreqTable`, `Design::delay(&self, &Element) ->
//!   Delay`

use crate::design::Design;
use crate::element::{DetailKind, Element};
use crate::id::{ExprId, ExprList, NameId, NameList};

/// A source's output or control quantity: a voltage across two nets or a current.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Quantity {
    /// A voltage (V).
    Voltage,
    /// A current (A).
    Current,
}

/// The relation a behavioural branch states (INV-semantics 5.31, 6.16). `expr` is the
/// right-hand side; it may read the circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OutKind {
    /// v = expr (V). ng/es/lt `V=`, xy `V={}`, hs `E vol=`, sp `bsource v=`, va `v=`.
    Voltage,
    /// i = expr (A). ng/es/lt `I=`, hs `G cur=`, sp `bsource i=`, va `i=`.
    Current,
    /// i = d(expr)/dt, expr a charge (C). The `ChargeCapacitor` kind (D5).
    Charge,
    /// v = d(expr)/dt, expr a flux (Wb) of the element's own current. The `FluxInductor` kind
    /// (D80).
    Flux,
    /// i = v/expr, expr a resistance (Ω). sp `bsource r=`, lt `B R=` (D83, D130).
    Resistance,
    /// i = expr·v, expr a conductance (S). sp `bsource g=` (D130).
    Conductance,
    /// i = expr·dv/dt, expr an incremental capacitance (F). sp `bsource c=` with `ctype=0`
    /// (6.16).
    Capacitance,
    /// i = d(expr·v)/dt (F). sp `bsource c=` with `ctype=1` (6.16).
    CapacitanceCharge,
    /// v = expr·di/dt, expr an incremental inductance (H). sp `bsource l=` (5.31).
    Inductance,
    /// i = expr/v, expr a power (W). lt `B P=` (D83).
    Power,
}

/// The interpolation law of a table source (IR-CIRCUIT §2; INV-semantics 5.25).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TableLaw {
    /// Linear between points, ends held, a vertical step takes the later point (xy `TABLE`,
    /// lt `table=`/`tbl=`).
    Exact,
    /// ngspice and ESPice native `TABLE`: corners rounded.
    NgspiceRounded,
    /// hs `G table`: corners smoothed over `delta` (D92).
    Smoothed,
}

/// How a `Laplace` row's two lists read (D91).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LaplaceForm {
    /// `num` and `den` are polynomial coefficients in ascending powers of s (hs `LAPLACE`,
    /// lt, sp, va; 5.27).
    Polynomial,
    /// `num` = [a, z1_re, z1_im, …], `den` = [b, p1_re, p1_im, …]: H = a·Π(s + z)/(b·Π(s + p))
    /// with roots in rad/s (hs `POLE`, sp `svcvs` poles/zeros).
    PoleZero,
}

/// How a frequency table interpolates between its points (INV-semantics 5.28).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FreqLaw {
    /// The complex value linear in linear frequency, ends clamped (ng `FREQ`, XSPICE `xfer`).
    ComplexLinear,
    /// Magnitude in dB linear in log f, phase linear in f (hs `FREQ`).
    DbLogFreq,
}

/// How a frequency table's value columns read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FreqUnit {
    /// (f, magnitude dB, phase degrees).
    Db,
    /// (f, magnitude, phase degrees).
    Magnitude,
    /// (f, real, imaginary).
    ReIm,
}

/// What a delay element computes (INV-semantics 5.30).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DelayLaw {
    /// out = gain·in(t − td), exactly (xy `YDELAY`, sp `delay`).
    Ideal,
    /// A band-limited sampled delay of `samples` points (hs `E … DELAY`, `NPDELAY`).
    Sampled,
}

/// The detail row of a linear controlled source (`Vcvs`, `Vccs`, `Cccs`, `Ccvs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controlled {
    /// Gain: V/V, S, A/A or Ω by kind; may be an expression of parameters (lt, ng; D78). Never
    /// `NONE`. A multiplicity is the header's `mult`, never folded in here (D17, D18).
    pub gain: ExprId,
    /// The sensing element (B17) whose branch current controls a `Cccs`/`Ccvs`, by name;
    /// `NONE` on `Vcvs`/`Vccs`. sp `probe=`, va `ctlinst=`.
    pub sense: NameId,
}

/// A polynomial controlled source (D19): output = Σ coefficient × monomial over the controls,
/// in the SPICE `POLY(n)` order: c0, then c1…cn on the controls, then every product of degree
/// 2 in lexicographic order (a1², a1·a2, …, a2², …), then degree 3, and so on (sp `pvcvs`
/// follows the same order, INV-semantics 5.26).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    /// The controls: for voltage control, 2·`dimension` net names as (ctl+, ctl−) pairs; for
    /// current control, `dimension` sensing element names.
    pub controls: NameList,
    /// The coefficients, at least one.
    pub coeffs: ExprList,
    /// The number of controlling quantities, at least 1.
    pub dimension: u8,
    /// The output: a voltage (E, H; sp `pvcvs`, `pccvs`) or a current (G, F).
    pub output: Quantity,
    /// The controls: voltages (E, G) or branch currents (F, H).
    pub control: Quantity,
}

/// A table source (D21, D76, D92, D102): output = the table of `points` at `input`, under
/// `law`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableSource {
    /// The table's input, an expression that may read the circuit (`{v(a,b)}`; lt `tbl=` is
    /// V(n+, n−)). Never `NONE`.
    pub input: ExprId,
    /// Flat (x, y) pairs, x non-decreasing, at least one pair.
    pub points: ExprList,
    /// The smoothing width of `TableLaw::Smoothed`; `NONE` there means hs's default, a quarter
    /// of the smallest gap between breakpoints. `NONE` under every other law.
    pub delta: ExprId,
    /// Interpolation law.
    pub law: TableLaw,
    /// The output quantity (E: voltage, G and lt `I tbl=`: current).
    pub output: Quantity,
}

/// A behavioural branch (D20, D22): `output` = `expr`, with B-source temperature terms. The
/// detail of `Behav`, `ChargeCapacitor` (`Charge`) and `FluxInductor` (`Flux`). HSPICE `SCALE=`,
/// `MAX=`, `MIN=` modifiers and lt's implicit `x` are part of `expr` as their reader writes
/// them (a product, `Min`/`Max`/`Clamp`; the device's own `V`/`I` probe, 6.31).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Behav {
    /// The right-hand side; may read the circuit. Never `NONE`.
    pub expr: ExprId,
    /// Temperature coefficient 1 (1/K): the output is multiplied by 1 + tc1·ΔT + tc2·ΔT²,
    /// ΔT = T_inst − tnom (ng B `tc1=`, D63; xy rejects it). `NONE`: 0.
    pub tc1: ExprId,
    /// Temperature coefficient 2 (1/K²), as `tc1`. `NONE`: 0.
    pub tc2: ExprId,
    /// Instance temperature (°C, ng B `temp=`). `NONE`: the circuit temperature.
    pub temp_abs: ExprId,
    /// Instance temperature offset (K, ng B `dtemp=`). `NONE`: 0.
    pub dtemp: ExprId,
    /// Parallel resistance across the terminals (Ω, lt B `Rpar=`, D83, D101). `NONE`: none.
    pub rpar: ExprId,
    /// The relation.
    pub output: OutKind,
}

/// A Laplace source (D23, D91): output = H(s) applied to the input, H as `form` reads `num`
/// and `den`, the absolute gain already applied (sp's DC-gain normalisation is undone by its
/// reader, 5.27).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Laplace {
    /// Numerator list, at least one value.
    pub num: ExprList,
    /// Denominator list, at least one value, not all zero.
    pub den: ExprList,
    /// The input as an expression (lt `B … V=V(a) Laplace=`); `NONE` when the input is the
    /// voltage across terminals 3 and 4.
    pub input: ExprId,
    /// The output quantity.
    pub output: Quantity,
    /// How `num` and `den` read.
    pub form: LaplaceForm,
}

/// A frequency-table source (D61; INV-semantics 5.28): AC only; output = the table at the
/// analysis frequency times the input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreqTable {
    /// The input as an expression; `NONE` when it is the voltage across terminals 3 and 4.
    pub input: ExprId,
    /// Flat (f, a, b) triples, f non-decreasing (Hz), a and b as `unit` says.
    pub points: ExprList,
    /// The interpolation law.
    pub law: FreqLaw,
    /// How the value columns read.
    pub unit: FreqUnit,
    /// The output quantity.
    pub output: Quantity,
}

/// A delay element (INV-semantics 5.30): out = gain·in(t − td) under `law`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Delay {
    /// The delay (s). Never `NONE`.
    pub td: ExprId,
    /// The gain (V/V). Never `NONE` (1 when the card has none).
    pub gain: ExprId,
    /// The sample count of `DelayLaw::Sampled`; `NONE` under `Ideal`.
    pub samples: ExprId,
    /// The law.
    pub law: DelayLaw,
}

const _: () = assert!(size_of::<OutKind>() == 1);
const _: () = assert!(size_of::<Controlled>() == 8);
const _: () = assert!(size_of::<Poly>() == 20);
const _: () = assert!(size_of::<TableSource>() == 20);
const _: () = assert!(size_of::<Behav>() == 28);
const _: () = assert!(size_of::<Laplace>() == 24);
const _: () = assert!(size_of::<FreqTable>() == 16);
const _: () = assert!(size_of::<Delay>() == 16);

impl Quantity {
    /// Every member, in declaration order.
    pub const ALL: [Quantity; 2] = [Quantity::Voltage, Quantity::Current];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl OutKind {
    /// Every member, in declaration order.
    pub const ALL: [OutKind; 10] = [
        OutKind::Voltage,
        OutKind::Current,
        OutKind::Charge,
        OutKind::Flux,
        OutKind::Resistance,
        OutKind::Conductance,
        OutKind::Capacitance,
        OutKind::CapacitanceCharge,
        OutKind::Inductance,
        OutKind::Power,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl TableLaw {
    /// Every member, in declaration order.
    pub const ALL: [TableLaw; 3] = [
        TableLaw::Exact,
        TableLaw::NgspiceRounded,
        TableLaw::Smoothed,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl LaplaceForm {
    /// Every member, in declaration order.
    pub const ALL: [LaplaceForm; 2] = [LaplaceForm::Polynomial, LaplaceForm::PoleZero];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl FreqLaw {
    /// Every member, in declaration order.
    pub const ALL: [FreqLaw; 2] = [FreqLaw::ComplexLinear, FreqLaw::DbLogFreq];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl FreqUnit {
    /// Every member, in declaration order.
    pub const ALL: [FreqUnit; 3] = [FreqUnit::Db, FreqUnit::Magnitude, FreqUnit::ReIm];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl DelayLaw {
    /// Every member, in declaration order.
    pub const ALL: [DelayLaw; 2] = [DelayLaw::Ideal, DelayLaw::Sampled];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The row of a `Vcvs`, `Vccs`, `Cccs` or `Ccvs`. Asserts the kind.
    pub fn controlled(&self, element: &Element) -> Controlled {
        assert!(
            element.kind.detail() == DetailKind::Controlled,
            "{:?} has no Controlled row",
            element.kind
        );
        self.detail(element.detail)
    }

    /// The row of a `Poly`. Asserts the kind.
    pub fn poly(&self, element: &Element) -> Poly {
        assert!(
            element.kind.detail() == DetailKind::Poly,
            "{:?} has no Poly row",
            element.kind
        );
        self.detail(element.detail)
    }

    /// The row of a `Table`. Asserts the kind.
    pub fn table_source(&self, element: &Element) -> TableSource {
        assert!(
            element.kind.detail() == DetailKind::Table,
            "{:?} has no Table row",
            element.kind
        );
        self.detail(element.detail)
    }

    /// The row of a `Behav`, `ChargeCapacitor` or `FluxInductor`. Asserts the kind.
    pub fn behav(&self, element: &Element) -> Behav {
        assert!(
            element.kind.detail() == DetailKind::Behav,
            "{:?} has no Behav row",
            element.kind
        );
        self.detail(element.detail)
    }

    /// The row of a `Laplace`. Asserts the kind.
    pub fn laplace(&self, element: &Element) -> Laplace {
        assert!(
            element.kind.detail() == DetailKind::Laplace,
            "{:?} has no Laplace row",
            element.kind
        );
        self.detail(element.detail)
    }

    /// The row of a `FreqTable`. Asserts the kind.
    pub fn freq_table(&self, element: &Element) -> FreqTable {
        assert!(
            element.kind.detail() == DetailKind::FreqTable,
            "{:?} has no FreqTable row",
            element.kind
        );
        self.detail(element.detail)
    }

    /// The row of a `Delay`. Asserts the kind.
    pub fn delay(&self, element: &Element) -> Delay {
        assert!(
            element.kind.detail() == DetailKind::Delay,
            "{:?} has no Delay row",
            element.kind
        );
        self.detail(element.detail)
    }
}
