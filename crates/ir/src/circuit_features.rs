//! What a circuit row uses beyond its item kind: an `Element` or `Model` row in, its kind
//! features (`Feature::Elem`, `Feature::Wave`) and its `CircuitExtra` members out. Each member is
//! set exactly when its field holds a non-default value (`circuit_extra.rs` member docs); the
//! expression features of the fields are the kernel's (from `RowFacts::handles`). Total over any
//! table contents: a row out of range contributes nothing.
//!
//! Cites: `rewrite/IR-KERNEL.md` §8, §7.2; `circuit_extra.rs`; `rewrite/IR-DECISIONS.md` A1
//! (a field reads the circuit when it holds `V`, `I`, `Time`, `Freq`, `Ddt` or `Idt`).
//!
//! Contents (crate-private): `pub(crate) fn element_features`, `pub(crate) fn model_features`,
//! `pub(crate) fn reads_circuit`, `pub(crate) fn dc_at_zero`, `pub(crate) fn library_features`,
//! `pub(crate) fn mult_class`

use crate::circuit_check::{bound_family, builtin_kind};
use crate::circuit_extra::CircuitExtra as X;
use crate::circuit_text::get_row;
use crate::controlled::{
    Behav, DelayLaw, FreqLaw, FreqTable, Laplace, LaplaceForm, OutKind, TableLaw, TableSource,
};
use crate::design::Design;
use crate::element::MultLaw;
use crate::element::{DetailKind, ElemKind, Element, NO_ROW};
use crate::expr::Expr;
use crate::feature::{Feature, FeatureSet};
use crate::id::{ExprId, NameId};
use crate::item::ItemKind;
use crate::letter::spice_letters;
use crate::library::DeviceLibrary;
use crate::line::Line;
use crate::model::{Model, ModelFamily};
use crate::passive::{Coupling, IcMode, Passive, TcBase, TcLaw};
use crate::semi::{InitialState, Semi, Switch};
use crate::source::{DcRule, PortSpec, Source};
use crate::wave::{
    Am, AmModTime, Exp, PreDelay, Pulse, PulseLaw, Pwl, PwlJoin, PwlRepeat, Sffm, SffmLaw, Sin,
    TdFrom, WaveKind, WaveRef,
};

