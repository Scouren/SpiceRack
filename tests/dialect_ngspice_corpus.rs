//! The ngspice corpus gate (S3.5, first line): every `.cir` and `.sp` deck of the pinned
//! ngspice-45 source's `examples/` (`NGSPICE_EXAMPLES`, set by `nix develop .#testing`), each
//! through four steps in order, the first failing one named:
//!
//! - `read`: read with its includes (`Sources::load`), no panic (the reader asserts law L1:
//!   every statement accounted for);
//! - `check`: `ir::check` clean;
//! - `legalize`: `dialect::legalize` for ngspice leaves no `U0001` (every feature a plain
//!   ngspice deck produces is in ngspice's `FEATURES`);
//! - `round_trip`: written, read back, equal to the legalized design by `eq_meaning` (L5).
//!
//! Oracle: the decks are ngspice's own examples, so each step's expectation is ngspice reading
//! its own text. The failing decks are compared by name and step with [`KNOWN`], strictly both
//! ways. A missing corpus fails, never skips. Nightly tier: built with `--features corpus`.
//!
//! Contents: tests only.

#![cfg(feature = "corpus")]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use spicerack::dialect::{self, DialectRow, Sources};
use spicerack::ir;

/// The decks that fail today: (deck relative to `examples/`, step, why). Strict both ways.
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "Monte_Carlo/MC_ring_ts.sp",
        "round_trip",
        "control: a .control block with no .endc reads to the end of the file, so the written `.end` lands inside the Script opaque",
    ),
    (
        "Monte_Carlo/OpWien.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "TransmissionLines/cpl4_txl_2_line.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "cider/serial/meclgate.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "digital/compare/adder_bip.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "measure/buggy-meas-tran.sp",
        "round_trip",
        "outputs/control: a .measure kept as an Unread `.control\nmeas …` opaque is written as script commands and read back as a Measure",
    ),
    (
        "measure/meas_expression.cir",
        "round_trip",
        "outputs/control: as buggy-meas-tran.sp",
    ),
    (
        "measure/simple-meas-tran.sp",
        "round_trip",
        "control: a .control `tran` read as a run item reads back as a Script opaque; an Inert plot block is lost",
    ),
    (
        "memristor/memristor_x.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "numparam/param-example.cir",
        "round_trip",
        "sources: `V1 input 0 dc dcvoltage ac amplitude phase` reads back Unread",
    ),
    (
        "optran/HiPass3opamps_optran.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "osdi/bsimcmg/inverter_ro.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "p-to-n-examples/555-timer-2.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "p-to-n-examples/MOS1_out.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "probe/555-timer-2.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "tclspice/tcl-testbench2/example.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "tclspice/tcl-testbench3/FB14.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "tclspice/tcl-testbench4/example.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "transient-noise/noise_vnoi.cir",
        "round_trip",
        "outputs/control: as buggy-meas-tran.sp",
    ),
    (
        "transient-noise/shot_ng.cir",
        "round_trip",
        "outputs/control: as buggy-meas-tran.sp",
    ),
    (
        "various/integrate1.cir",
        "round_trip",
        "outputs/control: as buggy-meas-tran.sp",
    ),
    (
        "vdmos/100W.sp",
        "round_trip",
        "sources: `Vin N013 0 dc 0 ac 0 sin(0 V 1k)` is written in a form that reads back InvalidInSource",
    ),
    (
        "xspice/delay/analog-delay1.cir",
        "round_trip",
        "sources: `V1 in1 0 dc 0 sin(0 1 freq)` reads back Unread",
    ),
    (
        "xspice/see/CMOSOpAmp/CMOS-OP1.cir",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "xspice/table/table-generator-q-2d.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "xspice/table/table-model-bip-2d-1-oc.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "xspice/table/table-model-bip-2d-1.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
    (
        "xspice/table/table-model-bip-2d-2.sp",
        "legalize",
        "Extra(CaseSensitiveNames): the kernel's case_clash compares names across namespaces (an element `RS` and a diode key `rs`); the kernel narrows it to one namespace (lead, A25 follow-up)",
    ),
];

/// The step names, in order.
const STEPS: [&str; 4] = ["read", "check", "legalize", "round_trip"];

/// The corpus's decks at most (ngspice-45 has 419): a bound on the walk.
const DECKS_MAX: usize = 4096;

// ---- private ----

