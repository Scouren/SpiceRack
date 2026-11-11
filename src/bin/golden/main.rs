//! Every input file -> every writer -> a tree of golden files: what each
//! phase of the transpiler rewrite must reproduce byte for byte against the
//! recorded `9d1d6bb` baseline (TESTING.md, "Golden gate").
//!
//!   golden record <out_dir>
//!   golden diff <a> <b> [--xfail migration.xfail] [--refusal-status-only <report>] [--skip <target>]
//!   golden check-determinism [<tree>]
//!   golden zero <record> [<list>]
//!
//! `record` reads every input `inputs.rs` finds (run it in `nix develop
//! .#testing`, which sets the corpus and PDK variables; `$IHP_PDK` names an
//! IHP-Open-PDK checkout) and writes, under `<out_dir>/<set>/<relpath>/`,
//! for every target dialect, `<target>.txt`, the exact text
//! `syntax::render` writes from the file's AST and kit, or
//! `<target>.refused`: its codes, sorted, on a first line (`codes: P0001
//! W0013`), then its messages (`diag::messages`: `<deck>:<line>:<column>:
//! <code> <title>`), which `diff` does not compare; beside a `.txt`, its
//! notes as `<target>.notes` in the same form when it has any (not
//! compared: the census reads them); `parse.refused` instead
//! when no lexicon reads the file's dialect; `<name>.panicked`, the panic
//! message, when a call panics.
//!
//! The reader is the transpiler's front end: every input is read in its
//! `syntax::resolve::Kit` (a corpus deck with the files it includes, a PDK
//! kit file or a fixture with its kit). A
//! record of `9d1d6bb` (the PEG reader and the old writers) is the oracle a
//! new record is diffed against; its `ir.json` and `report.json` went with
//! the IR (phase 6), so `diff` passes over them.
//!
//! `record` walks the inputs largest file first with `GOLDEN_JOBS` workers
//! (4 by default), an input of 1 MB or more read once and rendered as one
//! task per target; each corpus deck's and fixture's kit is loaded by its
//! worker. `MANIFEST` lists the sets, each kit's read order, each input's
//! content hash and each output file's, its lines sorted, so neither the
//! order nor the job count changes a byte of the tree. `diff` prints
//! `IDENTICAL` or every differing file and exits 1, with strict xfail
//! (`diff.rs`). `check-determinism <tree>` renders every input again in
//! its own process, with its own `GOLDEN_JOBS` and the queue reversed, and
//! compares each output's bytes, then the whole MANIFEST, with `<tree>`'s,
//! writing nothing: it prints `IDENTICAL`, or the first difference it
//! finds and exits 1. With no `<tree>` it records into a temp dir in a
//! child process first and verifies against that. `zero` prints a
//! record's Z1 names (its outputs that carry `W0001` or `W0032`), or
//! compares them with the checked-in list and exits 1 on any difference
//! (`zero.rs`).
//!
//! Cites: the compiler-construction skill, rule 9; VerA `tools/vera.xfail`
//! (strict xfail); `research/BUILD-TEST-SPEED.md` §3.1, §3.2 (the task
//! queue, the split, the bound, determinism against the record's tree).
//!
//! Contents: none (a binary; `main` is its entry).

use spicerack::syntax::Lang;

use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use spicerack::import::decode;
use spicerack::syntax::Dialect;
use spicerack::syntax::render::{self, Diags};
use spicerack::syntax::resolve::{self, FileId};

mod diff;
mod inputs;
mod zero;

use inputs::{Form, Input, inputs, source};

const USAGE: &str = "usage: golden record <out_dir> | \
                     golden diff <a> <b> [--xfail <file>] [--refusal-status-only <report>] [--skip <target>] | \
                     golden check-determinism [<tree>] | \
                     golden zero <record> [<list>]";

/// Input dialects no set reads, and why: the manifest's `skip` lines.
/// Every target is written for every input that parses; none is skipped.
// The text is recorded in every MANIFEST, which `diff` compares, so it stays
// byte for byte as the `9d1d6bb` baseline holds it (`import::lang` was the
// PEG reader's; `import::parse` now refuses the two dialects the same way).
const SKIPPED: [&str; 3] = [
    "skip input espice: reads ngspice text (Dialect::family), so an ESPice input is an ngspice input",
    "skip input cdl: no reader (import::lang refuses it)",
    "skip input verilog: no reader (import::lang refuses it)",
];

