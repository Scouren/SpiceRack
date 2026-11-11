//! The VACASK `model` lines: an ngspice `.model` kind and LEVEL → the
//! `sp_<module>` VACASK builds from ngspice's device code, or a module in
//! a file of its own (`Behaviours::model_load`: Xyce's VBIC 1.3 is
//! `vbic13`, PSP 103 `psp103va`), each parameter checked against the names
//! the module declares (VACASK stops on another: "Parameter 'utra' not
//! found.") and those ngspice-45 reads on the same device's card (ngspice
//! ignores another: "unrecognized parameter - ignored"). Both name tables
//! are generated, one line per module (`<module> <name> <name> …`,
//! lowercase): `vacask_params.txt` from VACASK 36be560
//! `devices/spice/<module>.va` (`parameter` and `aliasparam` names),
//! `devices/vbic/vbic_1p3.va` (its model parameters) and the OSDI
//! descriptors of `psp103v4.osdi` and `vbic_1p3*.osdi` (`psp103va`'s model
//! parameters; a `<module>.inst` row, the instance parameters of a module
//! in a file of its own), `ngspice_params.txt` from ngspice-45
//! `src/spicelib/devices/<dev>/*.c` (the `IOP*`/`IP*` rows; `OP` rows are
//! outputs).
//!
//! Cites: VACASK 36be560 `devices/spice/*.va`, `devices/vbic/vbic_1p3.va`
//! L194-L229 (its three modules), `devices/psp103v4/psp103.va` L46
//! (`PSP103VA`), `devices/CMakeLists.txt` L10-L13; Xyce RG 7.10 §2.3.17
//! (`Q` levels 11 and 12), §2.3.20.12 (`M` level 103); ngspice-45
//! `parser/inpgmod.c`, `parser/inpdomod.c` L271-L314 (BSIM versions),
//! `devices/*/*.c` parameter tables.
//!
//! Contents:
//! - `W::module_of(&self, n: u32, model: &str) -> Option<&'static str>`
//! - `W::model_lines(&mut self, b: &Block, loads: &mut BTreeSet<String>) -> Vec<String>`
//! - `W::module_card(&self, n: u32, model: &str, nodes: usize, params: &[(String, String)]) -> Option<Code>`
//! - `W::is_vdmos(&self, n: u32, model: &str) -> bool`

use std::collections::BTreeSet;

use super::W;
use super::vacask::q;
use crate::syntax::ast::NONE;
use crate::syntax::behaviour::{MasterAnswer, ModelSemantics, NetlistRules};
use crate::syntax::diag::Code;
use crate::syntax::render::read::{Block, Model};

const VACASK: &str = include_str!("vacask_params.txt");
const NGSPICE: &str = include_str!("ngspice_params.txt");