/// Adds the element's kind, its waveform's kind and its circuit extras.
pub(crate) fn element_features(e: &Element, design: &Design, set: &mut FeatureSet) {
    let mut got = FeatureSet::EMPTY.with(Feature::Elem(e.kind));
    let mut add = |x: X| got = got.with(Feature::Circuit(x));
    let some = |x: ExprId| x != ExprId::NONE;
    let mut wave_kind = None;
    if e.model.name == NameId::NONE {
        add(X::ElementWithoutModel);
    }
    let source_like = matches!(
        e.kind,
        ElemKind::VSource
            | ElemKind::ISource
            | ElemKind::Vcvs
            | ElemKind::Vccs
            | ElemKind::Cccs
            | ElemKind::Ccvs
    );
    let mutual = bound_family(design, e.model) == Some(ModelFamily::Mutual);
    if source_like && e.model.name != NameId::NONE || e.kind == ElemKind::Coupling && mutual {
        add(X::BuiltinMaster);
    }
    // a name begins with one of the letters a SPICE card may give it (A13; a voltage B may be
    // an E card, a current one a G card, a port a V card)
    let letters = spice_letters(design, e);
    if !letters.is_empty() {
        let first = design.str(e.name).as_bytes().first().copied().unwrap_or(0);
        if !letters.iter().any(|l| first.eq_ignore_ascii_case(l)) {
            add(X::NameWithoutLetter);
        }
    }
    if some(e.mult) {
        add(match e.mult_law {
            MultLaw::NgspiceAppend => X::MultInstanceNgspiceAppend,
            MultLaw::Parallel => mult_class(e.kind),
        });
    }
    match e.kind.detail() {
        DetailKind::Passive => {
            if let Some(p) = get_row::<Passive>(design, e.detail) {
                passive(design, e.kind, p, &mut add);
            }
        }
        DetailKind::Coupling => {
            if let Some(c) = get_row::<Coupling>(design, e.detail) {
                if c.inductors.len() > 2 {
                    add(X::CouplingMany);
                }
                // a coupling bound to the builtin mutual inductance is no core (A12 C5)
                if e.model.name != NameId::NONE && !mutual {
                    add(X::CouplingCore);
                }
            }
        }
        DetailKind::Source => {
            if let Some(s) = get_row::<Source>(design, e.detail) {
                source(design, s, &mut add);
                match s.dc_rule {
                    DcRule::Given if s.wave != WaveRef::NONE => add(X::DcGivenWithWave),
                    DcRule::Given => {}
                    DcRule::WaveAtZero => add(dc_at_zero(s.wave.kind)),
                }
                if s.wave != WaveRef::NONE {
                    wave_kind = Some(s.wave.kind);
                    wave(design, s.wave, &mut add);
                }
            }
        }
        DetailKind::Table => {
            if let Some(t) = get_row::<TableSource>(design, e.detail) {
                add(match t.law {
                    TableLaw::Exact => X::TableExact,
                    TableLaw::NgspiceRounded => X::TableNgspiceRounded,
                    TableLaw::Smoothed => X::TableSmoothed,
                });
            }
        }
        DetailKind::Behav => {
            if let Some(b) = get_row::<Behav>(design, e.detail) {
                behav(b, &mut add);
            }
        }
        DetailKind::Laplace => {
            if let Some(l) = get_row::<Laplace>(design, e.detail) {
                if l.form == LaplaceForm::PoleZero {
                    add(X::LaplacePoleZero);
                }
                if some(l.input) {
                    add(X::LaplaceExprInput);
                }
            }
        }
        DetailKind::FreqTable => {
            if let Some(f) = get_row::<FreqTable>(design, e.detail) {
                add(match f.law {
                    FreqLaw::ComplexLinear => X::FreqComplexLinear,
                    FreqLaw::DbLogFreq => X::FreqDbLogFreq,
                });
                if some(f.input) {
                    add(X::LaplaceExprInput);
                }
            }
        }
        DetailKind::Delay => {
            if get_row::<crate::controlled::Delay>(design, e.detail)
                .is_some_and(|d| d.law == DelayLaw::Sampled)
            {
                add(X::DelaySampled);
            }
        }
        DetailKind::Semi => {
            if let Some(s) = get_row::<Semi>(design, e.detail) {
                ic_mode(s.ic_mode, &mut add);
                if some(s.area) {
                    add(X::SemiArea);
                }
                if s.initial != InitialState::Unset {
                    add(X::SemiInitial);
                }
                if some(s.temp_abs) {
                    add(X::InstanceTemp);
                }
                if some(s.dtemp) {
                    add(X::InstanceDtemp);
                }
            }
            if e.kind == ElemKind::Diode && e.terminals.len() == 3 {
                add(X::DiodeThermal);
            }
        }
        DetailKind::Switch => {
            if let Some(s) = get_row::<Switch>(design, e.detail) {
                if s.initial != InitialState::Unset {
                    add(X::SwitchInitial);
                }
                if some(s.control) {
                    add(X::SwitchControlExpr);
                }
            }
        }
        DetailKind::Line => {
            if let Some(l) = get_row::<Line>(design, e.detail) {
                if e.kind == ElemKind::LosslessLine && some(l.freq) {
                    add(X::LineFreqNl);
                }
                if e.kind == ElemKind::LosslessLine && some(l.length) {
                    add(X::LineLengthVel);
                }
                if some(l.lumps) {
                    add(X::LineLumps);
                }
            }
        }
        DetailKind::None | DetailKind::Controlled | DetailKind::Poly => {}
    }
    if let Some(w) = wave_kind {
        got = got.with(Feature::Wave(w));
    }
    *set = set.union(got);
}

