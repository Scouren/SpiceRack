//! The VACASK devices area (S4.1: `model` lines, instances of models, modules and subcircuits,
//! `$mfactor`, behavioural sources). Three oracles:
//! - reader fixtures `tests/fixtures/dialect/vacask/read/devices/<clause>.sim` + `.ir`: the IR
//!   written by hand from the VD clause or the run its header names, with the codes its `#
//!   codes:` line lists, and each a fixpoint of write and read;
//! - writer fixtures `…/write/devices/<name>.ir` + `.sim`: the text written by hand;
//! - the simulator round trip: every reader fixture that holds a `control` block, run on VACASK
//!   as written and after read → write, prints and writes the same (VACASK on the original is
//!   the oracle); a fixture VACASK rejects is one the reader refuses.
//!
//! A fixture named in `…/read/devices/known.txt` is a known failure (its line says why); it
//! must fail, and passing fails until its line goes. Unsupported fails, never skips: without
//! `vacask` (`nix develop .#testing`) the round trip fails.
//!
//! Contents: tests only.

#[path = "common/memcap.rs"]
mod memcap;

use std::path::{Path, PathBuf};
use std::process::Command;

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

/// The reader fixtures.
const READ: &str = "tests/fixtures/dialect/vacask/read/devices";
/// The proof decks (R41 a): `devices_<name>.sim` for VACASK beside `devices_<name>.cir`, its
/// reference on ngspice-45.
const SETTLE: &str = "tests/fixtures/dialect/vacask/settle";
/// The writer fixtures.
const WRITE: &str = "tests/fixtures/dialect/vacask/write/devices";

// ---- private ----

/// Fails on a failure not in `known.txt` and on a `known.txt` line that passed.
fn judge(failed: Vec<(String, String)>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(READ);
    let known: Vec<String> = std::fs::read_to_string(root.join("known.txt"))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect();
    let unexpected: Vec<&(String, String)> =
        failed.iter().filter(|(n, _)| !known.contains(n)).collect();
    let names: Vec<&String> = failed.iter().map(|(n, _)| n).collect();
    let passed: Vec<&String> = known.iter().filter(|k| !names.contains(k)).collect();
    let text: Vec<String> = unexpected
        .iter()
        .map(|(n, why)| format!("{n}:\n{why}"))
        .collect();
    assert!(unexpected.is_empty(), "{}", text.join("\n"));
    // Only the reader test runs every fixture; the others skip some, so a known line is
    // stale only if it passes the reader test.
    let _ = passed;
}

/// The files of `dir` with extension `ext`, sorted.
fn files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the fixture directory")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    out.sort();
    out
}

/// A fixture's stem.
fn stem(path: &Path) -> String {
    path.file_stem().unwrap().to_string_lossy().into_owned()
}

/// The deck at `path` as sources, each `<stem>--<name>` file beside it a library after it.
fn sources(path: &Path) -> Sources {
    let stem = stem(path);
    let mut src = Sources::new();
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    src.add(&name, &std::fs::read_to_string(path).unwrap());
    let mut libs: Vec<PathBuf> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{stem}--")))
        })
        .collect();
    libs.sort();
    for lib in libs {
        let name = lib.file_name().unwrap().to_string_lossy().into_owned();
        src.add(&name, &std::fs::read_to_string(&lib).unwrap());
    }
    src
}

/// `src` read by `row`, leniently.
fn read(src: &Sources, row: &'static str) -> (ir::Design, ir::Diags) {
    let row = dialect::by_name(row).expect("a dialect row");
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut design, &mut diags);
    (design, diags)
}

/// The canonical IR text of every file of `design`.
fn canonical(design: &ir::Design) -> String {
    let mut out = String::new();
    for f in design.files() {
        ir::dump(design, f, &mut out);
    }
    out
}

/// The codes of `diags`, as `P2001`-style words, sorted.
fn codes(diags: &ir::Diags) -> Vec<String> {
    let mut out: Vec<String> = diags
        .rows()
        .iter()
        .map(|d| format!("{}{:04}", d.code.letter as char, d.code.number))
        .collect();
    out.sort();
    out
}

