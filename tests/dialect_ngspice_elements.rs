//! The ngspice element cards (sub-slice S3.1a): fixtures under
//! `tests/fixtures/dialect/ngspice/{read,write}/elements/` and the simulator round trip.
//!
//! - `read/elements/<clause>.sp` + `<clause>.ir`: a deck and the IR it reads to, written by hand
//!   from the oracle its second line names (the ngspice-45 source line, a run); the `.ir`'s
//!   `# codes:` line lists the notes in order. Refusal twins sit beside the forms (`*_refused`,
//!   `resistor_ignored`).
//! - `write/elements/<name>.ir` + `<name>.sp`: a design in IR text and the text it writes to.
//! - Every read fixture is a fixpoint of read → write → read (L6 on these decks).
//! - Simulator round trip (L9): a deck of every numeric card form, as written and after
//!   read → write, run on ngspice-45 (`nix develop .#testing`; a missing simulator fails), prints
//!   the same operating point. The oracle is ngspice on the original.
//!
//! Lists are compared by name, never by count.
//!
//! Contents: tests only.

use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The read fixtures, by name.
const READ: &[&str] = &[
    "bsource",
    "bsource_refused",
    "capacitor_inductor",
    "controlled",
    "controlled_m",
    "controlled_refused",
    "coupling",
    "defaults_duplicates",
    "diode_bjt",
    "instance",
    "lines",
    "mosfet_jfet_mesfet",
    "mosfet_soi3",
    "osdi_codemodel",
    "rcl_dynamic",
    "rcl_dynamic_refused",
    "resistor",
    "resistor_ignored",
    "semi_refused",
    "switches",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &["cards"];

/// Known failures, by fixture name, with why: the design reads (or is written) but uses a feature
/// ngspice's `FEATURES` lacks. Strict both ways (`known`): an unexpected pass fails until the
/// marker goes.
const KNOWN: &[(&str, &str)] = &[];

/// The simulator deck: every card form that carries a number and runs in an operating point.
const SIM: &str = "elements
.options tnom=50
.temp 77
V1 in 0 1
R17 a q {1k+0*v(a)} tc1=0.01
R18 q 0 1k
B3 vb 0 V=v(a) m=2
R19 vb 0 1k
R1 in a 1k
R2 a 0 r=2k tc1=1m m=2
C1 a 0 1p
L1 a b 1u
R3 b 0 1k
E1 e 0 a 0 2
R4 e 0 1k
G1 0 g a 0 1m m=3
R5 g 0 1k
F1 0 f vs 2 m=2
Vs b s 0
R11 s 0 1k
R12 f 0 1k
H1 h2 0 vs 1k
R13 h2 0 1k
E2 p 0 poly(2) a 0 b 0 0.1 1 2
R14 p 0 1k
B1 h 0 V=v(a)*3
R6 h 0 1k
B2 0 bi I={v(a)*1m} m=2
R15 bi 0 1k
E3 t 0 table {v(a)} = (0,0) (1,2)
R16 t 0 1k
D1 a d dmod area=2
R7 d 0 10k
.model dmod d is=1e-14
X1 a x sub m=2
.subckt sub p q
R1 p q 1k m=5
.ends
R8 x 0 1k
R9 a k {1k+v(a)}
R10 k 0 1k
.control
op
print v(a) v(b) v(e) v(g) v(f) v(h2) v(p) v(h) v(bi) v(t) v(d) v(x) v(k) v(q) v(vb)
.endc
.end
";

/// The nets `SIM` prints, in order.
const SIM_NETS: &[&str] = &[
    "a", "b", "e", "g", "f", "h2", "p", "h", "bi", "t", "d", "x", "k", "q", "vb",
];

/// A six-terminal SOI3 MOSFET (`soi3.c:30`), apart: its model does not converge beside `SIM`'s
/// `.temp`.
const SOI3: &str = "soi3
V1 in 0 1
Rd in d 100k
M1 d in 0 bg 0 t nsoi l=1u w=10u
Rt t 0 1meg
Rb bg 0 1meg
.model nsoi nsoi level=60 tb=1e-7 tof=1e-8 tob=1e-7
.control
op
print v(d) v(t)
.endc
.end
";

/// One card of every element form the reader types (A19's class check).
const BASES: &[&str] = &[
    "R1 a b 1k",
    "R2 a b {v(a)*1k}",
    "C1 a b 1p",
    "C2 a b q={1p*v(a)}",
    "L1 a b 1u",
    "K1 L1 L2 0.5",
    "E1 a b c d 2",
    "G1 a b c d 2",
    "F1 a b vx 2",
    "H1 a b vx 2",
    "E2 a b poly(1) c d 0 1",
    "E3 a b table {v(c)} = (0,0) (1,1)",
    "E4 a b vol={v(c)}",
    "G4 a b cur={v(c)}",
    "B1 a b V=v(c)",
    "B2 a b I=v(c)",
    "D1 a k dmod",
    "Q1 c b e qmod",
    "M1 d g s b nmod",
    "J1 d g s jmod",
    "Z1 d g s zmod",
    "S1 a b c d smod",
    "W1 a b vx wmod",
    "T1 a b c d z0=50 td=1n",
    "O1 a b c d omod",
    "U1 a b c umod",
    "Y1 a b c d ymod",
    "P1 a b 0 c d 0 pmod",
    "X1 a b sub",
    "N1 a b nmod",
    "A1 a b amod",
];

/// Every typed field's IR name (the detail rows' fields, `m`, `mult`, `temp`: A19's
/// `typed_key`), and every key the reader maps to a typed field.
const KEYS: &[&str] = &[
    "value",
    "ic",
    "tc1",
    "tc2",
    "tce",
    "temp_abs",
    "dtemp",
    "ac",
    "scale",
    "parasitics",
    "ic_mode",
    "noisy",
    "tc_base",
    "tc_law",
    "inductors",
    "k",
    "gain",
    "sense",
    "controls",
    "coeffs",
    "dimension",
    "output",
    "control",
    "input",
    "points",
    "delta",
    "law",
    "expr",
    "rpar",
    "area",
    "initial",
    "z0",
    "td",
    "freq",
    "nl",
    "length",
    "vel",
    "lumps",
    "m",
    "mult",
    "temp",
    "r",
    "resistance",
    "c",
    "cap",
    "capacitance",
    "l",
    "inductance",
    "tc",
    "noise",
    "coefficient",
    "zo",
    "f",
    "len",
    "n",
    "nt",
    "w",
    "bv_max",
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
        assert!(
            got.eq_ignoring_spans(&want),
            "{name}:\n{}\nwant:\n{}",
            dump(&got),
            dump(&want)
        );
    }
}

