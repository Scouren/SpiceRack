//! The ngspice control area's fixtures (`tests/fixtures/dialect/ngspice/{read,write}/control/`)
//! and its simulator round trip.
//!
//! - `read/control/<clause>.sp` + `<clause>.ir`: a deck with a control script and the IR it
//!   reads to, written by hand from the oracle its second line names (the ngspice-45 source
//!   line, a row of `rewrite/S3-CONTROL.md`, a settle deck under `settle/`); the `.ir`'s
//!   `# codes:` line lists the notes the read gives, in order.
//! - `write/control/<name>.ir` + `<name>.sp`: a design in IR text and the ngspice text it writes
//!   to, by hand.
//! - Every read fixture is a fixpoint of read → write → read (law L6 on these decks).
//! - The simulator round trip: each deck of [`DECKS`] and its `write(read(deck))` run on
//!   ngspice-45 give the same vectors, byte for byte but the date lines: every plot the run
//!   makes, written by an instrumentation block the test appends (a script's plots never reach
//!   a `-r` raw file: `S3-CONTROL.md` O4). The oracle is ngspice on the original deck (R1: the
//!   writer spells the reader's digits, so the doubles are the same). ngspice comes from
//!   `nix develop .#testing`; a missing simulator fails.
//!
//! Fixture lists are compared by name, never by count.
//!
//! Contents: tests only.

use std::path::{Path, PathBuf};

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The read fixtures, by name.
const READ: &[&str] = &[
    "alter_forms",
    "alter_param_case",
    "alter_unread",
    "altermod",
    "alterparam_no_reset",
    "alterparam_reset",
    "analysis_commands",
    "continuation",
    "foreach_alterparam",
    "foreach_inert_var",
    "foreach_loop",
    "foreach_unrolled",
    "hash_line",
    "in_branch",
    "inert_commands",
    "meas_card_run",
    "meas_command",
    "meas_command_unread",
    "no_command",
    "option_commands",
    "osdi_late",
    "pre_osdi",
    "private_commands",
    "repeat",
    "reset_restores",
    "run_no_cards",
    "run_with_cards",
    "save_command",
    "save_later_unread",
    "script_effect",
    "script_flow",
    "script_set_var",
    "script_state_cards",
    "script_vector",
    "two_blocks",
    "unclosed",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &[
    "load",
    "load_then_deck",
    "opaque_pairs",
    "script_alter",
    "script_loop",
    "script_loop_unnamed",
    "script_mixed",
    "script_option",
    "script_param_loop",
    "script_reset",
];

/// The fixtures and decks that fail today, by name, with the test that fails and why. Strict
/// both ways: a listed one that passes fails the test until its line goes.
const PENDING: &[(&str, &str, &str)] = &[(
    "save_command",
    "fixpoint",
    "ng-outputs: a script save as Save { holds: FromHere } (A28) and its script spelling; today a WholeRun save, whose SaveWholeRun the row lacks",
)];

/// Decks whose vectors ngspice must reproduce after `write(read(deck))`. The element cards are
/// other areas' (copied as text while they are stubs); the script is this area's, some of it in
/// upper case so that the written text differs from the deck (ngspice folds case).
const DECKS: &[(&str, &str)] = &[
    (
        "alters",
        "alters\nV1 a 0 1\nR1 a b 1k\nR2 b 0 1k\nD1 b 0 dmod\n.model dmod d is=1e-14\n.control\nop\nALTER R1 2K\nAltermod dmod is=1e-10\nop\nalter v1 = 2\nop\n.endc\n.end\n",
    ),
    (
        "foreach",
        "foreach\nV1 a 0 1\nR1 a b 1k\nR2 b 0 1k\n.control\nforeach v 1k 2.2k 0.5\nalter r1 $v\nop\nend\nop\n.endc\n.end\n",
    ),
    (
        "param_loop",
        "param loop\n.param p=1k\nV1 a 0 1\nR1 a b 2k\nR2 b 0 {p}\n.control\nforeach val 1k 3k\nalterparam p=$val\nreset\nop\nend\nreset\nop\n.endc\n.end\n",
    ),
    (
        "options",
        "options\nV1 a 0 pulse(0 1 0 1u 1u 5u 10u)\nR1 a b 1k tc1=0.01\nC1 b 0 1n\n.control\noption reltol=1e-4\nset temp=60\ntran 0.1u 20u\nrepeat 2\nop\nend\n.endc\n.end\n",
    ),
];

// ---- private ----

#[test]
fn the_fixture_lists_are_the_files() {
    assert_eq!(names("read", "sp"), READ);
    assert_eq!(names("read", "ir"), READ);
    assert_eq!(names("write", "ir"), WRITE);
    assert_eq!(names("write", "sp"), WRITE);
}

#[test]
fn each_read_fixture_gives_the_codes_and_the_ir_it_lists() {
    let mut failed = Vec::new();
    for name in READ {
        let (got, diags) = read_deck(&format!("{name}.sp"), &text("read", name, "sp"));
        let codes: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
        let ir = text("read", name, "ir");
        let line = ir
            .lines()
            .find_map(|l| l.strip_prefix("# codes:"))
            .expect("a codes line");
        let want: Vec<&str> = line.split_whitespace().collect();
        let same = got.eq_ignoring_spans(&parse_ir(name, &ir));
        if codes != want || !same {
            eprintln!("{name}: codes {codes:?}, read as\n{}", dump(&got));
            failed.push(*name);
        }
    }
    assert_eq!(
        failed,
        pending("read"),
        "the failing fixtures are the pending ones"
    );
}

#[test]
fn each_read_fixture_is_a_fixpoint_of_read_write_read() {
    let mut failed = Vec::new();
    for name in READ {
        let path = format!("{name}.sp");
        let (first, _) = read_deck(&path, &text("read", name, "sp"));
        let mut legal = first.clone();
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut legal, row(), &mut diags);
        if diags.rows().iter().any(|d| dialect::refuses(d.code)) {
            eprintln!("{name}: the read is refused for writing: {diags:?}");
            failed.push(*name);
            continue;
        }
        let once = write_entry(&first);
        let (second, _) = read_deck(&path, &once);
        if !first.eq_ignoring_spans(&second) || write_entry(&second) != once {
            eprintln!("{name}: not a fixpoint; written as\n{once}");
            failed.push(*name);
        }
    }
    assert_eq!(
        failed,
        pending("fixpoint"),
        "the failing fixtures are the pending ones"
    );
}

