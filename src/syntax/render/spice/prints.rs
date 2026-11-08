//! The testbench's `.print` cards. ngspice-45's `.print` (manual §15.4.1)
//! takes a kind the deck runs and bare vectors or expressions: a print of
//! a kind no analysis runs is "no … analysis found" (Xyce's `.print
//! homotopy`, `.print dc` beside `.op` alone), a braced expression "no
//! such function as {v", and Xyce's power accessors `p(…)`, `w(…)` (RG
//! `.PRINT`) leave the analysis "not run" (each run by hand). On ngspice
//! such a print or output is left out (W0023), a print format
//! (`PRECISION=`) too (W0024), and Xyce's accessors are spelled as
//! ngspice's (`ngspice_output`); the other targets get the card as read.
//!
//! Contents:
//! - `W::prints(&mut self, prints: &[(String, Vec<String>, u32)], analyses: &[(Ana, u32)])`

use super::W;
use crate::syntax::behaviour::AnalysisForm;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::Ana;

/// What an output of a Xyce-style `.print` is on ngspice.
enum Out {
    Kept(String),
    /// A print format (`PRECISION=`, `WIDTH=`, `FILE=`): W0024.
    Format,
    /// No ngspice form: W0023.
    Gone,
}

/// Outputs nested deeper than this are left as written.
const DEPTH: u32 = 64;

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
                let mut kept: Vec<String> = Vec::with_capacity(outs.len());
                let (mut format, mut gone) = (false, !runs);
                // An ngspice deck's outputs are ngspice's own.
                let own = !self.pair.foreign_prints;
                for o in outs {
                    match if own {
                        Out::Kept(o.clone())
                    } else {
                        ngspice_output(o)
                    } {
                        Out::Kept(k) => kept.push(k),
                        Out::Format => format = true,
                        Out::Gone => gone = true,
                    }
                }
                if format {
                    self.note(Code::W0024, *tok);
                }
                if gone {
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

/// An output as ngspice's `.print` reads it (manual §15.4.1, §31.1
/// `@dev[param]`; each run by hand on ngspice-45): a braced expression
/// bare (its blanks removed, as the card splits on them); Xyce's device
/// current `I(R1)` is `@R1[i]` (`@D1[id]`, `@I1[current]`; ngspice's
/// `i()` reads a branch, so a V, L, E, H or B keeps it), a lead current
/// `IB(Q1)` `@Q1[ib]`, `V(X1:N1)` `V(X1.N1)`, a parameter `R1:R`
/// `@R1[R]`. Xyce's power accessors
/// `P()`, `W()`, its noise outputs `ONOISE`, `INOISE` (V²/Hz; ngspice's
/// `onoise_spectrum` is V/√Hz), `DNO()`, `DNI()`, `N()`, and arithmetic
/// on a current have no form: [`Out::Gone`].
fn ngspice_output(o: &str) -> Out {
    let format = o.split_once('=').is_some_and(|(k, v)| {
        !k.is_empty() && k.bytes().all(|c| c.is_ascii_alphabetic()) && !v.starts_with('=')
    });
    if format {
        return Out::Format;
    }
    let e = match o.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        Some(e) => e.split_whitespace().collect(),
        None => o.to_string(),
    };
    match calls(&e, 0) {
        // ngspice-45's `.print` evaluates arithmetic on node voltages
        // only: `i(V1)*2` prints i(V1), `v(2)*i(V1)` and `@R1[i]*2`
        // print nothing (run by hand).
        Some(k)
            if single(k.strip_prefix('-').unwrap_or(&k))
                || !(k.contains('@') || k.to_ascii_lowercase().contains("i(")) =>
        {
            Out::Kept(k)
        }
        _ => Out::Gone,
    }
}

/// Whether `k` is one vector: a name, `name(…)` or `@dev[param]`.
fn single(k: &str) -> bool {
    let b = k.as_bytes();
    let name = k.trim_start_matches('@');
    let head = name.find(['(', '[']).unwrap_or(name.len());
    let at = k.len() - name.len() + head;
    name[..head]
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.')
        && match b.get(at) {
            None => true,
            Some(b'(') => paren(b, at) == Some(b.len() - 1),
            Some(_) => k.ends_with(']') && !k[at..].contains(['*', '+', '/', '-', '(']),
        }
}

/// `s` with each Xyce accessor spelled for ngspice, at nesting `depth`;
/// `None` when one has no ngspice form.
fn calls(s: &str, depth: u32) -> Option<String> {
    if depth > DEPTH {
        return Some(s.to_string());
    }
    let b = s.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        let start = b[i].is_ascii_alphabetic()
            && (i == 0 || !(ident(b[i - 1]) || matches!(b[i - 1], b'@' | b'.' | b':')));
        if !start {
            let next = s[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&s[i..i + next]);
            i += next;
            continue;
        }
        let mut j = i;
        while j < b.len() && ident(b[j]) {
            j += 1;
        }
        // Xyce's `R1:R`, a device's parameter, is ngspice's `@R1[R]`
        // (one colon; a deeper one names a subcircuit's, left as written).
        let mut p = j + 1;
        while b.get(j) == Some(&b':') && p < b.len() && ident(b[p]) {
            p += 1;
        }
        if p > j + 1
            && b.get(p) != Some(&b':')
            && b.get(p) != Some(&b'(')
            && b.get(i.wrapping_sub(1)) != Some(&b':')
        {
            out.push_str(&format!("@{}[{}]", &s[i..j], &s[j + 1..p]));
            i = p;
            continue;
        }
        let word = &s[i..j];
        let Some(k) = (b.get(j) == Some(&b'(')).then(|| paren(b, j)).flatten() else {
            // Xyce's ONOISE is V²/Hz, ngspice's onoise_spectrum V/√Hz,
            // and ngspice's `.print` squares no vector (below).
            if matches!(word.to_ascii_lowercase().as_str(), "onoise" | "inoise") {
                return None;
            }
            out.push_str(word);
            i = j;
            continue;
        };
        let args = &s[j + 1..k];
        out.push_str(&accessor(word, args, depth)?);
        i = k + 1;
    }
    Some(out)
}

/// The call `word(args)` spelled for ngspice.
fn accessor(word: &str, args: &str, depth: u32) -> Option<String> {
    let lower = word.to_ascii_lowercase();
    let dev = args.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_');
    let letter = args.bytes().next().map(|c| c.to_ascii_lowercase());
    let at = |p: &str| format!("@{args}[{p}]");
    Some(match (lower.as_str(), letter) {
        ("p" | "w" | "dno" | "dni" | "n", _) => return None,
        ("i", Some(b'r' | b'c' | b'g' | b'f')) if dev => at("i"),
        ("i", Some(b'd')) if dev => at("id"),
        ("i", Some(b'i')) if dev => at("current"),
        ("ib" | "ic" | "ie" | "is", Some(b'q')) if dev => at(&lower),
        ("id" | "ig" | "is" | "ib", Some(b'm')) if dev => at(&lower),
        ("id", Some(b'd')) if dev => at("id"),
        ("v" | "vr" | "vi" | "vm" | "vp" | "vdb", _) => {
            format!("{word}({})", args.replace(':', "."))
        }
        _ => format!("{word}({})", calls(args, depth + 1)?),
    })
}

/// The index of the `)` closing the `(` at `b[i]`.
fn paren(b: &[u8], i: usize) -> Option<usize> {
    let mut open = 0u32;
    for (k, &c) in b.iter().enumerate().skip(i) {
        match c {
            b'(' => open += 1,
            b')' if open == 1 => return Some(k),
            b')' => open -= 1,
            _ => {}
        }
    }
    None
}
