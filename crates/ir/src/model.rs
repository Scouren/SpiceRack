//! Device models: the model row (equation family, polarity, version, parameters, bin and
//! derivation links) and the closed list of equation families. Builder `model` in (it defines
//! `DefKind::Model` `name`), rows out to writers.
//!
//! A family is an equation, not a level: a level number or a type word is one dialect's
//! spelling of a family, mapped by that dialect's reader and writer tables (INV-semantics
//! 7.01). There is no neutral level. Look-alikes are separate families (7.18): the same word
//! with two equations is two members, and a writer whose target lacks one refuses it (7.02,
//! 7.03). Parameter names are lowercase and the family's own published names; a dialect's
//! aliases and renames live in its tables (7.10, 7.12). A card is default-complete only where a
//! reader's simulator fills a value the family leaves open (7.13).
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4; `rewrite/INV-constructs.md` F1–F24, D68, D120, D132;
//! `rewrite/INV-semantics.md` 7.01–7.22, 3.09.
//!
//! Contents:
//! - `pub enum ModelFamily`, `pub enum Polarity`; on each: `ALL`, `COUNT`
//! - `pub struct Model`
//! - `RowFacts` for `Model`

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{Handle, RowFacts};
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ExprId, ItemRef, NameId, ParamList, Ref, ScopeId};
use crate::item::DefKind;
use crate::text::Words;

