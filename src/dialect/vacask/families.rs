//! VACASK's module words ↔ `ir::ModelFamily`, the library file each module ships in and its
//! terminal count, and the parameter names of each shipped module: the tables the model and
//! element readers, their writers and `Write::admits` share (S4.1).
//!
//! Facts (each a run on 36be560 or the store's own listing):
//! - each shipped OSDI file holds exactly one module (run mod: a `load` of the file, a `model`
//!   of each candidate word and an instance; every other word is "Device … not found");
//! - the builtins `vsource isource vcvs vccs ccvs cccs mutual` need a `model` line and no `load`
//!   (VD dev-builtin; RUN n2);
//! - `params.txt` lists every parameter of every shipped module as VACASK prints it (`print
//!   model(…)` all, `print instance(…)` the instance ones) and the `aliasparam` names of its
//!   Verilog-A source; `params.sh` beside it regenerates it from the pinned store.
//!
//! The SPICE modules rename a model parameter that clashes with an instance parameter
//! `model_<p>` and an instance parameter that clashes with a model one `instance_<p>` (the
//! headers of `spice/*.va`; C F15): the IR holds `<p>`, the family's own name.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (model, instance); VD dev-spice, dev-vacask,
//! dev-builtin, cir-instance.
//!
//! Contents: `Module`, `FAMILIES`, `PROVEN`, `Params`, `Name`, `module`, `file_word`, `default_word`,
//! `Words` (`Words::of`, `Words::get`), `params`, `kind`, `has_polarity`, `model_name_in`, `model_name_out`, `instance_name_in`,
//! `canonical`, `instance_name_out` (crate-private)

use std::collections::HashMap;
use std::sync::LazyLock;

use super::tables;
use crate::ir;

/// One module VACASK ships: its word, the family its equations are, the file that holds it
/// (`""` for a builtin) and its terminal count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Module {
    /// The module word a `model` line names.
    pub(super) word: &'static str,
    /// Its equation family.
    pub(super) family: ir::ModelFamily,
    /// The file a `load` names for it (VD dev-spice, dev-vacask); `""` for a builtin.
    pub(super) file: &'static str,
    /// Its terminals (the module's port list).
    pub(super) terminals: u8,
}

/// Every module VACASK ships, by word. VACASK's own `diode` (diode.osdi) is left out: its
/// equations are not SPICE3's (a question to the lead), so a model of it is `Private`.
pub(super) const FAMILIES: &[Module] = &[
    m(
        "sp_resistor",
        ir::ModelFamily::Resistor,
        "spice/resistor.osdi",
        2,
    ),
    m(
        "sp_capacitor",
        ir::ModelFamily::Capacitor,
        "spice/capacitor.osdi",
        2,
    ),
    m(
        "sp_inductor",
        ir::ModelFamily::Inductor,
        "spice/inductor.osdi",
        2,
    ),
    m("sp_diode", ir::ModelFamily::Diode, "spice/diode.osdi", 2),
    m("sp_bjt", ir::ModelFamily::GummelPoon, "spice/bjt.osdi", 4),
    m(
        "sp_jfet1",
        ir::ModelFamily::JfetShichman,
        "spice/jfet1.osdi",
        3,
    ),
    m(
        "sp_jfet2",
        ir::ModelFamily::JfetParkerSkellern,
        "spice/jfet2.osdi",
        3,
    ),
    m("sp_mes1", ir::ModelFamily::Mesfet, "spice/mes1.osdi", 3),
    m("sp_mos1", ir::ModelFamily::Mos1, "spice/mos1.osdi", 4),
    m("sp_mos2", ir::ModelFamily::Mos2, "spice/mos2.osdi", 4),
    m("sp_mos3", ir::ModelFamily::Mos3, "spice/mos3.osdi", 4),
    m("sp_mos6", ir::ModelFamily::Mos6, "spice/mos6.osdi", 4),
    m("sp_mos9", ir::ModelFamily::Mos9, "spice/mos9.osdi", 4),
    m("sp_vdmos", ir::ModelFamily::Vdmos, "spice/vdmos.osdi", 5),
    m(
        "sp_bsim3v3",
        ir::ModelFamily::Bsim3,
        "spice/bsim3v3.osdi",
        4,
    ),
    m(
        "sp_bsim4v8",
        ir::ModelFamily::Bsim4,
        "spice/bsim4v8.osdi",
        4,
    ),
    m("resistor", ir::ModelFamily::Resistor, "resistor.osdi", 2),
    m("capacitor", ir::ModelFamily::Capacitor, "capacitor.osdi", 2),
    m("inductor", ir::ModelFamily::Inductor, "inductor.osdi", 2),
    m("bsim3", ir::ModelFamily::Bsim3, "bsim3v3.osdi", 4),
    m("bsim4", ir::ModelFamily::Bsim4, "bsim4v8.osdi", 4),
    m("psp103va", ir::ModelFamily::Psp103, "psp103v4.osdi", 4),
    m("bsimbulk", ir::ModelFamily::BsimBulk, "bsimbulk106.osdi", 5),
    m("vbic13", ir::ModelFamily::Vbic, "vbic_1p3.osdi", 3),
    m("vbic13_4t", ir::ModelFamily::Vbic, "vbic_1p3_4t.osdi", 4),
    m("vbic13_5t", ir::ModelFamily::Vbic, "vbic_1p3_5t.osdi", 5),
    m(
        "tline_ideal",
        ir::ModelFamily::TlineIdeal,
        "tline_ideal.osdi",
        4,
    ),
    m("vsource", ir::ModelFamily::SourceV, "", 2),
    m("isource", ir::ModelFamily::SourceI, "", 2),
    m("vcvs", ir::ModelFamily::Vcvs, "", 4),
    m("vccs", ir::ModelFamily::Vccs, "", 4),
    m("ccvs", ir::ModelFamily::Ccvs, "", 2),
    m("cccs", ir::ModelFamily::Cccs, "", 2),
    m("mutual", ir::ModelFamily::Mutual, "", 0),
];

