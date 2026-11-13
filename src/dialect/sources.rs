//! The caller-owned set of files a read works on: `(path, text)` pairs in add order, how many
//! of the first are the entries, and whether the entries are decks or libraries. A reader
//! never touches a file system; `Sources::load` is the convenience that walks a reader's
//! references through the caller's loader, breadth first, with today's kit caps
//! (`syntax::resolve::Kit::load`: depth 16, 4,096 files, each path once, relative to its
//! includer's directory; confining paths to a root is the loader's rule).
//!
//! Cites: `rewrite/SPINE.md` §2; `REWRITE-PLAN.md` §2.4 (case 2, its twin).
//!
//! Contents:
//! - `pub struct Sources`, `pub enum Reference`, `pub const DEPTH_MAX: u8`
//! - `Sources::new() -> Sources`, `Sources::one(path: &str, text: &str) -> Sources`,
//!   `Sources::add(&mut self, path: &str, text: &str) -> usize`,
//!   `Sources::has(&self, path: &str) -> bool`, `Sources::files(&self) -> &[(String, String)]`,
//!   `Sources::entries(&self) -> &[(String, String)]`, `Sources::set_form(&mut self, form: ir::Role)`,
//!   `Sources::form(&self) -> ir::Role`,
//!   `Sources::load(entries: &[&str], form: ir::Role, reader: &dyn Read, loader: &mut dyn FnMut(&str) -> Option<String>) -> Sources`
//! - `Reference::path(&self) -> &str`, `Reference::resolve(&self, from: &str) -> String`
//! - `pub fn relative_path(from: &str, to: &str) -> Option<String>`,
//!   `pub fn written_path(design: &ir::Design, from: ir::FileId, path: &str) -> Result<String, String>`

use std::collections::{HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};

use super::Read;
use crate::ir;

/// The files of one read, in add order. Invariant: `entries <= files.len()`, paths distinct,
/// `form` is `Deck` or `Library`.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    files: Vec<(String, String)>,
    entries: usize,
    form: Form,
}

/// A file a text names, as `Read::references` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reference {
    /// An include, a library call or `include … section=`: a netlist file, followed by
    /// [`Sources::load`].
    File(String),
    /// A Verilog-A source a load names; [`Sources::load`] does not follow it.
    VerilogA(String),
}

/// The deepest include chain [`Sources::load`] follows (an entry is depth 0).
pub const DEPTH_MAX: u8 = 16;

/// Whether the entries are decks or libraries (`ir::Role` has no `Default`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Form {
    #[default]
    Deck,
    Library,
}

impl Sources {
    /// No file; entries are decks.
    pub fn new() -> Sources {
        Sources::default()
    }

    /// One deck, the entry.
    pub fn one(path: &str, text: &str) -> Sources {
        let mut src = Sources::new();
        src.add(path, text);
        src
    }

    /// Appends a file and returns its position; the first file added is the entry.
    ///
    /// # Panics
    /// When `path` is already in, or past `ir::id::FILES_MAX` files.
    pub fn add(&mut self, path: &str, text: &str) -> usize {
        assert!(!self.has(path), "Sources::add: {path} added twice");
        assert!(
            self.files.len() < ir::id::FILES_MAX,
            "Sources::add: past FILES_MAX"
        );
        self.files.push((path.to_string(), text.to_string()));
        self.entries = self.entries.max(1);
        self.files.len() - 1
    }

    /// Whether `path` is in.
    pub fn has(&self, path: &str) -> bool {
        self.files.iter().any(|(p, _)| p == path)
    }

    /// Every file, entries first, in add order.
    pub fn files(&self) -> &[(String, String)] {
        &self.files
    }

    /// The entries: the first files added.
    pub fn entries(&self) -> &[(String, String)] {
        &self.files[..self.entries]
    }

    /// Sets what the entries are.
    ///
    /// # Panics
    /// When `form` is neither `Deck` nor `Library`.
    pub fn set_form(&mut self, form: ir::Role) {
        self.form = match form {
            ir::Role::Deck => Form::Deck,
            ir::Role::Library => Form::Library,
            ir::Role::VerilogA | ir::Role::Missing => panic!("Sources::set_form: deck or library"),
        };
    }

    /// `Role::Deck` or `Role::Library`: what the entries are.
    pub fn form(&self) -> ir::Role {
        match self.form {
            Form::Deck => ir::Role::Deck,
            Form::Library => ir::Role::Library,
        }
    }