/// An equation family. Each member's doc names the simulators that have it and the type word
/// and level (or master) each spells it with (INV-constructs F3–F5, F22, D68, D120;
/// INV-semantics 7.01–7.04, 7.18). Versions within a family are `Model::version`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ModelFamily {
    // ---- passives ----
    /// Resistor model whose `r` is the default resistance (ng `r`/`res`, hs, lt; sp
    /// `resistor` model; va `sp_resistor`; F9, F10, 6.08).
    Resistor,
    /// Xyce resistor model: its `r` multiplies the instance value, default 1 (xy `R`; 6.08).
    ResistorMultiplier,
    /// Capacitor model (ng `c`/`cap`, F11).
    Capacitor,
    /// Xyce capacitor model: its parameters multiply the instance value (F9).
    CapacitorMultiplier,
    /// Inductor model (ng `l`/`ind`).
    Inductor,
    /// Xyce inductor model: its parameters multiply the instance value (F9).
    InductorMultiplier,
    // ---- diodes ----
    /// SPICE3 junction diode, knee cd/(1+√(cd/IKF)) (ng `d` level 1, hs `d`, va `sp_diode`;
    /// 7.06).
    Diode,
    /// ngspice diode level 3: geometry `w`/`l` → area and perimeter, `m` applied twice (ng `d
    /// level=3`; D66, 6.27, F12).
    DiodeLevel3,
    /// Xyce diode level 1: PSpice knee law cd·√(IKF/(IKF+cd)) (xy `d` level 1; 7.06).
    DiodePspice,
    /// Xyce diode level 2: level 1 with PSpice's VJ/CJO temperature law (xy `d` level 2; 7.07).
    DiodePspiceTemp,
    /// LTspice's standard diode: its own breakdown law (lt `D`; 7.19).
    DiodeLtspice,
    /// LTspice's ideal piecewise diode, selected by `ron roff vfwd vrev` (lt `D`; 7.19).
    DiodeIdeal,
    /// Spectre diode: its `ik` law and `tbv1` breakdown temperature law (sp `diode`; 7.06,
    /// 7.08).
    DiodeSpectre,
    /// JUNCAP200 (xy `d` level 200; 7.02).
    Juncap200,
    /// DIODE_CMC (level 2002 cards; 7.02).
    DiodeCmc,
    // ---- bipolar ----
    /// Gummel-Poon (ng/hs/xy `npn`/`pnp` level 1, lt, sp `bjt`, va `sp_bjt`; F4, F5).
    GummelPoon,
    /// VBIC (ng level 4: 1.2; xy 11 and 12: 1.3, 12 with the thermal node; sp `vbic`: 1.1.5;
    /// va `vbic13`, `vbic13_5t`; 7.04). The version is the model's.
    Vbic,
    /// HICUM/L0 (xy level 230; es v2.1; 7.04).
    Hicum0,
    /// HICUM/L2 (ng level 8, xy 234; 7.01).
    Hicum2,
    /// MEXTRAM (xy 504, hs level 6, sp `bjt504`; es `bjt505`; 7.01, 7.04).
    Mextram,
    /// UCSD HBT (hs `npn`/`pnp` level 11; F3).
    HbtUcsd,
    // ---- MOSFET ----
    /// Spectre `mos0`: level 1 with γ = 0 (a look-alike, 7.18).
    Mos0,
    /// Shichman-Hodges level 1 (ng/hs/xy/lt `nmos`/`pmos` level 1, sp `mos1`, va `sp_mos1`).
    Mos1,
    /// Grove-Frohman level 2 (level 2, sp `mos2`, va `sp_mos2`).
    Mos2,
    /// Semi-empirical level 3 (level 3, sp `mos3`, va `sp_mos3`).
    Mos3,
    /// Sakurai-Newton level 6 (ng/xy level 6, va `sp_mos6`; 7.09).
    Mos6,
    /// Philips MOS9 (ng level 9, va `sp_mos9`; xy level 9 is `Bsim3`, 7.01).
    Mos9,
    /// BSIM1 (ng level 4, hs 13, sp `bsim1`; 7.05).
    Bsim1,
    /// BSIM2 (ng level 5, hs 39, sp `bsim2`; 7.05).
    Bsim2,
    /// BSIM3 (ng 8/49, xy 9 (8, 49 accepted), hs 49/53, sp `bsim3v3`, sp `bsim3` = 2.x, va
    /// `sp_bsim3v3`; F3–F5, 7.01, 7.04).
    Bsim3,
    /// BSIM4 (ng/xy 14/54, hs 54, sp `bsim4`, va `sp_bsim4v8`; 7.04).
    Bsim4,
    /// BSIM-SOI (B3SOI) (xy level 10: 3.2; sp `bsimsoi`: 2.0; 7.01, 7.04).
    BsimSoi,
    /// B3SOI-FD (ng level 55; D68).
    B3SoiFd,
    /// B3SOI-DD (ng level 56; D68).
    B3SoiDd,
    /// B3SOI-PD (ng level 57, sp `b3soipd`; D68, 7.04).
    B3SoiPd,
    /// BSIM-SOI 4 (ng 10/58, xy 70; 7.01).
    B4Soi,
    /// STAG SOI3 (ng level 60; D68).
    Soi3,
    /// EKV 2.6 (xy 260, hs 55; 7.01).
    Ekv,
    /// HiSIM1 (sp `hisim`; 7.04).
    Hisim1,
    /// HiSIM2 (ng level 68; D68).
    Hisim2,
    /// HiSIM-HV (ng level 73; D68).
    HisimHv,
    /// PSP 103 (xy 103, es 1040, va `psp103va`, `psp103v4`; 7.01).
    Psp103,
    /// BSIM-BULK (va `bsimbulk106`; D128).
    BsimBulk,
    /// Power VDMOS (ng/lt `vdmos`, va `sp_vdmos`; D68, D126).
    Vdmos,
    /// RPI UCCM VDMOS (xy `nmos`/`pmos` level 18; a look-alike, 7.18).
    VdmosRpi,
    // ---- JFET, MESFET, IGBT ----
    /// Shichman-Hodges JFET (ng/xy `njf`/`pjf` level 1, lt, sp `jfet`, va `sp_jfet1`; 7.03).
    JfetShichman,
    /// Parker-Skellern JFET (ng level 2, va `sp_jfet2`; 7.03).
    JfetParkerSkellern,
    /// Shockley JFET with θ and δ (xy level 2; 7.03).
    JfetShockley,
    /// HSPICE JFET level 2 (`LAM1`; 7.03).
    JfetHspice2,
    /// Statz MESFET (ng `nmf`/`pmf` level 1, xy level 1, va `sp_mes1`; hs spells it
    /// `njf`/`pjf`; 7.21).
    Mesfet,
    /// Spectre `gaas` (DC as `Mesfet`, its own capacitances, 7.18).
    Gaas,
    /// ngspice's MESA device (`nmf`/`pmf`/`nhfet`/`phfet` levels 2 to 4, `inpdomod.c`; the
    /// level is a parameter of the device, not a family; AMEND-A12 A15 C3).
    Mesa,
    /// HFET level 5 (ng `nhfet`/`phfet` level 5; D28).
    Hfet1,
    /// HFET level 6 (ng `nhfet`/`phfet` level 6; D28).
    Hfet2,
    /// LTspice IGBT (`nigbt`/`pigbt`; D103).
    Igbt,
    // ---- switches ----
    /// Abrupt hysteretic voltage switch: on at vt + vh, off at vt − vh, a negative vh its own
    /// branch (ng `sw`; 6.19).
    SwitchHyst,
    /// LTspice voltage switch: as `SwitchHyst` for vh ≥ 0, a smooth transition for vh < 0 (lt
    /// `SW`; 6.19).
    SwitchHystSmooth,
    /// Xyce voltage switch, log-cubic resistance blend (xy `VSWITCH`; 6.19, D117).
    SwitchLogCubic,
    /// Abrupt hysteretic current switch (ng `csw`).
    CurrentSwitchHyst,
    /// LTspice current switch (lt `CSW`), negative `ih` smooth as `SwitchHystSmooth`.
    CurrentSwitchHystSmooth,
    /// Xyce current switch, log-cubic (xy `ISWITCH`).
    CurrentSwitchLogCubic,
    /// Spectre relay: cubic conductance between `vt1` and `vt2`, clamps on `ropen`/`rclosed`
    /// (sp `relay`; 6.19).
    Relay,
    // ---- lines, cores ----
    /// LTRA lossy line (ng/xy/lt `ltra`; D72).
    Ltra,
    /// TXL single lossy line (ng `txl`; D72).
    Txl,
    /// CPL coupled lines, matrices in ngspice's upper-triangle order (ng `cpl`; D72, 6.21).
    Cpl,
    /// URC distributed RC (ng/lt `urc`; D71, 6.23).
    Urc,
    /// HSPICE W-element RLGC model (`W MODELTYPE=RLGC`; D95, 6.22).
    HspiceW,
    /// Jiles-Atherton magnetic core (xy `core`; D11, D114, 7.18).
    CoreJilesAtherton,
    // ---- builtins (AMEND-A12 C5) ----
    /// The simulator's built-in independent voltage source, no parameters (va `vsource`; run
    /// n2: VACASK instantiates every device through a model). Only a `VSource` names it.
    SourceV,
    /// The built-in independent current source, no parameters (va `isource`). Only an
    /// `ISource` names it.
    SourceI,
    /// The built-in VCVS, no parameters (va `vcvs`). Only a `Vcvs` names it.
    Vcvs,
    /// The built-in VCCS, no parameters (va `vccs`). Only a `Vccs` names it.
    Vccs,
    /// The built-in CCVS, no parameters (va `ccvs`). Only a `Ccvs` names it.
    Ccvs,
    /// The built-in CCCS, no parameters (va `cccs`). Only a `Cccs` names it.
    Cccs,
    /// The built-in mutual inductance, no parameters (va `mutual`). Only a `Coupling` names it.
    Mutual,
    /// The simulator's own ideal transmission line, no parameters (va `tline_ideal`, a loaded
    /// OSDI module; AMEND-A12 A20, R39). Only a `LosslessLine` names it. Unlike the other
    /// builtins a target may need a `DeviceLibrary` for it (VACASK does), so a model of it
    /// without one sets `ModelWithoutLibrary`.
    TlineIdeal,
    // ---- modules ----
    /// XSPICE code model; `Model::word` is the code model's name (`s_xfer`, `d_cosim`; D37,
    /// F20).
    CodeModel,
    /// A model of a loaded Verilog-A/OSDI module (ng `.model m <module>`, va `model m
    /// <module>`); `Model::module` names it (B24, 7.15, 7.16).
    Module,
    /// A family the IR does not know; `Model::word` is the type word as written. A writer of
    /// another dialect refuses it.
    Other,
}

