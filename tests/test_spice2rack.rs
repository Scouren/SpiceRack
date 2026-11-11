//! `spice2rack` run as a caller runs it (ESPice calls it to read every
//! dialect but ngspice's, ZERO D55): its exit code, what lands under `-o`
//! and what stdout holds. The decks are the hand-written PDK-like kit of
//! `tests/fixtures/pdk_sections/` (`models.lib` and the two files it
//! reaches) and a deck with a corner library written here; no simulator
//! runs. The same files against the cross-simulator harness's, which the
//! simulators do run, are `tests/test_cross_sim/kit.rs`.
//!
//! Cites: ZERO-LEDGER P1-062 (the CLI's contract).
//!
//! Contents: none (tests only).

use std::path::{Path, PathBuf};
use std::process::Command;

// ---- private ----

/// `spice2rack <args>` run in `cwd`: its exit code, stdout and stderr.
fn run(cwd: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_spice2rack"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("spice2rack runs");
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    (o.status.code(), text(&o.stdout), text(&o.stderr))
}

/// The files under `dir`, each by its path from there, sorted.
fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).expect("a directory").flatten() {
            let p = e.path();
            match p.is_dir() {
                true => todo.push(p),
                false => {
                    let rel = p.strip_prefix(dir).expect("under dir");
                    out.push(rel.to_string_lossy().into_owned());
                }
            }
        }
    }
    out.sort();
    out
}

/// `tests/fixtures/pdk_sections/`.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdk_sections")
}

/// The include cards of a written SPICE file: `.include p` and
/// `.lib p section` (a `.lib name` alone opens a section).
fn include_cards(text: &str) -> Vec<&str> {
    let card = |l: &&str| {
        let w: Vec<&str> = l.split_whitespace().collect();
        matches!(
            (w.first(), w.len()),
            (Some(&".include"), 2..) | (Some(&".lib"), 3..)
        )
    };
    text.lines().filter(card).collect()
}

/// A deck and the corner library beside it, under `dir/src`: the deck's
/// path. ESPice writes both; LTspice writes the deck and refuses the
/// library (its `.lib` reads a file and has no sections: W0012).
fn corner_kit(dir: &Path) -> PathBuf {
    let src = dir.join("src");
    std::fs::create_dir_all(&src).expect("a folder");
    let deck = [
        "a deck and its corner library",
        "V1 a 0 1",
        "R1 a 0 {rload}",
        ".lib 'corners.lib' tt",
        ".op",
        ".end",
    ];
    let lib = ["* corners", ".lib tt", ".param rload=1k", ".endl tt"];
    std::fs::write(src.join("top.sp"), deck.join("\n") + "\n").expect("the deck");
    std::fs::write(src.join("corners.lib"), lib.join("\n") + "\n").expect("the library");
    src.join("top.sp")
}

#[test]
fn p1_062_to_writes_one_dialect_and_prints_its_path() {
    // Oracle: a metamorphic relation with the run that has no `--to`: a
    // dialect's file holds the same bytes either way, and a dialect that
    // run skips is one `--to` exits 1 on, writing nothing. The fixture is
    // written for ESPice and refused for LTspice (its sections: W0012;
    // golden record fx18, `fixtures/tests/fixtures/pdk_sections/models.lib/`),
    // so both outcomes are met. Checks: 6 dialects.
    let deck = fixtures().join("models.lib");
    let deck = deck.to_str().expect("a UTF-8 path");
    let every = tempfile::TempDir::new().expect("a temp dir");
    let (code, stdout, stderr) = run(every.path(), &["-o", ".", deck]);
    assert_eq!((code, stdout.as_str()), (Some(0), ""), "{stderr}");
    let all = files(every.path());
    assert!(all.iter().any(|f| f == "models.espice.sp"), "{all:?}");
    assert!(!all.iter().any(|f| f == "models.ltspice.cir"), "{all:?}");
    for (name, file) in [
        ("ngspice", "models.ngspice.sp"),
        ("hspice", "models.hspice.sp"),
        ("ltspice", "models.ltspice.cir"),
        ("spectre", "models.spectre.scs"),
        ("vacask", "models.vacask.sim"),
        ("espice", "models.espice.sp"),
    ] {
        let one = tempfile::TempDir::new().expect("a temp dir");
        // A relative `-o`: the printed path is absolute all the same.
        let (code, stdout, stderr) = run(one.path(), &["--to", name, "-o", "out", deck]);
        let out = one.path().join("out");
        if !all.iter().any(|f| f == file) {
            assert_eq!((code, stdout.as_str()), (Some(1), ""), "{name}: {stderr}");
            assert!(stderr.contains(" W0"), "{name}: its codes: {stderr}");
            assert!(files(&out).is_empty(), "{name}: nothing written");
            continue;
        }
        assert_eq!(code, Some(0), "{name}: {stderr}");
        let printed = Path::new(stdout.strip_suffix('\n').expect("one line"));
        assert!(printed.is_absolute(), "{name}: {stdout}");
        let real = |p: &Path| std::fs::canonicalize(p).expect("a written file");
        assert_eq!(real(printed), real(&out.join(file)), "{name}: {stdout}");
        assert_eq!(files(&out), [file], "{name}: that file alone");
        let same = std::fs::read(printed).ok() == std::fs::read(every.path().join(file)).ok();
        assert!(same, "{name}: the bytes of the run with no --to");
    }
}