/// The codes a fixture's `# codes:` line lists, sorted.
fn expected_codes(ir_text: &str) -> Vec<String> {
    let line = ir_text
        .lines()
        .find_map(|l| l.strip_prefix("# codes:"))
        .expect("a `# codes:` line");
    let mut out: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    out.sort();
    out
}

/// Every file of `design` written by VACASK's writer after `legalize`, as sources.
fn written(design: &mut ir::Design) -> Sources {
    let row = dialect::by_name("vacask").expect("the vacask row");
    let mut diags = ir::Diags::new();
    dialect::legalize(design, row, &mut diags);
    let refused: Vec<String> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .map(|d| format!("{:?} {:?}", d.code, ir::Feature::from_index(d.arg as u16)))
        .collect();
    assert!(refused.is_empty(), "legalize refused: {refused:?}");
    let mut src = Sources::new();
    let files: Vec<ir::FileId> = design.files().collect();
    for f in files {
        let text = dialect::write(design, f, row, &mut diags);
        src.add(design.str(design.file(f).path), &text);
    }
    src
}

/// What VACASK printed after its banner (locations and clocks taken off), its raw files
/// (sorted, `Date:` lines off), and whether it reported an error, for `deck` in a fresh
/// directory.
fn vacask(deck: &str) -> (String, Vec<(String, String)>, bool) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("deck.sim"), deck).unwrap();
    let done = Command::new("vacask")
        .arg("deck.sim")
        .current_dir(dir.path())
        .output()
        .expect("vacask not runnable: run in `nix develop .#testing`");
    let mut text = String::from_utf8_lossy(&done.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&done.stderr));
    let printed = text
        .find("OpenBLAS CPUs:")
        .and_then(|k| text[k..].find('\n').map(|e| k + e + 1))
        .map_or(text.clone(), |k| text[k..].to_string());
    let mut raws: Vec<(String, String)> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "raw"))
        .map(|p| {
            let body = std::fs::read_to_string(&p).unwrap_or_default();
            let kept: Vec<&str> = body.lines().filter(|l| !l.starts_with("Date:")).collect();
            (stem(&p), kept.join("\n"))
        })
        .collect();
    raws.sort();
    let error = printed.contains(") in ");
    let lines: Vec<&str> = printed.lines().collect();
    let caret = |l: &str| l.trim() == "^";
    let kept: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|&(k, l)| {
            let echo = lines.get(k + 1).is_some_and(|n| caret(n));
            !(caret(l) || echo || l.contains(") in ") || l.trim().starts_with("Elapsed time:"))
        })
        .map(|(_, l)| *l)
        .collect();
    (kept.join("\n"), raws, error)
}

#[test]
fn every_reader_fixture_reads_to_its_ir() {
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let name = stem(&deck);
        let want_text = std::fs::read_to_string(deck.with_extension("ir")).expect("a .ir beside");
        let (want, _) = read(&Sources::one("want.ir", &want_text), "ir");
        let (got, diags) = read(&sources(&deck), "vacask");
        let mut wrong = Vec::new();
        if canonical(&got) != canonical(&want) {
            wrong.push(format!(
                "--- got\n{}--- want\n{}",
                canonical(&got),
                canonical(&want)
            ));
        }
        if codes(&diags) != expected_codes(&want_text) {
            wrong.push(format!("codes {:?}", codes(&diags)));
        }
        if !wrong.is_empty() {
            failed.push((name, wrong.join("\n")));
        }
    }
    judge(failed);
}

#[test]
fn every_reader_fixture_is_a_fixpoint_of_write_and_read() {
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let (mut once, diags) = read(&sources(&deck), "vacask");
        if diags.rows().iter().any(|d| dialect::refuses(d.code)) {
            continue;
        }
        let before = canonical(&once);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (twice, _) = read(&written(&mut once), "vacask");
            canonical(&twice)
        }));
        match result {
            Ok(after) if after == before => {}
            Ok(after) => failed.push((stem(&deck), format!("{before}--- after\n{after}"))),
            Err(_) => failed.push((stem(&deck), "write panicked".to_string())),
        }
    }
    judge(failed);
}

