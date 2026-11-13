//! The circuit rows' per-family and per-kind tables that `check`, the builder, the features
//! and the generator share: a family's polarity and versions, the builtin families and the
//! kinds they serve, the family a reference is bound to, and the instance-parameter keys a kind
//! types (A12 C5, A19). A family or kind in, a fact out; no row is read but the bound model's.
//!
//! Cites: `rewrite/AMEND-A12.md` C5, A19; `rewrite/IR-CIRCUIT.md` §3a, §4a; `model.rs` family
//! docs.
//!
//! Contents (crate-private): `pub(crate) fn has_polarity`, `pub(crate) fn needs_version`,
//! `pub(crate) const VERSIONS`, `pub(crate) fn versioned`,
//! `pub(crate) fn typed_key`, `pub(crate) fn library_free`, `pub(crate) fn builtin_kind`,
//! `pub(crate) fn builtin_fits`,
//! `pub(crate) fn bound_family`, `pub(crate) fn mult_class`, `pub(crate) fn voltage_output`

use crate::circuit_extra::CircuitExtra as X;
use crate::circuit_fields::Fields;
use crate::circuit_text::get_row;
use crate::controlled::{Behav, FreqTable, Laplace, OutKind, Poly, Quantity, TableSource};
use crate::design::Design;
use crate::element::Element;
use crate::element::{DetailKind, ElemKind};
use crate::model::{Model, ModelFamily};

/// The generator's version vocabulary: per family, the model standard's versions (what another
/// simulator must match, not a simulator's version parameter: HiSIM2 2.8.0 where ngspice's
/// parameter is 280, B3SOI-FD/DD 2.1 and -PD 2.2.3 and B4SOI 4.4.0 by ngspice's device
/// descriptions where its parameter's default is 2.0 or 4.4) a simulator is known to accept, so a generated design names a device that exists. Data, not a `check` rule
/// (a version outside it is legal IR; a dialect's `admits` decides what it runs). A versioned
/// family without a row draws a fresh string. Provenance: ngspice-45 (tarball sha256
/// f1aad8ab…, `src/spicelib/`), as `ng-models` read it: parser/inpdomod.c (dispatch by level
/// and version prefix), bsim3v32/b3v32set.c:86-92, bsim3/b3check.c:49, bsim4v5/b4v5check.c:64-69,
/// bsim4v6/b4v6check.c:815, bsim4v7/b4v7check.c:834, bsim4/b4check.c:68-70,
/// hisimhv1/hsmhvset.c:76-81, hisimhv2/hsmhv2set.c:136-150, hisim2/hsm2set.c:132-138,
/// hicum2/hicum2setup.c:45, bsimsoi/b4soiset.c:124, b3soi{fd,dd,pd}set.c:69,
/// vbic/vbicsetup.c:360. VACASK 36be560, as `vc-devices` read it (module file names and the VD
/// dev-spice and dev-vacask tables; no run of each module's default yet): PSP 103.4
/// (psp103v4.osdi), BSIM-BULK 106 (bsimbulk106.osdi), VBIC 1.3 (vbic13*), BSIM4 4.8.0–4.8.3 and
/// BSIM3 3.3.0 (already listed).
#[cfg(any(test, feature = "gen"))]
pub(crate) const VERSIONS: [(ModelFamily, &[&str]); 12] = [
    (
        ModelFamily::Bsim3,
        &["3.0", "3.1", "3.2", "3.2.2", "3.2.3", "3.2.4", "3.3.0"],
    ),
    (
        ModelFamily::Bsim4,
        &[
            "4.0.0", "4.1.0", "4.2.0", "4.3.0", "4.4.0", "4.5.0", "4.6.5", "4.7.0", "4.8.0",
            "4.8.1", "4.8.2", "4.8.3",
        ],
    ),
    (ModelFamily::HisimHv, &["1.24", "2.20"]),
    (ModelFamily::Hisim2, &["2.8.0"]),
    (ModelFamily::Hicum2, &["2.4.0"]),
    (ModelFamily::B4Soi, &["4.4.0"]),
    (ModelFamily::B3SoiFd, &["2.1"]),
    (ModelFamily::B3SoiDd, &["2.1"]),
    (ModelFamily::B3SoiPd, &["2.2.3"]),
    (ModelFamily::Vbic, &["1.2", "1.3"]),
    (ModelFamily::Psp103, &["103.4"]),
    (ModelFamily::BsimBulk, &["106"]),
];

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