impl W<'_, '_> {
    /// The module instance `n`'s model `model` is written as (`vbic13`): a
    /// card of this file's whose type and neutral LEVEL
    /// (`Behaviours::neutral_level`) have a master row. `None`: no card
    /// here, or no row.
    // ponytail: a card in another kit file is not looked at (its instance
    // is written as read): `read::file_models`, as the Xyce writer's
    // verdicts, when a deck needs it.
    pub(super) fn module_of(&self, n: u32, model: &str) -> Option<&'static str> {
        let (None, card) = self.rd.instance_model(n, model)? else {
            return None;
        };
        let m = self.rd.model_view(card, crate::syntax::ast::NONE);
        let kind = m.kind.to_ascii_lowercase();
        // The source's number, in the numbering the rows are keyed by.
        let level = self.rd.b.neutral_level(&kind, super::params::level(&m))?;
        match self.b.model_master(&kind, level) {
            MasterAnswer::Master(r) => Some(r.master),
            MasterAnswer::Refused | MasterAnswer::AsWritten => None,
        }
    }

    /// Block `b`'s `model` lines, the OSDI files their modules need added
    /// to `loads`; a model with no VACASK module is refused.
    pub(super) fn model_lines(&mut self, b: &Block, loads: &mut BTreeSet<String>) -> Vec<String> {
        let mut out = Vec::new();
        for &(m, bin) in &b.models {
            let view = self.rd.model_view(m, bin);
            if let Some(k) = self.table_kind(&view) {
                if let Some(rows) = self.model_rows(&view, m, k) {
                    loads.insert(format!("spice/{}.osdi", k.vacask));
                    out.push(format!(
                        "model {} sp_{}{}",
                        q(&view.name),
                        k.vacask,
                        self.vacask_list(None, &rows)
                    ));
                }
                continue;
            }
            let tok = self.rd.main(m);
            match self.vacask_model(&view, tok, loads) {
                // Empty: a parameter refused (`W::osdi_rows`).
                Some(l) if l.is_empty() => {}
                Some(l) => out.push(l),
                None => self.refuse(Code::W0104, tok),
            }
        }
        out
    }

    /// The refusal of a card of `nodes` nodes and `params` on instance
    /// `n`'s model `model`, when that is a module in a file of its own
    /// (`Behaviours::model_load`): such a module takes the terminals and
    /// the instance parameters it declares (`m` aside, the writer's
    /// `$mfactor`), and VACASK stops on another ("Parameter 'dta' not
    /// found.": Xyce and ESPice read an instance `DTA` on a PSP 103 card,
    /// `psp103va` has the model's alone). `None`: written.
    // ponytail: one model view a card, as `W::is_vdmos`: a table of the
    // block's models with a load row when `benches/perf.py` shows it.
    pub(super) fn module_card(
        &self,
        n: u32,
        model: &str,
        nodes: usize,
        params: &[(String, String)],
    ) -> Option<Code> {
        let module = self.module_of(n, model)?;
        // An `sp_` module is ngspice's device: its card is ngspice's.
        self.b.model_load(module)?;
        // Xyce's level 11 card may name a fourth node, the thermal `dt`
        // (RG 7.10 §2.3.17.2); `vbic13` keeps it inside (`vbic_1p3.va`
        // L196-L200), and VACASK stops on the card: "Too many terminals
        // specified at instantiation."
        if module == "vbic13" && nodes > 3 {
            return Some(Code::W0105);
        }
        let rows = format!("{module}.inst");
        debug_assert!(VACASK.lines().any(|l| l.starts_with(&rows)), "{rows}");
        let lacks = |(k, _): &(String, String)| {
            let k = k.to_ascii_lowercase();
            k != "m" && !has(VACASK, &rows, &k)
        };
        params.iter().any(lacks).then_some(Code::W0129)
    }

    /// Whether instance `n`'s model `model` is this file's `vdmos` card
    /// (ngspice's and LTspice's power MOSFET: `d g s` and an optional bulk).
    // ponytail: one model view per M card, and a card in another kit file
    // is not looked at (its instance keeps the MOSFET rule, W0105):
    // `read::file_models`, as the Xyce writer's verdicts, when a deck needs it.
    pub(super) fn is_vdmos(&self, n: u32, model: &str) -> bool {
        match self.rd.instance_model(n, model) {
            Some((None, card)) => {
                let kind = self.rd.model_view(card, NONE).kind;
                kind.eq_ignore_ascii_case("vdmos")
            }
            _ => false,
        }
    }
}

// ---- private ----

