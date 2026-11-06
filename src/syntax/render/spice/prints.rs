//! The testbench's `.print` cards. ngspice-45's `.print` (manual §15.4.1)
//! takes a kind the deck runs and bare vectors or expressions: a print of
//! a kind no analysis runs is "no … analysis found" (Xyce's `.print
//! homotopy`, `.print dc` beside `.op` alone), a braced expression "no
//! such function as {v", and Xyce's power accessors `p(…)`, `w(…)` (RG
//! `.PRINT`) leave the analysis "not run" (each run by hand). On ngspice
//! such a print or output is left out (W0023) and a braced expression is
//! written bare; the other targets get the card as read.
//!
//! Contents:
//! - `W::prints(&mut self, prints: &[(String, Vec<String>, u32)], analyses: &[(Ana, u32)])`

use super::W;
use crate::syntax::behaviour::AnalysisForm;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::Ana;

impl W<'_, '_> {
    /// Each `.print` the target takes (`Construct::Print`), spelled for
    /// ngspice as the module says.
    pub(super) fn prints(
        &mut self,
        prints: &[(String, Vec<String>, u32)],
        analyses: &[(Ana, u32)],
    ) {
        for (kind, outs, tok) in prints {
            if self.ask(Construct::Print, *tok) != Action::Native {
                continue;
            }
            let outs: Vec<String> = if self.b.analysis_form == AnalysisForm::Ngspice {
                // The reader keeps a kind it does not know (Xyce's
                // `homotopy`) as the first output: a bare word.
                let word = |o: &String| o.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                let (kind, outs) = match outs.split_first() {
                    Some((k, rest)) if kind.is_empty() && word(k) => (k.as_str(), rest),
                    _ => (kind.as_str(), outs.as_slice()),
                };
                let runs = kind.is_empty() || analyses.iter().any(|(a, _)| runs(a, kind));
                let kept: Vec<String> = outs.iter().filter_map(|o| ngspice_output(o)).collect();
                if !runs || kept.len() < outs.len() {
                    self.note(Code::W0023, *tok);
                }
                if !runs || kept.is_empty() {
                    continue;
                }
                kept
            } else {
                outs.clone()
            };
            let sep = if kind.is_empty() { "" } else { " " };
            let line = format!(".print{sep}{kind} {}", outs.join(" "));
            self.o.token(&line, Space::Line);
        }
    }
}

// ---- private ----

/// Whether analysis `a` produces the vectors a `.print kind` reads.
fn runs(a: &Ana, kind: &str) -> bool {
    let k = kind.to_ascii_lowercase();
    match a {
        Ana::Dc(_) | Ana::DcAxes(_) => k == "dc",
        Ana::Ac(_) => k == "ac",
        Ana::Tran { .. } | Ana::Four(..) => k == "tran",
        Ana::Noise { .. } => k == "noise",
        Ana::Sp(_) => k == "sp",
        Ana::Tf(..) => k == "tf",
        Ana::Sens(..) => k == "sens",
        Ana::Pz(..) => k == "pz",
        // Its inner analysis's kind, which the fold does not keep.
        Ana::MonteCarlo { .. } => true,
        // Refused on every SPICE target (`read::spectre_refusal`).
        Ana::Op | Ana::Spectre { .. } => false,
    }
}

/// An output as ngspice's `.print` reads it: a braced expression bare
/// (its blanks removed, as the card splits on them), a Xyce power
/// accessor `None`.
fn ngspice_output(o: &str) -> Option<String> {
    let lower = o.to_ascii_lowercase();
    if lower.starts_with("p(") || lower.starts_with("w(") {
        return None;
    }
    match o.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        Some(e) => Some(e.split_whitespace().collect()),
        None => Some(o.to_string()),
    }
}