/// Adds the model's family (`Feature::Family`) and its circuit extras.
pub(crate) fn model_features(m: &Model, set: &mut FeatureSet) {
    // A15 K5 (R32): every model is a feature of its family
    *set = set.with(Feature::Family(m.family));
    if m.bin_of != NameId::NONE {
        *set = set.with(Feature::Circuit(X::ModelBin));
    }
    if m.ako.name != NameId::NONE {
        *set = set.with(Feature::Circuit(X::ModelAko));
    }
}

/// Whether the expression holds a `V`, `I`, `Time` or `Freq` leaf or a call of a function that
/// reads the circuit's history (`Ddt`, `Idt`). Explicit stack (C11).
pub(crate) fn reads_circuit(design: &Design, root: ExprId) -> bool {
    if root == ExprId::NONE {
        return false;
    }
    let mut stack = vec![root];
    // bounded: each expression row is pushed once per parent edge; trees are acyclic (children
    // have lower ids), so the walk ends after at most every row of the tree
    while let Some(e) = stack.pop() {
        match design.expr(e) {
            Expr::V(..) | Expr::I(_) | Expr::Time | Expr::Freq => return true,
            Expr::Call(f, args) => {
                if f.reads_circuit() {
                    return true;
                }
                stack.extend_from_slice(design.exprs(args));
            }
            Expr::UserCall(_, args) | Expr::Draw(_, _, _, args) | Expr::List(args) => {
                stack.extend_from_slice(design.exprs(args));
            }
            Expr::Unary(_, a) => stack.push(a),
            Expr::Binary(_, a, b) => stack.extend([a, b]),
            Expr::Ternary(c, t, o) => stack.extend([c, t, o]),
            Expr::Num(_)
            | Expr::Str(_)
            | Expr::Param(_)
            | Expr::Temp
            | Expr::TStep
            | Expr::TStop
            | Expr::Tnom => {}
        }
    }
    false
}

/// The `DcRule::WaveAtZero` member of a wave kind.
pub(crate) fn dc_at_zero(kind: WaveKind) -> X {
    match kind {
        WaveKind::Pulse => X::DcAtZeroPulse,
        WaveKind::Sin => X::DcAtZeroSin,
        WaveKind::Exp => X::DcAtZeroExp,
        WaveKind::Pwl => X::DcAtZeroPwl,
        WaveKind::Sffm => X::DcAtZeroSffm,
        WaveKind::Am => X::DcAtZeroAm,
        WaveKind::TrNoise => X::DcAtZeroTrNoise,
        WaveKind::TrRandom => X::DcAtZeroTrRandom,
        WaveKind::Pattern => X::DcAtZeroPattern,
    }
}

/// Adds `ModelWithoutLibrary` when a reachable model's family has no reachable
/// `DeviceLibrary` of that family (A12 C5); a builtin family needs none. One pass over the
/// reachable items.
pub(crate) fn library_features(design: &Design, set: &mut FeatureSet) {
    let reachable = design.reachable();
    let mut has_library = [false; ModelFamily::COUNT];
    for &(item, _) in &reachable {
        if item.kind == ItemKind::DeviceLibrary {
            has_library[design.row::<DeviceLibrary>(item.index).family as usize] = true;
        }
    }
    let missing = reachable.iter().any(|&(item, _)| {
        item.kind == ItemKind::Model && {
            let family = design.row::<Model>(item.index).family;
            builtin_kind(family).is_none() && !has_library[family as usize]
        }
    });
    if missing {
        *set = set.with(Feature::Circuit(X::ModelWithoutLibrary));
    }
}

