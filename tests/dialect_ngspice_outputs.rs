//! The ngspice output cards (`.save`, `.print`, `.plot`, `.meas`): fixtures under
//! `tests/fixtures/dialect/ngspice/{read,write}/outputs/`, the writer's refusals, and the
//! simulator round trip.
//!
//! - `read/outputs/<clause>.sp` + `<clause>.ir`: a deck and the IR it reads to, written by hand
//!   from the oracle its second line names (an ngspice-45 source line or a run); the `.ir`'s
//!   `# codes:` line lists the notes, in order. Each read fixture is also a fixpoint of
//!   read → write → read.
//! - `write/outputs/<name>.ir` + `<name>.sp`: a design in IR text and the text it writes, by
//!   hand.
//! - Refusals: an IR design whose output ngspice cannot spell is refused by `legalize`
//!   (`Write::admits`) with `W1350`.
//! - Simulator round trip: a deck of measures and its read → write text, both run on
//!   ngspice-45 (`nix develop .#testing`; a missing simulator fails), print the same values
//!   (ngspice is the oracle of its own dialect).
//!
//! Fixture lists are compared by name, never by count.
//!
//! Contents: tests only.

#[path = "common/memcap.rs"]
mod memcap;

use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The read fixtures, by name.
const READ: &[&str] = &[
    "meas_ac",
    "meas_dc_window",
    "meas_find",
    "meas_invalid",
    "meas_param",
    "meas_stat",
    "meas_trig_targ",
    "meas_unread",
    "meas_when",
    "plot_inert",
    "print_ac_parts",
    "print_invalid",
    "print_tran",
    "probe_unread",
    "save_all",
    "save_probes",
    "save_script",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &["measure", "print", "save", "save_script"];

/// IR designs `legalize` refuses for ngspice with `W1350`, each with why.
const REFUSED: &[(&str, &str)] = &[
    (
        "save of a difference (ngspice's .save v(a,b) keeps a and b)",
        "  save (vdiff (a) (b))",
    ),
    ("save of a part", "  save (v mag (a))"),
    (
        "F13: a save of an output the device has no vector for (settle device_output_missing.sp)",
        "  element isource i1 (a 0) dc=1m\n  save (all_v) (all_branch_i) (param (i1) (i))",
    ),
    ("print of an op table", "  print op (v (a))"),
    (
        "measure of an op analysis",
        "  measure m op find (v (a)) (at value=1n)",
    ),
    (
        "ac measure of the complex value",
        "  measure m ac find (v (a)) (at value=1k)",
    ),
    (
        "tran measure of a part",
        "  measure m tran find (v mag (a)) (at value=1n)",
    ),
    (
        "measure of a difference",
        "  measure m tran find (vdiff (a) (b)) (at value=1n)",
    ),
    (
        "threshold expression of the circuit",
        "  measure m tran when (cross rise (v (a)) value=(v(b) * 2) count=1)",
    ),
    (
        "ac threshold of the circuit (ngspice compares the real parts)",
        "  measure m ac when (cross rise (v re (a)) value=(v(b)) count=1)",
    ),
    (
        "dc window from high to low (ngspice swaps it)",
        "  measure m dc stat avg (v (a)) (at value=0.8) (at value=0.2)",
    ),
    (
        "delay outside tran",
        "  measure m dc when (cross rise (v (a)) value=1 count=1 delay=1)",
    ),
    (
        "crossing as a window bound",
        "  measure m tran stat avg (v (a)) (cross rise (v (b)) value=1 count=1) (bound)",
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
        assert!(got.eq_ignoring_spans(&want), "{name}: got\n{}", dump(&got));
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
fn outputs_ngspice_cannot_spell_are_refused() {
    // Oracle: the ngspice-45 forms of each card (save: gettoks splits v(a,b) into two saves,
    // dotcards.c; meas: com_measure2.c get_value, measure_parse_when, do_measure's analysis
    // check), so these IR rows have no ngspice text that reads back to them.
    for (why, item) in REFUSED {
        let mut design = parse_ir(&format!("file \"r.sp\" deck\n  title \"r\"\n{item}\n"));
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row(), &mut diags);
        let codes: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
        assert_eq!(codes, ["W1350"], "{why}");
    }
}

#[test]
fn measures_read_and_written_run_on_ngspice_to_the_original_values() {
    // Oracle: ngspice-45 on the original deck; the written deck must print the same values.
    let deck = "meas round trip
V1 a 0 sin(0 1 1meg)
R1 a b 1k
C1 b 0 100p
.param half=0.5
.tran 10n 3u
.meas tran tr trig v(a) val=0.2 rise=1 targ v(a) val=0.8 rise=1
.meas tran w1 when v(a)=0.5
.meas tran wl when v(a)=half fall=last
.meas tran wvb when v(a)=v(b) rise=2 td=1u
.meas tran f1 find v(b) when v(a)=0.5 cross=3
.meas tran f2 find v(b) at=1u
.meas tran av avg v(a) from=0.5u to=1.5u
.meas tran mx max v(b)
.meas tran mn min v(b) to=2u
.meas tran pp1 pp v(b) from=1u
.meas tran rm rms v(a)
.meas tran ig integ v(a) from=0 to=1u
.meas tran ib find i(v1) at=1u
.meas tran p1 param='tr*2'
.meas tran p2 param='av+half'
.meas tran w5 when v(a)=0.5 from=1u
.meas tran f4 find v(b) when v(a)=0.5 rise=2 to=2u
.meas tran mxa max_at v(b) from=1u
.meas tran t4 trig v(a) val=0.2 rise=1 from=1u targ v(b) val=0.5 fall=1
.end
";
    let (design, _) = read_deck("rt.sp", deck);
    let written = write_entry(&design);
    assert_ne!(written, deck, "the measures are written, not copied");
    let want = ngspice_measures(deck);
    assert_eq!(
        want.len(),
        19,
        "every measure of the original ran:\n{want:?}"
    );
    assert_eq!(ngspice_measures(&written), want, "{written}");
}

#[test]
fn ac_measures_read_and_written_run_on_ngspice_to_the_original_values() {
    // Oracle: ngspice-45 on the original deck (an ac v() is its real part, vp is radians).
    let deck = "meas ac round trip
V1 in 0 dc 0 ac 1
R1 in out 1k
C1 out 0 1u
.ac dec 10 1 1meg
.meas ac g find v(out) at=100
.meas ac gm max vdb(out)
.meas ac ph find vp(out) at=100
.meas ac bw when vdb(out)=-3 fall=1
.meas ac m find vm(out) when vr(out)=0.5
.meas ac b2 when vdb(out)=-3 from=100
.end
";
    let (design, _) = read_deck("ac.sp", deck);
    let written = write_entry(&design);
    assert_ne!(written, deck, "the measures are written, not copied");
    let want = ngspice_measures(deck);
    assert_eq!(
        want.len(),
        6,
        "every measure of the original ran:\n{want:?}"
    );
    assert_eq!(ngspice_measures(&written), want, "{written}");
}

#[test]
fn generated_outputs_the_writer_admits_read_back_as_themselves() {
    // Oracle: law L5 on this area's rows. Designs of one output item over this area's features
    // (ir::gen, valid by construction); each one `legalize` admits is written and read back,
    // and must mean the original (`eq_meaning`: a literal form ngspice has no letter for is
    // respelled, R28). Every item form must be admitted at least once.
    use ir::{ExprTag as T, Feature as F, ItemKind as K, RunExtra as X};
    let row = row();
    let set = [
        F::Item(K::Title),
        F::Item(K::Save),
        F::Run(X::SaveWholeRun),
        F::Item(K::Print),
        F::Item(K::Measure),
        F::Expr(T::Num),
        F::Expr(T::Binary),
        F::Expr(T::V),
        F::Expr(T::I),
        F::Run(X::ProbeVdiff),
        F::Run(X::ProbeI),
        F::Run(X::ProbeParam),
        F::Run(X::ProbeHierarchical),
        F::Run(X::PartRe),
        F::Run(X::PartMag),
        F::Run(X::PartPhaseRad),
        F::Run(X::PartDb),
        F::Run(X::MeasTrigTarg),
        F::Run(X::MeasWhen),
        F::Run(X::MeasFind),
        F::Run(X::MeasStat),
        F::Run(X::MeasParam),
        F::Run(X::MeasLast),
        F::Run(X::MeasDelay),
        F::Run(X::MeasCircuitThreshold),
        F::Run(X::ProbeAllV),
        F::Run(X::ProbeAllBranchI),
        F::Run(X::MeasStatAt),
        F::Run(X::MeasUntil),
        F::Run(X::MeasSkipInitial),
    ]
    .iter()
    .fold(ir::FeatureSet::EMPTY, |s, f| s.with(*f));
    let size = ir::r#gen::Size {
        items: 2,
        scope_depth: 0,
        expr_depth: 2,
        files: 1,
    };
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..80_000 {
        let mut design = ir::r#gen::generate(seed, &set, size);
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row, &mut diags);
        let titled = design
            .items(design.file(design.entry()).top)
            .first()
            .map(|i| i.kind)
            == Some(ir::ItemKind::Title);
        // a deck with no title reads back with an empty one (the structure area's rule, not
        // this area's)
        if !titled || diags.rows().iter().any(|r| dialect::refuses(r.code)) {
            continue;
        }
        let want = dump(&design);
        let text = dialect::write(&design, design.entry(), row, &mut diags);
        let path = design.str(design.file(design.entry()).path).to_string();
        let (back, _) = read_deck(&path, &text);
        assert!(
            back.eq_meaning(&design),
            "seed {seed}:\n{text}\n{want}\n{}",
            dump(&back)
        );
        for form in [
            "save ",
            "print ",
            "trig_targ ",
            "when ",
            "find ",
            "stat ",
            "param value",
            "skip_initial",
            "until=",
            "_at ",
        ] {
            if want.contains(form) {
                seen.insert(form);
            }
        }
    }
    let all = [
        "_at ",
        "find ",
        "param value",
        "print ",
        "save ",
        "skip_initial",
        "stat ",
        "trig_targ ",
        "until=",
        "when ",
    ];
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), all);
}