/// The modules read as a model of their family (R41 a): the builtins (the IR's builtin
/// families are VACASK's own devices, A12 C5) and each shipped module a run proved equal to its
/// family's reference device on the parameters they share (the settle fixture named beside
/// it). Every other shipped module is read as a user module (`ModelFamily::Module`).
pub(super) const PROVEN: &[&str] = &[
    "vsource",
    "isource",
    "vcvs",
    "vccs",
    "ccvs",
    "cccs",
    "mutual",
    // VACASK's own ideal line is the IR's TlineIdeal family by definition (R39, A20).
    "tline_ideal",
    // settle devices_rcl (runs prc, own): every printed digit equal to ngspice-45's R, C, L
    "sp_resistor",
    "sp_capacitor",
    "sp_inductor",
    "resistor",
    "capacitor",
    "inductor",
    // settle devices_semi (run semi): sp_diode (level 1) and sp_mos1, n and p, equal to
    // ngspice-45's D and level-1 M. Not sp_bjt: with area 1.5 and no `ibc` it differs by 0.2 %
    // (run semi_bjt; VD dev-spice "Known differences", base-collector saturation current).
    "sp_diode",
    "sp_mos1",
    // F11: each a pair deck at step 4 (tests/fixtures/pair/vacask-ngspice/sp_*inv.sim).
    "sp_mos3",
    "sp_mos6",
    "sp_mos9",
    "sp_jfet1",
];

/// A shipped module's parameter names, as `params.txt` lists them.
#[derive(Debug, Default)]
pub(super) struct Params {
    /// The instance parameters (`$mfactor` included).
    pub(super) instance: Vec<&'static str>,
    /// The parameters only a `model` line may give.
    pub(super) model: Vec<&'static str>,
    /// `(alias, target)`: a second name VACASK accepts for `target`.
    pub(super) alias: Vec<(&'static str, &'static str)>,
}

/// What a parameter name written on a VACASK line is to its module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Name {
    /// A parameter of the line's kind, under this IR name (the `model_`/`instance_` prefix off).
    Ir(String),
    /// An instance parameter on a model line, or a model parameter on an instance line.
    Misplaced,
    /// No parameter of the module (VACASK refuses it, S 7.11).
    Unknown,
}

/// The module word a model of each family is written with in one design: the module of the file
/// its reachable `DeviceLibrary` names (no path: the default file), the default file's when it
/// has none; `None` when two libraries of the family name files of different modules (then
/// `admits` refuses). Built in one walk per `admits` or `write` call, read in O(1) per model.
pub(super) struct Words(HashMap<ir::ModelFamily, Option<&'static str>>);

/// `params.txt`, generated by `params.sh` from the pinned store.
const PARAMS_TXT: &str = include_str!("families/params.txt");

/// `params.txt` by module word.
static TABLE: LazyLock<HashMap<&'static str, Params>> = LazyLock::new(|| {
    let mut out: HashMap<&'static str, Params> = HashMap::new();
    for line in PARAMS_TXT.lines().filter(|l| !l.starts_with('#')) {
        let mut words = line.split_whitespace();
        let (Some(module), Some(class)) = (words.next(), words.next()) else {
            continue;
        };
        let row = out.entry(module).or_default();
        match class {
            "i" => row.instance.extend(words),
            "m" => row.model.extend(words),
            _ => row.alias.extend(words.filter_map(|w| w.split_once('='))),
        }
    }
    out
});

