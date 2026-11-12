//! The VACASK sources area (S4.2, `rewrite/DIALECT-VACASK.md` §3.2 sources … mutual,
//! tline_ideal): reader fixtures (`tests/fixtures/dialect/vacask/read/sources/<clause>.sim` and
//! `.ir`, the IR written by hand from the clause or run each header names), writer fixtures
//! (`…/write/sources/<name>.ir` and `.sim`), the fixpoint of read, write, read on every reader
//! fixture, and the settle decks `…/settle/src_*.sim`: what VACASK printed or simulated for each
//! rule (the oracle: VACASK 36be560 itself, R5; a wave against VD's closed form at every time
//! point the run wrote). The simulator round trip of the settle decks is the scaffold's
//! (`dialect_vacask::write::the_settle_decks_run_the_same_after_read_and_write`).
//!
//! Unsupported fails, never skips: without `vacask` the settle tests fail; run them in
//! `nix develop .#testing`.
//!
//! Contents: tests only.

use std::path::{Path, PathBuf};
use std::process::Command;

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;
use spicerack::rawfile::parse_raw;
use spicerack::result::RawData;

/// The reader fixtures, relative to the crate root.
const READ: &str = "tests/fixtures/dialect/vacask/read/sources";
/// The writer fixtures.
const WRITE: &str = "tests/fixtures/dialect/vacask/write/sources";
/// The settle decks.
const SETTLE: &str = "tests/fixtures/dialect/vacask/settle";

/// Known failures, by fixture and cause; a fixture that passes while listed fails the test.
const KNOWN: &[(&str, &str)] = &[(
    "tline.sim",
    "A20: the ideal-line family and LosslessLine's model master wait for ir-circuit",
)];

/// Marker: the `model` statements are the devices area's (S4.1), not read on this branch yet;
/// while false, `model` rows and the `Unread` opaques of `model` lines (and their codes) are
/// left out of both sides of a comparison. Goes the day vc-devices' builtin models merge.
const MODEL_LINES_READ: bool = false;

// ---- private ----

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

/// `src` read by `row`, leniently.
fn read(src: &Sources, row: &'static str) -> (ir::Design, ir::Diags) {
    let row = dialect::by_name(row).expect("a dialect row");
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut design, &mut diags);
    (design, diags)
}

/// The deck at `path` as sources.
fn deck(path: &Path) -> Sources {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    Sources::one(&name, &std::fs::read_to_string(path).unwrap())
}

/// Whether a canonical line is a `model` row or a `model` line's `Unread` opaque.
fn model_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("model ") || t.starts_with("opaque vacask unread \"model ")
}

/// The canonical IR text of every file of `design` (without model lines, per the marker).
fn canonical(design: &ir::Design) -> String {
    let mut out = String::new();
    for f in design.files() {
        ir::dump(design, f, &mut out);
    }
    match MODEL_LINES_READ {
        true => out,
        false => out
            .lines()
            .filter(|l| !model_line(l))
            .map(|l| format!("{l}\n"))
            .collect(),
    }
}

/// The codes of `diags`, as `P2001`-style words, sorted; per the marker without one `P2004`
/// and one `S0001` per model line of `text`.
fn codes(diags: &ir::Diags, text: &str) -> Vec<String> {
    let mut out: Vec<String> = diags
        .rows()
        .iter()
        .map(|d| format!("{}{:04}", d.code.letter as char, d.code.number))
        .collect();
    if !MODEL_LINES_READ {
        let models = text
            .lines()
            .filter(|l| l.trim_start().starts_with("model "))
            .count();
        for code in ["P2004", "S0001"] {
            for _ in 0..models {
                let k = out
                    .iter()
                    .position(|c| c == code)
                    .expect("a model line's code");
                out.remove(k);
            }
        }
    }
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

/// `design` legalized for VACASK and each file written, as sources under the same paths.
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

/// One VACASK run of settle deck `name`: what it printed and the directory it wrote in.
fn vacask(name: &str) -> (String, tempfile::TempDir) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(SETTLE)
        .join(name);
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::copy(&src, dir.path().join(name)).expect("the settle deck");
    let done = Command::new("vacask")
        .arg(name)
        .current_dir(dir.path())
        .output()
        .expect("vacask not runnable: run in `nix develop .#testing`");
    let mut out = String::from_utf8_lossy(&done.stdout).into_owned();
    out.push_str(&String::from_utf8_lossy(&done.stderr));
    (out, dir)
}

