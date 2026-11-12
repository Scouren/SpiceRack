//! A `V` or `I` card's text → its parts as ngspice's device parser takes them (`inp2v.c`,
//! `inp2i.c`, `inpdpar.c` `INPdevParse`, `inpgval.c` `INPgetValue`): two nodes, an optional
//! leading value, then keywords in any order, each with its value or vector. Tokens split as
//! `INPgetTok` splits them (blank, tab, `=`, `(`, `)`, `,`), a `{…}` or `'…'` held whole (numparam
//! substitutes it before the device parser runs). A later keyword replaces an earlier one; a
//! leading value is the DC value whatever `dc` says (`inp2v.c` sets it after the parse: run
//! `V1 a 0 1 dc 3` gives 1).
//!
//! Out: the parts as token text, or why ngspice rejects the card. No IR rows: the caller reads
//! the values.
//!
//! Cites: ngspice-45 `vsrc.c`, `isrc.c` (the parameter tables), `vsrcpar.c`, `isrcpar.c`;
//! runs in `tests/dialect_ngspice_sources.rs`'s fixtures.
//!
//! Contents: `pub(super) struct Card`, `pub(super) enum Bad`,
//! `pub(super) fn card(text: &str, current: bool) -> Result<Card, Bad>`,
//! `pub(super) fn is_value(tok: &str) -> bool`

use crate::ir::WaveKind;