#[test]
fn each_write_fixture_writes_to_its_text() {
    for name in WRITE {
        if pending("write").contains(name) {
            continue; // a writer's assertion cannot be caught by name: listed, not run
        }
        let design = parse_ir(name, &text("write", name, "ir"));
        assert_eq!(write_entry(&design), text("write", name, "sp"), "{name}");
    }
}

#[test]
fn each_write_fixture_reads_back_to_its_design() {
    for name in WRITE {
        if pending("write").contains(name) {
            continue;
        }
        let design = parse_ir(name, &text("write", name, "ir"));
        let (back, _) = read_deck(&format!("{name}.sp"), &text("write", name, "sp"));
        assert!(
            back.eq_ignoring_spans(&design),
            "{name}: read back as\n{}",
            dump(&back)
        );
    }
}

#[test]
fn a_written_deck_runs_on_ngspice_to_the_original_vectors() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("dialect_ngspice_control");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    for (name, deck) in DECKS {
        if pending("simulator").contains(name) {
            continue; // listed, not run: the writer asserts
        }
        let (design, _) = read_deck(&format!("{name}.sp"), deck);
        let written = write_entry(&design);
        assert_ne!(&written, deck, "{name}: the script is written, not copied");
        let want = raw(&dir, &format!("{name}_original"), deck);
        let got = raw(&dir, &format!("{name}_written"), &written);
        assert!(
            got == want,
            "{name}: the written deck runs to other vectors (both kept in {})\n{written}",
            dir.display()
        );
    }
}

