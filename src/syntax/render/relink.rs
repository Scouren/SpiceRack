//! A written kit's files named where their caller put them. Each file of a
//! kit is written on its own (`render_kit`) and names the files it includes
//! as its source did, so a caller that stores the written files elsewhere
//! (the cross-simulator harness's run directory, `spice2rack --kit`'s
//! output directory) rewrites those names: text → text, by a table of the
//! kit's paths and their new names.
//!
//! Cites: ngspice manual §2.6 (`.include`), §2.7 (`.lib`); the writers'
//! path quoting (`spelling::Spelling::path_quote`, `spice::blocks`).
//!
//! Contents:
//! - `pub fn relink(text: &str, base: &Path, moved: &HashMap<PathBuf, String>) -> String`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::syntax::resolve::normalize;

/// `text` (a file of a kit written for some target, its source read from
/// the directory `base`) with each path that names a file of `moved`
/// replaced by that file's new name. A path is resolved from `base` as the
/// kit joins an include (`.` and `..` lexically), and `moved` is keyed by
/// the kit's own paths ([`crate::syntax::resolve::File::path`]). Two forms
/// are paths: a quoted span anywhere (the HSPICE, VACASK and Spectre
/// writers quote an include's path; `.hdl` and `ahdl_include` quote a
/// Verilog-A source's) and the bare second word of an `.include`, `.inc`
/// or `.lib` card (the ngspice, ESPice, Xyce and LTspice writers quote
/// only a path with a blank, and a new name with one is quoted the same
/// way).
///
/// A new name holding a quote character or a line break is the caller's
/// to rule out: it would end the span or the card it is written in.
pub fn relink(text: &str, base: &Path, moved: &HashMap<PathBuf, String>) -> String {
    if moved.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(['"', '\'']) {
        let q = &rest[open..=open];
        let Some(len) = rest[open + 1..]
            .find(q)
            .filter(|&n| !rest[open + 1..open + 1 + n].contains('\n'))
        else {
            out.push_str(&rest[..=open]);
            rest = &rest[open + 1..];
            continue;
        };
        let span = &rest[open + 1..open + 1 + len];
        out.push_str(&rest[..=open]);
        out.push_str(
            moved
                .get(&normalize(&base.join(span)))
                .map_or(span, String::as_str),
        );
        out.push_str(q);
        rest = &rest[open + 2 + len..];
    }
    out.push_str(rest);
    bare(&out, base, moved)
}

// ---- private ----