/// A model's polarity (INV-semantics 7.21): from the type word (`nmos`, `pnp`, `pchan`) or a
/// type parameter (sp `type=n`, va `type=-1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Polarity {
    /// The family has no polarity (passives, switches, lines).
    None,
    /// n-type or npn.
    N,
    /// p-type or pnp.
    P,
}

/// A device model, one row per bin. Defines `DefKind::Model` `name` in its scope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    /// The model's name (a bin's full name, `nch.1`).
    pub name: NameId,
    /// Parameters, lowercase, the family's own names, in source order; `level`, `type` and
    /// `version` are never here (they are `family`, `polarity`, `version`), except under
    /// `Other`, `Module` and `CodeModel` (their own parameters) and `level` on `Mesa` (a device
    /// parameter there). No value reads the circuit (the kernel's `ProbeContext`). A19.
    pub params: ParamList,
    /// The base name of a bin (`nch` for `nch.1`; a Spectre model group is a run of bins of
    /// one base); `NONE` when not a bin (F7, 7.14).
    pub bin_of: NameId,
    /// The model this one derives from (lt/PSpice `ako:`; its parameters with these
    /// overrides; F8, 7.14); `name` `NONE` when not derived. Expects `DefKind::Model`.
    pub ako: Ref,
    /// The loaded module a `Module` family model instantiates; `name` `NONE` for every other
    /// family. Expects `DefKind::Module` (7.15, 7.16).
    pub module: Ref,
    /// The family version (`version="4.8"`, a number or a string; 7.04): only on a versioned
    /// family (`circuit_check::versioned`: BSIM, SOI, HiSIM, PSP, VBIC, HICUM, MEXTRAM, EKV,
    /// BSIM-BULK, JUNCAP, DIODE_CMC, and the open families), `NONE` on every other (A19). A
    /// reader whose card is silent on a versioned family writes its simulator's default.
    pub version: ExprId,
    /// The code model's name for `CodeModel`, the type word for `Other`; `NONE` otherwise.
    pub word: NameId,
    /// The equation family.
    pub family: ModelFamily,
    /// The polarity; `Polarity::None` exactly for families without one.
    pub polarity: Polarity,
}