/// Whether a model of `f` must state its version (round 7, ng-models' L5 finding): a versioned
/// family of a model standard, where "no version" would be each simulator's own default. A
/// `Module` or `Other` model may state one but need not.
pub(crate) fn needs_version(f: ModelFamily) -> bool {
    versioned(f) && !matches!(f, ModelFamily::Module | ModelFamily::Other)
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

/// Whether a model of `f` needs no `DeviceLibrary` on any target: the builtins of A12 C5, and
/// the families with no library at all (R44, A24: a user module's code comes from a `Load`, a
/// code model's and an unknown family's is unbound). The ideal line (A20) is a builtin that
/// VACASK loads, so it is not one of them.
pub(crate) fn library_free(f: ModelFamily) -> bool {
    matches!(
        f,
        ModelFamily::Module | ModelFamily::CodeModel | ModelFamily::Other
    ) || builtin_kind(f).is_some() && f != ModelFamily::TlineIdeal
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
        ModelFamily::TlineIdeal => Some(ElemKind::LosslessLine),
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
            | ElemKind::LosslessLine
    );
    match builtin_kind(family) {
        Some(k) => k == kind,
        None => !only_builtin,
    }
}

/// The family of the model `r` is bound to, if it is bound to a model row (total: a bad
/// definition index is the kernel's `HandleRange`).
pub(crate) fn bound_family(design: &Design, r: crate::id::Ref) -> Option<ModelFamily> {
    let def = design.definitions_all().get(r.def.0 as usize)?;
    (def.item.kind == crate::item::ItemKind::Model)
        .then(|| get_item::<Model>(design, def.item.index))
        .flatten()
        .map(|m| m.family)
}

/// Whether `e` is a controlled source whose output is a voltage: `Vcvs`, `Ccvs`, `Delay`, a
/// `Poly`, `Table`, `Laplace` or `FreqTable` of voltage output, a `Behav` of `OutKind::Voltage`.
/// `false` for any other element or a detail row out of range.
pub(crate) fn voltage_output(design: &Design, e: &Element) -> bool {
    use ElemKind as E;
    let v = Quantity::Voltage;
    match e.kind {
        E::Vcvs | E::Ccvs | E::Delay => true,
        E::Poly => get_row::<Poly>(design, e.detail).is_some_and(|r| r.output == v),
        E::Table => get_row::<TableSource>(design, e.detail).is_some_and(|r| r.output == v),
        E::Laplace => get_row::<Laplace>(design, e.detail).is_some_and(|r| r.output == v),
        E::FreqTable => get_row::<FreqTable>(design, e.detail).is_some_and(|r| r.output == v),
        E::Behav => {
            get_row::<Behav>(design, e.detail).is_some_and(|r| r.output == OutKind::Voltage)
        }
        _ => false,
    }
}

/// The multiplicity member of a kind's class; a controlled source's by whether its output is a
/// voltage (`voltage`, `voltage_output`; ignored for every other kind).
pub(crate) fn mult_class(kind: ElemKind, voltage: bool) -> X {
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
        | E::Delay
            if voltage =>
        {
            X::MultControlledVoltage
        }
        E::Vcvs
        | E::Vccs
        | E::Cccs
        | E::Ccvs
        | E::Poly
        | E::Table
        | E::Behav
        | E::Laplace
        | E::FreqTable
        | E::Delay => X::MultControlledCurrent,
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
