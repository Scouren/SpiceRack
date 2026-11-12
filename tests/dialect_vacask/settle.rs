//! The settle fixtures (`tests/fixtures/dialect/vacask/settle/`): each deck is a run
//! DIALECT-VACASK.md Appendix A records, and each test asserts what the binary printed then. A
//! reader or writer rule of the VACASK slice cites one of these runs; when a binary update
//! changes a fact, the test goes red before a rule quietly goes wrong.
//!
//! Oracle: VACASK 36be560 itself (its own dialect, R5), and ngspice-45 for the decks that
//! settle a pair (`q13.cir`, `sp2.cir`); closed forms where the appendix gives one.
//!
//! Contents: tests only.

use std::path::{Path, PathBuf};
use std::process::Command;

use spicerack::rawfile::parse_raw;
use spicerack::result::RawData;

/// The settle decks, relative to the crate root.
const DIR: &str = "tests/fixtures/dialect/vacask/settle";

/// One simulator run: what it printed (stdout then stderr) and the directory it wrote in.
struct Run {
    out: String,
    dir: tempfile::TempDir,
}

// ---- private ----

/// `deck` run by `program` (`vacask` or `ngspice -b`) in a fresh directory.
fn run(program: &str, deck: &str) -> Run {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR).join(deck);
    let dir = tempfile::tempdir().expect("a temporary directory");
    let local: PathBuf = dir.path().join(deck);
    std::fs::copy(&src, &local).expect("the settle deck");
    let mut cmd = Command::new(program);
    if program == "ngspice" {
        cmd.arg("-b");
    }
    let done = cmd
        .arg(deck)
        .current_dir(dir.path())
        .output()
        .unwrap_or_else(|e| panic!("{program} not runnable ({e}): run in `nix develop .#testing`"));
    let mut out = String::from_utf8_lossy(&done.stdout).into_owned();
    out.push_str(&String::from_utf8_lossy(&done.stderr));
    Run { out, dir }
}

/// The values a `print (…)` line printed after each label, in order.
fn printed(run: &Run, labels: &[&str]) -> Vec<f64> {
    let line = run
        .out
        .lines()
        .find(|l| l.starts_with(labels[0]))
        .unwrap_or_else(|| panic!("no `{}` line in:\n{}", labels[0], run.out));
    let words: Vec<&str> = line.split_whitespace().collect();
    labels
        .iter()
        .map(|label| {
            let k = words.iter().position(|w| w == label).expect("the label");
            words[k + 1].parse().expect("a number after the label")
        })
        .collect()
}

/// The `resistance` each `print instance(…)` dump shows, by instance name, in order.
fn resistances(run: &Run) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    let mut name = None;
    for line in run.out.lines() {
        if let Some(rest) = line.strip_prefix("Osdi device instance ") {
            name = rest.split_whitespace().next().map(str::to_string);
        } else if let Some(v) = line.trim().strip_prefix("resistance = ") {
            let v = v.split_whitespace().next().expect("a value");
            out.push((
                name.take().expect("a dump header"),
                v.parse().expect("a number"),
            ));
        }
    }
    out
}

/// Whether the run printed `message`.
fn says(run: &Run, message: &str) -> bool {
    run.out.contains(message)
}

/// The plot of raw file `name` the run wrote.
fn raw(run: &Run, name: &str) -> RawData {
    let bytes = std::fs::read(run.dir.path().join(name))
        .unwrap_or_else(|_| panic!("no {name}; the run said:\n{}", run.out));
    parse_raw(&bytes).expect("a raw file")
}

