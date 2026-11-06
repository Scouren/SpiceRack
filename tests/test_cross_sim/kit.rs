//! A corpus deck and the files it includes -> each written for one target,
//! as a user converts a deck and its model files (`spice2rack` on each):
//! the deck's `.lib "sm141064.ngspice" typical` then reaches the model file
//! in the target's dialect, under the name the deck gives it.
//!
//! Cites: none (the kit walk is `spicerack::import::kit`).
//!
//! Contents:
//! - `pub type Files = (String, Vec<(String, String)>)`
//! - `pub fn write(kit: &Kit, deck: &Path, target: Dialect) -> Result<Files, String>`
//! - `pub fn codes(messages: &str) -> String`

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use spicerack::syntax::Dialect;
use spicerack::syntax::render::render_kit;
use spicerack::syntax::resolve::{FileId, Kit, Role};

/// The deck written, and each included file written, by its name beside
/// the deck.
pub type Files = (String, Vec<(String, String)>);

/// Every included library rendered once per target: a PDK model file
/// (gf180's sm141064.ngspice) is reached by a thousand decks.
static WRITTEN: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A library's path and target -> its text written, or the refusal's codes.
type Cache = HashMap<(PathBuf, &'static str), Result<String, String>>;

/// The kit's deck (file 0) written for `target`, with each file it reaches
/// beside it. A refusal anywhere refuses the deck: its codes.
pub fn write(kit: &Kit, deck: &Path, target: Dialect) -> Result<Files, String> {
    let top = render_kit(kit, FileId(0), "t", target).map_err(|e| codes(&e.messages))?;
    let dir = deck.parent().unwrap_or(Path::new("."));
    let mut files = Vec::new();
    for (i, f) in kit.files().iter().enumerate().skip(1) {
        if !matches!(f.role, Role::Deck | Role::Library) {
            continue;
        }
        // ponytail: only a file beside the deck is written for the target; one
        // in another directory runs as the source wrote it (rare in the corpora).
        let Some(name) = f
            .path
            .strip_prefix(dir)
            .ok()
            .filter(|r| r.components().count() == 1)
        else {
            continue;
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
        files.push((name.to_string_lossy().into_owned(), text));
    }
    Ok((top.text, files))
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
