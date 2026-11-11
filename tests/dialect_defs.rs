//! Oracle 2 of `DIALECT-PLUGIN.md` §8, no back door: every built-in's
//! definition file (`src/syntax/dialects/<d>.rs`) compiled here, in
//! another crate that sees only `pub` items (the `syntax` shim below makes
//! their `crate::syntax::…` paths the public API), registered again under
//! a new name with `DialectDef::renamed`.
//!
//! Oracles: the built-in's own row (`Lang::dump`, equal but for the name
//! where the row names the language itself); the built-in's own output
//! (a metamorphic relation: a clone writes and reads as its built-in):
//! the golden fixtures (`tests/**/*.{sp,lib,spice,cir,scs,sim,net}`, each
//! read as `golden` reads it, a kit in the extension's dialect) and the
//! pinned 200 decks of `$SPICE_BENCH` (`tests/fixtures/dialects/spice-bench-200.txt`,
//! its header the provenance), written to the clone and to the built-in,
//! the same text or the same refusal codes; read as the clone and as the
//! built-in, written to ngspice, the same. Where a built-in's text family
//! is itself, so is its clone's: a deck in that family is read and
//! written as the clone, as the built-in reads and writes its own.
//!
//! Contents: none (tests only).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use spicerack::import;
use spicerack::syntax::def::{DialectDef, register};
use spicerack::syntax::render::{self, Diags, Written};
use spicerack::syntax::resolve::{FileId, Form, Kit};
use spicerack::syntax::{Dialect, Lang};

/// The crate's `syntax`, as the definition files name it: public items only.
mod syntax {
    pub use spicerack::syntax::*;
}

#[path = "../src/syntax/dialects/cdl.rs"]
mod cdl;
#[path = "../src/syntax/dialects/espice.rs"]
mod espice;
#[path = "../src/syntax/dialects/hspice.rs"]
mod hspice;
#[path = "../src/syntax/dialects/ltspice.rs"]
mod ltspice;
#[path = "../src/syntax/dialects/spicerack.rs"]
mod neutral;
#[path = "../src/syntax/dialects/ngspice.rs"]
mod ngspice;
#[path = "../src/syntax/dialects/ngspice_given.rs"]
mod ngspice_given;
#[path = "../src/syntax/dialects/spectre.rs"]
mod spectre;
#[path = "../src/syntax/dialects/vacask.rs"]
mod vacask;
#[path = "../src/syntax/dialects/verilog.rs"]
mod verilog;
#[path = "../src/syntax/dialects/xyce.rs"]
mod xyce;

/// Decks reaching the writers' expression rules, which no fixture or
/// sampled deck holds: a Xyce ternary, `if()` and `sdt()` (ngspice has no
/// `if` and no time integral), an ngspice then-branch ternary (its `{}`
/// evaluator's grouping).
const PROBES: [(&str, Lang, &str); 2] = [
    (
        "probe-xyce",
        Lang::XYCE,
        "* t\n.param a=1\n.param p={a>0 ? 2 : 3}\n.param q={if(a>0, 2, 3)}\nR1 n 0 {p+q}\nB1 m 0 V={sdt(v(n))}\nR2 m 0 1k\n.tran 1n 1u\n.end\n",
    ),
    (
        "probe-ngspice",
        Lang::NGSPICE,
        "* t\n.param a=1\n.param p={a>0 ? a>1 ? 4 : 8 : 16}\nR1 n 0 {p}\nV1 n 0 1\n.op\n.end\n",
    ),
];

/// One input: its text, its deck name, the dialect it is read in, and
/// the kit `golden` reads a fixture through.
struct Input {
    rel: String,
    text: String,
    name: String,
    lang: Lang,
    kit: Option<Kit>,
}

// ---- private ----

/// Every built-in, each with its definition file's function.
fn builtins() -> [(Lang, fn() -> DialectDef); Lang::BUILTIN_COUNT] {
    [
        (Lang::NGSPICE, ngspice::ngspice),
        (Lang::ESPICE, espice::espice),
        (Lang::HSPICE, hspice::hspice),
        (Lang::XYCE, xyce::xyce),
        (Lang::LTSPICE, ltspice::ltspice),
        (Lang::SPECTRE, spectre::spectre),
        (Lang::VACASK, vacask::vacask),
        (Lang::CDL, cdl::cdl),
        (Lang::VERILOG, verilog::verilog),
        (Lang::NGSPICE_GIVEN, ngspice_given::ngspice_given),
        (Lang::SPICERACK, neutral::spicerack),
    ]
}

/// The clone's name: the built-in's, prefixed.
fn clone_name(b: Lang) -> String {
    format!("dup-{}", b.name())
}

/// Each built-in and its clone, registered (an equal re-registration
/// returns the held row, so every test may call this).
fn clones() -> Vec<(Lang, Lang)> {
    let all = builtins().map(|(b, def)| {
        let c = register(def().renamed(&clone_name(b))).expect("the clone registers");
        (b, c)
    });
    all.to_vec()
}

/// What a render gave: the text, the sorted refusal codes, or a panic.
fn outcome(r: impl FnOnce() -> Result<Written, Diags>) -> String {
    match catch_unwind(AssertUnwindSafe(r)) {
        Ok(Ok(w)) => w.text,
        Ok(Err(d)) => {
            let mut codes: Vec<String> = d.diags.iter().map(|d| format!("{:?}", d.code)).collect();
            codes.sort();
            format!("refused {}", codes.join(" "))
        }
        Err(_) => "panicked".into(),
    }
}