/// A source card's parts, as token text (a value is one token: a number, `{…}`, `'…'` or a
/// parameter name).
#[derive(Clone, Debug, Default)]
pub(super) struct Card<'t> {
    /// The two nodes.
    pub(super) nodes: [&'t str; 2],
    /// The DC value: `Some(None)` is `dc` with no number (ngspice reads 0).
    pub(super) dc: Option<Option<&'t str>>,
    /// The AC magnitude, `Some(None)` given as `ac` alone (magnitude 1).
    pub(super) ac_mag: Option<Option<&'t str>>,
    /// The AC phase.
    pub(super) ac_phase: Option<Option<&'t str>>,
    /// The last waveform keyword and its values.
    pub(super) wave: Option<(WaveKind, Vec<&'t str>)>,
    /// `distof1`, `distof2`: one or two values each.
    pub(super) distof: [Option<Vec<&'t str>>; 2],
    /// `m` (a current source's multiplier); `Some(None)`: 0.
    pub(super) m: Option<Option<&'t str>>,
    /// PWL `r`, and whether it stands after the last waveform keyword.
    pub(super) r: Option<(Option<&'t str>, bool)>,
    /// PWL `td`.
    pub(super) td: Option<Option<&'t str>>,
    /// `portnum`.
    pub(super) portnum: Option<Option<&'t str>>,
    /// `z0`.
    pub(super) z0: Option<Option<&'t str>>,
    /// Whether `pwr`, `freq` or `phase` is given (the port drive).
    pub(super) drive: bool,
    /// Whether a keyword was given twice, or a leading value and `dc` both (the later wins).
    pub(super) replaced: bool,
}

/// Why ngspice rejects a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bad {
    /// Fewer than two nodes, a stray value, an unknown keyword, a wrong value count, or an
    /// unclosed `{`/`'`.
    Invalid,
    /// `external` (a shared-library build's; the pinned binary rejects it).
    External,
}

/// The parts of a card's text after its name; `current` for an `I` card.
pub(super) fn card(text: &str, current: bool) -> Result<Card<'_>, Bad> {
    let toks = tokens(text).ok_or(Bad::Invalid)?;
    if toks.len() < 2 {
        return Err(Bad::Invalid);
    }
    let mut c = Card {
        nodes: [toks[0], toks[1]],
        ..Card::default()
    };
    let mut k = 2;
    let mut lead = None;
    if toks.get(k).is_some_and(|t| is_value(t)) {
        lead = Some(toks[k]);
        k += 1;
    }
    // Bounded: every step takes the keyword and at least itself.
    while k < toks.len() {
        let word = toks[k].to_ascii_lowercase();
        k += 1;
        let start = k;
        while k < toks.len() && is_value(toks[k]) {
            k += 1;
        }
        let vals = &toks[start..k];
        let one = || vals.first().copied();
        match (word.as_str(), current) {
            ("dc", _) | ("c", true) => real(vals, &mut c.dc, &mut c.replaced)?,
            ("acmag", _) => real(vals, &mut c.ac_mag, &mut c.replaced)?,
            ("acphase", _) => real(vals, &mut c.ac_phase, &mut c.replaced)?,
            ("m", true) => real(vals, &mut c.m, &mut c.replaced)?,
            ("td", false) => real(vals, &mut c.td, &mut c.replaced)?,
            ("portnum", false) => real(vals, &mut c.portnum, &mut c.replaced)?,
            ("z0", false) => real(vals, &mut c.z0, &mut c.replaced)?,
            ("pwr" | "freq" | "phase", false) => {
                if vals.len() > 1 {
                    return Err(Bad::Invalid);
                }
                c.drive = true;
            }
            ("r", false) => {
                if vals.len() > 1 {
                    return Err(Bad::Invalid);
                }
                c.replaced |= c.r.replace((one(), c.wave.is_some())).is_some();
            }
            ("ac", _) => {
                if vals.len() > 2 {
                    return Err(Bad::Invalid);
                }
                if let Some(&m) = vals.first() {
                    c.replaced |= c.ac_mag.replace(Some(m)).is_some();
                } else if c.ac_mag.is_none() {
                    c.ac_mag = Some(None);
                }
                if let Some(&p) = vals.get(1) {
                    c.replaced |= c.ac_phase.replace(Some(p)).is_some();
                }
            }
            ("distof1" | "distof2", _) => {
                if vals.is_empty() || vals.len() > 2 {
                    return Err(Bad::Invalid);
                }
                let at = usize::from(word == "distof2");
                c.replaced |= c.distof[at].replace(vals.to_vec()).is_some();
            }
            ("external", _) => return Err(Bad::External),
            (w, _) => {
                let Some(kind) = wave_kind(w) else {
                    return Err(Bad::Invalid);
                };
                // every waveform reads at least two values (vsrcpar.c: `numValue < 2`; TRNOISE
                // and TRRANDOM read their second unchecked)
                if vals.len() < 2 || kind == WaveKind::Pwl && vals.len() % 2 == 1 {
                    return Err(Bad::Invalid);
                }
                c.replaced |= c.wave.replace((kind, vals.to_vec())).is_some();
                // an `r` given before this wave no longer stands after the last one
                if let Some((_, after)) = c.r.as_mut() {
                    *after = false;
                }
            }
        }
    }
    debug_assert!(k == toks.len(), "every token is taken");
    if let Some(v) = lead {
        c.replaced |= c.dc.is_some();
        c.dc = Some(Some(v));
    }
    Ok(c)
}

/// Whether `tok` is a value where ngspice reads one: a number, a `{…}` or `'…'` expression, or a
/// name that is no keyword of a `V` or `I` card (a parameter numparam substitutes; run
/// `V1 a 0 vdd`). ponytail: an undefined name reads as a parameter (unbound), where ngspice
/// stops; the declare step's names would tell them apart.
pub(super) fn is_value(tok: &str) -> bool {
    let Some(&first) = tok.as_bytes().first() else {
        return false;
    };
    if matches!(first, b'0'..=b'9' | b'.' | b'+' | b'-' | b'{' | b'\'') {
        return true;
    }
    // a keyword of either device is no value: numparam leaves an unknown word, and the
    // device parser then reads it as a parameter (`r` on an `I` card: "unknown parameter")
    let word = tok.to_ascii_lowercase();
    (first.is_ascii_alphabetic() || first == b'_')
        && !is_keyword(&word, false)
        && !is_keyword(&word, true)
}

// ---- private ----

/// An `IF_REAL` keyword: one number at most, a missing one reading as 0 (`INPevaluate`'s error
/// ignored); `replaced` notes a second one.
fn real<'t>(
    vals: &[&'t str],
    slot: &mut Option<Option<&'t str>>,
    replaced: &mut bool,
) -> Result<(), Bad> {
    if vals.len() > 1 {
        return Err(Bad::Invalid);
    }
    *replaced |= slot.replace(vals.first().copied()).is_some();
    Ok(())
}

/// A waveform keyword's kind.
fn wave_kind(word: &str) -> Option<WaveKind> {
    Some(match word {
        "pulse" => WaveKind::Pulse,
        "sin" | "sine" => WaveKind::Sin,
        "exp" => WaveKind::Exp,
        "pwl" => WaveKind::Pwl,
        "sffm" => WaveKind::Sffm,
        "am" => WaveKind::Am,
        "trnoise" => WaveKind::TrNoise,
        "trrandom" => WaveKind::TrRandom,
        _ => return None,
    })
}

/// Whether `word` (lowercase) names a parameter of the card's device (`vsrc.c`, `isrc.c`
/// tables, the output-only rows included: ngspice finds them and then refuses them).
fn is_keyword(word: &str, current: bool) -> bool {
    const BOTH: &[&str] = &[
        "dc", "acmag", "acphase", "ac", "external", "distof1", "distof2", "pos_node", "neg_node",
        "function", "order", "coeffs", "acreal", "acimag", "p",
    ];
    const V: &[&str] = &["portnum", "z0", "pwr", "freq", "phase", "i", "r", "td"];
    const I: &[&str] = &["c", "m", "v", "current"];
    wave_kind(word).is_some() || BOTH.contains(&word) || if current { I } else { V }.contains(&word)
}

/// The tokens of `text` as `INPgetTok` splits them, a `{…}` (nested) or `'…'` whole; `None` for
/// an unclosed one.
fn tokens(text: &str) -> Option<Vec<&str>> {
    let bytes = text.as_bytes();
    let sep = |b: u8| matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'=' | b'(' | b')' | b',');
    let mut out = Vec::new();
    let mut at = 0;
    // Bounded: every step moves `at` forward.
    while at < bytes.len() {
        if sep(bytes[at]) {
            at += 1;
            continue;
        }
        let start = at;
        match bytes[at] {
            b'{' => {
                let mut depth = 0usize;
                // Bounded: by the text's length.
                loop {
                    let b = *bytes.get(at)?;
                    at += 1;
                    depth = match b {
                        b'{' => depth + 1,
                        b'}' => depth - 1,
                        _ => depth,
                    };
                    if depth == 0 {
                        break;
                    }
                }
            }
            b'\'' => {
                at += 1 + text[at + 1..].find('\'')? + 1;
            }
            _ => {
                while at < bytes.len() && !sep(bytes[at]) {
                    at += 1;
                }
            }
        }
        out.push(&text[start..at]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_split_as_inpgettok_and_hold_braces_whole() {
        // Oracle: inpgtok.c's separators; numparam's braces and quotes.
        assert_eq!(
            tokens("a 0 pulse(0,1 {x + (y)} '2*z')=3").expect("closed"),
            ["a", "0", "pulse", "0", "1", "{x + (y)}", "'2*z'", "3"]
        );
        assert_eq!(tokens("a 0 {x"), None);
    }

    #[test]
    fn a_leading_value_beats_dc_and_the_last_wave_wins() {
        // Oracle: runs `V1 a 0 1 dc 3` (op 1) and `sin(0 1 1k) pulse(5 6)` (the pulse).
        let c = card("a 0 1 dc 3 sin(0 1 1k) pulse(5 6)", false).expect("valid");
        assert_eq!(c.dc, Some(Some("1")));
        assert_eq!(c.wave, Some((WaveKind::Pulse, vec!["5", "6"])));
        assert!(c.replaced);
        assert_eq!(card("a 0 1 2", false).err(), Some(Bad::Invalid));
        assert_eq!(card("a 0 dc 1 m 2", false).err(), Some(Bad::Invalid));
        assert!(card("a 0 dc 1 m 2", true).is_ok());
    }
}