impl W<'_, '_> {
    /// Model `m`'s `k=v` rows for VACASK's `sp_<module>`, lowercased and
    /// without `level`: a name the module declares is written; one an
    /// ngspice card sets that ngspice does not read either is left out
    /// (W0112) when the module is ngspice's device code (a module in a
    /// file of its own has no ngspice device behind it: a `level=1040`
    /// card is ESPice's, and ngspice-45 runs it as level 1); a BSIM
    /// `version` is the module's (ngspice-45 `inpdomod.c` L271-L314:
    /// `3.3…` is BSIM3, `4.8…` BSIM4, each the module VACASK builds;
    /// `bsim4v8.va` L164 takes it as a string). `None`: another version,
    /// no module (the caller's W0104); `Some(None)`: a name the source
    /// reads and the module lacks, refused (W0129).
    fn osdi_rows(
        &mut self,
        m: &Model,
        module: &str,
        tok: u32,
    ) -> Option<Option<Vec<(String, String)>>> {
        let mut rows = Vec::with_capacity(m.params.len());
        for (k, v) in &m.params {
            let k = k.to_ascii_lowercase();
            if k == "level" {
                continue;
            }
            // A VDMOS card's polarity flag is the line's `type`
            // (`W::vacask_model`), never a parameter to note or refuse.
            if module == "vdmos" && v.is_empty() && matches!(k.as_str(), "pchan" | "nchan") {
                continue;
            }
            if k == "version" {
                let ver = v
                    .trim()
                    .trim_matches(['"', '\'', '{', '}'])
                    .to_ascii_lowercase();
                let own = match module {
                    "bsim3v3" => "3.3",
                    "bsim4v8" => "4.8",
                    _ => "",
                };
                if !own.is_empty() {
                    if !(ver.starts_with(own) || ver.contains("default")) {
                        return None;
                    }
                    if vacask_has(module, "version") {
                        rows.push((k, format!("\"{ver}\"")));
                    }
                    continue;
                }
            }
            // A Xyce diode's knee is the PSpice law, id·√(IKF/(IKF+id))
            // (`N_DEV_Diode.C:2396-2401`); `sp_diode`'s is ngspice's,
            // id/(1+√(id/IKF)) (`diode.va`, `dioload.c:382-386`).
            if module == "diode"
                && k == "ikf"
                && self.rd.b.netlist_rules == NetlistRules::Xyce
                && crate::syntax::render::models::fold(v) != Some(0.0)
            {
                self.refuse(Code::W0114, tok);
                return Some(None);
            }
            if vacask_has(module, &k) {
                rows.push((k, v.clone()));
            } else if self.rd.b.model_semantics == ModelSemantics::Ngspice
                && self.b.model_load(module).is_none()
                && !ngspice_reads(module, &k)
            {
                self.note(Code::W0112, tok);
            } else {
                self.refuse(Code::W0129, tok);
                return Some(None);
            }
        }
        Some(Some(rows))
    }

    /// Model `m`'s line: its type and LEVEL pick the module (the target's
    /// rows, `Behaviours::model_master`; a type or level with none is
    /// `None`); a kind no SPICE type names is a module the deck loads
    /// itself, passed through ("Model not found" beats a substitute).
    fn vacask_model(
        &mut self,
        m: &Model,
        tok: u32,
        loads: &mut BTreeSet<String>,
    ) -> Option<String> {
        let kind = m.kind.to_ascii_lowercase();
        let name = q(&m.name);
        let mut level = 1u32;
        for (k, v) in &m.params {
            if k.eq_ignore_ascii_case("level") {
                let t = v
                    .trim()
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .trim();
                level = super::spice_num(t)?.0 as u32;
            }
        }
        // The level is the source's number: its neutral one picks the
        // module, and a model the source alone has (`level_alias` to no
        // level: Xyce's JFET 2, where `sp_jfet2` is Parker-Skellern) is no
        // module's.
        let level = self.rd.b.neutral_level(&kind, level)?;
        // `.model x NPN (LEVEL=4)` is VBIC, `LEVEL=8` HICUM: other
        // equations under the same keyword, so no row. `sp_resistor`
        // renames the card's `r`/`tc1` to `model_r`…: a name-for-name card
        // would set the instance's, so R, C, L (and switch) types have none.
        let (module, sign) = match self.b.model_master(&kind, level) {
            MasterAnswer::Master(r) => (r.master, r.type_),
            MasterAnswer::Refused => return None,
            MasterAnswer::AsWritten => {
                return Some(format!(
                    "model {name} {kind}{}",
                    self.vacask_params(m, None)
                ));
            }
        };
        // A VDMOS card names its polarity by a flag (`pchan`; `nchan` is
        // the default: ngspice-45 manual, "Power MOSFET model (VDMOS)"),
        // the module by `type` (`devices/spice/vdmos.va` L113).
        let pchan = |(k, v): &(String, String)| v.is_empty() && k.eq_ignore_ascii_case("pchan");
        let sign = match module == "vdmos" && m.params.iter().any(pchan) {
            true => Some("-1"),
            false => sign,
        };
        let Some(rows) = self.osdi_rows(m, module, tok)? else {
            return Some(String::new());
        };
        // A master with a load row is a module of that name in that file
        // (`vbic13` of `vbic_1p3.osdi`); any other is ngspice's device
        // code, `sp_<master>` of `spice/<master>.osdi`.
        let (file, module) = match self.b.model_load(module) {
            Some(path) => (path.to_string(), module.to_string()),
            None => (format!("spice/{module}.osdi"), format!("sp_{module}")),
        };
        loads.insert(file);
        // `type=` is the polarity, and keeps an otherwise empty list
        // non-empty: `model x sp_diode ( )` is a syntax error.
        Some(format!(
            "model {name} {module}{}",
            self.vacask_list(sign, &rows)
        ))
    }

