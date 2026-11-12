//! ngspice-45's model table as data: a `.model` card's type word and `level` → `ir::ModelFamily`
//! and polarity (`dispatch`), and back (`card`); the `version` that picks a device within a
//! family; the device whose parameter tables a family's card is read against (`device`,
//! `accepts`); the XSPICE code models and built-in device names a type word may be.
//!
//! The level dispatch is `spicelib/parser/inpdomod.c` `INPdomodel`: one switch per group of type
//! words (`npn pnp`, `njf pjf`, `nmf pmf nhfet phfet`, `nmos pmos nsoi psoi`), each level a
//! device; a level no case has is an error and the card is ignored (runs: `nmos level=7`, `15`
//! "Device type MOS7 not available in this binary"). The reader's level is `INPfindLev`'s:
//! `(int)(value + 0.5)`, above 99 or below 0 "level=1 assumed" (`inpfindl.c`; INV 7.02), which
//! is not the card as written. BSIM3, BSIM4 and HiSIM-HV pick their device by the version's
//! prefix (`INPfindVer`, `prefix`); every other family has one device. Of each group's levels
//! the first is the writer's: 49 and 54 for BSIM3 and BSIM4 (the PDK spellings), no level where
//! level 1 is the family's; MESA's level (2-4) is its own parameter, kept in the row's params.
//!
//! The parameter names are `families/params.txt`, generated from the ngspice-45 source by
//! `families/params.py` (the settable rows of each device's model and instance `IFparm` tables,
//! as `inpgmod.c` `create_model` accepts them).
//!
//! Cites: ngspice-45 `inpdomod.c`, `inpfindl.c`, `inpfindv.c`, `inpgmod.c` (`create_model`,
//! `INPgetModBin`), `inptyplk.c`, the devices' `*set.c` default versions (`b3set.c:84`,
//! `b4set.c:205`, `hsmhv2set.c:137`, `hsm2set.c:133`, `hicum2setup.c:45`, `b4soiset.c:124`,
//! `b3soi*set.c:69`, `vbicsetup.c:360`); a `devhelp` run for the device and code-model names;
//! `rewrite/INV-semantics.md` 7.01, 7.02, 7.04, 7.21.
//!
//! Contents: `pub(super) enum Reading`, `pub(super) fn dispatch(&str, i64) -> Reading`,
//! `pub(super) fn levelled(&str) -> bool`, `pub(super) fn fixed(&str) -> Option<ir::ModelFamily>`,
//! `pub(super) fn card(ir::ModelFamily, ir::Polarity) -> Option<(&'static str, Option<u32>)>`,
//! `pub(super) fn polarity_word(&str) -> ir::Polarity`,
//! `pub(super) fn version(ir::ModelFamily) -> Option<(&'static str, &'static str)>`,
//! `pub(super) fn dispatches(ir::ModelFamily) -> bool`,
//! `pub(super) fn device(ir::ModelFamily, Option<&str>) -> Option<&'static str>`,
//! `pub(super) fn accepts(&str, &str) -> bool`, `pub(super) fn binnable(ir::ModelFamily) -> bool`,
//! `pub(super) fn code_model(&str) -> bool`, `pub(super) fn device_name(&str) -> bool`

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ir::{ModelFamily as F, Polarity};

/// What ngspice makes of a type word at a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reading {
    /// A family the IR has.
    Family(F),
    /// No device: ngspice ignores the card and its instances fail.
    Invalid,
}

/// The type-word groups of `INPdomodel` that dispatch on `level`, and their words (n, p).
const GROUPS: [(Group, [&str; 2]); 6] = [
    (Group::Bjt, ["npn", "pnp"]),
    (Group::Jfet, ["njf", "pjf"]),
    (Group::Mes, ["nmf", "pmf"]),
    (Group::Mes, ["nhfet", "phfet"]),
    (Group::Mos, ["nmos", "pmos"]),
    (Group::Mos, ["nsoi", "psoi"]),
];

