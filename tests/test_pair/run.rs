//! One deck through the six steps of the pair test (`rewrite/SPINE.md` §8a), for the authored
//! cells and the corpus cells alike: the spine (read, legalize, write), the written text read
//! back, both simulators, the names, and the bytes again on [`JOBS`] threads.
//!
//! In: a [`Deck`] (its sources, the read policy, whether it is simulated). Out: each failing
//! step with its class (what a list names) and message, and the written files.
//!
//! Cites: `rewrite/SPINE.md` §8a; REWRITE-PLAN §7.4 (which component each step names).
//!
//! Contents:
//! - `pub struct Deck`, `pub struct Fail`, `pub struct Done`, `pub type Written`, `pub const JOBS`
//! - `pub fn steps(c: &Cell, from: &DialectRow, to: &DialectRow, deck: &Deck, pair: &PairFile) -> Done`
//! - `pub fn determinism(from: &DialectRow, to: &DialectRow, decks: &[Deck], once: &BTreeMap<String, Written>) -> Vec<(String, String)>`
//! - `pub fn convert(from: &DialectRow, to: &DialectRow, deck: &Deck) -> Option<Written>`
//! - `pub fn fail(class: &str, said: String) -> Fail`

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;
use spicerack::ir::ItemRow as _;

use crate::files::PairFile;
use crate::sim::{self, Ran};
use crate::{Cell, cmp, laws, plots};

/// A deck to run: its name in the cell's list, the source file (the simulator runs it there,
/// beside its own files), the sources the reader reads (the entry first, paths relative to the
/// deck's folder), the read policy (strict for authored decks, lenient for a corpus), and
/// whether step 4 runs (`false`: a deck the corpus quarantines as `slow`).
pub struct Deck {
    /// The name the cell's list uses.
    pub name: String,
    /// The source deck on disk.
    pub path: PathBuf,
    /// What the source reader reads.
    pub src: Sources,
    /// Strict or lenient.
    pub policy: Policy,
    /// Whether the simulators run.
    pub simulate: bool,
}

/// A failed step: its class (what the expected list names) and what was seen.
pub struct Fail {
    /// `refused:<codes>[:<features>]`, `differs`, `mismatch`, …
    pub class: String,
    /// The message.
    pub said: String,
    /// For a step-2 refusal of a foreign `Opaque`: each refused opaque as
    /// `<dialect>/<class>/<reader code>/<first word>`, in item order (the census counts them).
    pub opaques: Vec<String>,
}

/// One deck's verdict: each failing step, the last step that ran, and the written files (when
/// the deck was written: step 6's one job).
pub struct Done {
    /// Each failing step with its failure.
    pub fails: BTreeMap<u8, Fail>,
    /// The last of steps 1 to 5 that ran.
    pub last: u8,
    /// The written files, the entry first.
    pub written: Option<Written>,
}

/// A deck's written files, the entry first: (path, text).
pub type Written = Vec<(String, String)>;

/// The jobs step 6 converts the decks on, against one.
pub const JOBS: usize = 8;

/// What the spine made of one deck: the legalized design, the written files, the rename map
/// (new name → source name, lower case) and the names of its devices (every element but a
/// subcircuit instance, under both names, lower case).
struct Spine {
    legal: ir::Design,
    files: Written,
    renames: HashMap<String, String>,
    devices: BTreeSet<String>,
}

/// CPU seconds per simulator run (the wall clock gets ten times as much: `sim::run`).
const SECS: u64 = 60;

/// `name_with_letter`'s note at a renamed element, `arg` the old name (the pass's `RENAMED`,
/// `src/passes/structure/name_with_letter.rs`; the passes are not public items).
const RENAMED: ir::Code = ir::Code::new(b'U', 311);

