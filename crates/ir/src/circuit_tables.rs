//! The circuit rows' per-family and per-kind tables that `check`, the builder, the features
//! and the generator share: a family's polarity and versions, the builtin families and the
//! kinds they serve, the family a reference is bound to, and the instance-parameter keys a kind
//! types (A12 C5, A19). A family or kind in, a fact out; no row is read but the bound model's.
//!
//! Cites: `rewrite/AMEND-A12.md` C5, A19; `rewrite/IR-CIRCUIT.md` §3a, §4a; `model.rs` family
//! docs.
//!
//! Contents (crate-private): `pub(crate) fn has_polarity`, `pub(crate) fn versioned`,
//! `pub(crate) fn typed_key`, `pub(crate) fn builtin_kind`, `pub(crate) fn builtin_fits`,
//! `pub(crate) fn bound_family`

use crate::circuit_fields::Fields;
use crate::design::Design;
use crate::element::{DetailKind, ElemKind};
use crate::model::{Model, ModelFamily};

/// Whether a family's models are n- or p-type (transistors); `Other` may be either way.
pub(crate) fn has_polarity(f: ModelFamily) -> bool {
    use ModelFamily as F;
    match f {
        F::GummelPoon
        | F::Vbic
        | F::Hicum0
        | F::Hicum2
        | F::Mextram
        | F::HbtUcsd
        | F::Mos0
        | F::Mos1
        | F::Mos2
        | F::Mos3
        | F::Mos6
        | F::Mos9
        | F::Bsim1
        | F::Bsim2
        | F::Bsim3
        | F::Bsim4
        | F::BsimSoi
        | F::B3SoiFd
        | F::B3SoiDd
        | F::B3SoiPd
        | F::B4Soi
        | F::Soi3
        | F::Ekv
        | F::Hisim1
        | F::Hisim2
        | F::HisimHv
        | F::Psp103
        | F::BsimBulk
        | F::Vdmos
        | F::VdmosRpi
        | F::JfetShichman
        | F::JfetParkerSkellern
        | F::JfetShockley
        | F::JfetHspice2
        | F::Mesfet
        | F::Gaas
        | F::Mesa
        | F::Hfet1
        | F::Hfet2
        | F::Igbt => true,
        F::Resistor
        | F::ResistorMultiplier
        | F::Capacitor
        | F::CapacitorMultiplier
        | F::Inductor
        | F::InductorMultiplier
        | F::Diode
        | F::DiodeLevel3
        | F::DiodePspice
        | F::DiodePspiceTemp
        | F::DiodeLtspice
        | F::DiodeIdeal
        | F::DiodeSpectre
        | F::Juncap200
        | F::DiodeCmc
        | F::SwitchHyst
        | F::SwitchHystSmooth
        | F::SwitchLogCubic
        | F::CurrentSwitchHyst
        | F::CurrentSwitchHystSmooth
        | F::CurrentSwitchLogCubic
        | F::Relay
        | F::Ltra
        | F::Txl
        | F::Cpl
        | F::Urc
        | F::HspiceW
        | F::CoreJilesAtherton
        | F::SourceV
        | F::SourceI
        | F::Vcvs
        | F::Vccs
        | F::Ccvs
        | F::Cccs
        | F::Mutual
        | F::TlineIdeal
        | F::CodeModel
        | F::Module
        | F::Other => false,
    }
}

/// Whether a family has more than one version, so `Model::version` names one (BSIM, SOI, HiSIM,
/// PSP, VBIC, HICUM, MEXTRAM, EKV, BSIM-BULK, JUNCAP, DIODE_CMC; and the open families). Every
/// other family has one version everywhere (A19).
pub(crate) fn versioned(f: ModelFamily) -> bool {
    use ModelFamily as F;
    matches!(
        f,
        F::Vbic
            | F::Hicum0
            | F::Hicum2
            | F::Mextram
            | F::Bsim3
            | F::Bsim4
            | F::BsimSoi
            | F::B3SoiFd
            | F::B3SoiDd
            | F::B3SoiPd
            | F::B4Soi
            | F::Hisim1
            | F::Hisim2
            | F::HisimHv
            | F::Psp103
            | F::BsimBulk
            | F::Ekv
            | F::Juncap200
            | F::DiodeCmc
            | F::Module
            | F::Other
    )
}