/// The module VACASK names `word`, if it ships it.
pub(super) fn module(word: &str) -> Option<Module> {
    FAMILIES.iter().copied().find(|m| m.word == word)
}

/// The word of the module the library file `path` (as a `load` writes it) holds; the `sn` and
/// `full` variants hold the default variant's module (VD dev-spice, "Model variants").
pub(super) fn file_word(path: &str) -> Option<&'static str> {
    let path = ["spice/sn/", "spice/full/"]
        .iter()
        .find_map(|v| path.strip_prefix(v).map(|rest| format!("spice/{rest}")))
        .unwrap_or_else(|| path.to_string());
    FAMILIES
        .iter()
        .find(|m| !m.file.is_empty() && m.file == path)
        .map(|m| m.word)
}

/// The word of `family`'s module in its default file (`tables::default_file`), or a builtin's.
pub(super) fn default_word(family: ir::ModelFamily) -> Option<&'static str> {
    match FAMILIES
        .iter()
        .find(|m| m.family == family && m.file.is_empty())
    {
        Some(builtin) => Some(builtin.word),
        None => file_word(&tables::default_file(family)?),
    }
}

impl Words {
    /// The words of `design`'s reachable libraries.
    pub(super) fn of(design: &ir::Design) -> Words {
        let mut out: HashMap<ir::ModelFamily, Option<&'static str>> = HashMap::new();
        let mut seen: HashMap<ir::ModelFamily, Option<&'static str>> = HashMap::new();
        for (at, _) in design.reachable() {
            if at.kind != ir::ItemKind::DeviceLibrary {
                continue;
            }
            let lib: ir::DeviceLibrary = design.row(at.index);
            let word = match lib.path.is_none() {
                true => default_word(lib.family),
                false => file_word(design.str(lib.path)),
            };
            match seen.get(&lib.family) {
                None => {
                    seen.insert(lib.family, word);
                    out.insert(lib.family, word);
                }
                Some(&first) if first != word => {
                    out.insert(lib.family, None);
                }
                Some(_) => {}
            }
        }
        Words(out)
    }

    /// The word a model of `family` is written with.
    pub(super) fn get(&self, family: ir::ModelFamily) -> Option<&'static str> {
        match self.0.get(&family) {
            Some(&w) => w,
            None => default_word(family),
        }
    }
}

/// The parameter names of the module `word`, or `None` for a module VACASK does not ship.
pub(super) fn params(word: &str) -> Option<&'static Params> {
    TABLE.get(word)
}

/// The element kind an instance of a model of `family` is.
pub(super) fn kind(family: ir::ModelFamily) -> ir::ElemKind {
    use ir::{ElemKind as K, ModelFamily as F};
    match family {
        F::Resistor => K::Resistor,
        F::Capacitor => K::Capacitor,
        F::Inductor => K::Inductor,
        F::Diode => K::Diode,
        F::GummelPoon | F::Vbic => K::Bjt,
        F::JfetShichman | F::JfetParkerSkellern => K::Jfet,
        F::Mesfet => K::Mesfet,
        F::SourceV => K::VSource,
        F::SourceI => K::ISource,
        F::Vcvs => K::Vcvs,
        F::Vccs => K::Vccs,
        F::Ccvs => K::Ccvs,
        F::Cccs => K::Cccs,
        F::Mutual => K::Coupling,
        F::TlineIdeal => K::LosslessLine,
        F::Module => K::ModuleModel,
        _ => K::Mosfet,
    }
}

/// Whether a model of `family` has a polarity (`type=`): every shipped module of a
/// transistor family has `type` (default 1, the `spice/*.va` sources), as `ir::check` wants.
pub(super) fn has_polarity(family: ir::ModelFamily) -> bool {
    use ir::ModelFamily as F;
    !matches!(
        family,
        F::Resistor
            | F::Capacitor
            | F::Inductor
            | F::Diode
            | F::DiodeLevel3
            | F::SourceV
            | F::SourceI
            | F::Vcvs
            | F::Vccs
            | F::Ccvs
            | F::Cccs
            | F::Mutual
            | F::TlineIdeal
            | F::Module
    )
}

