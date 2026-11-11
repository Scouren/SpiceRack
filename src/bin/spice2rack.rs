//! SPICE deck -> every dialect, or one (`--to`), through the transpiler
//! (`syntax::render`).
//!
//!   cargo run --no-default-features --bin spice2rack -- -o out/ deck.sp [deck2.sp ...]
//!   spice2rack --dialect spectre --to espice --kit -o out/ tb.scs
//!
//! With no `--to`, each deck is written once per backend: `.ngspice.sp`,
//! `.hspice.sp`, `.ltspice.cir`, `.spectre.scs`, `.vacask.sim`,
//! `.espice.sp`. A backend that cannot express the circuit (VACASK has no B
//! sources, ngspice no `.step`) is reported with its codes and skipped; the
//! others are still written, each with its notes (what it kept or left out)
//! on stderr. `--dialect hspice` reads HSPICE decks (default ngspice).
//! `--check` requires each deck to be a fixpoint: written in its own
//! dialect and read back, it writes the same text.
//!
//! `--to <dialect>` writes that dialect alone (any of `TARGETS`, under its
//! suffix there) and prints the written deck's absolute path on stdout,
//! one line a deck and nothing else; the notes stay on stderr. A deck the
//! dialect refuses is not written, and the run exits 1.
//!
//! `--kit` (with `--to`) writes the deck and every file it reaches (its
//! `.include`s and `.lib`s, and theirs in turn), all or none: a file under
//! the deck's directory at its path from there, any other (a PDK's absolute
//! path, `../models/x.lib`) by its whole path under `_root/`. Every include
//! path in the written files is the absolute path of the written copy, so
//! the deck runs from any working directory and never reaches an
//! unconverted file of the same name. A Verilog-A source is not copied (its
//! own `include`s resolve where it is): the deck names it by its absolute
//! path. A file refused or not found, or a copy that would land on a file
//! the deck reads, exits 1 with nothing written.
//!
//! Exit code: 0; 1 when a deck failed (unreadable, not a fixpoint, with
//! `--to` refused); 2 on a usage error.
//!
//! A PDK model file converts like a deck: its library sections (`.lib tt`
//! ... `.endl tt`, Spectre and VACASK `section`) are written as sections in
//! every dialect that has them; LTspice has none and is refused.
//!
//! Cites: none (a CLI over `spicerack::import` and `syntax::render`).
//!
//! Contents: none (a binary; `main` is its entry).

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use spicerack::syntax::Dialect;
use spicerack::syntax::render::{relink, render_kit, render_text};
use spicerack::syntax::resolve::{FileId, Form, Role};

