//! The golden gate's inputs: every corpus deck, PDK kit file and test fixture,
//! each with the dialect it is read in and the form it is read as.
//!
//! Corpora come from the variables `nix develop .#testing` sets, walked and
//! filtered the way `tests/test_import_suite` walks them. A kit is its entry
//! files plus every file their includes and `.lib` calls reach, breadth first,
//! the way Interop's `pdk-models` walks one, and the read order goes into the
//! manifest. Each corpus deck, kit file and fixture is read in its
//! `syntax::resolve::Kit` (phase 3), a corpus deck with the files it includes.
//! A PDK kit is built here, once, and shared by its files; a corpus deck's or
//! a fixture's own kit is loaded by the worker that records it
//! ([`InputKit::Own`]), so the walk itself reads only the PDK kits.
//!
//! Cites: `tests/test_import_suite/main.rs` (`files`, `decks`, the corpus
//! filters); Interop `src/pdk_models.rs` (`references`, `convert`: the walk
//! and the empty title line).
//!
//! Contents:
//! - `pub(super) enum Form { Deck, Library }`
//! - `pub(super) enum InputKit { Shared(Arc<resolve::Kit>, FileId), Own(PathBuf) }`
//! - `pub(super) struct Input { pub(super) set: &'static str, pub(super) rel: String, pub(super) path: PathBuf, pub(super) dialect: Dialect, pub(super) form: Form, pub(super) kit: InputKit }`
//! - `pub(super) fn Input::load_kit(&self) -> (Arc<resolve::Kit>, FileId)`
//! - `pub(super) fn inputs(repo: &Path, manifest: &mut Vec<String>) -> Vec<Input>`
//! - `pub(super) fn source(text: &str, form: Form, path: &Path) -> (String, String)`

use std::collections::{BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use spicerack::import::{decode, parse};
use spicerack::syntax::Dialect;
use spicerack::syntax::render::{self, Reference};
use spicerack::syntax::resolve::{self, FileId};

/// How a file is read: a deck (line 1 is its title, the deck is named for
/// the file's stem) or a kit's library file (an empty title line is
/// prepended and the deck is named `pdk`, as Interop's `pdk-models` reads
/// it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Form {
    Deck,
    Library,
}

/// The kit an input is read in (`syntax::resolve`).
pub(super) enum InputKit {
    /// A PDK kit's, built once by the walk and shared by its files: the
    /// kit and the input's file in it.
    Shared(Arc<resolve::Kit>, FileId),
    /// Its own (it and the files it includes under this root, the input
    /// its `FileId(0)`), loaded by the worker that records it: a corpus
    /// deck or a fixture.
    Own(PathBuf),
}

/// One file to record.
pub(super) struct Input {
    /// The input set: a corpus, a kit or `fixtures`.
    pub(super) set: &'static str,
    /// The path under the set's root, `/`-separated.
    pub(super) rel: String,
    pub(super) path: PathBuf,
    pub(super) dialect: Dialect,
    pub(super) form: Form,
    pub(super) kit: InputKit,
}

/// A corpus: every `*.<ext>` under `$<var>` whose full path passes `keep`.
struct Corpus {
    set: &'static str,
    var: &'static str,
    dialect: Dialect,
    ext: &'static str,
    keep: fn(&str) -> bool,
    /// A deck's own dialect where it is not the set's, from its path under
    /// the root.
    own: fn(&str) -> Option<Dialect>,
    note: &'static str,
}

/// A kit: `entries` under `$<var>/<sub>`, and every file they reach.
struct Kit {
    set: &'static str,
    var: &'static str,
    sub: &'static str,
    dialect: Dialect,
    entries: fn(&Path) -> Vec<String>,
}

