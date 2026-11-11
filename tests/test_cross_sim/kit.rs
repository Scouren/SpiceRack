//! A corpus deck and the files it includes -> each written for one target,
//! as a user converts a deck and its model files (`spice2rack --kit`): the
//! deck's `.lib "sm141064.ngspice" typical` then reaches the model file in
//! the target's dialect, under the name the deck gives it. The run
//! directory's names are this file's; the paths are rewritten to them by
//! the library (`render::relink`), as the CLI rewrites them to its own.
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
use spicerack::syntax::render::{relink, render_kit};
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
                moved.insert(f.path.clone(), name.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use spicerack::syntax::resolve::Form;
    use std::process::Command;

    /// The path an include card of a SPICE target names, its quotes off:
    /// `.include p`, `.inc p`, `.lib p section` (a `.lib name` alone opens
    /// a section).
    fn include_path(line: &str) -> Option<&str> {
        let w: Vec<&str> = line.split_whitespace().collect();
        let card = w.first()?.to_ascii_lowercase();
        let names = matches!(
            (card.as_str(), w.len()),
            (".include" | ".inc", 2..) | (".lib", 3..)
        );
        names.then(|| w[1].trim_matches(['"', '\'']))
    }

    /// `deck` (read as `source`) through `spice2rack --to <target> --kit`
    /// and through [`write`]: the same texts, the CLI's naming each file by
    /// the absolute path of its copy. `suffix` is the target's file-name
    /// suffix.
    fn cli_kit_is_the_harness_kit(deck: &Path, source: Dialect, target: Dialect, suffix: &str) {
        let ctx = format!("{} for {}", deck.display(), target.name());
        let root = Path::new("/");
        let kit = spicerack::import::kit(&[deck.to_path_buf()], Form::Deck, source, root);
        let (top, files, _) = write(&kit, deck, target).unwrap_or_else(|c| panic!("{ctx}: {c}"));
        let tmp = tempfile::TempDir::new().expect("a temp dir");
        let out = tmp.path();
        let run = Command::new(env!("CARGO_BIN_EXE_spice2rack"))
            .args(["--dialect", source.name(), "--to", target.name()])
            .args(["--kit", "-o"])
            .arg(out)
            .arg(deck)
            .output()
            .expect("spice2rack runs");
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert_eq!(run.status.code(), Some(0), "{ctx}: {stderr}");
        // The layout, restated: the entry by its stem and the target's
        // suffix; a file under the deck's directory at its path from
        // there, another by its whole path under `_root`.
        let dir = deck.parent().expect("a deck in a directory");
        let stem = deck.file_stem().expect("a stem").to_string_lossy();
        let entry = out.join(format!("{stem}.{suffix}"));
        let place = |p: &Path| match p.strip_prefix(dir) {
            Ok(rel) => out.join(rel),
            Err(_) => out
                .join("_root")
                .join(p.strip_prefix(root).expect("an absolute path")),
        };
        let stdout = String::from_utf8_lossy(&run.stdout);
        assert_eq!(stdout, format!("{}\n", entry.display()), "{ctx}: stdout");
        // (the CLI's path, the harness's name) of each library, in the
        // kit's order. No path here is the start of another.
        let libs = kit.files()[1..].iter();
        let libs = libs.filter(|f| matches!(f.role, Role::Deck | Role::Library));
        let libs: Vec<&PathBuf> = libs.map(|f| &f.path).collect();
        assert_eq!(libs.len(), files.len(), "{ctx}: a harness file a library");
        let names: Vec<(PathBuf, &str)> = libs
            .iter()
            .zip(&files)
            .map(|(p, (name, _))| (place(p), name.as_str()))
            .collect();
        let read = |at: &Path| {
            std::fs::read_to_string(at).unwrap_or_else(|e| panic!("{ctx}: {}: {e}", at.display()))
        };
        // A text's lines with the harness's names for the CLI's, less the
        // title (the name each caller gave the file).
        let body = |text: &str| -> Vec<String> {
            let named = names.iter().fold(text.to_string(), |t, (at, name)| {
                t.replace(&*at.to_string_lossy(), name)
            });
            named.lines().skip(1).map(str::to_string).collect()
        };
        assert_eq!(body(&read(&entry)), body(&top), "{ctx}: the deck");
        for ((at, name), (_, text)) in names.iter().zip(&files) {
            assert_eq!(body(&read(at)), body(text), "{ctx}: {name}");
        }
        // espice-c9's guard: every include path is absolute, lies under
        // the output directory and names a file written there.
        let mut paths = 0;
        for at in std::iter::once(&entry).chain(names.iter().map(|n| &n.0)) {
            for (line, path) in read(at).lines().filter_map(|l| Some((l, include_path(l)?))) {
                let p = Path::new(path);
                let under = p.is_absolute() && p.starts_with(out) && p.is_file();
                assert!(under, "{ctx}: {}: {line}", at.display());
                paths += 1;
            }
        }
        assert!(paths > 0, "{ctx}: a kit names a file");
    }

    #[test]
    fn p1_062_the_cli_writes_the_harness_kit_under_absolute_paths() {
        // Oracle: the cross-simulator harness's own files (`write`), which
        // the cells run on the target's simulator: a round trip, the CLI's
        // absolute paths mapped back to the run directory's names. Three
        // kits (ZERO-LEDGER P1-062): gf180's cmcqa deck (two libraries
        // beside it by their bare names, `sm141064.ngspice` calling its
        // own sections); a deck naming a library in another tree by its
        // absolute path, the library calling itself by a relative one; an
        // LTspice deck with a `.lib` file in a folder under it and an
        // `.include` beside its parent, written here.
        let bench = std::env::var("SPICE_BENCH")
            .expect("SPICE_BENCH not set: run the tests in `nix develop .#testing`");
        let bench = Path::new(&bench);
        let lt = tempfile::TempDir::new().expect("a temp dir");
        let top: &[&str] = &[
            "* an LTspice deck and its libraries",
            "V1 in 0 1",
            "X1 in out lowpass",
            "D1 out 0 clamp",
            ".lib sub/filters.lib",
            ".include ../shared/diodes.mod",
            ".op",
            ".end",
        ];
        let filters: &[&str] = &[
            "* an RC low-pass",
            ".subckt lowpass a b",
            "R1 a b 1k",
            "C1 b 0 1u",
            ".ends lowpass",
        ];
        let diodes: &[&str] = &["* a clamp diode", ".model clamp D(Is=1e-14 N=1)"];
        for (at, lines) in [
            ("tb/top.net", top),
            ("tb/sub/filters.lib", filters),
            ("shared/diodes.mod", diodes),
        ] {
            let at = lt.path().join(at);
            std::fs::create_dir_all(at.parent().expect("in a folder")).expect("a folder");
            std::fs::write(&at, lines.join("\n") + "\n").expect("a file written");
        }
        let (ng, lts) = (Dialect::Ngspice, Dialect::Ltspice);
        for (deck, source, target, suffix) in [
            (
                bench.join("cmcqa/gf180/gf180_cv_cgc_nmos_3p3_0.sp"),
                ng,
                Dialect::Espice,
                "espice.sp",
            ),
            (
                bench.join("cmcqa/gf180/gf180_cv_cgc_nmos_3p3_0.sp"),
                ng,
                Dialect::Hspice,
                "hspice.sp",
            ),
            (
                bench.join("ngspice/tests/regression/lib-processing/ex2a.sp"),
                ng,
                Dialect::Espice,
                "espice.sp",
            ),
            (
                bench.join("ngspice/tests/regression/lib-processing/ex2a.sp"),
                ng,
                Dialect::Hspice,
                "hspice.sp",
            ),
            (lt.path().join("tb/top.net"), lts, ng, "ngspice.sp"),
            (
                lt.path().join("tb/top.net"),
                lts,
                Dialect::Espice,
                "espice.sp",
            ),
        ] {
            cli_kit_is_the_harness_kit(&deck, source, target, suffix);
        }
    }
}