/// A group's levels → its family; the first level is the writer's (0 or 1 mean no level).
const LEVELS: &[(Group, &[i64], Reading)] = &[
    (Group::Bjt, &[1, 0, 2], Reading::Family(F::GummelPoon)),
    (Group::Bjt, &[4, 9], Reading::Family(F::Vbic)),
    (Group::Bjt, &[8], Reading::Family(F::Hicum2)),
    (Group::Jfet, &[1, 0], Reading::Family(F::JfetShichman)),
    (Group::Jfet, &[2], Reading::Family(F::JfetParkerSkellern)),
    (Group::Mes, &[1, 0], Reading::Family(F::Mesfet)),
    (Group::Mes, &[2, 3, 4], Reading::Family(F::Mesa)),
    (Group::Mes, &[5], Reading::Family(F::Hfet1)),
    (Group::Mes, &[6], Reading::Family(F::Hfet2)),
    (Group::Mos, &[1, 0], Reading::Family(F::Mos1)),
    (Group::Mos, &[2], Reading::Family(F::Mos2)),
    (Group::Mos, &[3], Reading::Family(F::Mos3)),
    (Group::Mos, &[4], Reading::Family(F::Bsim1)),
    (Group::Mos, &[5], Reading::Family(F::Bsim2)),
    (Group::Mos, &[6], Reading::Family(F::Mos6)),
    (Group::Mos, &[49, 8], Reading::Family(F::Bsim3)),
    (Group::Mos, &[9], Reading::Family(F::Mos9)),
    (Group::Mos, &[10, 58], Reading::Family(F::B4Soi)),
    (Group::Mos, &[54, 14], Reading::Family(F::Bsim4)),
    (Group::Mos, &[55], Reading::Family(F::B3SoiFd)),
    (Group::Mos, &[56], Reading::Family(F::B3SoiDd)),
    (Group::Mos, &[57], Reading::Family(F::B3SoiPd)),
    (Group::Mos, &[60], Reading::Family(F::Soi3)),
    (Group::Mos, &[68], Reading::Family(F::Hisim2)),
    (Group::Mos, &[73], Reading::Family(F::HisimHv)),
];

/// The type words with one family whatever the level (`r`'s `level` is ignored; `d`'s is its
/// own model parameter, read by `read_models`). The first word of a family is the writer's.
const FIXED: &[(&str, F)] = &[
    ("r", F::Resistor),
    ("res", F::Resistor),
    ("c", F::Capacitor),
    ("l", F::Inductor),
    ("d", F::Diode),
    ("sw", F::SwitchHyst),
    ("csw", F::CurrentSwitchHyst),
    ("urc", F::Urc),
    ("ltra", F::Ltra),
    ("txl", F::Txl),
    ("cpl", F::Cpl),
    ("vdmos", F::Vdmos),
];

/// The families with a version parameter: its keyword and the device's default when the card
/// is silent.
const VERSIONS: &[(F, &str, &str)] = &[
    (F::Bsim3, "version", "3.3.0"),
    (F::Bsim4, "version", "4.8.3"),
    (F::HisimHv, "version", "2.20"),
    (F::Hisim2, "version", "280"),
    (F::Hicum2, "version", "2.4.0"),
    (F::B4Soi, "version", "4.4"),
    (F::B3SoiFd, "version", "2.0"),
    (F::B3SoiDd, "version", "2.0"),
    (F::B3SoiPd, "version", "2.0"),
    (F::Vbic, "vers", "1.2"),
];

/// The families with one device each, and the device's `params.txt` key.
const DEVICES: &[(F, &str)] = &[
    (F::Resistor, "res"),
    (F::Capacitor, "cap"),
    (F::Inductor, "ind"),
    (F::SwitchHyst, "sw"),
    (F::CurrentSwitchHyst, "csw"),
    (F::Diode, "dio"),
    (F::DiodeLevel3, "dio"),
    (F::GummelPoon, "bjt"),
    (F::Vbic, "vbic"),
    (F::Hicum2, "hicum2"),
    (F::JfetShichman, "jfet"),
    (F::JfetParkerSkellern, "jfet2"),
    (F::Mesfet, "mes"),
    (F::Mesa, "mesa"),
    (F::Hfet1, "hfet1"),
    (F::Hfet2, "hfet2"),
    (F::Mos1, "mos1"),
    (F::Mos2, "mos2"),
    (F::Mos3, "mos3"),
    (F::Mos6, "mos6"),
    (F::Mos9, "mos9"),
    (F::Bsim1, "bsim1"),
    (F::Bsim2, "bsim2"),
    (F::B3SoiFd, "bsim3soi_fd"),
    (F::B3SoiDd, "bsim3soi_dd"),
    (F::B3SoiPd, "bsim3soi_pd"),
    (F::B4Soi, "bsimsoi"),
    (F::Soi3, "soi3"),
    (F::Hisim2, "hisim2"),
    (F::Vdmos, "vdmos"),
    (F::Urc, "urc"),
    (F::Ltra, "ltra"),
    (F::Txl, "txl"),
    (F::Cpl, "cpl"),
];