/// `tests/test_import_suite`'s corpora. `NGSPICE_CORPUS` is `SPICE_BENCH`
/// in `.#testing` (flake.nix), and the two walks keep the same decks, so
/// the ngspice corpus is recorded once, as spice-bench.
const CORPORA: [Corpus; 5] = [
    Corpus {
        set: "spice-bench",
        var: "SPICE_BENCH",
        dialect: Dialect::Ngspice,
        ext: "sp",
        keep: |p| !p.rsplit('/').next().unwrap_or(p).starts_with('.'),
        own: |_| None,
        note: "also the ngspice corpus (NGSPICE_CORPUS == SPICE_BENCH)",
    },
    Corpus {
        set: "hspice",
        var: "HSPICE_CORPUS",
        dialect: Dialect::Hspice,
        ext: "sp",
        keep: |p| p.contains("hspice") && !p.contains("invalid"),
        own: |_| None,
        note: "ESPice conformance tests/netlist",
    },
    Corpus {
        set: "xyce",
        var: "XYCE_CORPUS",
        dialect: Dialect::Xyce,
        ext: "cir",
        keep: |_| true,
        // `NOISE/*_ngspice.cir` are ngspice decks (their `.control`
        // blocks, ngspice's `noise` vectors) kept beside the Xyce ones.
        own: |p| p.ends_with("_ngspice.cir").then_some(Dialect::Ngspice),
        note: "Xyce_Regression Netlists",
    },
    Corpus {
        set: "vacask",
        var: "VACASK_CORPUS",
        dialect: Dialect::Vacask,
        ext: "sim",
        // `benchmark/*/gnucap/runme.sim`: gnucap's SPICE mode
        // (`vs ( 1 0 ) pulse ( iv=0 … )`), no SpiceRack reader's input.
        keep: |p| !p.contains("/gnucap/"),
        own: vacask_benchmark,
        note: "VACASK sources",
    },
    Corpus {
        set: "ltspice",
        var: "LTSPICE_CORPUS",
        dialect: Dialect::Ltspice,
        ext: "net",
        keep: |_| true,
        own: |_| None,
        note: "spice-datasets",
    },
];

/// The PDK kits. sky130 and gf180mcu are the `PDK_ROOT` `.#testing` pins
/// (`pdks` in `flake.nix`); IHP is a checkout named by `$IHP_PDK`, entered at its
/// corner libraries; VACASK's own library is its `demo/cmos180n.lib`.
const KITS: [Kit; 4] = [
    Kit {
        set: "pdk-sky130A",
        var: "PDK_ROOT",
        sub: "sky130A",
        dialect: Dialect::Ngspice,
        entries: |_| vec!["libs.tech/combined/sky130.lib.spice".into()],
    },
    Kit {
        set: "pdk-gf180mcuD",
        var: "PDK_ROOT",
        sub: "gf180mcuD",
        dialect: Dialect::Ngspice,
        entries: |_| vec!["libs.tech/ngspice/sm141064.ngspice".into()],
    },
    Kit {
        set: "pdk-ihp-sg13g2",
        var: "IHP_PDK",
        sub: "ihp-sg13g2",
        dialect: Dialect::Ngspice,
        entries: ihp_corners,
    },
    Kit {
        set: "pdk-vacask",
        var: "VACASK_CORPUS",
        sub: "",
        dialect: Dialect::Vacask,
        entries: |_| vec!["demo/cmos180n.lib".into()],
    },
];

/// Fixture extensions under `tests/` and the dialect each is read in.
const FIXTURES: [(&str, Dialect); 7] = [
    ("sp", Dialect::Ngspice),
    ("lib", Dialect::Ngspice),
    ("spice", Dialect::Ngspice),
    ("cir", Dialect::Xyce),
    ("scs", Dialect::Spectre),
    ("sim", Dialect::Vacask),
    ("net", Dialect::Ltspice),
];

impl Input {
    /// Its kit and its file in it: the PDK kit it shares, or its own,
    /// loaded by this call (the caller keeps it; a second call loads it
    /// again).
    pub(super) fn load_kit(&self) -> (Arc<resolve::Kit>, FileId) {
        match &self.kit {
            InputKit::Shared(kit, f) => (Arc::clone(kit), *f),
            InputKit::Own(root) => {
                let paths = std::slice::from_ref(&self.path);
                let kit = spicerack::import::kit(paths, resolve::Form::Deck, self.dialect, root);
                (Arc::new(kit), FileId(0))
            }
        }
    }
}