/// Steps 1 to 5 of `deck` (step 6 is [`determinism`], over every deck at once).
pub fn steps(c: &Cell, from: &DialectRow, to: &DialectRow, deck: &Deck, pair: &PairFile) -> Done {
    let mut fails = BTreeMap::new();
    let stop = |mut fails: BTreeMap<u8, Fail>, step: u8, f: Fail, written| {
        fails.insert(step, f);
        Done {
            fails,
            last: step,
            written,
        }
    };
    let spine = match spine(from, to, &deck.src, deck.policy) {
        Ok(s) => s,
        Err((step, f)) => return stop(fails, step, f, None),
    };
    let written = Some(spine.files.clone());
    if let Err(f) = read_back(to, &spine) {
        fails.insert(3, f);
    }
    let (entry, rest) = spine.files.split_first().expect("an entry file");
    if !deck.simulate {
        let f = fail("slow", "quarantined by the corpus's source list".into());
        return stop(fails, 4, f, written);
    }
    // ponytail: a written include outside the run directory's top level is not run (the run
    // directory links the deck's folder, read-only in the store); place it when a corpus
    // deck needs it.
    if let Some((p, _)) = rest.iter().find(|(p, _)| p.contains('/')) {
        let f = fail(
            "harness-path",
            format!("written include {p} not at the top level"),
        );
        return stop(fails, 4, f, written);
    }
    let text = &deck.src.files()[0].1;
    let ran = guard(4, || {
        let a = sim::run(c.from_sim, &deck.path, text, &[], SECS);
        let b = sim::run(
            c.to_sim,
            &deck.path.with_extension(c.to_ext),
            &entry.1,
            rest,
            SECS,
        );
        (a, b)
    });
    let (a, b) = match ran {
        Ok((Ran::Plots(a), Ran::Plots(b))) => (
            plots::only(a, pair.vectors.as_ref()),
            plots::only(plots::renamed(b, &spine.renames), pair.vectors.as_ref()),
        ),
        Ok((a, b)) => return stop(fails, 4, ran_fail(a, b), written),
        Err((_, f)) => return stop(fails, 4, f, written),
    };
    let mut tol = cmp::Tol::of(&[text, &entry.1]);
    for (k, v) in &pair.tightened {
        tol.tighten(k, *v).expect("pair_file checked it");
    }
    if let Err(why) = cmp::compare(&a, &b, tol) {
        fails.insert(4, fail("mismatch", why));
    }
    if let Err(why) = plots::names(&a, &b, &spine.devices) {
        fails.insert(5, fail("missing-vector", why));
    }
    Done {
        fails,
        last: 5,
        written,
    }
}

/// Step 6: every deck of `decks` converted again on [`JOBS`] threads at once, against `once`
/// (the serial run's written files, by deck name); the decks whose bytes differ, with the first
/// file that does.
pub fn determinism(
    from: &DialectRow,
    to: &DialectRow,
    decks: &[Deck],
    once: &BTreeMap<String, Written>,
) -> Vec<(String, String)> {
    let again: Vec<(&str, Option<Written>)> = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..JOBS)
            .map(|j| {
                s.spawn(move || {
                    let mine = decks.iter().skip(j).step_by(JOBS);
                    mine.map(|d| {
                        let files = spine(from, to, &d.src, d.policy).ok().map(|s| s.files);
                        (d.name.as_str(), files)
                    })
                    .collect::<Vec<_>>()
                })
            })
            .collect();
        jobs.into_iter()
            .flat_map(|j| j.join().expect("a job"))
            .collect()
    });
    assert_eq!(again.len(), decks.len(), "every deck converted again");
    let mut out = Vec::new();
    for (name, files) in again {
        let first = once.get(name);
        if first != files.as_ref() {
            let file = first
                .zip(files.as_ref())
                .and_then(|(x, y)| {
                    x.iter()
                        .zip(y)
                        .find(|(p, q)| p != q)
                        .map(|(p, _)| p.0.clone())
                })
                .unwrap_or_else(|| "the file list".into());
            out.push((
                name.to_string(),
                format!("{file} differs between 1 and {JOBS} jobs"),
            ));
        }
    }
    out
}

/// `deck` read, legalized and written (steps 1 and 2 and the writing), or `None` when a step
/// refused or panicked.
#[cfg_attr(not(feature = "corpus"), allow(dead_code))] // the corpus cells call it
pub fn convert(from: &DialectRow, to: &DialectRow, deck: &Deck) -> Option<Written> {
    spine(from, to, &deck.src, deck.policy)
        .ok()
        .map(|s| s.files)
}

/// A failure of class `class`.
pub fn fail(class: &str, said: String) -> Fail {
    Fail {
        class: class.to_string(),
        said,
        opaques: Vec::new(),
    }
}

// ---- private ----