/// The version-dispatched families: (family, version prefix or `default`, device), in
/// `INPdomodel`'s order (a later match wins: "`strstr(ver, \"default\")`" is tested last).
const BY_VERSION: &[(F, &str, &str)] = &[
    (F::Bsim3, "3.0", "bsim3v0"),
    (F::Bsim3, "3.1", "bsim3v1"),
    (F::Bsim3, "3.2", "bsim3v32"),
    (F::Bsim3, "3.3", "bsim3"),
    (F::Bsim4, "4.0", "bsim4v5"),
    (F::Bsim4, "4.1", "bsim4v5"),
    (F::Bsim4, "4.2", "bsim4v5"),
    (F::Bsim4, "4.3", "bsim4v5"),
    (F::Bsim4, "4.4", "bsim4v5"),
    (F::Bsim4, "4.5", "bsim4v5"),
    (F::Bsim4, "4.6", "bsim4v6"),
    (F::Bsim4, "4.7", "bsim4v7"),
    (F::Bsim4, "4.8", "bsim4"),
    (F::HisimHv, "1.1", "hisimhv1"),
    (F::HisimHv, "1.2", "hisimhv1"),
    (F::HisimHv, "2.0", "hisimhv2"),
    (F::HisimHv, "2.1", "hisimhv2"),
    (F::HisimHv, "2.2", "hisimhv2"),
];

/// The XSPICE code models of the pinned build (a `devhelp` run in `nix develop .#testing`).
const CODE_MODELS: &str = "spice2poly climit divide d_dt gain hyst ilimit int limit mult multi_input_pwl oneshot \
     pwl sine slew square summer xfer s_xfer triangle filesource delay pwlts adc_bridge \
     dac_bridge bidi_bridge d_and d_buffer d_dff d_dlatch d_fdiv d_genlut d_inverter d_jkff \
     d_lut d_nand d_nor d_open_c d_open_e d_or d_osc d_process d_pulldown d_pullup d_pwm \
     d_ram d_source d_srff d_srlatch d_state d_tff d_tristate d_xnor d_xor d_cosim aswitch \
     capacitoric cmeter core inductoric lcouple lmeter potentiometer zener memristor sidiode \
     pswitch seegen d_to_real real_delay real_gain real_to_v table2d table3d mlin tline \
     cpline cpmlin msopen";

/// The built-in device names `INPtypelook` matches a type word against, any case (`devhelp`).
const DEVICE_NAMES: &str = "urc asrc bjt bsim1 bsim2 bsim3 bsim3v0 bsim3v1 bsim3v32 b4soi bsim4 bsim4v5 bsim4v6 \
     bsim4v7 b3soipd b3soifd b3soidd capacitor cccs ccvs cpllines cswitch diode hfet1 hfet2 \
     hicum2 hisim2 hisimhv1 hisimhv2 inductor mutual isource jfet jfet2 ltra mes mesa mos1 \
     mos2 mos3 mos6 mos9 resistor soi3 switch tranline transline vbic vccs vcvs vsource vdmos \
     nbjt nbjt2 numd numd2 numos";

/// A type-word group of `INPdomodel`'s level switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Bjt,
    Jfet,
    Mes,
    Mos,
}

/// What ngspice makes of `word` (lowercase, one of the level-dispatched words) at `level`
/// (`INPfindLev`'s integer, 0..=99).
pub(super) fn dispatch(word: &str, level: i64) -> Reading {
    assert!((0..=99).contains(&level), "INPfindLev limits the level");
    let group = group(word).expect("a level-dispatched word");
    LEVELS
        .iter()
        .find(|(g, levels, _)| *g == group && levels.contains(&level))
        .map_or(Reading::Invalid, |r| r.2)
}

/// Whether `word` (lowercase) is one whose family `INPdomodel` picks by `level`.
pub(super) fn levelled(word: &str) -> bool {
    group(word).is_some()
}

/// The family of a type word that has one whatever the level (`d`: level 1's).
pub(super) fn fixed(word: &str) -> Option<F> {
    FIXED.iter().find(|(w, _)| *w == word).map(|r| r.1)
}

/// The polarity a type word asks for: `P` for `pnp pjf pmf phfet pmos psoi`, else `N`.
pub(super) fn polarity_word(word: &str) -> Polarity {
    let p = GROUPS.iter().any(|(_, [_, p])| *p == word);
    if p { Polarity::P } else { Polarity::N }
}