/// Every input, sorted by set and path. Appends the manifest's set, kit and
/// read-order lines to `manifest`; an absent set is a line, not an error.
pub(super) fn inputs(repo: &Path, manifest: &mut Vec<String>) -> Vec<Input> {
    let mut all = Vec::new();
    for c in &CORPORA {
        let Some(root) = root(c.set, c.var, "", manifest) else {
            continue;
        };
        let mut found = Vec::new();
        files(&root, &mut found);
        found.retain(|p| p.extension() == Some(c.ext.as_ref()) && (c.keep)(&p.to_string_lossy()));
        manifest.push(format!(
            "set {} {} {} inputs: ${}, {}",
            c.set,
            c.dialect.name(),
            found.len(),
            c.var,
            c.note
        ));
        all.extend(found.into_iter().map(|path| {
            let rel = rel(&root, &path);
            let dialect = (c.own)(&rel).unwrap_or(c.dialect);
            Input {
                set: c.set,
                dialect,
                rel,
                path,
                form: Form::Deck,
                // With the files it includes, so a master they declare is
                // bound (VACASK c6288 and its includes: only `elaborate`
                // stays).
                kit: InputKit::Own(root.clone()),
            }
        }));
    }
    for k in &KITS {
        if let Some(root) = root(k.set, k.var, k.sub, manifest) {
            all.extend(kit(k, &root, manifest));
        }
    }
    let mut found = Vec::new();
    files(&repo.join("tests"), &mut found);
    let before = all.len();
    all.extend(found.into_iter().filter_map(|path| {
        let ext = path.extension()?.to_str()?;
        let &(_, by_ext) = FIXTURES.iter().find(|f| f.0 == ext)?;
        let tests = repo.join("tests");
        // `tests/ltspice/` holds LTspice decks under `.cir` (stream F's
        // pending-run decks), which the extension would read as Xyce.
        let dialect = match path.starts_with(tests.join("ltspice")) {
            true => Dialect::Ltspice,
            false => by_ext,
        };
        Some(Input {
            set: "fixtures",
            rel: rel(repo, &path),
            path,
            dialect,
            form: Form::Deck,
            kit: InputKit::Own(tests),
        })
    }));
    manifest.push(format!(
        "set fixtures {} inputs: tests/**/*.{{sp,lib,spice,cir,scs,sim,net}}, dialect by extension",
        all.len() - before
    ));
    all.sort_by(|a, b| (a.set, &a.rel).cmp(&(b.set, &b.rel)));
    all
}

/// The text the reader reads for a file and the name its deck is written
/// under.
pub(super) fn source(text: &str, form: Form, path: &Path) -> (String, String) {
    match form {
        Form::Deck => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("deck");
            (text.to_string(), stem.to_string())
        }
        Form::Library => (format!("\n{text}"), "pdk".to_string()),
    }
}

// ---- private ----

/// `$<var>/<sub>` if it is set and exists; else an `absent` manifest line.
fn root(set: &str, var: &str, sub: &str, manifest: &mut Vec<String>) -> Option<PathBuf> {
    let Ok(base) = std::env::var(var) else {
        manifest.push(format!("absent {set}: ${var} not set"));
        return None;
    };
    let root = Path::new(&base).join(sub);
    if !root.is_dir() {
        manifest.push(format!("absent {set}: ${var}/{sub} is not a directory"));
        return None;
    }
    Some(root)
}

/// Every file under `dir`, skipping `src*` directories (fetched sources and
/// their assets in spice-bench, as `test_import_suite` skips them).
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if !e.file_name().to_string_lossy().starts_with("src") {
                files(&p, out);
            }
        } else {
            out.push(p);
        }
    }
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// VACASK's `benchmark/<circuit>/<simulator>/` decks are each simulator's
/// own input, under VACASK's `.sim` name: `ngspice*` (ngspice, its `-sparse`,
/// `cg` and `va` variants) and `xyce` directories hold ngspice and Xyce
/// decks. `gnucap`'s are gnucap's SPICE mode, which no reader reads: the
/// corpus's `keep` leaves them out.
fn vacask_benchmark(rel: &str) -> Option<Dialect> {
    let mut parts = rel.split('/');
    if parts.next() != Some("benchmark") {
        return None;
    }
    let sim = rel.rsplit('/').nth(1)?;
    match sim {
        "xyce" => Some(Dialect::Xyce),
        s if s.starts_with("ngspice") => Some(Dialect::Ngspice),
        _ => None,
    }
}