/// Steps 1 and 2 and the writing: the legalized design and its files, or the step that
/// refused with its codes. A statement the source reader keeps as `InvalidInSource` (its
/// simulator rejects it) fails step 1, class `invalid-in-source`: the deck, not a pass, is at
/// fault, though `legalize` would refuse it too. A panic is the step's failure, class `panic`:
/// in the read step 1, in `legalize` step 2, in the writer step 3 (the target dialect).
fn spine(
    from: &DialectRow,
    to: &DialectRow,
    src: &Sources,
    policy: Policy,
) -> Result<Spine, (u8, Fail)> {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    guard(1, || {
        dialect::read(src, from, policy, &mut design, &mut diags)
    })?;
    let text = src.files().first().map_or("", |f| f.1.as_str());
    // `ir::check`'s violations (`X` rows) are the reader's bug: a debug build panics on them in
    // `dialect::read` (class `panic`), a release build only reports them (class `check:<codes>`).
    let bad: Vec<&ir::Diag> = diags
        .rows()
        .iter()
        .filter(|d| d.code.letter == b'X')
        .collect();
    if !bad.is_empty() {
        let codes: BTreeSet<String> = bad.iter().map(|d| d.code.to_string()).collect();
        let class = format!("check:{}", codes.into_iter().collect::<Vec<_>>().join(","));
        return Err((1, fail(&class, describe(&bad, text))));
    }
    if let Some(f) = refusal(&diags, 0, text) {
        return Err((1, f));
    }
    let table = ir::item::Opaque::table(&design);
    let invalid = table
        .rows()
        .iter()
        .filter(|o| o.class == ir::OpaqueClass::InvalidInSource);
    if invalid.count() > 0 {
        let notes: Vec<&ir::Diag> = diags.rows().iter().collect();
        return Err((1, fail("invalid-in-source", describe(&notes, text))));
    }
    let read = diags.len();
    guard(2, || dialect::legalize(&mut design, to, &mut diags))?;
    if let Some(mut f) = refusal(&diags, read, text) {
        f.opaques = opaques(&design, &diags, read);
        return Err((2, f));
    }
    let renames: HashMap<String, String> = diags.rows()[read..]
        .iter()
        .filter(|d| d.code == RENAMED)
        .map(|d| {
            let e = design.row::<ir::Element>(d.item.index);
            let new = design.str(e.name).to_ascii_lowercase();
            (new, design.str(ir::NameId(d.arg)).to_ascii_lowercase())
        })
        .collect();
    let files = guard(3, || laws::write_all(&design, to).files().to_vec())?;
    let mut devices = BTreeSet::new();
    for (item, _) in design.reachable() {
        if item.kind != ir::ItemKind::Element {
            continue;
        }
        let e = design.row::<ir::Element>(item.index);
        if e.kind != ir::ElemKind::Instance {
            let name = design.str(e.name).to_ascii_lowercase();
            devices.extend(renames.get(&name).cloned());
            devices.insert(name);
        }
    }
    Ok(Spine {
        legal: design,
        files,
        renames,
        devices,
    })
}

/// Step 3: the written files read back by `to`'s reader, leniently, mean the legalized design.
fn read_back(to: &DialectRow, spine: &Spine) -> Result<(), Fail> {
    let mut src = Sources::new();
    for (path, text) in &spine.files {
        src.add(path, text);
    }
    let mut back = ir::Design::new();
    let mut diags = ir::Diags::new();
    let read = guard(3, || {
        dialect::read(&src, to, Policy::Lenient, &mut back, &mut diags)
    });
    if let Err((_, f)) = read {
        return Err(f);
    }
    if let Some(f) = refusal(&diags, 0, &spine.files[0].1) {
        return Err(fail("refused-back", f.said));
    }
    if laws::same_meaning(&spine.legal, &back) {
        return Ok(());
    }
    let ir_row = dialect::by_name("ir").expect("the ir row");
    let normal =
        |d: &ir::Design| laws::canonical(&laws::read_all(ir_row, &laws::write_all(d, ir_row)));
    let (want, got) = (normal(&spine.legal), normal(&back));
    // Every differing line, the first four: a known difference must not hide another.
    let pairs = want.lines().zip(got.lines()).filter(|(w, g)| w != g);
    let lines: Vec<String> = pairs
        .take(4)
        .map(|(w, g)| format!("legalized `{}`, read back `{}`", w.trim(), g.trim()))
        .collect();
    let said = match lines.is_empty() {
        false => lines.join(" | "),
        true => format!(
            "{} lines legalized, {} read back",
            want.lines().count(),
            got.lines().count()
        ),
    };
    Err(fail("differs", said))
}