/// An input of this many bytes or more is read once and rendered as one
/// task per target: `ibmpg3.sp` (96.5 MB) held one worker for its nine
/// targets, 85 s, while the others sat idle (BUILD-TEST-SPEED.md F1).
const SPLIT_BYTES: u64 = 1 << 20;

/// An input of this many bytes or more renders at most `GIANT_TARGETS` of
/// its targets at once, which bounds the memory: 9.4 GB peak unbounded,
/// 6.0 GB at three (BUILD-TEST-SPEED.md §3.1, runs `r2`, `r5`).
const GIANT_BYTES: u64 = 32 << 20;
const GIANT_TARGETS: usize = 3;

const _: () = assert!(SPLIT_BYTES <= GIANT_BYTES, "a giant input is split");
const _: () = assert!(GIANT_TARGETS > 0, "a giant input renders");

/// Where a render's output files go.
#[derive(Clone, Copy)]
enum Sink<'a> {
    /// Written under this tree.
    Write(&'a Path),
    /// Compared with this tree's files; nothing is written.
    Verify(&'a Path),
}

/// Why a render stopped before its last task.
enum Stop {
    /// An input or an output could not be read or written.
    Io(std::io::Error),
    /// A verified output is not the tree's: what differs ([`differs`]).
    Differs(String),
}

/// One of the `GIANT_TARGETS` places a giant input's targets render in,
/// given back when dropped (on an unwind too, so no worker waits forever).
struct Place<'a>(&'a (Mutex<usize>, Condvar));

/// One task of the queue: an input's index, and the one target it renders
/// (an input of `SPLIT_BYTES` or more) or `None` for all of them.
type Task = (usize, Option<usize>);

/// An input read once: what its target tasks share.
struct Read {
    /// `<set>/<rel>`: its directory under the tree.
    key: String,
    /// Its `input` manifest line.
    head: String,
    /// The name its deck is written under.
    name: String,
    /// Its kit and its file in it; `None` when no lexicon reads its
    /// dialect (`parse.refused`).
    kit: Option<(Arc<resolve::Kit>, FileId)>,
}

impl From<std::io::Error> for Stop {
    fn from(e: std::io::Error) -> Self {
        Stop::Io(e)
    }
}

impl Drop for Place<'_> {
    fn drop(&mut self) {
        let (free, given) = self.0;
        let mut free = free
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(*free < GIANT_TARGETS, "a place taken before");
        *free += 1;
        given.notify_one();
    }
}

// ---- private ----

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut rest: Vec<&str> = args.iter().map(String::as_str).collect();
    let xfail = match rest.iter().position(|a| *a == "--xfail") {
        Some(i) if i + 1 < rest.len() => {
            let f = PathBuf::from(rest[i + 1]);
            rest.drain(i..i + 2);
            Some(f)
        }
        Some(_) => return usage(),
        None => None,
    };
    let Ok(report) = flag(&mut rest, "--refusal-status-only") else {
        return usage();
    };
    let Ok(skip) = flag(&mut rest, "--skip") else {
        return usage();
    };
    let run = match rest.as_slice() {
        ["record", out] if xfail.is_none() && report.is_none() && skip.is_none() => {
            record(Path::new(out)).map(|()| true)
        }
        ["diff", a, b] => diff::diff(
            Path::new(a),
            Path::new(b),
            xfail.as_deref(),
            report.as_deref().map(Path::new),
            skip.as_deref(),
        ),
        ["check-determinism"] if xfail.is_none() && report.is_none() && skip.is_none() => {
            check_determinism(None)
        }
        ["check-determinism", tree] if xfail.is_none() && report.is_none() && skip.is_none() => {
            check_determinism(Some(Path::new(tree)))
        }
        ["zero", tree] if xfail.is_none() && report.is_none() && skip.is_none() => {
            zero::zero(Path::new(tree), None)
        }
        ["zero", tree, list] if xfail.is_none() && report.is_none() && skip.is_none() => {
            zero::zero(Path::new(tree), Some(Path::new(list)))
        }
        _ => return usage(),
    };
    match run {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("golden: {e}");
            ExitCode::from(2)
        }
    }
}