#[test]
fn generated_script_plans_read_back_to_their_meaning() {
    // Oracle: law L5 (`read(write(x)) ≅ x`, `eq_meaning`) on designs this test builds from a
    // seed, with literal loop values, which the shared generator never draws (its loop values
    // are expressions, which ngspice's `foreach` cannot spell: W1402). Every loop form the
    // writer spells (a named and an unnamed KeepLast list sweep, nested), every alter target,
    // and bodies of analyses and alters. A design the writer refuses (W1401: an `alterparam`'s
    // `reset` would drop a live alter) is counted, not compared; the admitted count is pinned
    // above a floor so the property cannot go vacuous.
    let mut admitted = 0;
    let mut loops = 0;
    for seed in 0..256u64 {
        let text = plan_text(seed);
        let design = parse_ir("generated", &text);
        let mut legal = design.clone();
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut legal, row(), &mut diags);
        if diags.rows().iter().any(|d| dialect::refuses(d.code)) {
            continue;
        }
        let written = write_entry(&legal);
        let (back, _) = read_deck("g.sp", &written);
        assert!(
            back.eq_meaning(&design),
            "seed {seed}:\n{text}\nwritten as\n{written}\nread back as\n{}",
            dump(&back)
        );
        admitted += 1;
        loops += usize::from(text.contains("  loop "));
    }
    assert!(
        admitted >= 128 && loops >= 64,
        "{admitted} admitted, {loops} with a loop"
    );
}

#[test]
fn script_items_ngspice_cannot_spell_are_refused() {
    // Oracle: ngspice-45's forms. pre_osdi takes a path (inp.c:791-797) and the modules are the
    // binary's; a foreach word is no expression (control.c:462-466); an alter takes one number
    // (device.c ft_getpnames); alterparam applies at a reset that drops live alters
    // (runcoms2.c:172-240).
    const REFUSED: &[(&str, &str, &str)] = &[
        (
            "a load naming its modules",
            "  load osdi \"a.osdi\" modules=(m1)",
            "W1405",
        ),
        ("a Verilog-A load", "  load veriloga \"a.va\"", "W1404"),
        (
            "a loop over an expression",
            "  param p = 1\n  loop sweep keep_last (element_value r1) (list values=[p])\n    analysis op",
            "W1402",
        ),
        (
            "a parameter alter not right after a reset",
            "  param p = 1\n  alter (element_value r1) value=2k\n  alter (param p) value=3",
            "W1401",
        ),
        (
            "a parameter loop whose body does not start with a reset",
            "  param p = 1\n  loop sweep keep_last (param p) (list values=[2])\n    analysis op",
            "W1401",
        ),
        ("a partial reset", "  reset values saves", "W1406"),
    ];
    for (why, items, code) in REFUSED {
        let mut design = parse_ir(why, &format!("file \"r.sp\" deck\n{items}\n"));
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row(), &mut diags);
        let codes: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
        assert_eq!(codes, [*code], "{why}");
    }
}

/// A deck in IR text from `seed`: a parameter, then up to six run items, a loop holding up to
/// three (one nested loop at most).
fn plan_text(seed: u64) -> String {
    let mut r = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut next = |n: u64| {
        r = r
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (r >> 33) % n
    };
    const TARGETS: [&str; 5] = [
        "(element_value r1)",
        "(source_dc v1)",
        "(device_param m1 w)",
        "(model_param nch vth0)",
        "(param p)",
    ];
    const RESET: &str = "reset values options temp tnom saves";
    const VALUES: [&str; 6] = ["1k", "2.2k", "0.5", "10u", "-1", "3meg"];
    let mut out = String::from("file \"g.sp\" deck\n  title \"g\"\n  param p = 1k\n");
    let item = |depth: usize, out: &mut String, next: &mut dyn FnMut(u64) -> u64| {
        let pad = "  ".repeat(depth + 1);
        match next(3) {
            0 => out.push_str(&format!("{pad}analysis op\n")),
            1 => out.push_str(&format!(
                "{pad}analysis tran stop=5u print_step=1u cap=ngspice_rule\n"
            )),
            _ => {
                let t = TARGETS[next(5) as usize];
                if t == "(param p)" {
                    out.push_str(&format!("{pad}{RESET}\n"));
                }
                out.push_str(&format!(
                    "{pad}alter {t} value={}\n",
                    VALUES[next(6) as usize]
                ));
            }
        }
    };
    for _ in 0..1 + next(6) {
        if next(3) > 0 {
            item(0, &mut out, &mut next);
            continue;
        }
        let name = if next(2) == 0 { " name=v" } else { "" };
        let values: Vec<&str> = (0..1 + next(3)).map(|_| VALUES[next(6) as usize]).collect();
        let t = TARGETS[next(5) as usize];
        out.push_str(&format!(
            "  loop sweep keep_last{name} {t} (list values=[{}])\n",
            values.join(", ")
        ));
        if t == "(param p)" {
            out.push_str(&format!("    {RESET}\n"));
        }
        for _ in 0..1 + next(3) {
            if next(4) == 0 {
                let t = TARGETS[next(5) as usize];
                let lead = if t == "(param p)" {
                    format!("      {RESET}\n")
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "    loop sweep keep_last {t} (list values=[{}])\n{lead}      analysis op\n",
                    VALUES[next(6) as usize]
                ));
            } else {
                item(1, &mut out, &mut next);
            }
        }
    }
    out
}

