//! A cell's two kinds of text file: a deck's `.pair` file (`key = value` a line, `#` comments:
//! `owner`, `proves`, `vectors`, and the tolerances, which may only tighten) and the cell's list
//! of failing steps (`<deck> <step 1..6> <class> <proof>`).
//!
//! Cites: `rewrite/SPINE.md` §8a (both formats).
//!
//! Contents:
//! - `pub struct PairFile`
//! - `pub fn pair_file(deck: &Path) -> Result<PairFile, String>`,
//!   `pub fn pair_text(text: &str) -> Result<PairFile, String>`
//! - `pub fn listed(cell: &str) -> BTreeMap<(String, u8), String>`
//! - `pub fn relative(base: &Path, p: &str) -> Option<String>`

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::cmp;

/// A deck's `.pair` file, read.
#[derive(Clone, Default)]
pub struct PairFile {
    /// Each tightening, in file order: (key, value).
    pub tightened: Vec<(String, f64)>,
    /// The vectors to compare, by normal name; `None`: every vector.
    pub vectors: Option<BTreeSet<String>>,
}

/// The keys a `.pair` file may hold besides the tolerances.
const PAIR_WORDS: [&str; 3] = ["owner", "proves", "vectors"];

/// The `.pair` file beside `deck`, read; the default when there is none.
pub fn pair_file(deck: &Path) -> Result<PairFile, String> {
    match std::fs::read_to_string(deck.with_extension("pair")) {
        Ok(text) => pair_text(&text),
        Err(_) => Ok(PairFile::default()),
    }
}

/// A `.pair` file's text: `key = value` lines, `#` to the end a comment. `Err` for a line
/// with no `=`, an unknown key, a value that is no number, or a tolerance that loosens.
pub fn pair_text(text: &str) -> Result<PairFile, String> {
    let mut out = PairFile::default();
    let mut tol = cmp::Tol::of(&[]);
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("`{line}`: not `key = value`"))?;
        let (k, v) = (k.trim(), v.trim());
        if k == "vectors" {
            out.vectors = Some(v.split_whitespace().map(cmp::normal).collect());
        } else if !PAIR_WORDS.contains(&k) {
            let x = spicerack::unit::parse_spice_number(v)
                .ok_or_else(|| format!("{k} = {v}: not a number"))?
                .0;
            tol.tighten(k, x)?;
            out.tightened.push((k.to_string(), x));
        }
    }
    Ok(out)
}

/// The cell's list: (deck, step) → class. Panics on a line that is not `<deck> <step 1..6>
/// <class> <proof>` with a `key=value` proof.
pub fn listed(cell: &str) -> BTreeMap<(String, u8), String> {
    let path = format!(
        "{}/tests/fixtures/pair/{cell}.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let w: Vec<&str> = line.split_whitespace().collect();
        let step = w.get(1).and_then(|s| s.parse::<u8>().ok());
        let ok = w.len() == 4 && step.is_some_and(|s| (1..=6).contains(&s)) && w[3].contains('=');
        assert!(
            ok,
            "{path}: `{line}` is not `<deck> <step 1..6> <class> <proof>`"
        );
        let key = (w[0].to_string(), step.expect("checked"));
        assert!(
            out.insert(key, w[2].to_string()).is_none(),
            "{path}: `{line}` twice"
        );
    }
    out
}

/// `p` (a written or source file, the same root as `base`) named from the folder `base`: the
/// file name when beside it, `../…` when above it; `None` for a file in a subfolder.
pub fn relative(base: &std::path::Path, p: &str) -> Option<String> {
    let b: Vec<_> = base.components().collect();
    let q: Vec<_> = std::path::Path::new(p).components().collect();
    let common = b.iter().zip(&q).take_while(|(x, y)| x == y).count();
    let ups = b.len() - common;
    let rest: Vec<String> = q[common..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if rest.is_empty() || (ups == 0 && rest.len() > 1) {
        return None;
    }
    Some("../".repeat(ups) + &rest.join("/"))
}