/// `text` with the bare path of each `.include`, `.inc` or `.lib` card
/// that names a file of `moved`, resolved from `base`, replaced by its new
/// name (in double quotes when the name holds a blank, as `W::file_cards`
/// writes such a path). The path is the card's second word; a quoted one
/// is `relink`'s and names no file of `moved` here.
fn bare(text: &str, base: &Path, moved: &HashMap<PathBuf, String>) -> String {
    assert!(!moved.is_empty(), "relink returns early with nothing moved");
    let cards = [".include", ".inc", ".lib"];
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let mut words = line.split_whitespace();
        let first = words.next();
        let card = first.filter(|w| cards.iter().any(|c| w.eq_ignore_ascii_case(c)));
        let path = card.and(words.next());
        let name = path.and_then(|p| moved.get(&normalize(&base.join(p))));
        match (card, path, name) {
            (Some(card), Some(path), Some(name)) => {
                // The card is the line's first text, the path the first
                // match after it.
                let at = line.find(card).map_or(0, |i| i + card.len());
                let q = match name.contains(char::is_whitespace) {
                    true => "\"",
                    false => "",
                };
                out.push_str(&line[..at]);
                out.push_str(&line[at..].replacen(path, &format!("{q}{name}{q}"), 1));
            }
            _ => out.push_str(line),
        }
    }
    assert_eq!(
        out.lines().count(),
        text.lines().count(),
        "a relinked path holds no line break"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `pairs` as a `moved` table.
    fn table(pairs: &[(&str, &str)]) -> HashMap<PathBuf, String> {
        pairs
            .iter()
            .map(|(p, n)| (PathBuf::from(p), n.to_string()))
            .collect()
    }

    #[test]
    fn p1_058_a_bare_include_path_is_relinked() {
        // The ngspice, ESPice, Xyce and LTspice writers leave an include's
        // path bare (golden record fx17, spice-bench
        // ngspice/tests/regression/lib-processing/ex2a.sp: `.lib
        // /nix/store/.../ex2.lib MOS` in ngspice.txt, quoted in
        // hspice.txt). The run directory holds the written library under
        // `.cross_sim_inc/`, and the deck has to name it there, quoted or
        // not; a path beside the deck and a word of another card stay.
        // Oracle: each line's expected text written out by hand. Checks: 7.
        let base = Path::new("/corpus/deck");
        let moved = table(&[
            ("/corpus/assets/ex2.lib", ".cross_sim_inc/1-ex2.lib"),
            ("/corpus/deck/sub1/inc1", ".cross_sim_inc/2-inc1"),
            ("/corpus/mos.inc", ".cross_sim_inc/3-mos.inc"),
        ]);
        for (line, want) in [
            (
                ".lib /corpus/assets/ex2.lib MOS",
                ".lib .cross_sim_inc/1-ex2.lib MOS",
            ),
            (
                ".lib '/corpus/assets/ex2.lib' MOS",
                ".lib '.cross_sim_inc/1-ex2.lib' MOS",
            ),
            (".INCLUDE sub1/inc1", ".INCLUDE .cross_sim_inc/2-inc1"),
            ("  .inc ../mos.inc", "  .inc .cross_sim_inc/3-mos.inc"),
            (".include ./beside.inc", ".include ./beside.inc"),
            ("R1 a 0 sub1/inc1", "R1 a 0 sub1/inc1"),
            (".lib sub1/inc1x MOS", ".lib sub1/inc1x MOS"),
        ] {
            let text = format!("t\n{line}\nV1 a 0 1\n");
            let want = format!("t\n{want}\nV1 a 0 1\n");
            assert_eq!(relink(&text, base, &moved), want, "{line}");
        }
    }

    #[test]
    fn p1_062_every_file_named_by_an_absolute_path() {
        // `spice2rack --kit` names every file of the kit by the absolute
        // path of its copy: a file beside the deck too, a library's call
        // of its own section (gf180's sm141064.ngspice: `.lib
        // 'sm141064.ngspice' nmos_3p3_t`), a Verilog-A source by its own
        // path (`.hdl "…"`, `spice::W::loads`). An output directory with
        // a blank: the bare path is quoted, as `W::file_cards` quotes a
        // path with one (ngspice manual §2.6 reads `.include "…"`). A
        // `.lib` that opens a section names no file.
        // Oracle: each line's expected text written out by hand. Checks: 8.
        let base = Path::new("/u/tb");
        let moved = table(&[
            ("/u/tb/models.lib", "/o/kit/models.lib"),
            ("/u/tb/sub/inc1", "/o/kit/sub/inc1"),
            ("/pdk/mos.inc", "/o/kit/_root/pdk/mos.inc"),
            ("/u/tb/corners.lib", "/o/my kit/corners.lib"),
            ("/u/tb/va/res.va", "/u/tb/va/res.va"),
        ]);
        for (line, want) in [
            (".include models.lib", ".include /o/kit/models.lib"),
            (".include ./models.lib", ".include /o/kit/models.lib"),
            (".lib 'models.lib' tt", ".lib '/o/kit/models.lib' tt"),
            (".include sub/inc1", ".include /o/kit/sub/inc1"),
            (".include /pdk/mos.inc", ".include /o/kit/_root/pdk/mos.inc"),
            (".lib corners.lib tt", ".lib \"/o/my kit/corners.lib\" tt"),
            (".hdl \"va/res.va\"", ".hdl \"/u/tb/va/res.va\""),
            (".lib tt", ".lib tt"),
        ] {
            let text = format!("t\n{line}\nV1 a 0 1\n");
            let want = format!("t\n{want}\nV1 a 0 1\n");
            assert_eq!(relink(&text, base, &moved), want, "{line}");
        }
    }
}