const _: () = assert!(size_of::<ModelFamily>() == 1);
const _: () = assert!(size_of::<Model>() == 44);

impl ModelFamily {
    /// Every member, in declaration order.
    pub const ALL: [ModelFamily; 79] = [
        ModelFamily::Resistor,
        ModelFamily::ResistorMultiplier,
        ModelFamily::Capacitor,
        ModelFamily::CapacitorMultiplier,
        ModelFamily::Inductor,
        ModelFamily::InductorMultiplier,
        ModelFamily::Diode,
        ModelFamily::DiodeLevel3,
        ModelFamily::DiodePspice,
        ModelFamily::DiodePspiceTemp,
        ModelFamily::DiodeLtspice,
        ModelFamily::DiodeIdeal,
        ModelFamily::DiodeSpectre,
        ModelFamily::Juncap200,
        ModelFamily::DiodeCmc,
        ModelFamily::GummelPoon,
        ModelFamily::Vbic,
        ModelFamily::Hicum0,
        ModelFamily::Hicum2,
        ModelFamily::Mextram,
        ModelFamily::HbtUcsd,
        ModelFamily::Mos0,
        ModelFamily::Mos1,
        ModelFamily::Mos2,
        ModelFamily::Mos3,
        ModelFamily::Mos6,
        ModelFamily::Mos9,
        ModelFamily::Bsim1,
        ModelFamily::Bsim2,
        ModelFamily::Bsim3,
        ModelFamily::Bsim4,
        ModelFamily::BsimSoi,
        ModelFamily::B3SoiFd,
        ModelFamily::B3SoiDd,
        ModelFamily::B3SoiPd,
        ModelFamily::B4Soi,
        ModelFamily::Soi3,
        ModelFamily::Ekv,
        ModelFamily::Hisim1,
        ModelFamily::Hisim2,
        ModelFamily::HisimHv,
        ModelFamily::Psp103,
        ModelFamily::BsimBulk,
        ModelFamily::Vdmos,
        ModelFamily::VdmosRpi,
        ModelFamily::JfetShichman,
        ModelFamily::JfetParkerSkellern,
        ModelFamily::JfetShockley,
        ModelFamily::JfetHspice2,
        ModelFamily::Mesfet,
        ModelFamily::Gaas,
        ModelFamily::Mesa,
        ModelFamily::Hfet1,
        ModelFamily::Hfet2,
        ModelFamily::Igbt,
        ModelFamily::SwitchHyst,
        ModelFamily::SwitchHystSmooth,
        ModelFamily::SwitchLogCubic,
        ModelFamily::CurrentSwitchHyst,
        ModelFamily::CurrentSwitchHystSmooth,
        ModelFamily::CurrentSwitchLogCubic,
        ModelFamily::Relay,
        ModelFamily::Ltra,
        ModelFamily::Txl,
        ModelFamily::Cpl,
        ModelFamily::Urc,
        ModelFamily::HspiceW,
        ModelFamily::CoreJilesAtherton,
        ModelFamily::SourceV,
        ModelFamily::SourceI,
        ModelFamily::Vcvs,
        ModelFamily::Vccs,
        ModelFamily::Ccvs,
        ModelFamily::Cccs,
        ModelFamily::Mutual,
        ModelFamily::TlineIdeal,
        ModelFamily::CodeModel,
        ModelFamily::Module,
        ModelFamily::Other,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Polarity {
    /// Every member, in declaration order.
    pub const ALL: [Polarity; 3] = [Polarity::None, Polarity::N, Polarity::P];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl RowFacts for Model {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        crate::circuit_text::model_handles(self, out)
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // A real body: A4's contract for this row. A `NONE` name is no reference.
        let mut row: Model = design.row(at.index);
        if row.ako.name != NameId::NONE {
            f(&mut row.ako, DefKind::Model);
        }
        if row.module.name != NameId::NONE {
            f(&mut row.module, DefKind::Module);
        }
        design.editor().set(at.index, row);
    }

    fn refs(&self, _design: &Design, _at: ItemRef, out: &mut Vec<(Ref, DefKind)>) {
        // `map_refs`'s order
        if self.ako.name != NameId::NONE {
            out.push((self.ako, DefKind::Model));
        }
        if self.module.name != NameId::NONE {
            out.push((self.module, DefKind::Module));
        }
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        crate::circuit_features::model_features(self, set)
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        crate::circuit_check::check_model(self, design, at, out)
    }

    fn dump(&self, design: &Design, ctx: &crate::text_expr::Ctx, out: &mut String) {
        crate::circuit_text::dump_model(self, design, ctx, out)
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        crate::circuit_parse::parse_model(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::circuit_gen::generate_model(g, scope, b, features)
    }
}