    /// ` (type=1 k=v …)`, or nothing: `m`'s parameters but its `level`.
    fn vacask_params(&self, m: &Model, sign: Option<&str>) -> String {
        let rows = m
            .params
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("level"));
        let rows: Vec<(String, String)> = rows
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();
        self.vacask_list(sign, &rows)
    }

    /// ` (type=1 k=v …)`, or nothing.
    fn vacask_list(&self, sign: Option<&str>, rows: &[(String, String)]) -> String {
        let mut s = String::new();
        if let Some(t) = sign {
            s.push_str(&format!(" type={t}"));
        }
        for (k, v) in rows {
            s.push_str(&format!(" {k}={}", self.expr(v)));
        }
        if !s.is_empty() {
            s.replace_range(..1, " (");
            s.push(')');
        }
        s
    }
}

/// Whether VACASK's `sp_<module>` declares parameter `name` (lowercase).
/// A module the table lacks declares nothing.
fn vacask_has(module: &str, name: &str) -> bool {
    has(VACASK, module, name)
}

/// Whether ngspice-45 reads `name` (lowercase) on the `.model` card of the
/// device VACASK's `sp_<module>` is built from.
fn ngspice_reads(module: &str, name: &str) -> bool {
    has(NGSPICE, module, name)
}

fn has(table: &str, module: &str, name: &str) -> bool {
    debug_assert!(!module.is_empty());
    debug_assert!(name.bytes().all(|b| !b.is_ascii_uppercase()));
    table
        .lines()
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix(module)?.strip_prefix(' '))
        .is_some_and(|names| names.split(' ').any(|n| n == name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `devices/spice/mos2.va` L117, L140 (`aliasparam vt0 = vto`,
    /// `uo = u0`); ngspice-45 `mos2/mos2.c` has no `utra`.
    #[test]
    fn mos2_has_its_aliases_and_no_utra() {
        assert!(vacask_has("mos2", "vto") && vacask_has("mos2", "vt0"));
        assert!(vacask_has("mos2", "uo") && vacask_has("mos2", "u0"));
        assert!(!vacask_has("mos2", "utra") && !ngspice_reads("mos2", "utra"));
        assert!(ngspice_reads("mos2", "vto") && !vacask_has("nomodule", "vto"));
    }

    /// `devices/vbic/vbic_1p3.va` L453 (`rth`), L308 (`gmin`); L292-L294
    /// (`trise`, `sw_noise`, `sw_et` are instance parameters: Xyce 7.10
    /// ignores one on a model card, "No model parameter SW_ET found",
    /// where VACASK takes it); L300-L302 (`npn`, `pnp`, `type`: the
    /// polarity is the line's own `type=`).
    #[test]
    fn a_vbic_module_has_its_model_names_only() {
        for module in ["vbic13", "vbic13_5t"] {
            assert!(vacask_has(module, "rth") && vacask_has(module, "gmin"));
            assert!(!vacask_has(module, "sw_et") && !vacask_has(module, "type"));
        }
    }

    /// The descriptor of `psp103v4.osdi` (VACASK 36be560
    /// `lib/vacask/mod/`): `type` is the line's own, `dta` a model
    /// parameter and no instance one, `nf` an instance one; `vbic_1p3.osdi`
    /// has `trise` and its aliases. Every master with a load row has both
    /// of its rows.
    #[test]
    fn a_module_in_its_own_file_has_its_instance_names() {
        assert!(vacask_has("psp103va", "toxo") && vacask_has("psp103va", "dta"));
        assert!(!vacask_has("psp103va", "type") && !vacask_has("psp103va", "nf"));
        assert!(has(VACASK, "psp103va.inst", "nf") && !has(VACASK, "psp103va.inst", "dta"));
        assert!(has(VACASK, "vbic13.inst", "sw_et") && has(VACASK, "vbic13_5t.inst", "dtemp"));
        for (master, _) in crate::syntax::Lang::VACASK.behaviours().model_loads {
            for row in [format!("{master} "), format!("{master}.inst ")] {
                assert!(VACASK.lines().any(|l| l.starts_with(&row)), "no row {row}");
            }
        }
    }
}