#[test]
fn each_read_fixture_is_a_fixpoint_of_read_write_read() {
    for name in READ {
        let path = format!("{name}.sp");
        let (first, _) = read_deck(&path, &text("read", name, "sp"));
        if known(name, &first) {
            continue;
        }
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
        if known(name, &design) {
            continue;
        }
        assert_eq!(write_entry(&design), text("write", name, "sp"), "{name}");
    }
}

#[test]
fn written_elements_run_on_ngspice_to_the_original_operating_point() {
    // Oracle: ngspice-45 on the deck as written; the written deck must print the same values.
    let (design, _) = read_deck("sim.sp", SIM);
    let written = write_entry(&design);
    assert_ne!(
        written, SIM,
        "the cards are written by the writer, not copied"
    );
    let want = ngspice_op(SIM, SIM_NETS);
    let got = ngspice_op(&written, SIM_NETS);
    assert_eq!(got.len(), SIM_NETS.len(), "{written}");
    for (k, net) in SIM_NETS.iter().enumerate() {
        let (w, g) = (want[k], got[k]);
        assert!(
            (w - g).abs() <= 1e-12 * w.abs().max(1e-12),
            "v({net}) = {g}, ngspice on the original: {w}\n{written}"
        );
    }
}

/// Whether `name` is a known failure; asserts it still fails (an unexpected pass fails).
fn known(name: &str, design: &ir::Design) -> bool {
    let Some((_, why)) = KNOWN.iter().find(|(n, _)| *n == name) else {
        return false;
    };
    let lacked = ir::features_used(design).minus(spicerack::dialect::ngspice::FEATURES);
    assert!(
        !lacked.is_empty(),
        "{name} passes: remove its KNOWN marker ({why})"
    );
    true
}

