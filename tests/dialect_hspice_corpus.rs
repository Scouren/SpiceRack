//! The HSPICE same-dialect cell on the VAMS conformance suite's HSPICE decks (`$HSPICE_CORPUS`,
//! the flake's OpenFAT/VAMS-Conformance-Suite `tests/netlist`, every `.sp` whose path holds
//! `hspice`): each deck read by the hspice row and written back as HSPICE, through these steps
//! in order, the first failing one named:
//!
//! - `read`: read with its includes, no panic (the reader asserts law L1);
//! - `verdict`: the reader refuses (or reads an `InvalidInSource` statement) exactly when ESPice
//!   stops on the deck;
//! - `numbers`: the written deck runs on ESPice to the same printed tables and measures as the
//!   deck itself (1e-9 relative: the same simulator, numbers spelt differently; an `Unread`
//!   statement is written back as read);
//! - `unread`: the deck reads with no `Unread` statement.
//!
//! Oracle: ESPice 0.3.0 in HSPICE mode (`espice-hspice --tokenizer=hspice`), which is not HSPICE;
//! the suite's own expectations are `settle/conformance.py`'s. The failing decks are compared by
//! name and step with [`KNOWN`], strictly both ways. A missing corpus or simulator fails, never
//! skips. Nightly tier: built with `--features corpus`.
//!
//! Contents: tests only.

#![cfg(feature = "corpus")]

#[path = "common/memcap.rs"]
mod memcap;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The decks that fail today: (deck relative to the corpus, step, why). Strict both ways.
#[rustfmt::skip] // a table: one row per line
const KNOWN: &[(&str, &str, &str)] = &[
    ("hbac/two_poles_hspice.sp", "unread", HB),
    ("hbnoise/lti_rc_hspice.sp", "unread", HB),
    ("hbxf/two_poles_hspice.sp", "unread", HB),
    ("hspice/checks.sp", "unread", CHECK),
    ("hspice/connect.sp", "unread", "`.connect`: no rule yet (P4301)"),
    ("hspice/dcvolt_uic.sp", "unread", "`.dcvolt`: no rule yet (P4301)"),
    ("hspice/fft_hann.sp", "unread", "`.fft`: no rule yet (P4301)"),
    ("hspice/fft_kaiser.sp", "unread", "`.fft`: no rule yet (P4301)"),
    ("hspice/fft_meas.sp", "unread", MEAS),
    ("hspice/fft_rect.sp", "unread", "`.fft`: no rule yet (P4301)"),
    ("hspice/hblin_mixer.sp", "unread", HB),
    ("hspice/hblin_noise.sp", "unread", HB),
    ("hspice/laplace_source.sp", "unread", EFORM),
    ("hspice/lin_line.sp", "unread", LIN),
    ("hspice/lin_mixed_mode.sp", "unread", LIN),
    ("hspice/lin_pad.sp", "unread", LIN),
    ("hspice/lin_rc_noise.sp", "unread", LIN),
    ("hspice/load_ic.sp", "unread", "`.load`: no rule yet (P4301)"),
    ("hspice/match_diode.sp", "unread", MATCH),
    ("hspice/match_diode_is.sp", "unread", MATCH),
    ("hspice/meas_events.sp", "unread", MEAS),
    ("hspice/meas_forms.sp", "unread", MEAS),
    ("hspice/meas_lstb.sp", "unread", MEAS),
    ("hspice/meas_match.sp", "unread", MATCH),
    ("hspice/meas_ptdnoise.sp", "unread", MEAS),
    ("hspice/net_one_port.sp", "unread", "`.net` and its one-value V card: no rule yet (P4301, P4157)"),
    ("hspice/net_tpad_y.sp", "unread", "`.net` and its one-value V card: no rule yet (P4301, P4157)"),
    ("hspice/net_tpad_z.sp", "unread", "`.net` and its one-value V card: no rule yet (P4301, P4157)"),
    ("hspice/noise_ac_sweep.sp", "unread", "`.noise ov src inter` takes the `.ac` sweep: no rule yet (P4303)"),
    ("hspice/pattern_source.sp", "unread", "`.pat` and pattern sources: no rule yet (P4301, P4001)"),
    ("hspice/pole_source.sp", "unread", EFORM),
    ("hspice/pole_step.sp", "unread", EFORM),
    ("hspice/port_series_z0.sp", "unread", "the P element (a port): no rule yet (P4001)"),
    ("hspice/power.sp", "unread", "`.power` and its options: no rule yet (P4301, P4307)"),
    ("hspice/ptdnoise_diode.sp", "unread", PTD),
    ("hspice/ptdnoise_time_sweep.sp", "unread", PTD),
    ("hspice/pz_source.sp", "unread", "`.pz ov src`: no rule yet (P4303)"),
    ("hspice/s_element.sp", "unread", "the S element and `.model … s`: no rule yet (P4001, P4203)"),
    ("hspice/s_element_tran.sp", "unread", "the S element and `.model … s`: no rule yet (P4001, P4203)"),
    ("hspice/sample_rc.sp", "unread", "`.sample` and `.noise` forms: no rule yet (P4301, P4303)"),
    ("hspice/sample_rc_beta.sp", "unread", "`.sample` and `.noise` forms: no rule yet (P4301, P4303)"),
    ("hspice/sn_rc.sp", "unread", "`.sn`, `.snac`, `.snnoise`, `.snxf`: no rule yet (P4301)"),
    ("hspice/snac_rc.sp", "unread", "`.sn`, `.snac`, `.snnoise`, `.snxf`: no rule yet (P4301)"),
    ("hspice/snnoise_rc.sp", "unread", "`.sn`, `.snac`, `.snnoise`, `.snxf`: no rule yet (P4301)"),
    ("hspice/snxf_rc.sp", "unread", "`.sn`, `.snac`, `.snnoise`, `.snxf`: no rule yet (P4301)"),
    ("hspice/unknown_card.sp", "verdict", "`.stateye`: ESPice stops (unsupported card); HSPICE has the card (manual lead), so the reader keeps it Unread (P4301): ESPice's gap, not refused"),
    ("hspice/vec_stim.sp", "unread", "`.vec`: no rule yet (P4301)"),
    ("hspice/w_coupled_cpl.sp", "unread", "the W element and `.model … w`: no rule yet (P4001, P4203)"),
    ("hspice/w_element.sp", "unread", "the W element and `.model … w`: no rule yet (P4001, P4203)"),
    ("hspice/w_lossless_delay.sp", "unread", "the W element and `.model … w`: no rule yet (P4001, P4203)"),
    ("hspice/w_settle.sp", "unread", "the W element and `.model … w`: no rule yet (P4001, P4203)"),
    ("hspice/w_txl.sp", "unread", "the W element and `.model … w`: no rule yet (P4001, P4203)"),
    ("tran_noise/hspice_samples.sp", "unread", "`.trannoise`: no rule yet (P4301)"),
    ("tran_noise/hspice_sde_flicker.sp", "unread", "`.trannoise`: no rule yet (P4301)"),
    ("tran_noise/hspice_sde_rc.sp", "unread", "`.trannoise`: no rule yet (P4301)"),
];