/// The golden fixtures, each read as `golden` reads it, and [`PROBES`].
fn fixtures() -> Vec<Input> {
    const EXT: [(&str, Dialect); 7] = [
        ("sp", Dialect::Ngspice),
        ("lib", Dialect::Ngspice),
        ("spice", Dialect::Ngspice),
        ("cir", Dialect::Xyce),
        ("scs", Dialect::Spectre),
        ("sim", Dialect::Vacask),
        ("net", Dialect::Ltspice),
    ];
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests = repo.join("tests");
    let mut found = Vec::new();
    files(&tests, &mut found);
    found.sort();
    let mut out = Vec::new();
    for path in found {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let Some(&(_, d)) = EXT.iter().find(|e| e.0 == ext) else {
            continue;
        };
        let kit = import::kit(std::slice::from_ref(&path), Form::Deck, d, &tests);
        out.push(Input {
            rel: path
                .strip_prefix(repo)
                .expect("under the repo")
                .display()
                .to_string(),
            text: kit.file(FileId(0)).text.clone(),
            name: stem(&path),
            lang: d.into(),
            kit: Some(kit),
        });
    }
    assert!(out.len() > 50, "the fixtures: {}", out.len());
    out.extend(PROBES.map(|(name, lang, text)| Input {
        rel: name.into(),
        text: text.into(),
        name: name.into(),
        lang,
        kit: None,
    }));
    out
}

/// The pinned spice-bench sample, read in ngspice. `$SPICE_BENCH` unset
/// fails: the sample is part of the oracle.
fn bench() -> Vec<Input> {
    let root = std::env::var("SPICE_BENCH").expect("$SPICE_BENCH (nix develop .#testing)");
    let list = include_str!("fixtures/dialects/spice-bench-200.txt");
    let rels: Vec<&str> = list.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(rels.len(), 200, "the pinned sample");
    let read = |rel: &str| {
        let path = Path::new(&root).join(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
        Input {
            rel: rel.into(),
            text: import::decode(&bytes),
            name: stem(&path),
            lang: Lang::NGSPICE,
            kit: None,
        }
    };
    rels.into_iter().map(read).collect()
}

/// Every file under `dir`, as `golden` walks it (no `src*` folder).
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        match p.is_dir() {
            true if !e.file_name().to_string_lossy().starts_with("src") => files(&p, out),
            true => {}
            false => out.push(p),
        }
    }
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("deck")
        .into()
}

/// `input` written for `target`, read as golden reads it (its kit), or
/// read as `read` instead.
fn write(input: &Input, read: Option<Lang>, target: Lang) -> String {
    match (read, &input.kit) {
        (None, Some(kit)) => outcome(|| render::render_kit(kit, FileId(0), &input.name, target)),
        (r, _) => {
            let d = r.unwrap_or(input.lang);
            outcome(|| render::render_text(&input.text, d, &input.name, target))
        }
    }
}

/// The inputs whose rendering differs between built-in and clone, as
/// `rel: how`, for every pair the module header names.
fn mismatches(inputs: &[Input]) -> Vec<String> {
    let mut bad = Vec::new();
    let family = |l: Lang| l.behaviours().text_family;
    for (b, c) in clones() {
        // The readers with no input of their own read ngspice's.
        let borrowed = [Lang::ESPICE, Lang::HSPICE, Lang::NGSPICE_GIVEN].contains(&b);
        for i in inputs {
            // Written to the clone: from the source, or, where the source's
            // family is the built-in itself, read and written as the clone.
            let (want, got) = match family(i.lang) == b && family(c) == c {
                true => (write(i, Some(b), b), write(i, Some(c), c)),
                false => (write(i, None, b), write(i, None, c)),
            };
            if want != got {
                bad.push(format!("{}: written to {}", i.rel, c.name()));
            }
            // Read as the clone, written to ngspice (ngspice's clone to
            // Xyce: its writes to itself are above).
            let reads = i.lang == b || (borrowed && i.lang == Lang::NGSPICE);
            let to = match b == Lang::NGSPICE {
                true => Lang::XYCE,
                false => Lang::NGSPICE,
            };
            if b.lexicon().is_some() && reads {
                let want = write(i, Some(b), to);
                if want != write(i, Some(c), to) {
                    bad.push(format!("{}: read as {}", i.rel, c.name()));
                }
            }
        }
    }
    bad
}

#[test]
fn every_built_in_compiles_from_public_items_and_dumps_its_row() {
    for (b, c) in clones() {
        let (want, got) = (b.dump(), c.dump());
        let held = |k: &str| ["name", "lex.lang", "sp.lexicon", "behaviours"].contains(&k);
        for (w, g) in want.lines().zip(got.lines()) {
            if w != g {
                let key = w.split(' ').next().unwrap_or("");
                assert!(held(key), "{}: {w:?} against {g:?}", b.name());
                assert_eq!(g.replace(&clone_name(b), b.name()), w, "{}", b.name());
            }
        }
        assert_eq!(want.lines().count(), got.lines().count(), "{}", b.name());
    }
}

#[test]
fn a_clone_writes_and_reads_the_fixtures_as_its_built_in() {
    assert_eq!(mismatches(&fixtures()), Vec::<String>::new());
}

#[test]
fn a_clone_writes_and_reads_the_spice_bench_sample_as_its_built_in() {
    assert_eq!(mismatches(&bench()), Vec::<String>::new());
}