/// A written dialect and its file-name suffix.
type Target = (Dialect, &'static str);

/// Every written dialect. A run with no `--to` writes the first [`EVERY`],
/// the simulator backends.
static TARGETS: [Target; 9] = [
    (Dialect::Ngspice, "ngspice.sp"),
    (Dialect::Hspice, "hspice.sp"),
    (Dialect::Ltspice, "ltspice.cir"),
    (Dialect::Spectre, "spectre.scs"),
    (Dialect::Vacask, "vacask.sim"),
    (Dialect::Espice, "espice.sp"),
    (Dialect::Xyce, "xyce.cir"),
    (Dialect::Cdl, "cdl"),
    (Dialect::Verilog, "v"),
];

/// How many of [`TARGETS`] a run with no `--to` writes.
const EVERY: usize = 6;

/// The folder, under the output directory, of a kit's files that are not
/// under the deck's directory: each by its whole path.
const OUTSIDE: &str = "_root";

// ---- private ----

/// A file's stem: the name its deck is written under.
fn stem(p: &Path) -> &str {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("deck")
}

/// Prints `why` and the usage line, and exits 2.
fn usage(why: &str) -> ! {
    eprintln!("spice2rack: {why}");
    eprintln!(
        "usage: spice2rack [-o DIR] [--dialect ngspice|hspice|ltspice|xyce|spectre|vacask] [--to DIALECT [--kit]] [--check] DECK..."
    );
    std::process::exit(2)
}

/// Converts one deck under `out`: into `to` alone, its path printed and a
/// refusal an error; with no `to`, into every backend's dialect, a refusal
/// reported and skipped.
fn convert(
    deck: &Path,
    d: Dialect,
    to: Option<Target>,
    out: &Path,
    check: bool,
) -> std::io::Result<()> {
    let src = spicerack::import::decode(&std::fs::read(deck)?);
    let stem = stem(deck);
    let targets = match &to {
        Some(t) => std::slice::from_ref(t),
        None => &TARGETS[..EVERY],
    };
    for &(target, ext) in targets {
        match render_text(&src, d, stem, target) {
            Ok(w) => {
                eprint!("{}", w.notes.messages);
                let at = out.join(format!("{stem}.{ext}"));
                std::fs::write(&at, w.text + "\n")?;
                if to.is_some() {
                    println!("{}", at.display());
                }
            }
            Err(e) if to.is_some() => {
                let why = format!("{target:?} refused:\n{}", e.messages);
                return Err(std::io::Error::other(why));
            }
            Err(e) => eprint!("{}: {target:?} skipped:\n{}", deck.display(), e.messages),
        }
    }
    // Fixpoint: what SpiceRack writes in the deck's own dialect, it reads
    // back and writes as the same text.
    if check {
        let once = render_text(&src, d, stem, d).map_err(|e| std::io::Error::other(e.messages))?;
        let twice =
            render_text(&once.text, d, stem, d).map_err(|e| std::io::Error::other(e.messages))?;
        if once.text != twice.text {
            return Err(std::io::Error::other(format!(
                "not a fixpoint in {d:?}: re-reading the written deck changes it"
            )));
        }
    }
    Ok(())
}

/// Converts one deck and every file it reaches into `to` under `out`, all
/// or none, and prints the written deck's path. A file under the deck's
/// directory goes to its path from there, any other to its whole path under
/// [`OUTSIDE`]; each path in the written files that names one of them names
/// its copy by its absolute path (`render::relink`). A Verilog-A source is
/// not copied (its own `include`s resolve where it is) and is named by its
/// own absolute path.
///
/// # Errors
/// A file refused or not found, or a copy that would land on another copy
/// or on a file the deck reads: nothing is written. The file system's.
fn convert_kit(deck: &Path, d: Dialect, to: Target, out: &Path) -> std::io::Result<()> {
    assert!(out.is_absolute(), "main made the output directory absolute");
    let fail = |why: String| Err(std::io::Error::other(why));
    let (target, ext) = to;
    let deck = std::path::absolute(deck)?;
    // The whole file system is the root: a deck names its models wherever
    // they are (a PDK's absolute path), and the caller chose the deck.
    let root = deck.ancestors().last().unwrap_or(&deck);
    let dir = deck.parent().unwrap_or(root);
    let kit = spicerack::import::kit(std::slice::from_ref(&deck), Form::Deck, d, root);
    // The files the deck reads, by their real paths: none is written over.
    let sources: HashSet<PathBuf> = kit
        .files()
        .iter()
        .filter_map(|f| std::fs::canonicalize(&f.path).ok())
        .collect();
    // Each kit file's path -> the name the written files call it by.
    let mut moved: HashMap<PathBuf, String> = HashMap::new();
    // Each copy: where it goes, its text before `relink`, and the
    // directory its source was read from.
    let mut copies: Vec<(PathBuf, String, &Path)> = Vec::new();
    let mut refused = false;
    for (i, f) in kit.files().iter().enumerate() {
        let name = match f.role {
            Role::Deck | Role::Library => stem(&f.path),
            Role::VerilogA => {
                moved.insert(f.path.clone(), f.path.to_string_lossy().into_owned());
                continue;
            }
            Role::Unread(_) => return fail(format!("{}: not found", f.path.display())),
        };
        let at = match (i, f.path.strip_prefix(dir)) {
            (0, _) => out.join(format!("{name}.{ext}")),
            (_, Ok(rel)) => out.join(rel),
            (_, Err(_)) => {
                let whole = f.path.components();
                let whole = whole.filter(|c| matches!(c, Component::Normal(_)));
                out.join(OUTSIDE).join(whole.collect::<PathBuf>())
            }
        };
        // ponytail: a scan of the copies so far per file; a set if a kit
        // ever holds more than its 4096 files.
        let taken = copies.iter().any(|c| c.0 == at);
        if taken || std::fs::canonicalize(&at).is_ok_and(|c| sources.contains(&c)) {
            let why = "would be written over a file of the kit";
            return fail(format!("{}: {why}", at.display()));
        }
        moved.insert(f.path.clone(), at.to_string_lossy().into_owned());
        let id = FileId(u32::try_from(i).expect("a kit holds fewer than 2^32 files"));
        match render_kit(&kit, id, name, target) {
            Ok(w) => {
                eprint!("{}", w.notes.messages);
                copies.push((at, w.text + "\n", f.path.parent().unwrap_or(dir)));
            }
            Err(e) => {
                eprint!("{}: {target:?} refused:\n{}", f.path.display(), e.messages);
                refused = true;
            }
        }
    }
    if refused {
        return fail(format!(
            "nothing written: {target:?} refuses a file of the kit"
        ));
    }
    assert!(!copies.is_empty(), "the deck itself was written");
    for (at, text, base) in &copies {
        std::fs::create_dir_all(at.parent().unwrap_or(out))?;
        std::fs::write(at, relink(text, base, &moved))?;
    }
    println!("{}", copies[0].0.display());
    Ok(())
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |f: &str| match args.iter().position(|a| a == f) {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let (check, kit) = (flag("--check"), flag("--kit"));
    let mut value = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .filter(|i| i + 1 < args.len())
            .map(|i| {
                let v = args.remove(i + 1);
                args.remove(i);
                v
            })
    };
    let out = PathBuf::from(value("-o").unwrap_or_else(|| ".".into()));
    let dialect = match value("--dialect").map_or(Ok(Dialect::Ngspice), |v| v.parse()) {
        Ok(
            d @ (Dialect::Ngspice
            | Dialect::Hspice
            | Dialect::Ltspice
            | Dialect::Xyce
            | Dialect::Spectre
            | Dialect::Vacask),
        ) => d,
        _ => usage("--dialect: reads ngspice, hspice, ltspice, xyce, spectre or vacask"),
    };
    let to: Option<Target> = value("--to").map(|v| {
        let d = v.parse::<Dialect>().ok();
        let found = d.and_then(|d| TARGETS.iter().find(|t| t.0 == d));
        *found.unwrap_or_else(|| {
            usage(
                "--to: writes ngspice, hspice, ltspice, spectre, vacask, espice, xyce, cdl or verilog",
            )
        })
    });
    if let Some(a) = args.iter().find(|a| a.starts_with('-')) {
        usage(&format!("{a}: no such option, or an option with no value"));
    }
    if kit && (to.is_none() || check) {
        usage("--kit: with --to, and without --check");
    }
    if args.is_empty() {
        usage("no deck");
    }
    // Absolute: `--to` prints the path, and `--kit` writes it into every
    // include card, where a quote or a line break would end the path.
    let out = std::path::absolute(out).unwrap_or_else(|e| usage(&format!("-o: {e}")));
    if kit && out.to_string_lossy().contains(['"', '\'', '\n']) {
        usage("-o: with --kit, a path with no quote and no line break");
    }
    std::fs::create_dir_all(&out).expect("output directory");
    let mut failed = 0;
    for deck in &args {
        let done = match to {
            Some(to) if kit => convert_kit(Path::new(deck), dialect, to, &out),
            _ => convert(Path::new(deck), dialect, to, &out, check),
        };
        if let Err(e) = done {
            eprintln!("{deck}: {e}");
            failed += 1;
        }
    }
    if failed > 0 {
        eprintln!("{failed} deck(s) failed");
    }
    std::process::exit((failed > 0) as i32);
}