// Why, per HSPICE construct: every one runs on ESPice; the written deck reruns to the same numbers
// (the Unread statement written back as read).
const HB: &str = "harmonic balance (`.hb`, `.hbac`, `.hbnoise`, `.hbxf`, `.hblin`) and its P ports: no IR member yet (P4301, P4001)";
const CHECK: &str = "`.check`, `.dout`, `.biaschk`: no rule yet (P4301)";
const EFORM: &str = "E and G `LAPLACE` and `POLE` forms: no rule yet (P4101)";
const LIN: &str = "`.lin` and its P ports: no rule yet (P4301, P4001)";
const MATCH: &str =
    "`.acmatch`, `.dcmatch`, `.dcsens` and their variation block: no rule yet (P4301, P4001)";
const MEAS: &str = "HSPICE `.measure` forms past the ngspice ones read (FFT, LSTB, PTDNOISE, events, spellings): no rule yet (P4350, P4351, P4307)";
const PTD: &str = "`.ptdnoise`: no rule yet (P4301)";

// ---- private ----

#[test]
fn every_hspice_conformance_deck_round_trips_on_espice() {
    let root = PathBuf::from(std::env::var("HSPICE_CORPUS").expect(
        "HSPICE_CORPUS (nix develop .#testing provides the VAMS conformance suite; unsupported fails)",
    ));
    let decks = decks(&root);
    assert!(
        decks.len() >= 80,
        "the suite's HSPICE decks ({})",
        decks.len()
    );
    let row = dialect::by_name("hspice").expect("the hspice row");
    let mut got: Vec<(String, &'static str)> = Vec::new();
    for deck in &decks {
        let name = deck
            .strip_prefix(&root)
            .expect("under the root")
            .display()
            .to_string();
        if let Err((step, why)) = run(row, deck) {
            eprintln!("{name}: {step}: {why}");
            got.push((name, step));
        }
    }
    let known: Vec<(String, &str)> = KNOWN.iter().map(|k| (k.0.to_string(), k.1)).collect();
    assert_eq!(got, known, "failing decks against KNOWN");
}

/// Every `.sp` under `root` whose path holds `hspice`, sorted.
fn decks(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    // Bounded: each directory is entered once.
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("a corpus directory") {
            let p = e.expect("an entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "sp")
                && p.strip_prefix(root)
                    .is_ok_and(|r| r.display().to_string().contains("hspice"))
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// The steps for one deck; the first failing step and why.
fn run(row: &DialectRow, deck: &Path) -> Result<(), (&'static str, String)> {
    let dir = deck.parent().expect("a deck's directory");
    let file = deck
        .file_name()
        .expect("a file")
        .to_string_lossy()
        .to_string();
    let read = catch_unwind(AssertUnwindSafe(|| {
        let src = Sources::load(
            &[&file],
            ir::Role::Deck,
            row.read.expect("a reader"),
            None,
            &mut |p| std::fs::read_to_string(dir.join(p)).ok(),
        );
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        dialect::read(&src, row, Policy::Lenient, &mut design, &mut diags);
        let written = dialect::transpile(&src, row, row, Policy::Lenient).map(|w| w.text);
        (design, diags, written)
    }));
    let (design, diags, written) = read.map_err(|_| ("read", "the reader panicked".to_string()))?;
    let codes: Vec<String> = diags.rows().iter().map(|d| d.code.to_string()).collect();
    let before = espice(dir, &file, None);
    let refused = written.is_err() || has(&design, ir::OpaqueClass::InvalidInSource);
    if refused != before.starts_with("rc=") {
        return Err((
            "verdict",
            format!("ESPice {}, read {codes:?}", head(&before)),
        ));
    }
    let Ok(written) = written else { return Ok(()) };
    if refused {
        return Ok(());
    }
    let after = espice(dir, &file, Some(&written));
    if !same(&before, &after) {
        return Err((
            "numbers",
            format!("before {} after {}", head(&before), head(&after)),
        ));
    }
    if has(&design, ir::OpaqueClass::Unread) {
        return Err(("unread", format!("{codes:?}")));
    }
    Ok(())
}

/// The first line of an output, for a message.
fn head(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// Whether `design` holds an `Opaque` of `class`.
fn has(design: &ir::Design, class: ir::OpaqueClass) -> bool {
    design.reachable().into_iter().any(|(item, _)| {
        item.kind == ir::ItemKind::Opaque
            && design.row::<ir::item::Opaque>(item.index).class == class
    })
}

/// ESPice's print table and measures for the deck `file` of `dir` (or `text` in its place), run
/// in a writable copy of `dir`; its exit code and first error when it fails.
fn espice(dir: &Path, file: &str, text: Option<&str>) -> String {
    let tmp = tempfile::tempdir().expect("a temp dir");
    copy(dir, tmp.path());
    if let Some(t) = text {
        std::fs::write(tmp.path().join(file), t).expect("the deck is written");
    }
    let out = std::process::Command::new("espice-hspice")
        .current_dir(tmp.path())
        .args([
            "--tokenizer=hspice",
            "--format=print",
            "-r",
            "out.txt",
            "--measures=m.json",
        ])
        .arg(file)
        .output()
        .expect("espice-hspice runs (nix develop .#testing provides it; unsupported fails)");
    let read = |f: &str| std::fs::read_to_string(tmp.path().join(f)).unwrap_or_default();
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err
            .lines()
            .find(|l| l.contains("rror"))
            .unwrap_or("")
            .to_string();
        return format!("rc={:?} {first}", out.status.code());
    }
    format!("{}\n{}", read("out.txt"), measures(&read("m.json")))
}

/// Copies `from` into `to`, recursively, writable.
fn copy(from: &Path, to: &Path) {
    let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
    // Bounded: each directory is entered once.
    while let Some((a, b)) = stack.pop() {
        std::fs::create_dir_all(&b).expect("a directory");
        for e in std::fs::read_dir(&a).expect("a directory") {
            let p = e.expect("an entry").path();
            let q = b.join(p.file_name().expect("a name"));
            if p.is_dir() {
                stack.push((p, q));
            } else {
                std::fs::write(&q, std::fs::read(&p).expect("a file")).expect("a copy");
            }
        }
    }
}

/// The `"name": …, "value": …` pairs of ESPice's measures JSON, one per line.
fn measures(json: &str) -> String {
    json.lines()
        .filter(|l| l.contains("\"name\""))
        .map(|l| {
            let field = |k: &str| {
                l.split(&format!("\"{k}\": ")).nth(1).map_or("", |r| {
                    r.split([',', '}']).next().unwrap_or("").trim_matches('"')
                })
            };
            format!("{} {}", field("name"), field("value"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether two ESPice outputs agree: the same words, numbers within 1e-9 relative.
fn same(a: &str, b: &str) -> bool {
    let (wa, wb): (Vec<&str>, Vec<&str>) = (
        a.split_whitespace().collect(),
        b.split_whitespace().collect(),
    );
    wa.len() == wb.len()
        && !a.starts_with("rc=")
        && wa
            .iter()
            .zip(&wb)
            .all(|(x, y)| match (x.parse::<f64>(), y.parse::<f64>()) {
                (Ok(p), Ok(q)) => (p - q).abs() <= 1e-9 * p.abs().max(q.abs()).max(1e-30),
                _ => x == y,
            })
}