/// The type word and level ngspice spells `family` with (`level` `None`: no `level=`), for the
/// polarity given; `None` for a family ngspice lacks. Inverse of [`dispatch`] and [`fixed`] on
/// every family it spells (law L5). `CodeModel` and `Module` are spelled by their name.
pub(super) fn card(family: F, polarity: Polarity) -> Option<(&'static str, Option<u32>)> {
    if family == F::DiodeLevel3 {
        return Some(("d", Some(3)));
    }
    if let Some(&(word, _)) = FIXED.iter().find(|(_, f)| *f == family) {
        return Some((word, None));
    }
    let &(group, levels, _) = LEVELS
        .iter()
        .find(|(_, _, r)| *r == Reading::Family(family))?;
    // the words the device has as its polarity flags (HFET nhfet/phfet, MES nmf/pmf, SOI3
    // nsoi/psoi, the other MOS devices nmos/pmos): only they set the polarity
    let device = device(family, None).expect("a dispatched family has a device");
    let words = GROUPS
        .iter()
        .find(|(g, [n, p])| *g == group && accepts(device, n) && accepts(device, p))
        .map(|g| g.1)
        .expect("each device has its group's flags");
    let word = if polarity == Polarity::P {
        words[1]
    } else {
        words[0]
    };
    // MESA's level (2, 3 or 4) is its own model parameter (mesa.c:108), written with the rest
    let level = (levels[0] > 1 && family != F::Mesa).then_some(levels[0] as u32);
    Some((word, level))
}

/// The keyword and silent default of `family`'s version, or `None` when ngspice's device has
/// no version parameter.
pub(super) fn version(family: F) -> Option<(&'static str, &'static str)> {
    VERSIONS
        .iter()
        .find(|(f, _, _)| *f == family)
        .map(|&(_, k, d)| (k, d))
}

/// Whether the version picks the device (BSIM3, BSIM4, HiSIM-HV), so a version no prefix
/// matches has no device.
pub(super) fn dispatches(family: F) -> bool {
    BY_VERSION.iter().any(|(f, _, _)| *f == family)
}

/// The device (its `params.txt` key) a card of `family` at `version` (`None`: silent) runs, or
/// `None` when ngspice has no device for it. `INPdomodel`'s prefix tests, a later match
/// winning; a silent version is `default`.
pub(super) fn device(family: F, version: Option<&str>) -> Option<&'static str> {
    if !dispatches(family) {
        return DEVICES.iter().find(|(f, _)| *f == family).map(|r| r.1);
    }
    let ver = version.unwrap_or("default");
    let mut found = None;
    for &(_, prefix, dev) in BY_VERSION.iter().filter(|r| r.0 == family) {
        if ver.starts_with(prefix) {
            found = Some(dev);
        }
    }
    // the last test of each switch: `strstr(ver, "default")` picks the newest device
    if ver.contains("default") {
        found = BY_VERSION.iter().rev().find(|r| r.0 == family).map(|r| r.2);
    }
    found
}

/// Whether a `.model` card of `device` sets `name` (lowercase): a settable model parameter, or
/// a settable instance parameter (the instances' default) but `level` and `m`.
pub(super) fn accepts(device: &str, name: &str) -> bool {
    table()
        .get(device)
        .is_some_and(|names| names.binary_search(&name).is_ok())
}

/// Whether ngspice bins `family`'s models (`INPgetModBin`: BSIM3, BSIM4, HiSIM2, HiSIM-HV).
pub(super) fn binnable(family: F) -> bool {
    matches!(family, F::Bsim3 | F::Bsim4 | F::Hisim2 | F::HisimHv)
}

/// Whether `word` (lowercase) is an XSPICE code model of the pinned build.
pub(super) fn code_model(word: &str) -> bool {
    CODE_MODELS.split_whitespace().any(|w| w == word)
}

/// Whether `word` (lowercase) is a built-in device name (`INPtypelook` matches it).
pub(super) fn device_name(word: &str) -> bool {
    DEVICE_NAMES.split_whitespace().any(|w| w == word)
}

// ---- private ----

/// The level group of a type word.
fn group(word: &str) -> Option<Group> {
    GROUPS
        .iter()
        .find(|(_, w)| w.contains(&word))
        .map(|(g, _)| *g)
}

