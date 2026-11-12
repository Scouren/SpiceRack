//! VACASK names: a name token's bytes → the name (quoted parts `'…'` unquoted, `''` a quote),
//! and a name → its spelling (bare when it is an identifier VACASK reads bare, else quoted
//! whole). Names are exact bytes: VACASK keeps case (RUN n4), so nothing here folds.
//!
//! Cites: VD input-identifiers, input-reserved, cir-nodes (a node may be a non-negative
//! integer; `0` is ground).
//!
//! Contents: `Place`, `unquote`, `spell`, `GROUND` (crate-private)

/// Ground's name in VACASK and in the IR (`NameId::GROUND`).
pub(super) const GROUND: &str = "0";

/// Where a name stands: a node may be a bare non-negative integer, nothing else may.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
    /// A node of a terminal list, `global`, `ground` or a probe.
    Node,
    /// Any other name: an instance, model, subcircuit, parameter or section.
    Other,
}

/// The words VACASK reads as keywords at a statement's head (VD input-reserved).
const RESERVED: &[&str] = &[
    "include",
    "section",
    "endsection",
    "load",
    "model",
    "global",
    "ground",
    "subckt",
    "ends",
    "parameters",
    "control",
    "endc",
    "analysis",
    "sweep",
    "embed",
    "save",
];

/// The name `text` (a name token as written) spells.
pub(super) fn unquote(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('\'', true) if chars.peek() == Some(&'\'') => {
                chars.next();
                out.push('\'');
            }
            ('\'', _) => quoted = !quoted,
            (c, _) => out.push(c),
        }
    }
    out
}

/// Appends `name` as VACASK reads it back: bare when it is an identifier (a letter, `_` or `$`,
/// then letters, digits, `_`, `$`) and no reserved word, or at a [`Place::Node`] a
/// non-negative integer; else `'…'` with each `'` doubled.
pub(super) fn spell(name: &str, place: Place, out: &mut String) {
    let ident = name
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_' || b == b'$')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
        && !RESERVED.contains(&name);
    let number =
        place == Place::Node && !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit());
    if ident || number {
        out.push_str(name);
        return;
    }
    out.push('\'');
    out.push_str(&name.replace('\'', "''"));
    out.push('\'');
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: VD input-identifiers (quoted and mixed identifiers, `''` a quote).
    #[test]
    fn unquote_reads_quoted_parts() {
        for (text, want) in [
            ("r1", "r1"),
            ("'x1:out'", "x1:out"),
            ("R'1'", "R1"),
            ("'3.3'V", "3.3V"),
            ("'it''s'", "it's"),
            ("$mfactor", "$mfactor"),
        ] {
            assert_eq!(unquote(text), want, "{text}");
        }
    }

    /// Oracle: VD input-identifiers, input-reserved, cir-nodes; read back by `unquote`.
    #[test]
    fn spell_quotes_what_is_no_identifier() {
        for (name, place, want) in [
            ("r1", Place::Other, "r1"),
            ("$temp", Place::Other, "$temp"),
            ("0", Place::Node, "0"),
            ("12", Place::Node, "12"),
            ("12", Place::Other, "'12'"),
            ("x1:out", Place::Node, "'x1:out'"),
            ("model", Place::Other, "'model'"),
            ("it's", Place::Other, "'it''s'"),
        ] {
            let mut out = String::new();
            spell(name, place, &mut out);
            assert_eq!(out, want, "{name}");
            assert_eq!(unquote(&out), name);
        }
    }
}