/// The names `PENDING` lists for `test`, in list order.
fn pending(test: &str) -> Vec<&'static str> {
    PENDING
        .iter()
        .filter(|(_, t, _)| *t == test)
        .map(|(n, _, _)| *n)
        .collect()
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

/// The fixture directory's `<sub>/control/<name>.<ext>`.
fn text(sub: &str, name: &str, ext: &str) -> String {
    let p = dir()
        .join(sub)
        .join("control")
        .join(format!("{name}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `tests/fixtures/dialect/ngspice`.
fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dialect/ngspice")
}

/// The sorted names of the `<sub>/control/*.<ext>` files.
fn names(sub: &str, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir().join(sub).join("control"))
        .expect("the fixture directory")
        .filter_map(|e| {
            let p = e.expect("an entry").path();
            (p.extension()? == ext)
                .then(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))?
        })
        .collect();
    out.sort();
    out
}

/// One deck read by the ngspice row, leniently; no `X` row (the spine checks).
fn read_deck(path: &str, text: &str) -> (ir::Design, ir::Diags) {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    let src = Sources::one(path, text);
    dialect::read(&src, row(), Policy::Lenient, &mut design, &mut diags);
    assert!(
        !diags.rows().iter().any(|d| d.code.letter == b'X'),
        "{path}: {diags:?}"
    );
    (design, diags)
}

/// The entry file written by the ngspice row; no refusal.
fn write_entry(design: &ir::Design) -> String {
    let mut diags = ir::Diags::new();
    let text = dialect::write(design, design.entry(), row(), &mut diags);
    let refused: Vec<_> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    text
}

/// The entry file of `design` as IR text.
fn dump(design: &ir::Design) -> String {
    let mut out = String::new();
    ir::dump(design, design.entry(), &mut out);
    out
}

/// A design from IR text.
fn parse_ir(name: &str, text: &str) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{name}: {diags:?}");
    design
}

/// Every plot `deck` makes on ngspice-45 (`ngspice -b`), written to one raw file by a block
/// appended before `.end` (the test's instrumentation, never the writer's), without its
/// `Date:` lines. Asserts the run exits 0 and writes one.
fn raw(dir: &Path, stem: &str, deck: &str) -> Vec<u8> {
    let cir = dir.join(format!("{stem}.cir"));
    let out = dir.join(format!("{stem}.raw"));
    let _ = std::fs::remove_file(&out);
    let block = format!(
        ".control\nset appendwrite\nforeach p $plots\nstrcmp _t_const $p const\nif $_t_const ne 0\nsetplot $p\nwrite {}\nend\nend\n.endc\n",
        out.display()
    );
    let at = deck
        .rfind(".end\n")
        .expect("a deck ends with .end (the fixtures do)");
    std::fs::write(&cir, format!("{}{block}{}", &deck[..at], &deck[at..])).expect("the deck");
    let run = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&cir)
        .output()
        .expect("ngspice runs (nix develop .#testing)");
    assert!(
        run.status.success(),
        "{stem}: ngspice failed\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let bytes = std::fs::read(&out).unwrap_or_else(|e| panic!("{stem}: no raw file: {e}"));
    let mut kept = Vec::with_capacity(bytes.len());
    let mut at = 0;
    // Bounded: each step moves `at` forward.
    while at < bytes.len() {
        let line_start = at == 0 || bytes[at - 1] == b'\n';
        if line_start && bytes[at..].starts_with(b"Date:") {
            at += bytes[at..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len() - at, |k| k + 1);
            continue;
        }
        kept.push(bytes[at]);
        at += 1;
    }
    kept
}
