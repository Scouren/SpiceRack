//! The VACASK `model` lines: an ngspice `.model` kind and LEVEL → the
//! `sp_<module>` VACASK builds from ngspice's device code, each parameter
//! checked against the names the module declares (VACASK stops on another:
//! "Parameter 'utra' not found.") and those ngspice-45 reads on the same
//! device's card (ngspice ignores another: "unrecognized parameter -
//! ignored"). Both name tables are generated, one line per module
//! (`<module> <name> <name> …`, lowercase): `vacask_params.txt` from VACASK
//! 36be560 `devices/spice/<module>.va` (`parameter` and `aliasparam`
//! names), `ngspice_params.txt` from ngspice-45
//! `src/spicelib/devices/<dev>/*.c` (the `IOP*`/`IP*` rows; `OP` rows are
//! outputs).
//!
//! Cites: VACASK 36be560 `devices/spice/*.va`; ngspice-45
//! `parser/inpgmod.c`, `parser/inpdomod.c` L271-L314 (BSIM versions),
//! `devices/*/*.c` parameter tables.
//!
//! Contents:
//! - `W::model_lines(&mut self, b: &Block, loads: &mut BTreeSet<String>) -> Vec<String>`

use std::collections::BTreeSet;

use super::W;
use super::vacask::q;
use crate::syntax::behaviour::{MasterAnswer, ModelSemantics};
use crate::syntax::diag::Code;
use crate::syntax::render::read::{Block, Model};

const VACASK: &str = include_str!("vacask_params.txt");
const NGSPICE: &str = include_str!("ngspice_params.txt");

impl W<'_, '_> {
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
}

// ---- private ----

impl W<'_, '_> {
    /// Model `m`'s `k=v` rows for VACASK's `sp_<module>`, lowercased and
    /// without `level`: a name the module declares is written; one an
    /// ngspice card sets that ngspice does not read either is left out
    /// (W0112); a BSIM `version` is the module's (ngspice-45 `inpdomod.c`
    /// L271-L314: `3.3…` is BSIM3, `4.8…` BSIM4, each the module VACASK
    /// builds; `bsim4v8.va` L164 takes it as a string). `None`: another
    /// version, no module (the caller's W0104); `Some(None)`: a name the
    /// source reads and the module lacks, refused (W0129).
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
            if vacask_has(module, &k) {
                rows.push((k, v.clone()));
            } else if self.rd.b.model_semantics == ModelSemantics::Ngspice
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
        let Some(rows) = self.osdi_rows(m, module, tok)? else {
            return Some(String::new());
        };
        loads.insert(format!("spice/{module}.osdi"));
        // `type=` is the polarity, and keeps an otherwise empty list
        // non-empty: `model x sp_diode ( )` is a syntax error.
        Some(format!(
            "model {name} sp_{module}{}",
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
}
