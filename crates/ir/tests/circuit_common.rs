//! Shared helpers of the circuit owner's tests: a design with one deck file, valid rows of every
//! circuit row type built field by field (the tests' own model of a valid row, written from the
//! field docs, never by calling the code under test another way), and diagnostic code lists.
//!
//! Cites: `rewrite/IR-IMPL.md` §2; `rewrite/IR-CIRCUIT.md` §2–§4.
//!
//! Contents: `pub fn deck`, `pub fn n`, `pub fn k`, `pub fn source`, `pub fn port`,
//! `pub fn pulse`, `pub fn sin`, `pub fn semi`, `pub fn line`, `pub fn behav`, `pub fn model`, `pub fn codes`,
//! `pub fn full_set`

#![allow(dead_code)] // each test crate uses a subset; compiled alone it uses none

use spicerack_ir::controlled::Behav;
use spicerack_ir::line::Line;
use spicerack_ir::passive::{IcMode, TcBase};
use spicerack_ir::semi::{InitialState, Semi};
use spicerack_ir::source::{DcRule, PortSpec, Source};
use spicerack_ir::wave::{PreDelay, Pulse, PulseLaw, Sin};
use spicerack_ir::{
    Builder, Code, Design, Diags, ExprId, Feature, FeatureSet, List, Literal, Model, ModelFamily,
    NO_ROW, NameId, NumForm, Polarity, Ref, Role, ScopeId, WaveRef,
};

/// A new design and the id of its deck file's top scope (`t.sp`).
pub fn deck(design: &mut Design) -> ScopeId {
    design.builder().file("t.sp", Role::Deck).1
}

/// A plain integer literal (`3` → mantissa 3, `Plain`).
pub fn n(b: &mut Builder<'_>, value: i64) -> ExprId {
    b.literal(Literal {
        mantissa: value,
        frac: 0,
        exp10: 0,
        form: NumForm::Plain,
    })
}

/// A scale literal (`1k` → `k(b, 1, 3)`).
pub fn k(b: &mut Builder<'_>, mantissa: i64, exp10: i16) -> ExprId {
    b.literal(Literal {
        mantissa,
        frac: 0,
        exp10,
        form: NumForm::Scale,
    })
}

/// A DC-only source row.
pub fn source(dc: ExprId) -> Source {
    Source {
        dc,
        ac_mag: ExprId::NONE,
        ac_phase_deg: ExprId::NONE,
        tc1: ExprId::NONE,
        tc2: ExprId::NONE,
        pac_mag: ExprId::NONE,
        pac_phase_deg: ExprId::NONE,
        wave: WaveRef::NONE,
        noise_psd: List::EMPTY,
        parasitics: NO_ROW,
        port: NO_ROW,
        distof: NO_ROW,
        dc_rule: DcRule::Given,
    }
}

/// A port row with only its required fields.
pub fn port(index: ExprId, z0: ExprId) -> PortSpec {
    PortSpec {
        index,
        z0,
        noise_temp: ExprId::NONE,
        power: ExprId::NONE,
        freq: ExprId::NONE,
    }
}

/// A pulse with every required field `v` and no count.
pub fn pulse(v: ExprId) -> Pulse {
    Pulse {
        v1: v,
        v2: v,
        delay: v,
        rise: v,
        fall: v,
        width: v,
        period: v,
        cycles: ExprId::NONE,
        law: PulseLaw::Spice,
        given_zero: spicerack_ir::wave::GivenZero::AsGiven,
    }
}

/// A sine with every required field `v`, no cycles, no second tone, `Offset` before the delay.
pub fn sin(v: ExprId) -> Sin {
    Sin {
        offset: v,
        amplitude: v,
        freq: v,
        delay: v,
        damping: v,
        phase_deg: v,
        cycles: ExprId::NONE,
        amplitude2: ExprId::NONE,
        freq2: ExprId::NONE,
        pre: PreDelay::Offset,
        given_zero: spicerack_ir::wave::GivenZero::AsGiven,
    }
}

/// A semiconductor row with nothing set.
pub fn semi() -> Semi {
    Semi {
        area: ExprId::NONE,
        ic: List::EMPTY,
        temp_abs: ExprId::NONE,
        dtemp: ExprId::NONE,
        ic_mode: IcMode::None,
        initial: InitialState::Unset,
    }
}

/// A line row with nothing set.
pub fn line() -> Line {
    Line {
        z0: ExprId::NONE,
        td: ExprId::NONE,
        freq: ExprId::NONE,
        nl: ExprId::NONE,
        length: ExprId::NONE,
        vel: ExprId::NONE,
        lumps: ExprId::NONE,
    }
}

/// A behavioural row of `expr` with no temperature terms.
pub fn behav(expr: ExprId, output: spicerack_ir::controlled::OutKind) -> Behav {
    Behav {
        expr,
        tc1: ExprId::NONE,
        tc2: ExprId::NONE,
        temp_abs: ExprId::NONE,
        dtemp: ExprId::NONE,
        rpar: ExprId::NONE,
        output,
        tc_base: TcBase::Tnom,
        tc_law: spicerack_ir::passive::TcLaw::Direct,
    }
}

/// A model row of `family` with no parameters and nothing optional.
pub fn model(name: NameId, family: ModelFamily, polarity: Polarity) -> Model {
    Model {
        name,
        params: List::EMPTY,
        bin_of: NameId::NONE,
        ako: Ref {
            name: NameId::NONE,
            def: spicerack_ir::DefId::NONE,
        },
        module: Ref {
            name: NameId::NONE,
            def: spicerack_ir::DefId::NONE,
        },
        version: ExprId::NONE,
        word: NameId::NONE,
        family,
        polarity,
        bc_sat_area: spicerack_ir::BcSatArea::AreaTimesAreab,
        sub_sat_temp: spicerack_ir::SubSatTemp::LastFactor,
    }
}

/// The codes of every diagnostic, in order.
pub fn codes(diags: &Diags) -> Vec<Code> {
    diags.rows().iter().map(|d| d.code).collect()
}

/// Every feature.
pub fn full_set() -> FeatureSet {
    (0..Feature::COUNT)
        .map(Feature::from_index)
        .fold(FeatureSet::EMPTY, FeatureSet::with)
}