/// Removes `name <value>` from `rest`; `Err` when the value is missing.
fn flag(rest: &mut Vec<&str>, name: &str) -> Result<Option<String>, ()> {
    match rest.iter().position(|a| *a == name) {
        Some(i) if i + 1 < rest.len() => {
            let v = rest[i + 1].to_string();
            rest.drain(i..i + 2);
            Ok(Some(v))
        }
        Some(_) => Err(()),
        None => Ok(None),
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// Records every input into `out`, which must be absent or empty, so no
/// stale file from an earlier record survives into the tree.
fn record(out: &Path) -> std::io::Result<()> {
    if std::fs::read_dir(out).is_ok_and(|mut d| d.next().is_some()) {
        return Err(std::io::Error::other(format!(
            "{} is not empty",
            out.display()
        )));
    }
    match render_all(Sink::Write(out)) {
        Ok(manifest) => std::fs::write(out.join("MANIFEST"), manifest),
        Err(Stop::Io(e)) => Err(e),
        Err(Stop::Differs(why)) => unreachable!("a record compares nothing: {why}"),
    }
}

/// Renders every input again, the queue reversed, and compares each
/// output's bytes, then the whole MANIFEST (so a missing or an extra file
/// fails too), with `tree`'s; writes nothing. Prints `IDENTICAL` and
/// returns `Ok(true)`, or prints the first difference found and returns
/// `Ok(false)`; an error when `tree` holds no `MANIFEST`.
fn verify(tree: &Path) -> std::io::Result<bool> {
    if !tree.join("MANIFEST").is_file() {
        let why = format!("{} holds no MANIFEST: not a record", tree.display());
        return Err(std::io::Error::other(why));
    }
    let found = render_all(Sink::Verify(tree)).and_then(|manifest| {
        match differs(tree, "MANIFEST", manifest.as_bytes()) {
            None => Ok(()),
            Some(why) => Err(Stop::Differs(why)),
        }
    });
    match found {
        Ok(()) => {
            println!("IDENTICAL");
            Ok(true)
        }
        Err(Stop::Differs(why)) => {
            println!("DIFFERS {why}");
            Ok(false)
        }
        Err(Stop::Io(e)) => Err(e),
    }
}

/// Renders every input into `sink` with `GOLDEN_JOBS` workers (4 by
/// default), largest input first; a verify walks the queue reversed, in
/// another order than the record it checks. Returns the MANIFEST text: its
/// lines are sorted, so the order and the job count change no byte of it.
/// Stops at the first error or difference.
fn render_all(sink: Sink<'_>) -> Result<String, Stop> {
    let mut manifest: Vec<String> = SKIPPED.iter().map(|s| s.to_string()).collect();
    let all = inputs(Path::new(env!("CARGO_MANIFEST_DIR")), &mut manifest);
    eprintln!("golden: {} inputs", all.len());
    let jobs: usize = std::env::var("GOLDEN_JOBS")
        .ok()
        .and_then(|j| j.parse().ok())
        .unwrap_or(4);
    let sizes: Vec<u64> = all
        .iter()
        .map(|i| std::fs::metadata(&i.path).map_or(0, |m| m.len()))
        .collect();
    let mut queue = tasks(&sizes);
    if let Sink::Verify(_) = sink {
        queue.reverse();
    }
    // A split input's read, made by the first of its tasks to run.
    let reads: Vec<OnceLock<Result<Read, String>>> = all.iter().map(|_| OnceLock::new()).collect();
    // The places a giant input's targets render in, and their signal.
    let places = (Mutex::new(GIANT_TARGETS), Condvar::new());
    let next = AtomicUsize::new(0);
    let lines = Mutex::new(manifest);
    let stopped: Mutex<Option<Stop>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..jobs {
            let spawned = std::thread::Builder::new().spawn_scoped(s, || {
                while let Some(&(i, target)) = queue.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let input = &all[i];
                    let giant = target.is_some() && sizes[i] >= GIANT_BYTES;
                    let run = || -> Result<Vec<String>, Stop> {
                        let own;
                        let read = match target {
                            Some(_) => reads[i]
                                .get_or_init(|| read_input(input).map_err(|e| e.to_string()))
                                .as_ref()
                                .map_err(|e| std::io::Error::other(e.clone()))?,
                            None => {
                                own = read_input(input)?;
                                &own
                            }
                        };
                        let _place = giant.then(|| Place::take(&places));
                        render_input(sink, input, read, target)
                    };
                    match run() {
                        Ok(v) => lines.lock().expect("no worker panics").extend(v),
                        Err(stop) => {
                            let stop = match stop {
                                Stop::Io(e) => Stop::Io(std::io::Error::other(format!(
                                    "{}/{}: {e}",
                                    input.set, input.rel
                                ))),
                                differs => differs,
                            };
                            let mut first = stopped.lock().expect("no worker panics");
                            if first.is_none() {
                                *first = Some(stop);
                            }
                            next.store(queue.len(), Ordering::Relaxed);
                        }
                    }
                }
            });
            spawned.expect("spawn a worker");
        }
    });
    if let Some(stop) = stopped.into_inner().expect("no worker panics") {
        return Err(stop);
    }
    let mut lines = lines.into_inner().expect("no worker panics");
    lines.sort();
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