/// Column `var` of `plot` at the point nearest `x` on its axis.
fn at(plot: &RawData, var: &str, x: f64) -> f64 {
    let col = |n: &str| {
        plot.variables
            .iter()
            .position(|v| v.name == n)
            .unwrap_or_else(|| panic!("no vector {n}"))
    };
    let (axis, v) = (&plot.real_data[0], &plot.real_data[col(var)]);
    let k = (0..axis.len())
        .min_by(|&a, &b| (axis[a] - x).abs().total_cmp(&(axis[b] - x).abs()))
        .expect("a point");
    v[k]
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

#[test]
fn e1_functions_and_operators() {
    let r = run("vacask", "e1.sim");
    let got: Vec<f64> = resistances(&r).into_iter().map(|x| x.1).collect();
    // sgn(0)+10, int(-2.5)+10, integer, round(2.5), round(-2.5), (-2)**3+100, -2**2+10,
    // 2**3**2, fmod(-7,3)+10, 7^3+10, sgn(-0.0)+10, 1?0?4:8:16 +10, int(2.7)+10, floor(-2.5)+10
    let want = [
        11., 8., 8., 13., 7., 92., 14., 512., 9., 14., 11., 18., 12., 7.,
    ];
    assert_eq!(got, want, "{}", r.out);
}

#[test]
fn i1_integer_division() {
    let r = run("vacask", "i1.sim");
    let got: Vec<f64> = resistances(&r).into_iter().map(|x| x.1).collect();
    let want = [10., 10., 20., 7., 20., 25., 343.333, 343.333];
    assert!(
        got.iter().zip(want).all(|(g, w)| close(*g, w, 1e-5)),
        "{got:?}"
    );
    assert_eq!(got.len(), want.len());
}

#[test]
fn t3_the_type_follows_the_instances_value() {
    let r = run("vacask", "t3.sim");
    let got: Vec<f64> = resistances(&r).into_iter().map(|x| x.1).collect();
    assert_eq!(got, [11., 11.5], "{}", r.out);
}

#[test]
fn f1_sign_and_number_forms() {
    let r = run("vacask", "f1.sim");
    let labels = [
        "sign30", "sign3m0", "sign3m1", "lit2dot", "hex", "M", "x", "mil", "u100",
    ];
    let want = [3., 3., -3., 0.666667, 5., 1e6, 2e6, 2.54e-5, 1e-4];
    let got = printed(&r, &labels);
    assert!(
        got.iter().zip(want).all(|(g, w)| close(*g, w, 1e-5)),
        "{got:?}"
    );
}

#[test]
fn f2_f3_result_types() {
    let r = run("vacask", "f2.sim");
    let labels = [
        "integer", "int", "floor", "ceil", "round", "abs", "min", "minr", "sgn", "tern", "neg",
        "pow", "mul", "cmp",
    ];
    let want = [
        1.5, 1., 1.5, 1.5, 1.5, 1.5, 1., 1.5, 0., 1., -1., 1.5, 1., 0.,
    ];
    assert_eq!(printed(&r, &labels), want, "{}", r.out);
    let r = run("vacask", "f3.sim");
    let labels = [
        "min_ir", "min_ri", "max_ir", "max_ri", "hypot", "pow", "neg", "not", "and", "bitand",
        "shl", "add_ir", "sub", "ternr",
    ];
    let want = [
        1.5, 1.5, 1.5, 1.5, 2.5, 1.5, -1., 0., 0., 1., 1., 1.5, 1., 2.,
    ];
    assert_eq!(printed(&r, &labels), want, "{}", r.out);
}

#[test]
fn f4_sgn_keeps_its_operands_type_and_sign_is_real() {
    let r = run("vacask", "f4.sim");
    let labels = ["sgnr", "sgnmr", "sgni", "sgn0r", "signr", "signi", "signir"];
    let want = [0.5, -0.5, 0., 0.5, 1.5, 1.5, 1.5];
    assert_eq!(printed(&r, &labels), want, "{}", r.out);
}

#[test]
fn n_scoping_and_naming() {
    assert!(says(
        &run("vacask", "n1.sim"),
        "Variable or constant 'p' not defined."
    ));
    assert!(says(&run("vacask", "m1.sim"), "Master 'res' not found."));
    assert!(says(
        &run("vacask", "n2a.sim"),
        "Master 'vsource' not found."
    ));
    assert!(says(
        &run("vacask", "n2.sim"),
        "Parameter '$mfactor' not found."
    ));
    assert!(says(&run("vacask", "n3.sim"), "Parameter redefinition."));
    let r = run("vacask", "n4.sim");
    assert_eq!(
        resistances(&r),
        [("R1".into(), 1.), ("r1".into(), 2.)],
        "{}",
        r.out
    );
    let r = run("vacask", "n5.sim");
    assert_eq!(resistances(&r), [("r1".into(), 6.)], "{}", r.out);
}

#[test]
fn c_t_formals_and_dependent_parameters() {
    let r = run("vacask", "c1a.sim");
    let got: Vec<f64> = resistances(&r).into_iter().map(|x| x.1).collect();
    assert_eq!(got, [7., 7., 7.], "{}", r.out);
    assert!(says(&run("vacask", "c1b.sim"), "Parameter 'x' not found."));
    assert!(says(&run("vacask", "t2.sim"), "Parameter 'y' not found."));
    let r = run("vacask", "t1.sim");
    assert!(
        says(&r, "noisy = 0 (integer)") && says(&r, "type = 1 (integer)"),
        "{}",
        r.out
    );
}

#[test]
fn p_q13_pulse_law() {
    let r = run("vacask", "p1.sim");
    let plot = raw(&r, "tr.raw");
    let a: Vec<f64> = [2.5e-6, 3.5e-6, 4.5e-6]
        .iter()
        .map(|&t| at(&plot, "a", t))
        .collect();
    let b: Vec<f64> = [2.5e-6, 3.5e-6, 4.5e-6]
        .iter()
        .map(|&t| at(&plot, "b", t))
        .collect();
    // fall=0 holds val1 to the period's end; fall=1u falls (rise 1u, sampled 2.5 ns off).
    assert!(
        close(a[0], 1.0, 1e-9) && close(a[1], 1.0, 1e-9) && close(a[2], 0.497, 1e-2),
        "{a:?}"
    );
    assert!(close(b[0], 0.5, 1e-2) && b[1].abs() < 1e-9, "{b:?}");
    assert!(says(
        &run("vacask", "p2.sim"),
        "Rise time of pulse transient must be grater than 0."
    ));
    assert!(says(
        &run("vacask", "q13.sim"),
        "Period of pulse transient must be greater than rise+fall+width."
    ));
    let ng = run("ngspice", "q13.cir");
    for (m, want) in [("a1", 0.5), ("a2", 0.5), ("a3", 0.5), ("a4", 1.0)] {
        let line = ng
            .out
            .lines()
            .find(|l| l.starts_with(m))
            .expect("a measure");
        let v: f64 = line.split('=').nth(1).unwrap().trim().parse().unwrap();
        assert!(close(v, want, 1e-6), "{m} = {v}");
    }
}

#[test]
fn s1_a_sweep_restores_its_value() {
    let r = run("vacask", "s1.sim");
    let dc = raw(&r, "dc1.raw");
    assert_eq!(dc.real_data[0], [2., 3.]);
    assert!(
        close(at(&raw(&r, "op2.raw"), "a", 0.0), 1.0, 1e-12),
        "{}",
        r.out
    );
}

#[test]
fn g2_a_draw_outside_mc_is_its_nominal() {
    let r = run("vacask", "g2.sim");
    let got: Vec<f64> = resistances(&r).into_iter().map(|x| x.1).collect();
    assert_eq!(got, [5., 5.], "{}", r.out);
}

#[test]
fn tr1_lte_the_default_step_cap() {
    let r = run("vacask", "tr1.sim");
    for (file, cap) in [("tq.raw", 10e-3 / 50.0), ("tw.raw", 4e-3 / 50.0)] {
        let t = &raw(&r, file).real_data[0];
        let widest = t.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
        assert!(close(widest, cap, 1e-9), "{file}: {widest}");
    }
    for deck in ["lte35.sim", "lte7.sim"] {
        let r = run("vacask", deck);
        let plot = raw(&r, "lt.raw");
        let (t, v) = (&plot.real_data[0], &plot.real_data[2]);
        let worst = t
            .iter()
            .zip(v)
            .map(|(&t, &v)| (v - (1.0 - (-(t - 1e-9).max(0.0) / 1e-3).exp())).abs())
            .fold(0.0, f64::max);
        assert!(
            worst < 1e-2 && close(worst, 2.811e-4, 1e-2),
            "{deck}: {worst}"
        );
    }
}

#[test]
fn sp2_the_acsp_port_is_ngspices_port() {
    let r = run("vacask", "sp2.sim");
    let plot = raw(&r, "s2.raw");
    for s in &plot.complex_data[..4] {
        assert!(
            (s[0].re - 0.5).abs() < 1e-12 && s[0].im.abs() < 1e-12,
            "{s:?}"
        );
    }
    let ng = run("ngspice", "sp2.cir");
    for name in ["s_1_1", "s_2_1"] {
        let line = ng
            .out
            .lines()
            .find(|l| l.starts_with(name))
            .expect("a printed S");
        let re: f64 = line
            .split('=')
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!((re - 0.5).abs() < 1e-6, "{name} = {re}");
    }
}
