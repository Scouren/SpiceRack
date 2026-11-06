//! An LTspice capacitor's or inductor's own parasitics → explicit
//! elements for a target with none: the series resistance and inductance
//! on an internal node, the parallel resistance and capacitance across the
//! card's nodes.
//!
//! Cites: LTspice help, Capacitor (`Rser`, `Lser` in series; `Rpar`,
//! `Cpar` across it; `V`, `Irms`, `Ipk`, `mfg`, `pn`, `type`, `pppkg`,
//! `mtbf`: the part's ratings and catalogue entry, no simulation meaning)
//! and Inductor (`Rser` in series; `Rpar`, `Cpar` across it); ngspice
//! manual §3.2 (C and L take none of these instance parameters: ngspice-45
//! stops at `rser`, "unknown parameter").
//!
//! Contents:
//! - `pub(in crate::syntax::render) struct Part`
//! - `pub(in crate::syntax::render) fn parts(decl: &str, n1: &str, n2: &str, params: &mut Vec<(String, String)>) -> Option<(String, Vec<Part>)>`
//! - `pub(super) fn expand(decl: &str, n1: &str, n2: &str, params: &mut Vec<(String, String)>) -> Option<(String, Vec<String>)>`

/// One element of a part's parasitics: `{letter}{name} a b value`, the
/// value as the card wrote it.
pub(in crate::syntax::render) struct Part {
    pub(in crate::syntax::render) letter: char,
    pub(in crate::syntax::render) name: String,
    pub(in crate::syntax::render) a: String,
    pub(in crate::syntax::render) b: String,
    pub(in crate::syntax::render) value: String,
}

/// The words of a part's ratings and catalogue entry (LTspice help,
/// Capacitor and Inductor): no number of a run reads them.
const RATINGS: [&str; 8] = ["v", "irms", "ipk", "mfg", "pn", "type", "pppkg", "mtbf"];

/// Card `decl n1 n2`'s parasitics, taken out of `params` (the ratings
/// always): the node the element itself then ends at in place of `n1`
/// (`n1` when it has no series part), and the extra elements; `None` when
/// the card has none of them. A zero series part is left out (`Lser=0`).
// ponytail: internal nodes are `{decl}_s1`, `{decl}_s2` and elements
// `R{decl}_ser`…; a deck already using such a name collides (the name
// map knows none of them).
pub(in crate::syntax::render) fn parts(
    decl: &str,
    n1: &str,
    n2: &str,
    params: &mut Vec<(String, String)>,
) -> Option<(String, Vec<Part>)> {
    let take = |params: &mut Vec<(String, String)>, k: &str| {
        let i = params.iter().position(|(p, _)| p.eq_ignore_ascii_case(k))?;
        Some(params.remove(i).1)
    };
    params.retain(|(k, _)| !RATINGS.iter().any(|r| k.eq_ignore_ascii_case(r)));
    let keys = ["rser", "lser", "rpar", "cpar"];
    if !params
        .iter()
        .any(|(k, _)| keys.iter().any(|w| k.eq_ignore_ascii_case(w)))
    {
        return None;
    }
    let zero =
        |v: &String| crate::unit::parse_spice_number(v.trim()).is_some_and(|(x, ..)| x == 0.0);
    let (rser, lser, rpar, cpar) = (
        take(params, "rser").filter(|v| !zero(v)),
        take(params, "lser").filter(|v| !zero(v)),
        take(params, "rpar"),
        take(params, "cpar"),
    );
    let mut out = Vec::new();
    let mut at = n1.to_string();
    // The chain n1 — Rser — Lser — element — n2.
    for (letter, v, node) in [('R', rser, "s1"), ('L', lser, "s2")] {
        if let Some(value) = v {
            let next = format!("{decl}_{node}");
            out.push(Part {
                letter,
                name: format!("{decl}_ser"),
                a: at,
                b: next.clone(),
                value,
            });
            at = next;
        }
    }
    for (letter, v) in [('R', rpar), ('C', cpar)] {
        if let Some(value) = v {
            out.push(Part {
                letter,
                name: format!("{decl}_par"),
                a: n1.to_string(),
                b: n2.to_string(),
                value,
            });
        }
    }
    Some((at, out))
}

/// [`parts`] as SPICE lines.
pub(super) fn expand(
    decl: &str,
    n1: &str,
    n2: &str,
    params: &mut Vec<(String, String)>,
) -> Option<(String, Vec<String>)> {
    let (at, parts) = parts(decl, n1, n2, params)?;
    let lines = parts
        .iter()
        .map(|p| format!("{}{} {} {} {}", p.letter, p.name, p.a, p.b, p.value));
    Some((at, lines.collect()))
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn a_capacitors_parasitics_are_a_chain_and_a_shunt() {
        // LTspice help, Capacitor: Rser and Lser in series, Rpar and Cpar
        // across; ratings dropped; a zero Lser left out. Checks: 3.
        let mut p: Vec<(String, String)> = [
            ("V", "4"),
            ("Rser", "5m"),
            ("Lser", "0"),
            ("Rpar", "1meg"),
            ("ic", "1"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let (at, lines) = expand("C4", "a", "b", &mut p).expect("parasitics");
        assert_eq!(at, "C4_s1");
        assert_eq!(lines, ["RC4_ser a C4_s1 5m", "RC4_par a b 1meg"]);
        assert_eq!(p, [("ic".to_string(), "1".to_string())]);
    }
}