/// The multiplicity member of a kind's class.
pub(crate) fn mult_class(kind: ElemKind) -> X {
    use ElemKind as E;
    match kind {
        E::Resistor => X::MultResistor,
        E::Capacitor => X::MultCapacitor,
        E::Inductor => X::MultInductor,
        E::VSource => X::MultVSource,
        E::ISource => X::MultISource,
        E::Vcvs
        | E::Vccs
        | E::Cccs
        | E::Ccvs
        | E::Poly
        | E::Table
        | E::Behav
        | E::Laplace
        | E::FreqTable
        | E::Delay => X::MultControlled,
        E::Diode | E::Bjt | E::Mosfet | E::Jfet | E::Mesfet | E::Igbt => X::MultSemi,
        E::Instance => X::MultInstanceParallel, // the law's own member: `element_features`
        E::CodeModel | E::ModuleModel | E::Module => X::MultModule,
        E::ChargeCapacitor
        | E::FluxInductor
        | E::Coupling
        | E::Port
        | E::VSwitch
        | E::ISwitch
        | E::LosslessLine
        | E::LossyLine
        | E::RcLine
        | E::SingleLossyLine
        | E::CoupledLines
        | E::Unresolved => X::MultOther,
    }
}

// ---- private ----

fn tc_base(base: TcBase) -> X {
    match base {
        TcBase::Tnom => X::TcBaseTnom,
        TcBase::Fixed27 => X::TcBaseFixed27,
    }
}

fn ic_mode(mode: IcMode, add: &mut impl FnMut(X)) {
    match mode {
        IcMode::None => {}
        IcMode::UicOnly => add(X::IcUicOnly),
        IcMode::InOp => add(X::IcInOp),
    }
}

fn passive(design: &Design, kind: ElemKind, p: Passive, add: &mut impl FnMut(X)) {
    let some = |x: ExprId| x != ExprId::NONE;
    ic_mode(p.ic_mode, add);
    // A15 C2: a circuit-valued element's coefficient states its base (and a capacitor's, its law)
    if reads_circuit(design, p.value) && (some(p.tc1) || some(p.tc2)) {
        add(tc_base(p.tc_base));
        if kind == ElemKind::Capacitor {
            add(match p.tc_law {
                TcLaw::Direct => X::TcLawDirect,
                TcLaw::Reciprocal => X::TcLawReciprocal,
            });
        }
    }
    let flags = [
        (reads_circuit(design, p.value), X::PassiveDynamicValue),
        (!p.noisy, X::PassiveNoiseless),
        (some(p.ac), X::PassiveAc),
        (some(p.scale), X::PassiveScale),
        (some(p.tc1) || some(p.tc2), X::PassiveTc),
        (some(p.tce), X::PassiveTce),
        (some(p.temp_abs), X::InstanceTemp),
        (some(p.dtemp), X::InstanceDtemp),
        (p.parasitics != NO_ROW, X::ElementParasitics),
    ];
    for (on, x) in flags {
        if on {
            add(x);
        }
    }
}

fn source(design: &Design, s: Source, add: &mut impl FnMut(X)) {
    let some = |x: ExprId| x != ExprId::NONE;
    let port = if s.port == NO_ROW {
        None
    } else {
        get_row::<PortSpec>(design, s.port)
    };
    let flags = [
        (some(s.tc1) || some(s.tc2), X::SourceTc),
        (some(s.pac_mag), X::SourcePac),
        (!s.noise_psd.is_empty(), X::SourceNoisePsd),
        (s.distof != NO_ROW, X::SourceDistof),
        (s.parasitics != NO_ROW, X::SourceParasitics),
        (port.is_some_and(|p| some(p.noise_temp)), X::PortNoiseTemp),
        (port.is_some_and(|p| some(p.power)), X::PortPower),
    ];
    for (on, x) in flags {
        if on {
            add(x);
        }
    }
}