    /// `entries` (in `form`) and every file their `Reference::File`s reach through `reader`,
    /// breadth first, each path once (normalized), to [`DEPTH_MAX`] and `FILES_MAX`. A path
    /// `loader` returns `None` for is left out: the reader sees a reference with no file.
    ///
    /// # Panics
    /// When `form` is neither `Deck` nor `Library`, or `entries` is empty.
    pub fn load(
        entries: &[&str],
        form: ir::Role,
        reader: &dyn Read,
        loader: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Sources {
        assert!(!entries.is_empty(), "Sources::load: at least one entry");
        let mut src = Sources::new();
        src.set_form(form);
        let mut seen: HashSet<String> = HashSet::new();
        let mut todo: VecDeque<(String, u8, bool)> =
            entries.iter().map(|e| (normalize(e), 0, true)).collect();
        // Bounded: each path is taken once, and at most FILES_MAX are added.
        while let Some((path, depth, entry)) = todo.pop_front() {
            if !seen.insert(path.clone()) || src.files.len() == ir::id::FILES_MAX {
                continue;
            }
            let Some(text) = loader(&path) else { continue };
            if depth < DEPTH_MAX {
                for r in reader.references(&text) {
                    if let Reference::File(_) = r {
                        todo.push_back((r.resolve(&path), depth + 1, false));
                    }
                }
            }
            src.files.push((path, text));
            if entry {
                src.entries = src.files.len();
            }
        }
        assert!(src.entries <= src.files.len(), "the entries lead the files");
        src
    }
}

impl Reference {
    /// The path as the text writes it.
    pub fn path(&self) -> &str {
        match self {
            Reference::File(p) | Reference::VerilogA(p) => p,
        }
    }

    /// The path, relative to the directory of `from` (the naming file's path) unless it is
    /// absolute, with `.` and `..` folded; a `..` above the root is kept (`../x`).
    pub fn resolve(&self, from: &str) -> String {
        let dir = Path::new(from).parent().unwrap_or(Path::new(""));
        normalize(&dir.join(self.path()).to_string_lossy())
    }
}

/// The text a writer spells for a reference from the file keyed `from` to the file keyed `to`
/// (R78): `to` relative to `from`'s directory, with `..` segments where `to` is not below it, so
/// the written files keep the source tree's layout. `None` when that cannot be stated: either
/// key absolute, or above the root (a key the loader kept a leading `..` in); the writer then
/// spells the path as it was read, with a note.
///
/// Why relative to the including file and nothing stronger (R78a): ngspice-45 resolves a
/// relative include against the working directory first, then against the including file's
/// directory (`settle/include_cwd/` on `step2-ngspice` 4f3de43f: `ngspice -b sub/deck.sp` reads
/// a file of that name in the working directory when one stands there). So this spelling works
/// from every working directory that holds no file of that name, and from none that does.
/// VACASK resolves in the other order (R78b): the including file's directory first, then the
/// working directory, a nested include from its own file's directory (`settle/b_include_cwd/` on
/// `step2-vacask-b` 883dc82b), so there the spelling holds from every working directory.
pub fn relative_path(from: &str, to: &str) -> Option<String> {
    let above = |k: &str| k.starts_with('/') || k == ".." || k.starts_with("../");
    if above(from) || above(to) {
        return None;
    }
    let dir: Vec<&str> = from.split('/').collect::<Vec<_>>().split_last()?.1.to_vec();
    let target: Vec<&str> = to.split('/').collect();
    let common = dir.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut out: Vec<&str> = vec![".."; dir.len() - common];
    out.extend_from_slice(&target[common..]);
    Some(out.join("/"))
}

/// The text a writer spells, in file `from` of `design`, for an include, library or load path
/// `path` as the IR holds it (R78): when `path` is a file of the design (a reader stores the key
/// of a file it resolved), that key relative to `from`'s directory ([`relative_path`]); a path
/// that names no file of the design is spelled as it was read. `Err(path)` when the file's key
/// cannot be stated relative to `from`: the writer spells `path` as read and notes it
/// ([`super::PATH_AS_READ`]).
///
/// # Errors
/// `Err` with the path as read, for a key above the root or an absolute one.
pub fn written_path(design: &ir::Design, from: ir::FileId, path: &str) -> Result<String, String> {
    let to = design.find_file(path);
    if to.is_none() {
        return Ok(path.to_string());
    }
    let from_key = design.str(design.file(from).path);
    let to_key = design.str(design.file(to).path);
    relative_path(from_key, to_key).ok_or_else(|| path.to_string())
}

// ---- private ----

/// `path` with `.` dropped and `..` folded into its parent. A `..` with no parent left to fold
/// (it climbs above the root) is kept, so the path names no file under the root, never a
/// different one (`a/deck.sp`'s `../lib.inc` from root `a/` is `../lib.inc`, not `lib.inc`).
fn normalize(path: &str) -> String {
    let mut out = PathBuf::new();
    for c in Path::new(path).components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(".."),
            },
            c => out.push(c),
        }
    }
    out.to_string_lossy().into_owned()
}