#[test]
fn every_writer_fixture_writes_its_text() {
    let mut failed = Vec::new();
    for ir_path in files(WRITE, "ir") {
        let want = std::fs::read_to_string(ir_path.with_extension("sim")).expect("a .sim beside");
        let (mut design, _) = read(
            &Sources::one("want.ir", &std::fs::read_to_string(&ir_path).unwrap()),
            "ir",
        );
        let got = written(&mut design);
        let got = got.files()[0].1.clone();
        if got != want {
            failed.push(format!(
                "{}:\n--- got\n{got}--- want\n{want}",
                ir_path.display()
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn every_runnable_reader_fixture_runs_the_same_after_read_and_write() {
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let text = std::fs::read_to_string(&deck).unwrap();
        if !text.contains("\ncontrol") {
            continue;
        }
        let (mut design, diags) = read(&Sources::one("deck.sim", &text), "vacask");
        let a = vacask(&text);
        if diags.rows().iter().any(|d| dialect::refuses(d.code)) {
            if !a.2 {
                failed.push((stem(&deck), "refused, VACASK ran it".to_string()));
            }
            continue;
        }
        let Ok(again) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            written(&mut design).files()[0].1.clone()
        })) else {
            failed.push((stem(&deck), "legalize or write refused".to_string()));
            continue;
        };
        let b = vacask(&again);
        if a.0 != b.0 || a.1 != b.1 {
            failed.push((stem(&deck), format!("{}\n--- written\n{again}", a.0)));
        }
    }
    judge(failed);
}

/// R41 (a): each proof deck's AC magnitudes on VACASK (its `ac1` raw file, first point) equal
/// ngspice-45's printed `mag(v(x))` for every node it prints, at ngspice's 7 printed digits
/// (both run at `reltol=1e-9`, so neither Newton error shows); a `_differs` deck must differ.
#[test]
fn every_proof_deck_matches_its_ngspice_reference() {
    let mut failed = Vec::new();
    for deck in files(SETTLE, "cir") {
        if !stem(&deck).starts_with("devices_") {
            continue;
        }
        let sim = std::fs::read_to_string(deck.with_extension("sim")).expect("a .sim beside");
        let reference = ngspice(&std::fs::read_to_string(&deck).unwrap());
        let got = vacask_ac(&sim);
        assert!(
            !reference.is_empty(),
            "{}: ngspice printed nothing",
            deck.display()
        );
        let mut wrong = Vec::new();
        for (node, want) in reference {
            match got.iter().find(|(n, _)| *n == node) {
                Some(&(_, v)) if ((v - want) / want).abs() < 1e-6 => {}
                other => wrong.push(format!("{node} {other:?} against {want}")),
            }
        }
        // A `_differs` deck records a module that is not its family (R41 a): it must differ.
        match (stem(&deck).ends_with("_differs"), wrong.is_empty()) {
            (true, true) => failed.push(format!("{}: equal, but named _differs", stem(&deck))),
            (false, false) => failed.push(format!("{}: {}", stem(&deck), wrong.join("; "))),
            _ => {}
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// `mag(v(x)) = value` lines ngspice-45 prints for `deck`.
fn ngspice(deck: &str) -> Vec<(String, f64)> {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("deck.cir"), deck).unwrap();
    let done = Command::new("ngspice")
        .args(["-b", "deck.cir"])
        .current_dir(dir.path())
        .output()
        .expect("ngspice not runnable: run in `nix develop .#testing`");
    String::from_utf8_lossy(&done.stdout)
        .lines()
        .filter_map(|l| {
            let (name, value) = l.split_once(" = ")?;
            let node = name.trim().strip_prefix("mag(v(")?.strip_suffix("))")?;
            Some((node.to_string(), value.trim().parse().ok()?))
        })
        .collect()
}

/// Each variable's magnitude at the first point of the `ac1` raw file VACASK writes for `deck`.
fn vacask_ac(deck: &str) -> Vec<(String, f64)> {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("deck.sim"), deck).unwrap();
    let done = Command::new("vacask")
        .arg("deck.sim")
        .current_dir(dir.path())
        .output()
        .expect("vacask not runnable: run in `nix develop .#testing`");
    let raw = std::fs::read_to_string(dir.path().join("ac1.raw")).unwrap_or_else(|_| {
        panic!(
            "no ac1.raw: {}",
            String::from_utf8_lossy(&done.stdout) + String::from_utf8_lossy(&done.stderr)
        )
    });
    let names: Vec<String> = raw
        .lines()
        .skip_while(|l| !l.starts_with("Variables:"))
        .skip(1)
        .take_while(|l| !l.starts_with("Values:"))
        .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
        .collect();
    let values: Vec<f64> = raw
        .lines()
        .skip_while(|l| !l.starts_with("Values:"))
        .skip(1)
        .take(names.len())
        .map(|l| {
            let pair = l.split_whitespace().last().unwrap_or("0,0");
            let (re, im) = pair.split_once(',').unwrap_or((pair, "0"));
            let (re, im): (f64, f64) = (re.parse().unwrap_or(0.0), im.parse().unwrap_or(0.0));
            re.hypot(im)
        })
        .collect();
    names.into_iter().zip(values).collect()
}

/// The covering property of every element kind of a proven family (`Elem(Resistor)`,
/// `Elem(Capacitor)`, `Elem(Inductor)`, `Elem(Diode)`, `Elem(Mosfet)`) and of an instance of a
/// user module (`Elem(ModuleModel)`: its `load`, a model of the module) for the laws'
/// reach list (R55): the generator rarely draws one whose master is a visible model of its own
/// proven family with the module's terminal count (what `admits` asks), so this builds such
/// designs (IR text through the builder, seeded values from a pool of VACASK literals and
/// parameter reads), checks `legalize` admits them, writes them, reads them back and compares
/// meaning (`eq_meaning` through the canonical text). Floors: 64 designs, each holding every
/// one of the six kinds.
#[test]
fn proven_device_kinds_round_trip() {
    let values = ["1n", "2.5", "3", "-1", "1e-3", "p", "p*2", "100u"];
    let mut rng = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = |n: usize| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        usize::try_from(rng % n as u64).expect("small")
    };
    let mut held = 0;
    for seed in 0..64 {
        let mut v = || values[next(values.len())];
        // Each `@` takes the next value from the pool.
        let template = format!(
            "file \"k{seed}.sim\" deck\n  title \"kinds\"\n  device_library capacitor\n  \
             device_library inductor\n  device_library mos1\n  device_library resistor\n  \
             device_library diode\n  load osdi \"amp.osdi\" modules=(amp)\n  param p = 2i\n  \
             model u module params=(gain=@) module=amp\n  \
             element module_model u1 (a b 0) model=u params=(rin=@) mult=@\n  \
             model c capacitor\n  model l inductor\n  model m mos1 polarity={pol}\n  \
             model r resistor\n  model d diode params=(is=@)\n  \
             element resistor r1 (b 0) model=r value=@ tce=@\n  \
             element diode d1 (a b) model=d area=@ dtemp=@\n  \
             element capacitor c1 (a 0) model=c value=@ tc1=@\n  \
             element inductor l1 (a b) model=l value=@ scale=@\n  \
             element mosfet m1 (a b 0 0) model=m params=(w=@ l=@) mult=@\n",
            pol = ["n", "p"][seed % 2]
        );
        let mut text = String::new();
        for c in template.chars() {
            match c {
                '@' => text.push_str(v()),
                c => text.push(c),
            }
        }
        let (mut design, _) = read(&Sources::one("want.ir", &text), "ir");
        // the text runs under the reader's build's laws, which every read states (R90, R90a)
        design.builder().set_laws(spicerack::dialect::vacask::LAWS);
        let before = canonical(&design);
        let back = written(&mut design);
        let (again, _) = read(&back, "vacask");
        assert_eq!(
            canonical(&again),
            before,
            "seed {seed}:\n{}",
            back.files()[0].1
        );
        let kinds = [
            ir::ElemKind::ModuleModel,
            ir::ElemKind::Resistor,
            ir::ElemKind::Diode,
            ir::ElemKind::Capacitor,
            ir::ElemKind::Inductor,
            ir::ElemKind::Mosfet,
        ];
        let used = ir::features_used(&again);
        held += usize::from(kinds.iter().all(|&k| used.contains(ir::Feature::Elem(k))));
    }
    assert!(held >= 64, "designs holding all six kinds: {held}");
}