/// Whether an instance parameter `key` names one of the element's own typed fields (A19): a
/// field of its detail row by its IR name, its multiplicity (`m`, `mult`) or its temperature
/// (`temp`, where the row has `temp_abs`). The names come from the field table, so they follow
/// it. Kinds with no detail row (instances, modules) have none: their parameters are the
/// master's.
pub(crate) fn typed_key(kind: DetailKind, key: &str) -> bool {
    let names = match kind {
        DetailKind::None => return false,
        DetailKind::Passive => names_of::<crate::passive::Passive>(),
        DetailKind::Coupling => names_of::<crate::passive::Coupling>(),
        DetailKind::Source => names_of::<crate::source::Source>(),
        DetailKind::Controlled => names_of::<crate::controlled::Controlled>(),
        DetailKind::Poly => names_of::<crate::controlled::Poly>(),
        DetailKind::Table => names_of::<crate::controlled::TableSource>(),
        DetailKind::Behav => names_of::<crate::controlled::Behav>(),
        DetailKind::Laplace => names_of::<crate::controlled::Laplace>(),
        DetailKind::FreqTable => names_of::<crate::controlled::FreqTable>(),
        DetailKind::Delay => names_of::<crate::controlled::Delay>(),
        DetailKind::Semi => names_of::<crate::semi::Semi>(),
        DetailKind::Switch => names_of::<crate::semi::Switch>(),
        DetailKind::Line => names_of::<crate::line::Line>(),
    };
    let is = |name: &str| key.eq_ignore_ascii_case(name);
    is("m")
        || is("mult")
        || names.iter().any(|&n| is(n))
        || names.contains(&"temp_abs") && is("temp")
}

/// The element kind a builtin family's model serves (AMEND-A12 C5), or `None` for every other
/// family.
pub(crate) fn builtin_kind(f: ModelFamily) -> Option<ElemKind> {
    match f {
        ModelFamily::SourceV => Some(ElemKind::VSource),
        ModelFamily::SourceI => Some(ElemKind::ISource),
        ModelFamily::Vcvs => Some(ElemKind::Vcvs),
        ModelFamily::Vccs => Some(ElemKind::Vccs),
        ModelFamily::Ccvs => Some(ElemKind::Ccvs),
        ModelFamily::Cccs => Some(ElemKind::Cccs),
        ModelFamily::Mutual => Some(ElemKind::Coupling),
        _ => None, // every other family serves the kinds its own docs name
    }
}

/// Whether an element of `kind` may name a model of `family`: a builtin family serves only its
/// kind, and a source or linear controlled source names only its builtin (a coupling may also
/// name a core).
pub(crate) fn builtin_fits(kind: ElemKind, family: ModelFamily) -> bool {
    let only_builtin = matches!(
        kind,
        ElemKind::VSource
            | ElemKind::ISource
            | ElemKind::Vcvs
            | ElemKind::Vccs
            | ElemKind::Cccs
            | ElemKind::Ccvs
    );
    match builtin_kind(family) {
        Some(k) => k == kind,
        None => !only_builtin,
    }
}

/// The family of the model `r` is bound to, if it is bound to a model row (total: a bad
/// definition index is the kernel's `HandleRange`).
pub(crate) fn bound_family(design: &Design, r: crate::id::Ref) -> Option<ModelFamily> {
    let def = design.definitions().get(r.def.0 as usize)?;
    (def.item.kind == crate::item::ItemKind::Model)
        .then(|| get_item::<Model>(design, def.item.index))
        .flatten()
        .map(|m| m.family)
}

// ---- private ----

/// The IR field names of row type `R`, in declaration order.
fn names_of<R: Fields>() -> Vec<&'static str> {
    let mut row = R::BLANK;
    let mut names = Vec::new();
    row.fields(&mut |name, _| names.push(name));
    names
}

/// Row `index` of `R`'s item table, or `None` when out of range.
fn get_item<R: crate::design::ItemRow>(design: &Design, index: u32) -> Option<R> {
    R::table(design).rows.get(index as usize).copied()
}
