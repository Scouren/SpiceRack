//! Acme's oracles 4 and 5 (`DIALECT-PLUGIN.md` §8) beyond the inline
//! decks of `test_dialect_plugin.rs`.
//!
//! Oracles: a round trip on the pinned 200 decks of `$SPICE_BENCH`
//! (`tests/fixtures/dialects/spice-bench-200.txt`): `A1 = write(D, acme)`;
//! where `A1` is written, `write(read(A1, acme), acme) == A1`, byte for
//! byte; where it is refused, its codes are the pinned list
//! `tests/fixtures/dialects/acme-bench-refused.txt` (`SPICERACK_BLESS=1`
//! rewrites it), names compared. Real ngspice (in `.#testing`; missing, the
//! test fails): `write(read(write(D, acme), acme), ngspice)` gives `D`'s
//! own results on ngspice, within each analysis's tolerance.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/acme.rs"]
mod acme;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use num_complex::Complex64;
use spicerack::backend::Dialect;
use spicerack::result::RawData;
use spicerack::syntax::Lang;
use spicerack::syntax::def::register;
use spicerack::syntax::render::render_text;

/// Decks left out of the round trip, with the reason: each holds a
/// `.noise` card, which acme's hook writes as `.noise_acme`, text acme
/// does not read (no read hooks, §6.4), so its round trip is no fixpoint.
const HOOKED: [&str; 7] = [
    "cmcqa/hicum/hicum_noise_npn_full_ac_1.sp",
    "cmcqa/hicum/hicum_noise_npn_full_subtran_ac_1.sp",
    "cmcqa/psp/psp_asym_nmos_noise1_T27_2.sp",
    "cmcqa/psp/psp_asym_pmos_noise1_T27_0.sp",
    "cmcqa/psp/psp_sym_nmos_noise1_T100_2.sp",
    "cmcqa/psp/psp_sym_pmos_noise1_T100_0.sp",
    "cmcqa/psp/psp_sym_pmos_noise2_fntexc_T27_3.sp",
];

/// Oracle 5's decks (`test_dialects.rs`'s circuit, no Laplace source
/// acme refuses), one per analysis, and the relative tolerance each is
/// compared at: op, dc and ac solve the same linear system (1e-9, a few
/// ulps of printed precision); tran steps on the same deck, ngspice's
/// RELTOL (1e-3, manual §11.1.1) the analysis's own accuracy.
const RUNS: [(&str, &str, f64); 4] = [
    ("op", ".op\n", 1e-9),
    ("dc", ".dc V1 0 2 0.5\n", 1e-9),
    ("ac", ".ac dec 5 1 1meg\n", 1e-9),
    ("tran", ".tran 10u 1m\n", 1e-3),
];

/// The circuit of oracle 5: a parametrised subcircuit, a B source, a
/// charge capacitor and an RC low pass.
const CIRCUIT: &str = "\
* acme on ngspice
.param rv=2k
.subckt load n PARAMS: r=1k
R1 n 0 {r}
.ends load
V1 in 0 1 AC 1
R1 in mid {rv}
B1 b 0 V=2*v(mid)+1
R3 b 0 1k
X1 mid load r=2k
R2 in lp 1k
C1 lp 0 1u
C2 b 0 q={1u*v(b)}
";

// ---- private ----

fn acme() -> Lang {
    register(acme::acme_as("acme")).expect("acme registers")
}

/// `text` read in `from`, written for `to`: the text, or the refusal's
/// sorted codes, or `panicked`.
fn write(text: &str, from: Lang, to: Lang) -> Result<String, String> {
    let r = catch_unwind(AssertUnwindSafe(|| render_text(text, from, "deck", to)));
    match r {
        Ok(Ok(w)) => Ok(w.text),
        Ok(Err(e)) => {
            let mut codes: Vec<String> = e.diags.iter().map(|d| format!("{:?}", d.code)).collect();
            codes.sort();
            codes.dedup();
            Err(codes.join(" "))
        }
        Err(_) => Err("panicked".into()),
    }
}