/// The task queue over inputs of `sizes` bytes: the largest first (ties in
/// `inputs`' order), an input of `SPLIT_BYTES` or more as one task per
/// target.
fn tasks(sizes: &[u64]) -> Vec<Task> {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(sizes[i]));
    let mut queue = Vec::with_capacity(sizes.len());
    for i in order {
        match sizes[i] >= SPLIT_BYTES {
            true => queue.extend((0..Dialect::ALL.len()).map(|t| (i, Some(t)))),
            false => queue.push((i, None)),
        }
    }
    assert!(queue.len() >= sizes.len(), "a task an input at least");
    queue
}

/// Reads `input` once: its bytes (hashed into its `input` line) and its
/// kit, loaded here for a corpus deck or a fixture
/// ([`inputs::InputKit::Own`]).
fn read_input(input: &Input) -> std::io::Result<Read> {
    let key = format!("{}/{}", input.set, input.rel);
    let bytes = std::fs::read(&input.path)?;
    let form = match input.form {
        Form::Deck => "deck",
        Form::Library => "library",
    };
    let head = format!(
        "input {key} {} {form} {}",
        input.dialect.name(),
        hash(&bytes)
    );
    let (src, name) = source(&decode(&bytes), input.form, &input.path);
    let readable = Lang::from(input.dialect).lexicon().is_some();
    let kit = readable.then(|| input.load_kit());
    if let Some((kit, f)) = &kit {
        assert_eq!(kit.file(*f).text, src, "the kit read this text");
    }
    Ok(Read {
        key,
        head,
        name,
        kit,
    })
}

/// Renders `read`'s targets (all, or `target` alone) into `sink`; returns
/// their manifest lines, with the `input` line from the task that renders
/// the first target.
fn render_input(
    sink: Sink<'_>,
    input: &Input,
    read: &Read,
    target: Option<usize>,
) -> Result<Vec<String>, Stop> {
    let first = target.is_none_or(|t| t == 0);
    let mut lines = Vec::new();
    if let Sink::Write(out) = sink {
        std::fs::create_dir_all(out.join(&read.key))?;
    }
    if first {
        lines.push(read.head.clone());
    }
    let Some((kit, f)) = &read.kit else {
        if first {
            let why = format!("no {:?} reader yet\n", input.dialect);
            lines.push(put(sink, &read.key, "parse.refused", &why)?);
        }
        return Ok(lines);
    };
    let all = Dialect::ALL;
    let targets = match target {
        Some(t) => &all[t..=t],
        None => &all[..],
    };
    for &d in targets {
        let t = d.name();
        let render = || render::render_kit(kit, *f, &read.name, d);
        let written = catch_unwind(AssertUnwindSafe(render))
            .map(|r| r.map(|w| (w.text, w.notes)).map_err(|e| refused(&e)));
        match written {
            Ok(Ok((text, notes))) => {
                lines.push(put(sink, &read.key, &format!("{t}.txt"), &text)?);
                // The written deck's notes (what it leaves out), the Z4
                // census's input; `diff` passes over them.
                if !notes.diags.is_empty() {
                    let body = refused(&notes);
                    lines.push(put(sink, &read.key, &format!("{t}.notes"), &body)?);
                }
            }
            Ok(Err(e)) => lines.push(put(sink, &read.key, &format!("{t}.refused"), &e)?),
            Err(p) => lines.push(put(
                sink,
                &read.key,
                &format!("{t}.panicked"),
                &panic_text(p),
            )?),
        }
    }
    Ok(lines)
}

/// Writes output `name` of the input at `key` under the tree, or compares
/// it with the tree's file (`sink`); returns its manifest line.
fn put(sink: Sink<'_>, key: &str, name: &str, body: &str) -> Result<String, Stop> {
    let rel = format!("{key}/{name}");
    match sink {
        Sink::Write(out) => std::fs::File::create(out.join(&rel))?.write_all(body.as_bytes())?,
        Sink::Verify(tree) => {
            if let Some(why) = differs(tree, &rel, body.as_bytes()) {
                return Err(Stop::Differs(why));
            }
        }
    }
    Ok(format!("output {rel} {}", hash(body.as_bytes())))
}