#[test]
fn every_example_deck_reads_checks_legalizes_and_round_trips() {
    let root = examples();
    let decks = decks(&root);
    assert!(!decks.is_empty(), "{}: no deck", root.display());
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let mut failing: Vec<(String, &str, String)> = Vec::new();
    let mut counts = [0usize; 5];
    for deck in &decks {
        let name = deck
            .strip_prefix(&root)
            .expect("a deck under the root")
            .to_string_lossy()
            .into_owned();
        match run(row, deck) {
            Ok(()) => counts[4] += 1,
            Err((step, why)) => {
                counts[STEPS.iter().position(|s| *s == step).expect("a step")] += 1;
                failing.push((name, step, why));
            }
        }
    }
    println!(
        "corpus: {} decks; first failing step: read {}, check {}, legalize {}, round_trip {}; passed {}",
        decks.len(),
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        counts[4]
    );
    let got: Vec<(&str, &str)> = failing.iter().map(|(n, s, _)| (n.as_str(), *s)).collect();
    let want: Vec<(&str, &str)> = KNOWN.iter().map(|(n, s, _)| (*n, *s)).collect();
    let why: Vec<String> = failing
        .iter()
        .map(|(n, s, w)| format!("{n} {s}: {w}"))
        .collect();
    assert_eq!(got, want, "failing decks against KNOWN\n{}", why.join("\n"));
}

/// `NGSPICE_EXAMPLES`: unsupported fails.
fn examples() -> PathBuf {
    let dir = std::env::var_os("NGSPICE_EXAMPLES")
        .expect("NGSPICE_EXAMPLES is set by `nix develop .#testing` (unsupported fails)");
    PathBuf::from(dir)
}

/// Every `.cir` and `.sp` file under `root`, sorted.
fn decks(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    // Bounded: each directory is entered once, and the decks are capped.
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("cir") || ext.eq_ignore_ascii_case("sp") {
                out.push(path);
            }
        }
        assert!(
            out.len() <= DECKS_MAX,
            "the corpus has at most DECKS_MAX decks"
        );
    }
    out.sort();
    out
}

/// The four steps on `deck`: the first failing step and why.
fn run(row: &DialectRow, deck: &Path) -> Result<(), (&'static str, String)> {
    let path = deck.to_string_lossy().into_owned();
    let reader = row.read.expect("ngspice reads");
    let src = Sources::load(&[&path], ir::Role::Deck, reader, &mut |p| {
        std::fs::read(p)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    });
    let read = catch_unwind(AssertUnwindSafe(|| {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        reader.read(&src, &mut design.builder(), &mut diags);
        design
    }));
    let design = read.map_err(|p| ("read", panic_text(&p)))?;
    let mut found = ir::Diags::new();
    ir::check(&design, &mut found);
    if !found.is_empty() {
        return Err(("check", codes(&found)));
    }
    let mut legal = design.clone();
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut legal, row, &mut diags);
    let lacked: Vec<String> = diags
        .rows()
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED)
        .map(|d| feature_name(d.arg))
        .collect();
    if !lacked.is_empty() {
        return Err(("legalize", lacked.join(" ")));
    }
    let trip = catch_unwind(AssertUnwindSafe(|| {
        let mut written = Sources::new();
        let entry = legal.entry();
        for f in std::iter::once(entry).chain(legal.files().filter(|&f| f != entry)) {
            let mut wd = ir::Diags::new();
            let text = dialect::write(&legal, f, row, &mut wd);
            written.add(legal.str(legal.file(f).path), &text);
        }
        written.set_form(ir::Role::Deck);
        let mut back = ir::Design::new();
        let mut bd = ir::Diags::new();
        reader.read(&written, &mut back.builder(), &mut bd);
        back.eq_meaning(&legal)
    }));
    match trip {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            "round_trip",
            "read(write(d)) differs by eq_meaning".to_string(),
        )),
        Err(p) => Err(("round_trip", panic_text(&p))),
    }
}

/// The codes of `diags`, joined.
fn codes(diags: &ir::Diags) -> String {
    let codes: Vec<String> = diags.rows().iter().map(|d| d.code.to_string()).collect();
    codes.join(" ")
}

/// A feature by its index, as `Debug` prints it.
fn feature_name(index: u32) -> String {
    ir::FeatureSet::ALL
        .iter()
        .find(|f| u32::from(f.index()) == index)
        .map_or_else(|| format!("#{index}"), |f| format!("{f:?}"))
}

/// A panic's message.
fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    let msg = p.downcast_ref::<String>().cloned();
    let msg = msg.or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()));
    format!("panic: {}", msg.unwrap_or_default())
}