/// The IR name of parameter `written` on a `model` line of the module `word`.
pub(super) fn model_name_in(word: &str, written: &str) -> Name {
    let Some(p) = params(word) else {
        return Name::Ir(written.to_string());
    };
    let target = p.target(written);
    if p.model.contains(&target) {
        let ir = written.strip_prefix("model_").unwrap_or(written);
        return Name::Ir(ir.to_string());
    }
    match p.instance.contains(&target) {
        true => Name::Misplaced,
        false => Name::Unknown,
    }
}

/// The name a `model` line of the module `word` spells the IR model parameter `ir` with.
pub(super) fn model_name_out(word: &str, ir: &str) -> String {
    let prefixed = format!("model_{ir}");
    match params(word).is_some_and(|p| p.has(&prefixed)) {
        true => prefixed,
        false => ir.to_string(),
    }
}

/// The IR name of parameter `written` on an instance of a model of the module `word`.
pub(super) fn instance_name_in(word: &str, written: &str) -> Name {
    let Some(p) = params(word) else {
        return Name::Ir(written.to_string());
    };
    let target = p.target(written);
    if p.instance.contains(&target) {
        let ir = written.strip_prefix("instance_").unwrap_or(written);
        return Name::Ir(ir.to_string());
    }
    match p.model.contains(&target) {
        true => Name::Misplaced,
        false => Name::Unknown,
    }
}

/// The parameter `written` stands for on the module `word` (an alias's target, else itself).
pub(super) fn canonical(word: &str, written: &str) -> String {
    params(word)
        .map_or(written, |p| p.target(written))
        .to_string()
}

/// The name an instance of a model of the module `word` spells the IR parameter `ir` with.
pub(super) fn instance_name_out(word: &str, ir: &str) -> String {
    let prefixed = format!("instance_{ir}");
    match params(word).is_some_and(|p| p.has(&prefixed)) {
        true => prefixed,
        false => ir.to_string(),
    }
}

// ---- private ----

impl Params {
    /// The parameter `name` stands for (itself, or an alias's target).
    fn target<'a>(&self, name: &'a str) -> &'a str {
        self.alias
            .iter()
            .find(|(a, _)| *a == name)
            .map_or(name, |&(_, t)| t)
    }

    /// Whether `name` is a parameter or an alias of the module.
    fn has(&self, name: &str) -> bool {
        self.instance.contains(&name)
            || self.model.contains(&name)
            || self.alias.iter().any(|(a, _)| *a == name)
    }
}

/// A `FAMILIES` row.
const fn m(
    word: &'static str,
    family: ir::ModelFamily,
    file: &'static str,
    terminals: u8,
) -> Module {
    Module {
        word,
        family,
        file,
        terminals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shipped file module has a listing, and its renames are inverse: a `model_<p>`
    /// model parameter's `<p>` is no model parameter, an `instance_<p>`'s `<p>` no instance one.
    #[test]
    fn every_module_is_listed_and_its_renames_invert() {
        for m in FAMILIES.iter().filter(|m| !m.file.is_empty()) {
            let p = params(m.word).unwrap_or_else(|| panic!("{} listed", m.word));
            assert!(p.instance.contains(&"$mfactor"), "{}", m.word);
            for name in p.model.iter().chain(p.instance.iter()) {
                for (class, prefix) in [("m", "model_"), ("i", "instance_")] {
                    if let Some(bare) = name.strip_prefix(prefix) {
                        let own = match class {
                            "m" => &p.model,
                            _ => &p.instance,
                        };
                        assert!(!own.contains(&bare), "{}: {name} and {bare}", m.word);
                    }
                }
            }
        }
    }

    /// A prefixed name reads to the bare IR name and is written back prefixed.
    #[test]
    fn renames_round_trip() {
        assert_eq!(
            model_name_in("sp_resistor", "model_tc1"),
            Name::Ir("tc1".into())
        );
        assert_eq!(model_name_out("sp_resistor", "tc1"), "model_tc1");
        assert_eq!(model_name_in("sp_resistor", "tc1"), Name::Misplaced);
        assert_eq!(model_name_in("sp_resistor", "bogus"), Name::Unknown);
        assert_eq!(
            instance_name_in("sp_bsim3v3", "instance_nqsmod"),
            Name::Ir("nqsmod".into())
        );
        assert_eq!(instance_name_out("sp_bsim3v3", "nqsmod"), "instance_nqsmod");
        assert_eq!(file_word("spice/sn/resistor.osdi"), Some("sp_resistor"));
        assert_eq!(default_word(ir::ModelFamily::Resistor), Some("sp_resistor"));
        assert_eq!(default_word(ir::ModelFamily::SourceV), Some("vsource"));
    }
}