/// The plot of raw file `name` in `dir`.
fn raw(dir: &tempfile::TempDir, name: &str, printed: &str) -> RawData {
    let bytes = std::fs::read(dir.path().join(name))
        .unwrap_or_else(|_| panic!("no {name}; the run said:\n{printed}"));
    parse_raw(&bytes).expect("a raw file")
}

/// Column `var` of `plot`.
fn column<'p>(plot: &'p RawData, var: &str) -> &'p [f64] {
    let k = plot
        .variables
        .iter()
        .position(|v| v.name == var)
        .unwrap_or_else(|| panic!("no vector {var}"));
    &plot.real_data[k]
}

/// The largest |v(t) − f(t)| of node `var` over every time point of `plot` (a transient).
fn worst(plot: &RawData, var: &str, f: impl Fn(f64) -> f64) -> f64 {
    let (t, v) = (&plot.real_data[0], column(plot, var));
    t.iter()
        .zip(v)
        .map(|(&t, &v)| (v - f(t)).abs())
        .fold(0.0, f64::max)
}

/// The operating point of node `var` (the one point of `op1.raw`).
fn op(plot: &RawData, var: &str) -> f64 {
    column(plot, var)[0]
}

/// The degrees `d` in radians.
fn rad(d: f64) -> f64 {
    d.to_radians()
}

/// A trapezoid of VACASK's pulse law at `t` (`fall <= 0`: no falling edge; `period <= 0`:
/// one pulse), VD dev-builtin-src.
fn pulse(t: f64, p: [f64; 7]) -> f64 {
    let [v0, v1, delay, rise, fall, width, period] = p;
    let mut x = t - delay;
    if x < 0.0 {
        x = 0.0;
    }
    if period > 0.0 {
        x %= period;
    }
    if x < rise {
        return v0 + (v1 - v0) * x / rise;
    }
    if fall <= 0.0 || x < rise + width {
        return v1;
    }
    if x < rise + width + fall {
        return v1 + (v0 - v1) * (x - rise - width) / fall;
    }
    v0
}