#[test]
fn a_six_terminal_soi3_card_runs_on_ngspice_after_the_round_trip() {
    // Oracle: ngspice-45 on the deck as written (soi3.c: six terminals; the four-node card
    // aborts this build in spbuild.c:267, so only the six-node form is run).
    let (design, _) = read_deck("soi3.sp", SOI3);
    let written = write_entry(&design);
    assert!(written.contains("M1 d in 0 bg 0 t nsoi"), "{written}");
    let nets = &["d", "t"];
    assert_eq!(
        ngspice_op(&written, nets),
        ngspice_op(SOI3, nets),
        "{written}"
    );
}

#[test]
fn no_card_reaches_a_builder_assert_and_every_read_checks_clean() {
    // Oracle: ir::check (A19: no parameter bears a typed field's name; every builder
    // invariant). Each element form with each typed key added once and twice reads without a
    // panic, and the design it reads to has no violation.
    for base in BASES {
        for key in KEYS {
            for card in [format!("{base} {key}=1"), format!("{base} {key}=1 {key}=2")] {
                let deck = format!("t\n{card}\n");
                let (design, _) = read_deck("t.sp", &deck);
                let mut diags = ir::Diags::new();
                ir::check(&design, &mut diags);
                assert!(diags.is_empty(), "{card}: {diags:?}");
            }
        }
    }
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

/// `tests/fixtures/dialect/ngspice/<sub>/elements/<name>.<ext>`.
fn text(sub: &str, name: &str, ext: &str) -> String {
    let p = dir(sub).join(format!("{name}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `tests/fixtures/dialect/ngspice/<sub>/elements`.
fn dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialect/ngspice")
        .join(sub)
        .join("elements")
}

/// The sorted names of the `<sub>/elements/*.<ext>` files.
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

/// The entry file legalized for and written by the ngspice row; no refusal, and the features
/// ngspice lacks named when there are some.
fn write_entry(design: &ir::Design) -> String {
    let lacked = ir::features_used(design).minus(spicerack::dialect::ngspice::FEATURES);
    let named: Vec<ir::Feature> = lacked.iter().collect();
    assert!(named.is_empty(), "lacked: {named:?}");
    let mut design = design.clone();
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut design, row(), &mut diags);
    let text = dialect::write(&design, design.entry(), row(), &mut diags);
    let refused: Vec<_> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    text
}

/// The IR text of a design's entry file.
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
    assert!(diags.is_empty(), "{diags:?}");
    design
}

/// The `print v(…)` values of `deck` run by `ngspice -b`, in `nets` order.
fn ngspice_op(deck: &str, nets: &[&str]) -> Vec<f64> {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("deck.cir");
    std::fs::write(&path, deck).expect("the deck is written");
    let out = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&path)
        .output()
        .expect("ngspice runs (nix develop .#testing provides it; unsupported fails)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    nets.iter()
        .map(|net| {
            let key = format!("v({net}) = ");
            let line = stdout
                .lines()
                .find_map(|l| l.trim().strip_prefix(&key))
                .unwrap_or_else(|| panic!("no {key} in ngspice's output:\n{stdout}"));
            line.trim().parse().expect("a number")
        })
        .collect()
}