/// The pinned sample: each deck's path under `$SPICE_BENCH` and its text.
fn bench() -> Vec<(String, String)> {
    let root = std::env::var("SPICE_BENCH").expect("$SPICE_BENCH (nix develop .#testing)");
    let list = include_str!("fixtures/dialects/spice-bench-200.txt");
    let rels: Vec<&str> = list.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(rels.len(), 200, "the pinned sample");
    let read = |rel: &str| {
        let bytes = std::fs::read(Path::new(&root).join(rel)).expect("a sampled deck");
        (rel.to_string(), spicerack::import::decode(&bytes))
    };
    rels.into_iter().map(read).collect()
}

/// `deck` run on ngspice (on PATH in `.#testing`, else the test fails).
fn run(deck: &str) -> RawData {
    let found = std::process::Command::new("ngspice")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    assert!(
        found,
        "ngspice not on PATH: run the tests in `nix develop .#testing`"
    );
    Dialect::Ngspice
        .run(deck)
        .unwrap_or_else(|e| panic!("deck failed: {e}\n{deck}"))
}

/// Whether `a` and `b` agree at relative tolerance `rel` (an absolute
/// floor of `rel` times the vector's largest magnitude).
fn agree(a: &[f64], b: &[f64], rel: f64) -> bool {
    let scale = a.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= rel * x.abs().max(scale))
}

/// A complex vector as its real parts, then its imaginary parts.
fn parts(v: &[Complex64]) -> Vec<f64> {
    v.iter()
        .map(|c| c.re)
        .chain(v.iter().map(|c| c.im))
        .collect()
}

#[test]
fn acme_round_trip_is_a_fixpoint_on_the_spice_bench_sample() {
    let acme = acme();
    let mut moved = Vec::new();
    let mut refused = Vec::new();
    for (rel, text) in bench() {
        let a1 = write(&text, Lang::NGSPICE, acme);
        if HOOKED.contains(&rel.as_str()) {
            let hooked = !matches!(&a1, Ok(a) if !a.contains(".noise_acme"));
            assert!(
                hooked,
                "{rel}: excluded as hooked, but written with no hooked card"
            );
            continue;
        }
        match a1 {
            Ok(a1) => {
                if write(&a1, acme, acme).as_ref() != Ok(&a1) {
                    moved.push(rel);
                }
            }
            Err(codes) => refused.push(format!("{rel}: {codes}")),
        }
    }
    assert_eq!(moved, Vec::<String>::new(), "not a fixpoint");
    let pin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialects/acme-bench-refused.txt");
    let got = refused.join("\n") + "\n";
    if std::env::var_os("SPICERACK_BLESS").is_some() {
        std::fs::write(&pin, &got).expect("written");
    }
    let want = std::fs::read_to_string(&pin).expect("the pinned refusals");
    assert_eq!(
        got.lines().collect::<Vec<_>>(),
        want.lines().collect::<Vec<_>>()
    );
}

#[test]
fn acme_round_trip_runs_on_ngspice_as_its_source() {
    let acme = acme();
    for (name, analysis, rel) in RUNS {
        let d = format!("{CIRCUIT}{analysis}.end\n");
        let a1 = write(&d, Lang::NGSPICE, acme).unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = write(&a1, acme, Lang::NGSPICE).unwrap_or_else(|e| panic!("{name}: {e}"));
        let want = run(&write(&d, Lang::NGSPICE, Lang::NGSPICE).expect("written"));
        let got = run(&back);
        assert_eq!(got.variables.len(), want.variables.len(), "{name}");
        for (k, v) in want.variables.iter().enumerate() {
            let (w, g) = match want.is_complex {
                true => (parts(&want.complex_data[k]), parts(&got.complex_data[k])),
                false => (want.real_data[k].clone(), got.real_data[k].clone()),
            };
            assert!(agree(&w, &g, rel), "{name}: {} {w:?} against {g:?}", v.name);
        }
    }
}