#[test]
fn every_reader_fixture_reads_to_its_ir() {
    let mut failed = Vec::new();
    for path in files(READ, "sim") {
        let ir_path = path.with_extension("ir");
        let want_text = std::fs::read_to_string(&ir_path).expect("a .ir beside the deck");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        // A panic (a want the IR cannot hold yet, a reader assertion) is a failure too.
        let compared = std::panic::catch_unwind(|| {
            let (want, _) = read(&Sources::one("want.ir", &want_text), "ir");
            let (got, diags) = read(&deck(&path), "vacask");
            let mut bad = Vec::new();
            if canonical(&got) != canonical(&want) {
                bad.push(format!(
                    "--- got\n{}--- want\n{}",
                    canonical(&got),
                    canonical(&want)
                ));
            }
            if codes(&diags, &text) != expected_codes(&want_text) {
                bad.push(format!("codes {:?}", codes(&diags, &text)));
            }
            bad
        });
        let bad = compared.unwrap_or_else(|_| vec!["panicked".to_string()]);
        let known = KNOWN.iter().any(|(k, _)| *k == name);
        match (bad.is_empty(), known) {
            (true, true) => failed.push(format!("{name}: passes; drop its KNOWN line")),
            (false, false) => failed.push(format!("{name}:\n{}", bad.join("\n"))),
            _ => {}
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn every_reader_fixture_is_a_fixpoint_of_write_and_read() {
    for path in files(READ, "sim") {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if KNOWN.iter().any(|(k, _)| *k == name) {
            continue;
        }
        let (mut once, _) = read(&deck(&path), "vacask");
        let before = canonical(&once);
        let (twice, _) = read(&written(&mut once), "vacask");
        assert_eq!(canonical(&twice), before, "{name}");
    }
}

#[test]
fn every_writer_fixture_writes_its_text() {
    let mut failed = Vec::new();
    for ir_path in files(WRITE, "ir") {
        let want = std::fs::read_to_string(ir_path.with_extension("sim")).expect("a .sim beside");
        let ir_text = std::fs::read_to_string(&ir_path).unwrap();
        let ir_text: String = match MODEL_LINES_READ {
            true => ir_text,
            false => ir_text
                .lines()
                .filter(|l| !model_line(l))
                .map(|l| format!("{l}\n"))
                .collect(),
        };
        let (mut design, _) = read(&Sources::one("sources.sim", &ir_text), "ir");
        let src = written(&mut design);
        let got = src.files()[0].1.clone();
        let (got, want) = match MODEL_LINES_READ {
            true => (got, want),
            false => {
                let strip = |s: &str| -> String {
                    s.lines()
                        .filter(|l| !l.starts_with("model "))
                        .map(|l| format!("{l}\n"))
                        .collect()
                };
                (strip(&got), strip(&want))
            }
        };
        if got != want {
            failed.push(format!(
                "{}:\n--- got\n{got}--- want\n{want}",
                ir_path.display()
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// Oracle: run src-dump (VACASK's own defaults of a bare vsource, a cccs and a mutual).
#[test]
fn settle_the_defaults_vacask_prints() {
    let (out, _dir) = vacask("src_dump.sim");
    for line in [
        "type = \"dc\" (string)",
        "rise = 1e-09 (real)",
        "fall = 0 (real)",
        "freq = 1000 (real)",
        "tdphase = 0 (real)",
        "phase = 0 (real)",
        "mag = 0 (real)",
        "modindex = 0.5 (real)",
        "break = \"all\" (string)",
        "slopeglob = 0.01 (real)",
        "scale = 1 (real)",
        "stretch = 1 (real)",
        "gain = 1 (real)",
        "ctlnode = \"flow(br)\" (string)",
        "$mfactor = 3 (real)",
        "k = 0 (real)",
        "ctlnode1 = \"flow(br)\" (string)",
    ] {
        assert!(out.contains(line), "no `{line}` in:\n{out}");
    }
}

/// Oracle: VD dev-builtin-src (Sine) and run src-sine: `dc` ignored (op 1 with dc=5), the
/// value at `delay` held before it (op 3), `phase` the AC phase only (0 at t = 0), damping.
#[test]
fn settle_sine() {
    let (out, dir) = vacask("src_sine.sim");
    let dc = raw(&dir, "op1.raw", &out);
    let ops: Vec<f64> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|n| op(&dc, n))
        .collect();
    let want = [1.0, 3.0, 0.0, 1.0, 3.0];
    for (g, w) in ops.iter().zip(want) {
        assert!((g - w).abs() < 1e-9, "op {ops:?}");
    }
    let tr = raw(&dir, "tr1.raw", &out);
    let sine = |o: f64, a: f64, f: f64, d: f64, th: f64, ph: f64| {
        move |t: f64| {
            let x = t.max(d) - d;
            o + a * (2.0 * std::f64::consts::PI * f * x + rad(ph)).sin() * (-th * x).exp()
        }
    };
    assert!(worst(&tr, "a", sine(1.0, 2.0, 1e3, 0.0, 0.0, 0.0)) < 1e-9);
    assert!(worst(&tr, "b", sine(1.0, 2.0, 1e3, 0.25e-3, 0.0, 90.0)) < 1e-9);
    assert!(worst(&tr, "c", sine(0.0, 1.0, 1e3, 0.0, 0.0, 0.0)) < 1e-9);
    assert!(worst(&tr, "d", sine(1.0, 2.0, 1e3, 0.25e-3, 1e3, 0.0)) < 1e-9);
    assert!(worst(&tr, "e", sine(1.0, 2.0, 1e3, -0.25e-3, 0.0, 0.0)) < 1e-9);
}

/// Oracle: VD dev-builtin-src (Pulse), runs p1, src-pulse: rise 1n by default, `period` 0 one
/// pulse, `fall` below 0 no falling edge, a negative delay, `dc` ignored (op = val0).
#[test]
fn settle_pulse() {
    let (out, dir) = vacask("src_pulse.sim");
    let dc = raw(&dir, "op1.raw", &out);
    assert_eq!(
        ["a", "b", "c", "d", "e"].map(|n| op(&dc, n)),
        [0.0, 0.0, 0.0, 0.5, 2.0]
    );
    let tr = raw(&dir, "tr1.raw", &out);
    let cases = [
        ("a", [0.0, 1.0, 0.0, 1e-9, 0.0, 0.0, 0.0]),
        ("b", [0.0, 1.0, 0.0, 1e-6, 1e-6, 1e-6, 0.0]),
        ("c", [0.0, 1.0, 0.0, 1e-6, -1.0, 1e-6, 4e-6]),
        ("d", [0.0, 1.0, -1e-6, 2e-6, 1e-6, 1e-6, 0.0]),
        ("e", [2.0, 3.0, 0.0, 1e-9, 0.0, 0.0, 0.0]),
    ];
    for (n, p) in cases {
        assert!(worst(&tr, n, |t| pulse(t, p)) < 1e-9, "{n}");
    }
}

/// Oracle: VD dev-builtin-src (Exp: td2 counted from delay) and run src-exp; the op the value
/// at t = 0 (a negative delay: 2 − (1 − e⁻¹)).
#[test]
fn settle_exp() {
    let (out, dir) = vacask("src_exp.sim");
    let dc = raw(&dir, "op1.raw", &out);
    assert!((op(&dc, "b") - (2.0 - (1.0 - (-1.0f64).exp()))).abs() < 1e-9);
    let tr = raw(&dir, "tr1.raw", &out);
    let exp = |v0: f64, v1: f64, d: f64, td2: f64, t1: f64, t2: f64| {
        move |t: f64| {
            if t < d {
                return v0;
            }
            let mut r = v0 + (v1 - v0) * (1.0 - (-(t - d) / t1).exp());
            if t >= d + td2 {
                r += (v0 - v1) * (1.0 - (-(t - d - td2) / t2).exp());
            }
            r
        }
    };
    assert!(worst(&tr, "a", exp(0.0, 1.0, 1e-6, 2e-6, 1e-6, 0.5e-6)) < 1e-9);
    assert!(worst(&tr, "b", exp(2.0, 1.0, -1e-6, 1e-6, 1e-6, 1e-6)) < 1e-9);
}

/// Oracle: run src-pwl: the first value held before the first point, `delay`, and VACASK's
/// periodic law (the line through (xN, yN) and (period + x1, y1) on [xN, period) and [0, x1)
/// of every period, at the wrapped τ; before `delay` the value at τ = 0).
#[test]
fn settle_pwl() {
    let (out, dir) = vacask("src_pwl.sim");
    let dc = raw(&dir, "op1.raw", &out);
    let ops = ["a", "b", "c", "d", "e"].map(|n| op(&dc, n));
    let want = [1.0, 1.0, 0.0, 13.0 / 3.0, 13.0 / 3.0];
    for (g, w) in ops.iter().zip(want) {
        assert!((g - w).abs() < 1e-9, "op {ops:?}");
    }
    let tr = raw(&dir, "tr1.raw", &out);
    let lin =
        |x: f64, (x0, y0): (f64, f64), (x1, y1): (f64, f64)| y0 + (y1 - y0) * (x - x0) / (x1 - x0);
    let once = |d: f64| {
        move |t: f64| {
            let x = t - d;
            match x {
                x if x < 1e-6 => 1.0,
                x if x < 2e-6 => lin(x, (1e-6, 1.0), (2e-6, 3.0)),
                _ => 3.0,
            }
        }
    };
    let periodic = |d: f64| {
        move |t: f64| {
            let x = (t - d).max(0.0) % 4e-6;
            match x {
                x if (1e-6..=2e-6).contains(&x) => lin(x, (1e-6, 1.0), (2e-6, 3.0)),
                _ => lin(x, (2e-6, 3.0), (5e-6, 1.0)),
            }
        }
    };
    let ramp = |t: f64| {
        let x = t % 2e-6;
        match x < 1e-6 {
            true => x / 1e-6,
            false => 1.0 - (x - 1e-6) / 1e-6,
        }
    };
    // The jumps (at each period start and at x1) fall between two time points of the run.
    let near_jump = |t: f64, d: f64| {
        let x = (t - d).rem_euclid(4e-6);
        x < 1e-8 || (x - 1e-6).abs() < 1e-8 || 4e-6 - x < 1e-8
    };
    let jumpless = |n: &str, d: f64, f: &dyn Fn(f64) -> f64| {
        let (t, v) = (&tr.real_data[0], column(&tr, n));
        t.iter()
            .zip(v)
            .filter(|(t, _)| !near_jump(**t, d))
            .map(|(&t, &v)| (v - f(t)).abs())
            .fold(0.0, f64::max)
    };
    assert!(worst(&tr, "a", once(0.0)) < 1e-9);
    assert!(worst(&tr, "b", once(1e-6)) < 1e-9);
    assert!(worst(&tr, "c", ramp) < 1e-9);
    assert!(jumpless("d", 0.0, &periodic(0.0)) < 1e-9);
    assert!(jumpless("e", 1e-6, &periodic(1e-6)) < 1e-9);
}

/// Oracle: VD dev-builtin-src (AM, FM formulas) and run src-amfm, every parameter off its
/// default, the value at `delay` held before it.
#[test]
fn settle_am_fm() {
    let (out, dir) = vacask("src_amfm.sim");
    let (o, a, fc, pc, d, fm, pm, mi) = (0.5, 2.0, 10e3, 30.0, 20e-6, 1e3, 45.0, 0.7);
    let tau = std::f64::consts::TAU;
    let am = |t: f64| {
        let x = t.max(d) - d;
        o + a * (tau * fc * x + rad(pc)).sin() * (1.0 + mi * (tau * fm * x + rad(pm)).sin())
    };
    let fmw = |t: f64| {
        let x = t.max(d) - d;
        o + a * (tau * fc * x + rad(pc) + mi * (tau * fm * x + rad(pm)).sin()).sin()
    };
    let dc = raw(&dir, "op1.raw", &out);
    assert!((op(&dc, "a") - am(0.0)).abs() < 1e-9);
    assert!((op(&dc, "b") - fmw(0.0)).abs() < 1e-9);
    let tr = raw(&dir, "tr1.raw", &out);
    assert!(worst(&tr, "a", am) < 1e-9);
    assert!(worst(&tr, "b", fmw) < 1e-9);
}

/// Oracle: run t3 (R39, A20): the far end of a matched line first passes 0.08 V (five 50 Ω lines in parallel behind 50 Ω: 1/6 V) at its delay:
/// td=2n 2 ns; f=100M 10 ns (nl 1); f=100M nl=0.5 5 ns; neither: nl/f = 1 ms (none in 20 ns);
/// td and f: td.
#[test]
fn settle_tline_delays() {
    let (out, dir) = vacask("src_tline.sim");
    let tr = raw(&dir, "tr1.raw", &out);
    let arrival = |n: &str| {
        let (t, v) = (&tr.real_data[0], column(&tr, n));
        t.iter().zip(v).find(|(_, v)| **v > 0.08).map(|(t, _)| *t)
    };
    let near = |got: Option<f64>, want: f64| got.is_some_and(|t| (t - want).abs() < 2e-11);
    assert!(near(arrival("b"), 2e-9), "{:?}", arrival("b"));
    assert!(near(arrival("c"), 10e-9), "{:?}", arrival("c"));
    assert!(near(arrival("d"), 5e-9), "{:?}", arrival("d"));
    assert_eq!(arrival("e"), None);
    assert!(near(arrival("g"), 2e-9), "{:?}", arrival("g"));
}

/// Oracle: runs src-r-*, each VACASK's own message (the reader refuses each statement:
/// `refused.sim`).
#[test]
fn settle_what_vacask_refuses() {
    for (deck, message) in [
        (
            "src_r_rise0.sim",
            "Rise time of pulse transient must be grater than 0.",
        ),
        (
            "src_r_td2.sim",
            "Parameter td2 of exponential transient must be greater than 0.",
        ),
        (
            "src_r_freq0.sim",
            "Frequency of sinusoidal transient must be greater than 0.",
        ),
        ("src_r_foo.sim", "Parameter 'foo' not found."),
        ("src_r_ctlinst.sim", "Peer instance '' not found."),
        ("src_r_mutual.sim", "Peer instance '' not found."),
        ("src_r_mfactor.sim", "Parameter '$mfactor' not found."),
    ] {
        let (out, _dir) = vacask(deck);
        assert!(out.contains(message), "{deck}: no `{message}` in:\n{out}");
    }
}