/// IHP's corner libraries, the files a deck `.lib`s (`cornerMOSlv.lib mos_tt`).
fn ihp_corners(root: &Path) -> Vec<String> {
    let models = root.join("libs.tech/ngspice/models");
    let mut found = Vec::new();
    files(&models, &mut found);
    let mut v: Vec<String> = found
        .iter()
        .map(|p| rel(root, p))
        .filter(|r| {
            let name = r.rsplit('/').next().unwrap_or(r);
            name.starts_with("corner") && name.ends_with(".lib")
        })
        .collect();
    v.sort();
    v
}

/// The files `k`'s entries reach, in read order: entries first, then breadth
/// first through each file's includes, `.lib` calls and Verilog-A sources.
/// Verilog-A sources, missing files and paths outside the root are manifest
/// lines, not inputs.
fn kit(k: &Kit, root: &Path, manifest: &mut Vec<String>) -> Vec<Input> {
    let entries = (k.entries)(root);
    for e in &entries {
        manifest.push(format!("kit {} entry {e}", k.set));
    }
    let mut todo: VecDeque<(PathBuf, bool)> = entries
        .iter()
        .map(|e| (normalize(&root.join(e)), false))
        .collect();
    // The files read, each with its path under the root.
    let (mut seen, mut read) = (BTreeSet::new(), Vec::new());
    while let Some((p, verilog_a)) = todo.pop_front() {
        if !seen.insert(p.clone()) {
            continue;
        }
        let shown = rel(root, &p);
        let n = seen.len();
        if !p.starts_with(root) {
            manifest.push(format!("outside {} {shown}", k.set));
            continue;
        }
        if !p.is_file() {
            manifest.push(format!("missing {} {shown}", k.set));
            continue;
        }
        if verilog_a {
            manifest.push(format!(
                "order {} {n:04} {shown} verilog-a, not read",
                k.set
            ));
            continue;
        }
        manifest.push(format!("order {} {n:04} {shown}", k.set));
        let text = std::fs::read(&p).map(|b| decode(&b)).unwrap_or_default();
        let (src, name) = source(&text, Form::Library, &p);
        let dir = p.parent().unwrap_or(Path::new("")).to_path_buf();
        // A file the reader panics on names nothing further; the panic is
        // recorded.
        if let Ok(refs) = std::panic::catch_unwind(|| references(&src, &name, k.dialect)) {
            for (f, va) in refs {
                todo.push_back((normalize(&dir.join(f)), va));
            }
        }
        read.push((shown, p));
    }
    // The kit `syntax::resolve` reads from the same entries: every file
    // above is one of its files (it also follows includes this walk skips,
    // inside a subckt or an `.if` branch; those are no inputs).
    let entries: Vec<PathBuf> = entries.iter().map(|e| root.join(e)).collect();
    let kit = Arc::new(spicerack::import::kit(
        &entries,
        resolve::Form::Library,
        k.dialect,
        root,
    ));
    let out: Vec<Input> = read
        .into_iter()
        .map(|(shown, path)| {
            let f = kit.files().iter().position(|f| f.path == path);
            let f = f.expect("the kit reads every file the walk reads");
            Input {
                set: k.set,
                rel: shown,
                path,
                dialect: k.dialect,
                form: Form::Library,
                kit: InputKit::Shared(Arc::clone(&kit), FileId(f as u32)),
            }
        })
        .collect();
    assert!(out.len() <= seen.len(), "an input a file the walk saw");
    out
}

/// The files `src` names (`render::references`): netlist files (`false`)
/// and Verilog-A sources (`true`), as Interop's `pdk-models` walks them.
fn references(src: &str, name: &str, d: Dialect) -> Vec<(String, bool)> {
    let Ok((tokens, ast)) = parse(src, d) else {
        return Vec::new();
    };
    let file = render::Source {
        ast: &ast,
        tokens: &tokens,
        lang: d.into(),
        name,
        kit: None,
        loads: &[],
        numbers: render::Numbers::Read,
    };
    render::references(&file)
        .into_iter()
        .map(|r| match r {
            Reference::File(f) => (f, false),
            Reference::VerilogA(f) => (f, true),
        })
        .collect()
}

/// `p` with `.` and `..` resolved lexically.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}
