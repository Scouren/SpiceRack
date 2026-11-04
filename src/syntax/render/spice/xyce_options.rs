//! The testbench's options → Xyce's `.options <pkg> k=v` lines, one per
//! run of a package: Xyce's own options keep their `pkg.key`, the portable
//! ones map through the Reference Guide's tables, and one with no Xyce
//! spelling is refused.
//!
//! Cites: Xyce RG pp.96-104 (`.options <pkg>`).
//!
//! Contents:
//! - `W::xyce_options(&mut self, tb: &Tb) -> Vec<String>`

use super::W;
use super::bench::Tb;
use crate::syntax::Dialect;
use crate::syntax::render::capability::Construct;
use crate::syntax::render::spelling::spelling;

impl W<'_, '_> {
    /// Xyce's options: `.options <pkg> k=v` per package. Xyce's own keep
    /// their `pkg.key`; portable ones map through the RG tables, and one
    /// with no Xyce spelling is refused. Consecutive entries of one package
    /// share a line, so a deck's own order reads back unchanged.
    pub(super) fn xyce_options(&mut self, tb: &Tb) -> Vec<String> {
        let mut items: Vec<(String, String)> = Vec::new();
        let kv = |k: &str, v: &str| {
            if v.is_empty() {
                k.to_string()
            } else {
                format!("{k}={v}")
            }
        };
        for (k, v, tok) in &tb.portable {
            let (pkg, key) = match k.to_ascii_lowercase().as_str() {
                "reltol" => ("timeint", "reltol"),
                "vntol" => ("nonlin", "abstol"),
                "gmin" => ("device", "gmin"),
                "tnom" => ("device", "tnom"),
                "temp" => ("device", "temp"),
                "itl1" | "max_iterations" => ("nonlin", "maxstep"),
                "itl4" => ("nonlin-tran", "maxstep"),
                _ => {
                    self.ask(Construct::UnmappedOption, *tok);
                    continue;
                }
            };
            items.push((pkg.into(), kv(key, v)));
        }
        let xyce = spelling(Dialect::Xyce);
        if let Some(t) = tb.temp {
            items.push(("device".into(), kv("temp", &xyce.number(t, false))));
        }
        if let Some(t) = tb.tnom {
            items.push(("device".into(), kv("tnom", &xyce.number(t, false))));
        }
        for (pk, v) in &tb.xyce {
            match pk.split_once('.') {
                Some((pkg, "")) => items.push((pkg.into(), String::new())),
                Some((pkg, k)) => items.push((pkg.into(), kv(k, v))),
                None => items.push((String::new(), kv(pk, v))),
            }
        }
        let mut out: Vec<(String, String)> = Vec::new();
        for (pkg, item) in items {
            match out.last_mut() {
                // A bare package (`.options linsol`) keeps its own line.
                Some((p, line)) if *p == pkg && !item.is_empty() && line != p => {
                    line.push(' ');
                    line.push_str(&item);
                }
                _ => out.push((
                    pkg.clone(),
                    if pkg.is_empty() {
                        item
                    } else if item.is_empty() {
                        pkg
                    } else {
                        format!("{pkg} {item}")
                    },
                )),
            }
        }
        out.into_iter()
            .map(|(_, l)| format!(".options {l}"))
            .collect()
    }
}
