//! The ngspice dialect's model cards (sub-slice S3.2): fixtures, the writer's refusals and the
//! simulator round trip.
//!
//! - `read/models/<clause>.sp` + `<clause>.ir`: a deck of `.model` cards and the IR it reads to,
//!   written by hand from the oracle its second line names (the ngspice-45 source line or a run);
//!   the `.ir`'s `# codes:` line lists the notes the read gives, in order. A refusal twin sits
//!   beside each clause that has an invalid form.
//! - `write/models/<name>.ir` + `<name>.sp`: a design in IR text and the ngspice text it writes
//!   to, by hand.
//! - Every read fixture is a fixpoint of read → write → read (law L6 on these decks).
//! - `Write::admits` (R21): a design ngspice cannot spell for a reason no feature states is
//!   refused with the code named (oracle: the ngspice-45 source line in each case's comment).
//! - The simulator round trip: each deck and its read → write image run on ngspice-45 and print
//!   the same value (L9; ngspice is the oracle of its own dialect). ngspice comes from
//!   `nix develop .#testing`; a missing simulator fails.
//!
//! Fixture lists are compared by name, never by count.
//!
//! Contents: tests only.

use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The read fixtures, by name.
const READ: &[&str] = &[
    "b3soi_dd",
    "b3soi_fd",
    "bins",
    "bjt",
    "branch",
    "bsim_version",
    "bsim_version_invalid",
    "code_module",
    "diode",
    "diode_level_invalid",
    "duplicate",
    "hisim2",
    "jfet_mes_hfet",
    "level_invalid",
    "local_model",
    "mos_levels",
    "params_ignored",
    "passives",
    "polarity_default",
    "unread_forms",
    "values",
    "vdmos",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &["families", "values"];

// ---- private ----

#[test]
fn the_fixture_lists_are_the_files() {
    assert_eq!(names("read", "sp"), READ);
    assert_eq!(names("read", "ir"), READ);
    assert_eq!(names("write", "ir"), WRITE);
    assert_eq!(names("write", "sp"), WRITE);
}

#[test]
fn each_read_fixture_gives_the_codes_its_ir_lists() {
    for name in READ {
        let (_, diags) = read_deck(&format!("{name}.sp"), &text("read", name, "sp"));
        let got: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
        let ir = text("read", name, "ir");
        let line = ir
            .lines()
            .find_map(|l| l.strip_prefix("# codes:"))
            .expect("a codes line");
        let want: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn each_read_fixture_reads_to_its_ir() {
    for name in READ {
        let (got, _) = read_deck(&format!("{name}.sp"), &text("read", name, "sp"));
        let want = parse_ir(&text("read", name, "ir"));
        assert!(got.eq_ignoring_spans(&want), "{name}: read\n{}", dump(&got));
    }
}

#[test]
fn each_read_fixture_is_a_fixpoint_of_read_write_read() {
    for name in READ {
        let path = format!("{name}.sp");
        let (first, _) = read_deck(&path, &text("read", name, "sp"));
        let once = write_entry(&first);
        let (second, _) = read_deck(&path, &once);
        assert!(first.eq_ignoring_spans(&second), "{name}:\n{once}");
        assert_eq!(write_entry(&second), once, "{name}: the text is a fixpoint");
    }
}

#[test]
fn each_write_fixture_writes_to_its_text() {
    for name in WRITE {
        let design = parse_ir(&text("write", name, "ir"));
        assert_eq!(write_entry(&design), text("write", name, "sp"), "{name}");
    }
}

#[test]
fn admits_refuses_what_ngspice_cannot_spell() {
    // (model line in IR text, the code `admits` gives). Oracles: inpdomod.c (the type words and
    // levels ngspice dispatches; BSIM4 versions 4.0-4.8 only, "Device type BSIM4 version %s not
    // available"), inpgmod.c create_model (a parameter the device's IFparm tables lack is
    // "unrecognized parameter - ignored": a silent wrong number), INPgetModBin (only BSIM3,
    // BSIM4, HiSIM2 and HiSIM-HV bin; a bin is `base.<digits>`).
    let cases = [
        // a family ngspice lacks is a feature its FEATURES lack (R32): check_legal's U row
        ("model a mos0 polarity=n", "U0001"),
        ("model a diode_pspice", "U0001"),
        ("model a other polarity=n word=foo", "U0001"),
        // MESA without its level reads back as MES (mesa.c:108, inpdomod.c nmf level 1)
        ("model a mesa polarity=n params=(vto=1)", "W1203"),
        ("model a bsim4 polarity=n version=\"4.9\"", "W1202"),
        ("model a hisim_hv polarity=n version=\"3.0\"", "W1202"),
        // a version is written as the vocabulary spells it (b4v5check.c:64: 4.5 is 4.5.0)
        ("model a bsim4 polarity=n version=\"4.5\"", "W1202"),
        // a module's version is no field ngspice has (its reader keeps `version=` a parameter)
        ("model a module module=m version=\"1.0\"", "W1202"),
        // the type word is a code model or an OSDI module by what ngspice loaded (inpdomod.c
        // default branch; run: `.model dd foo` is "Unknown model type foo - ignored"): a code
        // model the build lacks reads back as a module, a module named like a code model, a
        // device or a type word as those (L5 seed 9)
        ("model a code_model word=g50", "W1206"),
        ("model a module module=xfer", "W1206"),
        ("model a module module=bsim4", "W1206"),
        ("model a module module=nmos", "W1206"),
        ("model a mos1 polarity=n params=(vto=1 bogus=2)", "W1203"),
        ("model a.1 mos1 polarity=n bin_of=a", "W1204"),
        (
            "model b.1 bsim4 polarity=n bin_of=a version=\"4.8.3\"",
            "W1204",
        ),
    ];
    for (line, code) in cases {
        let mut design = parse_ir(&format!("file \"t.sp\" deck\n  title \"t\"\n  {line}\n"));
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row(), &mut diags);
        let got: Vec<String> = diags.rows().iter().map(|d| d.code.to_string()).collect();
        assert_eq!(got, [code], "{line}");
    }
    // and admits a card of each kind it spells
    let fine = "file \"t.sp\" deck\n  title \"t\"\n  model a mos1 polarity=n params=(vto=1 kp=2)\n  model b.1 bsim4 polarity=p params=(lmin=1 toxe=2) bin_of=b version=\"4.5.0\"\n  model c vbic polarity=n version=\"1.2\"\n  model d diode params=(m=0.3)\n";
    let mut design = parse_ir(fine);
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut design, row(), &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn written_models_run_on_ngspice_to_the_original_numbers() {
    // Oracle: ngspice-45 on the deck as written; the read → write image must print the same
    // value (the writer spells the reader's digits, so the doubles are the same). A silent
    // BSIM3, BSIM4 or B4SOI card is written with its device's default version (b3set.c:84,
    // b4set.c:205, b4soiset.c:124): the same numbers show the explicit default is the silent one.
    let cases: &[(&str, &str)] = &[
        (
            ".model mm nmos vto=0.5 kp=1e-4 lambda=0.02",
            "M1 d g 0 0 mm w=1u l=1u",
        ),
        (
            ".model mm nmos level=54 version=4.5 vth0=0.4 toxe=3n",
            "M1 d g 0 0 mm w=1u l=0.2u",
        ),
        (".model mm pmos level=14", "M1 0 g d d mm w=1u l=0.2u"),
        (".model mm nmos level=49", "M1 d g 0 0 mm w=1u l=0.5u"),
        (
            ".model mm nmos level=4.9e1 tox=4n",
            "M1 d g 0 0 mm w=1u l=0.5u",
        ),
        (".model q npn bf=80 is=1e-15 vaf=50", "Q1 d g 0 q"),
        (".model q npn level=9 is=1e-16", "Q1 d g 0 q"),
        (
            ".model dd d (level=3 is=1e-14 bv=0.5 ibv=1e-3 m=0.4)",
            "D1 0 d dd area=3",
        ),
        (".model mm vdmos pchan vto=-1 kp=1 ron=5", "M1 0 g d mm"),
        (".model jj njf level=2 beta=1e-3 vto=-1", "J1 d g 0 jj"),
        (".model rr res r=2k", "R2 d 0 rr"),
        (".model mz pmf level=2 vto=-1 beta=1e-3", "Z1 0 g d mz"),
        (
            ".model mm nmos vto={0.5+temper*1m} kp=1e-4\n.temp 127",
            "M1 d g 0 0 mm w=1u l=1u",
        ),
        (".model mm psoi level=10", "M1 d g 0 0 0 mm w=1u l=1u"),
        (
            ".model mm nmos level=1 vto=0.5 kp=1e-4 foo=3 kp=2e-4 m=2",
            "M1 d g 0 0 mm w=1u l=1u",
        ),
        (
            ".model mm.1 nmos level=54 lmin=0 lmax=2u wmin=0 wmax=2u vth0=0.3\n.model mm.2 nmos level=54 lmin=2u lmax=10u wmin=0 wmax=2u vth0=0.6",
            "M1 d g 0 0 mm w=1u l=5u",
        ),
        (
            ".param vt=0.45\n.model mm nmos vto={vt*2} kp=vt",
            "M1 d g 0 0 mm w=1u l=1u",
        ),
    ];
    for (cards, instance) in cases {
        let deck = format!(
            "probe\nVd d 0 1\nVg g 0 1\n{instance}\n{cards}\n.control\nop\nprint i(vd)\n.endc\n.end\n"
        );
        let (design, _) = read_deck("probe.sp", &deck);
        // the cards are written by the writer, not copied: every one is a model row
        let models = design
            .reachable()
            .into_iter()
            .filter(|(item, _)| item.kind == ir::ItemKind::Model)
            .count();
        assert_eq!(models, cards.matches(".model").count(), "{cards}");
        let written = write_entry(&design);
        let want = ngspice_print(&deck);
        assert_eq!(ngspice_print(&written), want, "{cards}\n---\n{written}");
    }
}

#[test]
fn code_models_of_the_build_round_trip_by_meaning() {
    // The covering property for `Family(CodeModel)` (the neutral generator does not know
    // ngspice's code-model names, R55): seeded designs of code models whose words are the
    // pinned build's (families.rs `CODE_MODELS`, a `devhelp` run), with and without parameters,
    // each with one A instance; written, read back, equal by meaning (oracle: the round trip).
    let source = include_str!("../src/dialect/ngspice/families.rs");
    let words: Vec<&str> = source
        .split("const CODE_MODELS: &str = \"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .expect("the code-model list")
        .split(|c: char| c.is_whitespace() || c == '\\')
        .filter(|w| !w.is_empty())
        .collect();
    assert!(words.len() > 50, "the list parsed: {words:?}");
    let (mut run, mut holding) = (0, 0);
    for seed in 0..64u64 {
        let mut state = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut next = |n: u64| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) % n
        };
        let mut ir = String::from("file \"p.sp\" deck\n  title \"p\"\n");
        let models = 1 + next(3);
        for k in 0..models {
            let word = words[next(words.len() as u64) as usize];
            let params: Vec<String> = (0..next(3))
                .map(|j| format!("p{j}={}.{}", next(100), next(10)))
                .collect();
            let params = if params.is_empty() {
                String::new()
            } else {
                format!(" params=({})", params.join(" "))
            };
            ir.push_str(&format!("  model m{k} code_model{params} word={word}\n"));
            ir.push_str(&format!("  element code_model a{k} (n{k} 0) model=m{k}\n"));
        }
        let mut design = parse_ir(&ir);
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row(), &mut diags);
        assert!(
            !diags.rows().iter().any(|d| dialect::refuses(d.code)),
            "seed {seed}: {diags:?}\n{ir}"
        );
        let text = write_entry(&design);
        let (back, _) = read_deck("p.sp", &text);
        assert!(
            back.eq_meaning(&design),
            "seed {seed}:\n{ir}\n{text}\n{}",
            dump(&back)
        );
        run += 1;
        holding += usize::from(ir.contains("code_model"));
    }
    // floors: every seed ran and held a code model
    assert!(run >= 64 && holding >= 64, "{run} run, {holding} holding");
}

#[test]
fn the_version_spellings_run_the_same_device() {
    // Settle decks for families::standard: each family's card in two spellings of one version
    // (or, where ngspice never reads the version again, two versions) prints the same
    // operating point on ngspice-45, and the reader stores the standard version both times.
    // (card head, spelling a, spelling b, the standard version, instance)
    let m4 = "M1 d g 0 0 mm w=1u l=0.5u";
    let soi = "M1 d g 0 0 0 mm w=1u l=1u";
    let cases: &[(&str, &str, &str, &str, &str)] = &[
        // b3v32set.c:86-92: 3.24 is 3.2.4, 3.20 is 3.2
        (
            "nmos level=49 tox=4n vth0=0.4",
            "version=3.24",
            "version=3.2.4",
            "3.2.4",
            m4,
        ),
        (
            "nmos level=49 tox=4n vth0=0.4",
            "version=3.20",
            "version=3.2",
            "3.2",
            m4,
        ),
        // b4v5check.c:64: 4.5 is 4.5.0; b4set.c:345: 4.81 is 4.8.1
        ("nmos level=54", "version=4.5", "version=4.5.0", "4.5.0", m4),
        (
            "nmos level=54",
            "version=4.81",
            "version=4.8.1",
            "4.8.1",
            m4,
        ),
        // hsmhv2set.c:136-150: 2.1 is reset to 2.20 (a warning); hsm2set.c:136: 281 to 280
        (
            "nmos level=73",
            "version=2.1",
            "version=2.20",
            "2.20",
            "M1 d g 0 0 mm w=10u l=10u",
        ),
        ("nmos level=68", "version=281", "version=280", "2.8.0", m4),
        // never read again (no use beyond *mpar.c/*mask.c): silently their one version
        (
            "npn level=8",
            "version=2.3",
            "version=2.4.0",
            "2.4.0",
            "Q1 d g 0 q",
        ),
        ("npn level=4", "vers=1.3", "vers=1.2", "1.2", "Q1 d g 0 q"),
        ("nmos level=10", "version=4.3", "version=4.4", "4.4.0", soi),
        ("nmos level=57", "version=2.2", "version=2.0", "2.2.3", soi),
    ];
    for (head, a, b, standard, instance) in cases {
        let name = if instance.starts_with('Q') { "q" } else { "mm" };
        let deck = |v: &str| {
            format!(
                "probe\n.model {name} {head} {v}\n{instance}\nVd d 0 1\nVg g 0 1\n.control\nop\nprint i(vd)\n.endc\n.end\n"
            )
        };
        assert_eq!(
            ngspice_print(&deck(a)),
            ngspice_print(&deck(b)),
            "{head} {a} / {b}"
        );
        for v in [a, b] {
            let (design, _) = read_deck("v.sp", &deck(v));
            let text = dump(&design);
            assert!(
                text.contains(&format!("version=\"{standard}\"")),
                "{head} {v}:\n{text}"
            );
        }
    }
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

/// The fixture directory's `<sub>/models/<name>.<ext>`.
fn text(sub: &str, name: &str, ext: &str) -> String {
    let p = dir(sub).join(format!("{name}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `tests/fixtures/dialect/ngspice/<sub>/models`.
fn dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialect/ngspice")
        .join(sub)
        .join("models")
}

/// The sorted names of the `<sub>/models/*.<ext>` files.
fn names(sub: &str, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir(sub))
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
fn parse_ir(text: &str) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{diags:?}\n{text}");
    design
}

/// The `i(vd) = …` line `ngspice -b` prints for `deck`.
fn ngspice_print(deck: &str) -> String {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("deck.cir");
    std::fs::write(&path, deck).expect("the deck is written");
    let out = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&path)
        // BSIM3 and B4SOI write their check logs into the working directory
        .current_dir(dir.path())
        .output()
        .expect("ngspice runs (nix develop .#testing provides it; unsupported fails)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .find(|l| l.trim().starts_with("i(vd) = "))
        .unwrap_or_else(|| panic!("no i(vd) in ngspice's output for\n{deck}\n{stdout}"))
        .trim()
        .to_string()
}