fn behav(b: Behav, add: &mut impl FnMut(X)) {
    let some = |x: ExprId| x != ExprId::NONE;
    if some(b.tc1) || some(b.tc2) {
        add(X::BehavTc);
        add(tc_base(b.tc_base));
    }
    if some(b.rpar) {
        add(X::BehavRpar);
    }
    if some(b.temp_abs) {
        add(X::InstanceTemp);
    }
    if some(b.dtemp) {
        add(X::InstanceDtemp);
    }
    match b.output {
        OutKind::Voltage | OutKind::Current | OutKind::Charge | OutKind::Flux => {}
        OutKind::Resistance => add(X::BehavResistance),
        OutKind::Conductance => add(X::BehavConductance),
        OutKind::Capacitance => add(X::BehavCapacitance),
        OutKind::CapacitanceCharge => add(X::BehavCapacitanceCharge),
        OutKind::Inductance => add(X::BehavInductance),
        OutKind::Power => add(X::BehavPower),
    }
}

fn pre(p: PreDelay, offset: X, hold: X, zero: X) -> X {
    match p {
        PreDelay::Offset => offset,
        PreDelay::HoldDelayValue => hold,
        PreDelay::Zero => zero,
    }
}

fn wave(design: &Design, w: WaveRef, add: &mut impl FnMut(X)) {
    let some = |x: ExprId| x != ExprId::NONE;
    match w.kind {
        WaveKind::Pulse => {
            if let Some(p) = get_row::<Pulse>(design, w.index) {
                add(match p.law {
                    PulseLaw::Spice => X::PulseSpice,
                    PulseLaw::Vacask => X::PulseVacask,
                });
                if some(p.cycles) {
                    add(X::PulseCycles);
                }
            }
        }
        WaveKind::Exp => {
            if let Some(x) = get_row::<Exp>(design, w.index) {
                add(match x.td2_from {
                    TdFrom::Zero => X::ExpTd2FromZero,
                    TdFrom::Td1 => X::ExpTd2FromTd1,
                });
            }
        }
        WaveKind::Sin => {
            if let Some(s) = get_row::<Sin>(design, w.index) {
                add(pre(s.pre, X::SinPreOffset, X::SinPreHold, X::SinPreZero));
                if some(s.cycles) {
                    add(X::SinCycles);
                }
                if some(s.amplitude2) {
                    add(X::SinSecondTone);
                }
            }
        }
        WaveKind::Pwl => {
            if let Some(p) = get_row::<Pwl>(design, w.index) {
                pwl(p, add);
            }
        }
        WaveKind::Sffm => {
            if let Some(s) = get_row::<Sffm>(design, w.index) {
                add(match s.law {
                    SffmLaw::Spice3 => X::SffmSpice3,
                    SffmLaw::Ngspice => X::SffmNgspice,
                });
                add(pre(s.pre, X::SffmPreOffset, X::SffmPreHold, X::SffmPreZero));
            }
        }
        WaveKind::Am => {
            if let Some(a) = get_row::<Am>(design, w.index) {
                add(pre(a.pre, X::AmPreOffset, X::AmPreHold, X::AmPreZero));
                if a.mod_time == AmModTime::CarrierPhase {
                    add(X::AmModCarrierPhase);
                }
            }
        }
        WaveKind::TrNoise | WaveKind::TrRandom | WaveKind::Pattern => {}
    }
}

fn pwl(p: Pwl, add: &mut impl FnMut(X)) {
    let some = |x: ExprId| x != ExprId::NONE;
    let flags = [
        (some(p.delay), X::PwlDelay),
        (p.repeat == PwlRepeat::From, X::PwlRepeatFrom),
        (p.repeat == PwlRepeat::Period, X::PwlRepeatPeriod),
        (some(p.repeat_count), X::PwlRepeatCount),
        (p.join == PwlJoin::Ramp, X::PwlJoinRamp),
        (p.join == PwlJoin::RampWidth, X::PwlJoinRampWidth),
        (p.join == PwlJoin::Extrapolate, X::PwlJoinExtrapolate),
        (p.file != NameId::NONE, X::PwlFile),
    ];
    for (on, x) in flags {
        if on {
            add(x);
        }
    }
}
