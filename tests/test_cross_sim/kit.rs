//! A corpus deck and the files it includes -> each written for one target,
//! as a user converts a deck and its model files (`spice2rack` on each):
//! the deck's `.lib "sm141064.ngspice" typical` then reaches the model file
//! in the target's dialect, under the name the deck gives it.
//!
//! Cites: none (the kit walk is `spicerack::import::kit`).
//!
//! Contents:
//! - `pub type Files = (String, Vec<(String, String)>, Vec<Code>)`
//! - `pub fn write(kit: &Kit, deck: &Path, target: Dialect) -> Result<Files, String>`
//! - `pub fn codes(messages: &str) -> String`

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::render_kit;
use spicerack::syntax::resolve::{FileId, Kit, Role};

/// The deck written, each included file written (by its name beside the
/// deck), and the deck's note codes (W0052: an output the target writes no
/// vector for).
pub type Files = (String, Vec<(String, String)>, Vec<Code>);

/// Every included library rendered once per target: a PDK model file
/// (gf180's sm141064.ngspice) is reached by a thousand decks.
static WRITTEN: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A library's path and target -> its text written, or the refusal's codes.
type Cache = HashMap<(PathBuf, &'static str), Result<String, String>>;

/// The run directory's folder for included files not beside the deck.
const INC: &str = ".cross_sim_inc";

/// The kit's deck (file 0) written for `target`, with each file it reaches
/// beside it: a file beside the deck under its own name, one elsewhere (an
/// absolute include into another tree, `models/x.lib`) under
/// `.cross_sim_inc/<k>-<name>`, every written text's spelling of its path
/// rewritten to that name. A refusal anywhere refuses the deck: its codes.
pub fn write(kit: &Kit, deck: &Path, target: Dialect) -> Result<Files, String> {
    let top = render_kit(kit, FileId(0), "t", target).map_err(|e| codes(&e.messages))?;
    let dir = deck.parent().unwrap_or(Path::new("."));
    let mut files = Vec::new();
    // Each moved file's path -> its name in the run directory.
    let mut moved: HashMap<PathBuf, String> = HashMap::new();
    for (i, f) in kit.files().iter().enumerate().skip(1) {
        if !matches!(f.role, Role::Deck | Role::Library) {
            continue;
        }
        let rel = f.path.strip_prefix(dir).ok();
        let name = match rel.filter(|r| r.components().count() == 1) {
            Some(r) => r.to_string_lossy().into_owned(),
            None => {
                let base = f.path.file_name().unwrap_or_default().to_string_lossy();
                let name = format!("{INC}/{i}-{base}");
                moved.insert(lexical(&f.path), name.clone());
                name
            }
        };
        let key = (f.path.clone(), target.name());
        let cached = WRITTEN.lock().unwrap().get(&key).cloned();
        let text = cached.unwrap_or_else(|| {
            let id = FileId(u32::try_from(i).expect("a kit holds fewer than 2^32 files"));
            let w = render_kit(kit, id, "t", target)
                .map(|w| w.text + "\n")
                .map_err(|e| codes(&e.messages));
            WRITTEN.lock().unwrap().insert(key, w.clone());
            w
        })?;
        let from = f.path.parent().unwrap_or(dir).to_path_buf();
        files.push((name, text, from));
    }
    let files = files
        .into_iter()
        .map(|(n, t, from)| (n, relink(&t, &from, &moved)))
        .collect();
    let notes = top.notes.diags.iter().map(|d| d.code).collect();
    Ok((relink(&top.text, dir, &moved), files, notes))
}

/// A refusal's codes, sorted and deduplicated, from its messages
/// (`<deck>:<line>:<col>: <code> <title>`).
pub fn codes(messages: &str) -> String {
    let mut c: Vec<&str> = messages
        .split_whitespace()
        .filter(|w| {
            w.len() == 5
                && w.starts_with(['W', 'P', 'R', 'E'])
                && w[1..].bytes().all(|b| b.is_ascii_digit())
        })
        .collect();
    c.sort_unstable();
    c.dedup();
    c.join(" ")
}

// ---- private ----

/// `text` (a file read from `base`) with each path that names a moved
/// file, resolved from `base`, replaced by its name in the run directory:
/// a quoted span anywhere (the HSPICE, VACASK and Spectre writers quote an
/// include's path) and an include card's bare path ([`bare`]: the ngspice,
/// ESPice, Xyce and LTspice writers do not).
fn relink(text: &str, base: &Path, moved: &HashMap<PathBuf, String>) -> String {
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
                .get(&lexical(&base.join(span)))
                .map_or(span, String::as_str),
        );
        out.push_str(q);
        rest = &rest[open + 2 + len..];
    }
    out.push_str(rest);
    bare(&out, base, moved)
}

/// `text` with the bare path of each `.include`, `.inc` or `.lib` card
/// that names a moved file, resolved from `base`, replaced by its name in
/// the run directory. The path is the card's second word; a quoted one is
/// [`relink`]'s and names no moved file here.
fn bare(text: &str, base: &Path, moved: &HashMap<PathBuf, String>) -> String {
    assert!(!moved.is_empty(), "relink returns early with nothing moved");
    let cards = [".include", ".inc", ".lib"];
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let mut words = line.split_whitespace();
        let first = words.next();
        let card = first.filter(|w| cards.iter().any(|c| w.eq_ignore_ascii_case(c)));
        let path = card.and(words.next());
        let name = path.and_then(|p| moved.get(&lexical(&base.join(p))));
        match (card, path, name) {
            (Some(card), Some(path), Some(name)) => {
                // The card is the line's first text, the path the first
                // match after it.
                let at = line.find(card).map_or(0, |i| i + card.len());
                out.push_str(&line[..at]);
                out.push_str(&line[at..].replacen(path, name, 1));
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

/// `p` with `.` and `..` resolved lexically, as the kit joins an include.
fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let moved: HashMap<PathBuf, String> = [
            ("/corpus/assets/ex2.lib", ".cross_sim_inc/1-ex2.lib"),
            ("/corpus/deck/sub1/inc1", ".cross_sim_inc/2-inc1"),
            ("/corpus/mos.inc", ".cross_sim_inc/3-mos.inc"),
        ]
        .into_iter()
        .map(|(p, n)| (PathBuf::from(p), n.to_string()))
        .collect();
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
}
