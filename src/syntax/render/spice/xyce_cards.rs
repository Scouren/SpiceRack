//! The Xyce target's spellings of another dialect's cards: a `k=v` value
//! that is one bare parameter name, braced; an instance of a model card
//! Xyce cannot compute (`xyce_models`, the card left out where it is
//! declared) refused where the deck elaborates it, its card judged in
//! whichever kit file holds it; a measure's `TRIG v(1) VAL=0.5` as Xyce's `TRIG
//! v(1)=0.5`.
//!
//! Cites: Xyce RG 7.10 `.SUBCKT` (`PARAMS:` defaults) and §2.2 (`{}`
//! expressions); Xyce 7.10 runs (a bare name as a subcircuit default is a
//! string: "Cannot convert 'RG' to double for expression R0"; as a MOSFET's
//! `w=w l=l` in a subcircuit no bin of a binned model is selected: "Model is
//! required for device M0 and no valid model card found"; braced, both
//! run); ngspice manual §2.4.3, §2.8 (a value naming a parameter);
//! Xyce RG 7.10 `.MEASURE` (`TRIG <variable>=<value>`; Xyce 7.10 stops on
//! `TRIG v(1) VAL=0.5`: "has invalid MEASURE line", run), ngspice manual
//! §11.4 (`TRIG <variable> VAL=<value>`).
//!
//! Contents:
//! - `W::param_value(&self, v: &str) -> String`, `W::xyce_measure(&self, line: String) -> String`,
//!   `W::xyce_instance(&mut self, n: u32, master: &str, tok: u32)`

use super::W;
use super::xyce_models::xyce_model;
use crate::syntax::ast::NONE;
use crate::syntax::behaviour::{ModelSemantics, ParamForm};
use crate::syntax::render::read::file_models;

impl W<'_, '_> {
    /// A `k=v` parameter's value, [`W::spelled`]; on Xyce, another
    /// dialect's bare parameter name (`w=w`, `r0=rg`) braced (`{w}`), which
    /// Xyce reads unbraced as a string.
    pub(super) fn param_value(&self, v: &str) -> String {
        if self.xyce_from_other() && bare_name(v) {
            return format!("{{{v}}}");
        }
        self.spelled(v)
    }

    /// Instance `n` (master `master`, its card's token `tok`) on Xyce,
    /// from ngspice's equations: refused with its model card's code when
    /// Xyce cannot compute the card (W0050, W0051).
    pub(super) fn xyce_instance(&mut self, n: u32, master: &str, tok: u32) {
        let ngspice = self.rd.b.model_semantics == ModelSemantics::Ngspice;
        if self.b.param_form != ParamForm::Xyce || !ngspice {
            return;
        }
        let Some(at) = self.rd.instance_model(n, master) else {
            return;
        };
        if let (Some(file), Some(kit)) = (at.0, self.rd.kit())
            && self.xyce_files.insert(file)
        {
            let judged = file_models(kit, file, |_, m| xyce_model(true, &m.kind, &m.params).err());
            for (card, code) in judged {
                self.xyce_verdicts.insert((Some(file), card), code);
            }
        }
        let code = match at.0 {
            None => {
                let m = self.rd.model_view(at.1, NONE);
                xyce_model(true, &m.kind, &m.params).err()
            }
            Some(_) => self.xyce_verdicts.get(&at).copied().flatten(),
        };
        if let Some(code) = code {
            self.refuse(code, tok);
        }
    }

    /// Measure card `line` with each `TRIG|TARG <variable> VAL=<value>`
    /// written `TRIG|TARG <variable>=<value>` on Xyce, from another
    /// dialect; `line` itself elsewhere.
    pub(super) fn xyce_measure(&self, line: String) -> String {
        if !self.xyce_from_other() {
            return line;
        }
        let words = words(&line);
        let word = |k: usize| &line[words[k].0..words[k].1];
        let mut out = String::with_capacity(line.len());
        let mut at = 0;
        for k in 2..words.len() {
            let val = word(k)
                .get(..4)
                .is_some_and(|p| p.eq_ignore_ascii_case("val="));
            let trig = ["trig", "targ"]
                .iter()
                .any(|t| word(k - 2).eq_ignore_ascii_case(t));
            if val && trig && !word(k - 1).contains('=') && word(k - 1) != "+" {
                // `<variable> VAL=` → `<variable>=`, the line breaks kept.
                out.push_str(&line[at..words[k - 1].1]);
                out.push('=');
                at = words[k].0 + 4;
            }
        }
        out.push_str(&line[at..]);
        out
    }
}

// ---- private ----

impl W<'_, '_> {
    /// The target writes Xyce's parameters (`ParamForm::Xyce`) and the
    /// source's are another dialect's.
    fn xyce_from_other(&self) -> bool {
        self.b.param_form == ParamForm::Xyce && self.rd.b.param_form != ParamForm::Xyce
    }
}

/// `v` is one name: a letter or `_`, then letters, digits and `_`.
fn bare_name(v: &str) -> bool {
    let mut chars = v.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The byte span of each blank-separated word of `line`.
fn words(line: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, line.len()));
    }
    out
}