/// `params.txt` as sorted name lists per device, read once.
fn table() -> &'static HashMap<&'static str, Vec<&'static str>> {
    static TABLE: OnceLock<HashMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        include_str!("families/params.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| {
                let mut words = l.split(' ');
                let device = words.next().expect("a device key");
                let mut names: Vec<&str> = words.collect();
                names.sort_unstable();
                (device, names)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_and_dispatch_are_inverse_on_every_family_ngspice_spells() {
        // Oracle: the round trip family → card → reading (law L5), and the list of families
        // ngspice lacks compared by name (inpdomod.c has no device for them).
        let mut lacks = Vec::new();
        for f in F::ALL {
            for p in [Polarity::N, Polarity::P] {
                let Some((word, level)) = card(f, p) else {
                    if p == Polarity::N {
                        lacks.push(format!("{f:?}"));
                    }
                    continue;
                };
                let back = if f == F::DiodeLevel3 {
                    Reading::Family(F::DiodeLevel3)
                } else if levelled(word) {
                    assert_eq!(polarity_word(word), p, "{f:?}");
                    let mesa = if f == F::Mesa { 2 } else { 1 };
                    dispatch(word, i64::from(level.unwrap_or(mesa)))
                } else {
                    assert_eq!(level, None, "{f:?}");
                    Reading::Family(fixed(word).expect("a fixed word"))
                };
                assert_eq!(back, Reading::Family(f), "{f:?} {p:?}");
            }
        }
        let want = [
            "ResistorMultiplier",
            "CapacitorMultiplier",
            "InductorMultiplier",
            "DiodePspice",
            "DiodePspiceTemp",
            "DiodeLtspice",
            "DiodeIdeal",
            "DiodeSpectre",
            "Juncap200",
            "DiodeCmc",
            "Hicum0",
            "Mextram",
            "HbtUcsd",
            "Mos0",
            "BsimSoi",
            "Ekv",
            "Hisim1",
            "Psp103",
            "BsimBulk",
            "VdmosRpi",
            "JfetShockley",
            "JfetHspice2",
            "Gaas",
            "Igbt",
            "SwitchHystSmooth",
            "SwitchLogCubic",
            "CurrentSwitchHystSmooth",
            "CurrentSwitchLogCubic",
            "Relay",
            "HspiceW",
            "CoreJilesAtherton",
            // A12 C5: builtin masters ngspice spells no model for
            "SourceV",
            "SourceI",
            "Vcvs",
            "Vccs",
            "Ccvs",
            "Cccs",
            "Mutual",
            "TlineIdeal",
            "CodeModel",
            "Module",
            "Other",
        ];
        assert_eq!(lacks, want);
    }

    #[test]
    fn features_list_exactly_the_families_card_spells() {
        // Oracle: `card`, the table the reader dispatches by (law L5: FEATURES is the image)
        let features = super::super::read_models::FEATURES;
        for f in F::ALL {
            let spelled = card(f, Polarity::N).is_some() || matches!(f, F::CodeModel | F::Module);
            let has = features.contains(crate::ir::Feature::Family(f));
            assert_eq!(has, spelled, "{f:?}");
        }
    }

    #[test]
    fn every_family_ngspice_spells_has_a_parameter_table() {
        // Oracle: params.txt, generated from the source; a family with no table would admit
        // any name (a silent wrong number).
        for f in F::ALL
            .into_iter()
            .filter(|&f| card(f, Polarity::N).is_some())
        {
            let dev = device(f, None).unwrap_or_else(|| panic!("{f:?}: a device"));
            assert!(table().contains_key(dev), "{f:?}: {dev}");
            // the word the writer spells a p-type model with sets the polarity on its device
            let (word, _) = card(f, Polarity::P).expect("a card");
            assert!(!levelled(word) || accepts(dev, word), "{f:?}: {word}");
        }
        for (_, _, dev) in BY_VERSION {
            assert!(table().contains_key(dev), "{dev}");
        }
    }

    #[test]
    fn versions_pick_the_device_inpdomod_picks() {
        // Oracle: inpdomod.c's prefix tests, case by case.
        assert_eq!(device(F::Bsim4, Some("4.5")), Some("bsim4v5"));
        assert_eq!(device(F::Bsim4, Some("4.6.5")), Some("bsim4v6"));
        assert_eq!(device(F::Bsim4, None), Some("bsim4"));
        assert_eq!(device(F::Bsim4, Some("4.9")), None);
        assert_eq!(device(F::Bsim3, Some("3.24")), Some("bsim3v32"));
        assert_eq!(device(F::Bsim3, Some("2.0")), None);
        assert_eq!(device(F::HisimHv, Some("1.2")), Some("hisimhv1"));
        assert_eq!(device(F::HisimHv, None), Some("hisimhv2"));
        assert!(accepts("bsim4", "toxe") && !accepts("bsim4", "level") && !accepts("mos1", "m"));
        assert!(
            accepts("dio", "m"),
            "a diode model's m is its grading coefficient"
        );
    }
}