/// `f`'s value, or step `step` failed with class `panic` and the panic's message.
fn guard<T>(step: u8, f: impl FnOnce() -> T) -> Result<T, (u8, Fail)> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|e| {
        let said = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "a panic with no message".into());
        (step, fail("panic", said))
    })
}

/// The refusing rows of `diags` from `from` on, as a class `refused:<codes>` (and, for `U0001`,
/// `:<features>`: the cause is the feature, so a row whose feature changes fails its line) and
/// a message naming each with its title and source line; `None` when none refuses.
fn refusal(diags: &ir::Diags, from: usize, text: &str) -> Option<Fail> {
    let rows: Vec<&ir::Diag> = diags.rows()[from..]
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .collect();
    if rows.is_empty() {
        return None;
    }
    let codes: BTreeSet<String> = rows.iter().map(|d| d.code.to_string()).collect();
    let features: BTreeSet<String> = rows
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED)
        .map(|d| format!("{:?}", ir::Feature::from_index(d.arg as u16)).replace(' ', ""))
        .collect();
    let mut class = format!(
        "refused:{}",
        codes.into_iter().collect::<Vec<_>>().join(",")
    );
    if !features.is_empty() {
        class += ":";
        class += &features.into_iter().collect::<Vec<_>>().join("+");
    }
    Some(fail(&class, describe(&rows, text)))
}

/// `rows`, each with its source line in `text` (the entry's), its code, title and (for
/// `U0001`) the feature; the first eight.
fn describe(rows: &[&ir::Diag], text: &str) -> String {
    let said: Vec<String> = rows
        .iter()
        .take(8)
        .map(|d| {
            let title = dialect::info(d.code).map_or("(no catalogue row)", |i| i.title);
            let line = 1 + text.as_bytes()[..(d.span.start as usize).min(text.len())]
                .iter()
                .filter(|&&b| b == b'\n')
                .count();
            let arg = match d.code == ir::diag::UNSUPPORTED {
                true => format!(" {:?}", ir::Feature::from_index(d.arg as u16)),
                false => String::new(),
            };
            format!("line {line}: {} {title}{arg}", d.code)
        })
        .collect();
    said.join(" | ")
}

/// Each `Opaque` `legalize` refused (a `U0001` row from `read` on whose feature is
/// `Item(Opaque)`), as `<dialect>/<class>/<code>/<first word>`: the code is the reader's note at
/// the same source bytes (its `P` code first; `-` when it gave none), the word the statement's
/// first (`<instance>` for an instance line).
fn opaques(design: &ir::Design, diags: &ir::Diags, read: usize) -> Vec<String> {
    let opaque = u32::from(ir::Feature::Item(ir::ItemKind::Opaque).index());
    let rows = diags.rows();
    rows[read..]
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == opaque)
        .filter(|d| d.item.kind == ir::ItemKind::Opaque)
        .map(|d| {
            let o: ir::item::Opaque = design.row(d.item.index);
            // The reader's own code at those bytes (`P…`) before the spine's class code (`S…`).
            let at: Vec<&ir::Diag> = rows[..read].iter().filter(|r| r.span == d.span).collect();
            let note = at
                .iter()
                .find(|r| r.code.letter == b'P')
                .or(at.first())
                .map_or("-".to_string(), |r| r.code.to_string());
            let words: Vec<&str> = design.text(o.text).split_whitespace().take(2).collect();
            let instance = words.iter().any(|w| w.contains('('));
            let word = match instance {
                true => "<instance>",
                false => words.first().copied().unwrap_or(""),
            };
            format!("{}/{:?}/{note}/{word}", design.str(o.dialect), o.class)
        })
        .collect()
}

/// Step 4's verdict when a run gave no plots: whose run, and why.
fn ran_fail(a: Ran, b: Ran) -> Fail {
    match (a, b) {
        (Ran::Timeout, _) | (_, Ran::Timeout) => fail("slow", "past the CPU budget".into()),
        (Ran::Failed(why), _) => fail("source-fails", why),
        (_, Ran::Failed(why)) => fail("target-fails", why),
        (Ran::Plots(_), Ran::Plots(_)) => unreachable!("a run without plots"),
    }
}