/// How `body` differs from `tree`'s file `rel`: `None` when the bytes are
/// the same, else `<rel>: ` and the first byte that differs, the two
/// lengths, or that the tree has no such file.
fn differs(tree: &Path, rel: &str, body: &[u8]) -> Option<String> {
    let had = match std::fs::read(tree.join(rel)) {
        Ok(had) => had,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Some(format!("{rel}: not in {}", tree.display()));
        }
        Err(e) => return Some(format!("{rel}: {e}")),
    };
    if had == body {
        return None;
    }
    Some(match had.iter().zip(body).position(|(a, b)| a != b) {
        Some(k) => format!("{rel}: byte {k} differs"),
        None => format!(
            "{rel}: {} bytes in the tree, {} rendered",
            had.len(),
            body.len()
        ),
    })
}

impl<'a> Place<'a> {
    /// Takes a place, waiting for one to be given back.
    fn take(places: &'a (Mutex<usize>, Condvar)) -> Self {
        let (free, given) = places;
        let mut free = free.lock().expect("no worker panics");
        while *free == 0 {
            free = given.wait(free).expect("no worker panics");
        }
        assert!(*free <= GIANT_TARGETS, "no more places than there are");
        *free -= 1;
        Place(places)
    }
}

/// A renderer's refusal as text: its codes, sorted, on one line (what
/// `diff` compares), then its messages.
fn refused(d: &Diags) -> String {
    let mut codes: Vec<String> = d.diags.iter().map(|d| format!("{:?}", d.code)).collect();
    codes.sort();
    format!("{}{}\n{}", diff::CODES, codes.join(" "), d.messages)
}

/// A panic's message, without its location (which moves with every edit).
fn panic_text(p: Box<dyn std::any::Any + Send>) -> String {
    let msg = p
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a non-string panic".into());
    format!("{msg}\n")
}

/// FNV-1a, 64 bits, as 16 hex digits: stable across builds and machines.
// ponytail: an identity check, not a security one; sha256 if a manifest ever
// gates something untrusted.
fn hash(bytes: &[u8]) -> String {
    let h = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{h:016x}")
}

/// [`verify`] against `tree`. With none, records into a temp dir in a
/// child process first (another process: another `HashMap` seed) and
/// verifies against that in this one; removes it when they agree, keeps it
/// and says where when they do not.
fn check_determinism(tree: Option<&Path>) -> std::io::Result<bool> {
    if let Some(tree) = tree {
        return verify(tree);
    }
    let base = std::env::temp_dir().join(format!("golden-determinism-{}", std::process::id()));
    let status = std::process::Command::new(std::env::current_exe()?)
        .arg("record")
        .arg(&base)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("record failed: {status}")));
    }
    let same = verify(&base)?;
    match same {
        true => std::fs::remove_dir_all(&base)?,
        false => eprintln!("golden: kept the record under {}", base.display()),
    }
    Ok(same)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_matches_the_reference_vectors() {
        assert_eq!(hash(b""), "cbf29ce484222325");
        assert_eq!(hash(b"a"), "af63dc4c8601ec8c");
        assert_eq!(hash(b"foobar"), "85944171f73967e8");
    }

    /// Oracle: the bytes written into a temp tree by hand. The in-memory
    /// compare `check-determinism` runs names a changed byte by its index
    /// and a file the tree lacks, and passes the same bytes.
    #[test]
    fn the_compare_names_a_changed_byte_and_a_missing_file() {
        let tree = std::env::temp_dir().join(format!("golden-differs-{}", std::process::id()));
        std::fs::create_dir_all(tree.join("set/deck")).expect("a temp tree");
        std::fs::write(tree.join("set/deck/ngspice.txt"), "R1 a b 1k\n").expect("a file");
        let same = differs(&tree, "set/deck/ngspice.txt", b"R1 a b 1k\n");
        let byte = differs(&tree, "set/deck/ngspice.txt", b"R1 a b 2k\n");
        let gone = differs(&tree, "set/deck/xyce.txt", b"R1 a b 1k\n");
        std::fs::remove_dir_all(&tree).expect("remove the temp tree");
        assert_eq!(same, None);
        assert_eq!(
            byte.as_deref(),
            Some("set/deck/ngspice.txt: byte 7 differs")
        );
        let gone = gone.expect("a missing file differs");
        assert!(gone.starts_with("set/deck/xyce.txt: not in "), "{gone}");
    }
}