#[test]
fn dc_measures_read_and_written_run_on_ngspice_to_the_original_values() {
    // Oracle: ngspice-45 on the original deck (a dc window given high to low is swapped).
    let deck = "meas dc round trip
V1 a 0 0
R1 a b 1k
R2 b 0 1k
.dc v1 0 1 0.05
.meas dc s1 avg v(b) from=0.8 to=0.2
.meas dc s2 max v(b) to=0.5
.meas dc w when v(b)=0.3
.end
";
    let (design, _) = read_deck("dc.sp", deck);
    let written = write_entry(&design);
    assert_ne!(written, deck, "the measures are written, not copied");
    let want = ngspice_measures(deck);
    assert_eq!(
        want.len(),
        3,
        "every measure of the original ran:\n{want:?}"
    );
    assert_eq!(ngspice_measures(&written), want, "{written}");
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

/// The fixture directory's `<sub>/outputs/<name>.<ext>`.
fn text(sub: &str, name: &str, ext: &str) -> String {
    let p = dir(sub).join(format!("{name}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `tests/fixtures/dialect/ngspice/<sub>/outputs`.
fn dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialect/ngspice")
        .join(sub)
        .join("outputs")
}

/// The sorted names of the `<sub>/outputs/*.<ext>` files.
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

/// The entry file written by the ngspice row, after `legalize`; no refusal.
fn write_entry(design: &ir::Design) -> String {
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

/// A design's entry file as IR text.
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

/// The `name = value` lines of `ngspice -b deck`'s measurement block, in order.
fn ngspice_measures(deck: &str) -> Vec<(String, String)> {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("deck.cir");
    std::fs::write(&path, deck).expect("the deck is written");
    let out = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&path)
        .output()
        .expect("ngspice runs (nix develop .#testing provides it; unsupported fails)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .skip_while(|l| !l.contains("Measurements for"))
        .take_while(|l| !l.starts_with("Total"))
        .filter_map(|l| {
            let (name, value) = l.split_once('=')?;
            let value = value.split_whitespace().next()?;
            Some((name.trim().to_string(), value.to_string()))
        })
        .collect()
}
