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
    Ok((relink(&top.text, dir, &moved), files))
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

/// `text` (a file read from `base`) with each quoted path that names a
/// moved file, resolved from `base`, replaced by its name in the run
/// directory. Each include is written quoted by every writer here.
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