#[test]
fn p1_062_a_kit_names_each_file_by_its_absolute_path_under_the_output() {
    // Oracle: the contract (ZERO-LEDGER P1-062; espice-c9's guard): the
    // deck and each file it reaches are written under the output
    // directory, and every include path in them is absolute and names the
    // copy there, so a run from the deck's own directory cannot reach the
    // unconverted `res.lib` beside it. The entry's four include cards are
    // the golden record's (fx18, `fixtures/tests/fixtures/pdk_sections/
    // models.lib/espice.txt`: `.include shared.spice`, `.lib res.lib
    // res_tt`, and the same in `ff`), their paths rewritten by hand.
    let tmp = tempfile::TempDir::new().expect("a temp dir");
    let out = tmp.path().join("kit");
    let o = out.to_str().expect("a UTF-8 path");
    // The deck by a relative path, run from its own directory.
    let args = ["--to", "espice", "--kit", "-o", o, "models.lib"];
    let (code, stdout, stderr) = run(&fixtures(), &args);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(stdout, format!("{o}/models.espice.sp\n"), "the entry alone");
    assert_eq!(files(&out), ["models.espice.sp", "res.lib", "shared.spice"]);
    let read = |name: &str| std::fs::read_to_string(out.join(name)).expect("a written file");
    let entry = read("models.espice.sp");
    let want = [
        format!(".include {o}/shared.spice"),
        format!(".lib {o}/res.lib res_tt"),
        format!(".include {o}/shared.spice"),
        format!(".lib {o}/res.lib res_ff"),
    ];
    assert_eq!(include_cards(&entry), want);
    // The libraries are written ones, not copies: each under its own name
    // as title, the original's comment line gone.
    let res = read("res.lib");
    assert_eq!(res.lines().next(), Some("* res"), "{res}");
    assert!(res.contains("\n.param rsh=100\n"), "{res}");
    assert_eq!(read("shared.spice").lines().next(), Some("* shared"));
    assert!(include_cards(&res).is_empty(), "{res}");
}

#[test]
fn p1_062_a_kit_with_a_refused_library_exits_1_and_writes_nothing() {
    // Oracle: the contract (exit 1 on any refusal, nothing silently
    // dropped) and LTspice's `.lib`, which reads a file: a library's
    // section is refused (W0012; fx18, `fixtures/tests/fixtures/
    // pdk_sections/res.lib/ltspice.refused`) while the deck's own `.lib
    // file section` call is written (fx18, `spice-bench/cmcqa/gf180/
    // gf180_cv_cgc_nmos_3p3_0.sp/ltspice.txt`). The refusal is the
    // library's alone (stderr names the file), and it refuses the kit.
    let tmp = tempfile::TempDir::new().expect("a temp dir");
    let deck = corner_kit(tmp.path());
    let deck = deck.to_str().expect("a UTF-8 path");
    let out = tmp.path().join("out");
    let o = out.to_str().expect("a UTF-8 path");
    let args = ["--to", "ltspice", "--kit", "-o", o, deck];
    let (code, stdout, stderr) = run(tmp.path(), &args);
    assert_eq!((code, stdout.as_str()), (Some(1), ""), "{stderr}");
    assert!(stderr.contains(" W0012 "), "{stderr}");
    assert!(stderr.contains("corners.lib: Ltspice refused"), "{stderr}");
    assert!(!stderr.contains("top.sp: Ltspice refused"), "{stderr}");
    assert!(files(&out).is_empty(), "nothing written");
    // ESPice writes the same kit.
    let args = ["--to", "espice", "--kit", "-o", o, deck];
    let (code, stdout, stderr) = run(tmp.path(), &args);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(stdout, format!("{o}/top.espice.sp\n"));
    assert_eq!(files(&out), ["corners.lib", "top.espice.sp"]);
    let entry = std::fs::read_to_string(out.join("top.espice.sp")).expect("the entry");
    assert_eq!(include_cards(&entry), [format!(".lib {o}/corners.lib tt")]);
}

#[test]
fn p1_062_a_kit_is_never_written_over_the_files_the_deck_reads() {
    // Oracle: the sources' bytes before and after. With the deck's own
    // directory as `-o`, the library's copy would land on the library.
    let tmp = tempfile::TempDir::new().expect("a temp dir");
    let deck = corner_kit(tmp.path());
    let src = deck.parent().expect("a folder").to_path_buf();
    let before = (
        std::fs::read(&deck).ok(),
        std::fs::read(src.join("corners.lib")).ok(),
    );
    let (deck, o) = (deck.to_str().expect("UTF-8"), src.to_str().expect("UTF-8"));
    let (code, stdout, stderr) = run(tmp.path(), &["--to", "espice", "--kit", "-o", o, deck]);
    assert_eq!((code, stdout.as_str()), (Some(1), ""), "{stderr}");
    assert!(stderr.contains("corners.lib"), "{stderr}");
    assert_eq!(files(&src), ["corners.lib", "top.sp"], "nothing written");
    let after = (
        std::fs::read(deck).ok(),
        std::fs::read(src.join("corners.lib")).ok(),
    );
    assert_eq!(before, after);
}

#[test]
fn p1_062_a_usage_error_exits_2() {
    // Oracle: the contract's exit codes (2 on usage), one row a misuse;
    // nothing is written and nothing printed on stdout.
    let tmp = tempfile::TempDir::new().expect("a temp dir");
    let cases: [&[&str]; 8] = [
        &[],
        &["--to", "espice"],
        &["--to", "pspice", "deck.sp"],
        &["--kit", "deck.sp"],
        &["--to", "espice", "--kit", "--check", "deck.sp"],
        &["--dialect", "cdl", "deck.sp"],
        &["--bogus", "deck.sp"],
        &["deck.sp", "-o"],
    ];
    for args in cases {
        let (code, stdout, stderr) = run(tmp.path(), args);
        assert_eq!((code, stdout.as_str()), (Some(2), ""), "{args:?}: {stderr}");
        assert!(stderr.contains("usage: spice2rack"), "{args:?}: {stderr}");
        assert!(files(tmp.path()).is_empty(), "{args:?}: nothing written");
    }
}
