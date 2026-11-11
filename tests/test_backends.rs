//! Raw files -> `spicerack::rawfile::parse_raw`: a truncated header is an
//! error, not a panic; a complete ngspice ASCII raw file reads back its
//! title, variables and values. LTspice itself runs a closed-form deck in
//! `nix develop .#testing-ltspice`.
//!
//! Cites: the ngspice raw file format (manual §17.6), as a spec example.
//!
//! Contents: none (tests only).

// ---- private ----

#[test]
fn test_rawfile_utf16_detection() {
    // Standard UTF-8 ngspice raw file should NOT be detected as UTF-16
    let raw_content = b"Title: test\nPlotname: Operating Point\n";
    let result = spicerack::rawfile::parse_raw(raw_content);
    // Should fail gracefully (incomplete data) but not panic
    assert!(result.is_err());
}

#[test]
fn test_rawfile_parse_existing_format() {
    // Verify our existing parser still works
    let raw_content = b"Title: test\n\
Plotname: Operating Point\n\
Flags: real\n\
No. Variables: 2\n\
No. Points: 1\n\
Variables:\n\
\t0\tv(out)\tvoltage\n\
\t1\tv(in)\tvoltage\n\
Values:\n\
0\t3.300000e+00\n\
\t1.000000e+00\n";

    let result = spicerack::rawfile::parse_raw(raw_content).unwrap();
    assert_eq!(result.title, "test");
    assert_eq!(result.variables.len(), 2);
    assert!((result.real_data[0][0] - 3.3).abs() < 1e-10);
}

#[test]
fn p1_001_rawfile_drops_xyce_second_plot() {
    // `Xyce -r p1-001.raw op-raw.cir` (Xyce 7.10, the corpus deck
    // Output/AC/op-raw.cir: `.op` + `.ac dec 10 100 1g`; tests/fixtures/raw/
    // README): two plots, the op and the 71-point ac, the second's header
    // starting at `Plotname:` with no `Title:`. R1 10 ohm, V1 0 V.
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/raw/p1-001.raw"
    ))
    .expect("the fixture");
    let plots = spicerack::rawfile::parse_raw_all(&data).expect("parsed");
    let names: Vec<&str> = plots.iter().map(|p| p.plot_name.as_str()).collect();
    assert_eq!(names, ["DC operating point", "AC Analysis"]);
    assert_eq!(plots[1].complex_data[0].len(), 71);
    assert_eq!(plots[1].complex_data[0][0].re, 100.0);
}

#[test]
fn p1_006_ltspice_raws_read_with_no_bom() {
    // LTspice 26.1.1 under Wine on tests/fixtures/raw/p1-006-<a>.cir.txt, one
    // analysis each (README): a 2 V source over a 1k/1k divider, AC 1. Its
    // raw header is UTF-16LE with no byte-order mark; a transient's vectors
    // past time are float32. Closed forms: v(out) = 1 V (op, tran), 0, 0.5,
    // 1 V over the dc sweep 0..2 V, 0.5 (ac, the noise gain, the tf).
    let read = |a: &str| {
        let p = format!(
            "{}/tests/fixtures/raw/p1-006-{a}.raw",
            env!("CARGO_MANIFEST_DIR")
        );
        let plots = spicerack::rawfile::parse_raw_all(&std::fs::read(p).expect("fixture"))
            .unwrap_or_else(|e| panic!("{a}: {e}"));
        assert_eq!(plots.len(), 1, "{a}");
        plots.into_iter().next().expect("one plot")
    };
    let col = |r: &spicerack::result::RawData, n: &str| {
        let i = r
            .variables
            .iter()
            .position(|v| v.name == n)
            .unwrap_or_else(|| panic!("{n}"));
        r.re(i).into_owned()
    };
    let close = |a: f64, b: f64| assert!((a - b).abs() <= 1e-6 * b.abs().max(1.0), "{a} vs {b}");
    close(col(&read("op"), "V(out)")[0], 1.0);
    let dc = col(&read("dc"), "V(out)");
    for (v, w) in dc.iter().zip([0.0, 0.5, 1.0]) {
        close(*v, w);
    }
    assert_eq!(dc.len(), 3);
    let ac = read("ac");
    let i = ac
        .variables
        .iter()
        .position(|v| v.name == "V(out)")
        .expect("v(out)");
    close(ac.complex_data[i][0].re, 0.5);
    let tran = read("tran");
    let (t, v) = (col(&tran, "time"), col(&tran, "V(out)"));
    close(*t.last().expect("points"), 1e-3);
    v.iter().for_each(|&x| close(x, 1.0));
    close(col(&read("noise"), "gain")[0], 0.5);
    let tf = read("tf");
    close(tf.re(0)[0], 0.5);
}

#[test]
fn p1_018_ltspice_wine_pinned_into_testing() {
    // `nix develop .#testing-ltspice` (ZERO D40) puts LTspice 26.1.1 on PATH
    // as `ltspice`, Wine under a headless X server. A 2 V source over a
    // 1k/1k divider, closed form V(out) = 1 V. Outside that shell this
    // fails. CI carries LTSPICE_CI_PENDING_MIRROR until the MSI is mirrored,
    // and there LTspice running is the failure: drop the marker.
    let dir = tempfile::tempdir().expect("temp dir");
    let deck = dir.path().join("d.cir");
    std::fs::write(
        &deck,
        "* divider\nV1 in 0 2\nR1 in out 1k\nR2 out 0 1k\n.op\n.end\n",
    )
    .expect("deck");
    let run = std::process::Command::new("ltspice")
        .arg("-b")
        .arg(&deck)
        .output();
    if std::env::var_os("LTSPICE_CI_PENDING_MIRROR").is_some() {
        assert!(
            run.is_err(),
            "LTspice ran: remove LTSPICE_CI_PENDING_MIRROR"
        );
        return;
    }
    let run = run.expect("`ltspice` on PATH: run in `nix develop .#testing-ltspice`");
    assert!(run.status.success(), "{run:?}");
    let raw = std::fs::read(deck.with_extension("raw")).expect("d.raw beside the deck");
    let plots = spicerack::rawfile::parse_raw_all(&raw).expect("parsed");
    let op = &plots[0];
    let i = op
        .variables
        .iter()
        .position(|v| v.name == "V(out)")
        .expect("V(out)");
    assert!((op.re(i)[0] - 1.0).abs() <= 1e-9, "{}", op.re(i)[0]);
}

#[cfg(feature = "backend")]
#[test]
fn p1_005_ltspice_backend_deck_path_under_wine() {
    // The LTspice backend end to end in `nix develop .#testing-ltspice`: the
    // 1k/1k divider of p1_018, closed form V(out) = 1 V. Under the CI
    // marker LTspice is absent (p1_018 checks it), so there is no run.
    if std::env::var_os("LTSPICE_CI_PENDING_MIRROR").is_some() {
        return;
    }
    let deck = "* divider\nV1 in 0 2\nR1 in out 1k\nR2 out 0 1k\n.op\n.end\n";
    let op = spicerack::syntax::Dialect::Ltspice
        .run(deck)
        .expect("LTspice runs: `nix develop .#testing-ltspice`");
    let i = op
        .variables
        .iter()
        .position(|v| v.name == "V(out)")
        .expect("V(out)");
    assert!((op.re(i)[0] - 1.0).abs() <= 1e-9, "{}", op.re(i)[0]);
}

#[test]
fn p1_039_xyce_verilog_plugin_toolchain_pinned() {
    // `.#testing`'s Xyce 7.10 is built with plugin support (ZERO D4): ADMS
    // compiles a Verilog-A resistor with `buildxyceplugin.sh`, and `Xyce
    // -plugin` runs it as the lower leg of a 2 V, 1k/1k divider. Closed
    // form V(out) = 1 V.
    let dir = tempfile::tempdir().expect("temp dir");
    let at = |f: &str| dir.path().join(f);
    std::fs::write(
        at("rva.va"),
        "`include \"disciplines.vams\"\nmodule rva(p, n);\n  inout p, n;\n  electrical p, n;\n  \
         parameter real r = 1k from (0:inf);\n  analog I(p, n) <+ V(p, n) / r;\nendmodule\n",
    )
    .expect("va");
    let built = std::process::Command::new("buildxyceplugin.sh")
        .args(["-o", "rva", "rva.va", "."])
        .current_dir(dir.path())
        .output()
        .expect("`buildxyceplugin.sh` on PATH: run in `nix develop .#testing`");
    assert!(built.status.success(), "{built:?}");
    std::fs::write(
        at("d.cir"),
        "* plugin divider\nV1 in 0 2\nR1 in out 1k\nYrva r2 out 0 rmod\n.model rmod rva r=1k\n\
         .op\n.print dc v(out)\n.end\n",
    )
    .expect("deck");
    let run = std::process::Command::new("Xyce")
        .arg("-plugin")
        .arg(at("librva.so"))
        .arg("d.cir")
        .current_dir(dir.path())
        .output()
        .expect("Xyce on PATH");
    assert!(run.status.success(), "{run:?}");
    let prn = std::fs::read_to_string(at("d.cir.prn")).expect("d.cir.prn");
    let v: f64 = prn
        .lines()
        .nth(1)
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|x| x.parse().ok())
        .unwrap_or_else(|| panic!("{prn}"));
    assert!((v - 1.0).abs() <= 1e-9, "{v}");
}

#[test]
fn p1_002_ltspice_stepped_raw_float32_read() {
    // LTspice 26.1.1 on tests/fixtures/raw/p1-002-<a>.cir.txt (README): 1 V
    // over R1 = {r}, stepped 1k 2k 3k, and R2 = 1k. A stepped raw holds
    // every step in one plot (`Flags: ... stepped`); read, it is one plot
    // per step (ZERO D42). Closed form V(out)/V(in) = 1k/(r + 1k): 0.5,
    // 1/3, 0.25, to float32's 2^-24.
    let read = |a: &str| {
        let p = format!(
            "{}/tests/fixtures/raw/p1-002-{a}.raw",
            env!("CARGO_MANIFEST_DIR")
        );
        spicerack::rawfile::parse_raw_all(&std::fs::read(p).expect("fixture"))
            .unwrap_or_else(|e| panic!("{a}: {e}"))
    };
    let col = |r: &spicerack::result::RawData, n: &str| {
        let i = r.variables.iter().position(|v| v.name == n).expect(n);
        r.re(i).into_owned()
    };
    let gains = [0.5, 1.0 / 3.0, 0.25];
    // An operating point stepped: one point a step, column 0 the parameter.
    let op = read("op");
    assert_eq!(op.len(), 3);
    for ((p, g), r) in op.iter().zip(gains).zip([1e3, 2e3, 3e3]) {
        assert_eq!(p.params, [("r".to_string(), r)]);
        let v = col(p, "V(out)");
        assert_eq!(v.len(), 1);
        assert!((v[0] - g).abs() <= g * 2f64.powi(-24), "{v:?} vs {g}");
    }
    // A transient stepped: time restarts at 0 for each step; the step
    // values are in the log, which the backend reads (params empty here).
    let tran = read("tran");
    assert_eq!(tran.len(), 3);
    for (p, g) in tran.iter().zip(gains) {
        assert!(p.params.is_empty());
        let (t, vin, vout) = (col(p, "time"), col(p, "V(in)"), col(p, "V(out)"));
        assert_eq!(t[0], 0.0);
        assert!((t.last().expect("points") - 3e-6).abs() < 1e-12);
        for (i, o) in vin.iter().zip(&vout).filter(|(i, _)| **i > 0.1) {
            assert!((o / i - g).abs() <= 1e-6, "{o}/{i} vs {g}");
        }
    }
}

#[cfg(feature = "backend")]
#[test]
fn p1_007_ltspice_runs_one_analysis_per_deck() {
    // LTspice runs one analysis a deck ("More than one analysis
    // specified"); written for it, a deck of several holds them after
    // render::LTSPICE_EACH and run_each runs each alone. In
    // `nix develop .#testing-ltspice`: 1 V (AC 1) over 1k/1k, closed form
    // V(out) = 0.5 in the op, the ac and the transient (the .four rides
    // with the .tran and writes no plot).
    if std::env::var_os("LTSPICE_CI_PENDING_MIRROR").is_some() {
        return;
    }
    use spicerack::backend::Kind;
    use spicerack::syntax::Dialect;
    let src = "* each\nV1 in 0 DC 1 AC 1\nR1 in out 1k\nR2 out 0 1k\n.op\n.ac dec 2 1k 10k\n\
               .tran 1u 10u\n.four 100k V(out)\n.end\n";
    let deck =
        spicerack::syntax::render::render_text(src, Dialect::Ngspice, "each", Dialect::Ltspice)
            .expect("written")
            .text;
    assert!(
        deck.contains(spicerack::syntax::render::LTSPICE_EACH),
        "{deck}"
    );
    let kinds = [Kind::Op, Kind::Ac, Kind::Tran, Kind::Fourier];
    let each = spicerack::simulation::run_each(Dialect::Ltspice, &deck, &kinds).expect("runs");
    assert_eq!(each.len(), 4);
    assert!(each[3].is_empty(), "a .four writes no plot");
    for (k, plots) in kinds.iter().zip(&each).take(3) {
        assert_eq!(plots.len(), 1, "{k:?}");
        let p = &plots[0];
        let i = p
            .variables
            .iter()
            .position(|v| v.name == "V(out)")
            .expect("V(out)");
        p.re(i)
            .iter()
            .for_each(|&v| assert!((v - 0.5).abs() < 1e-6, "{k:?}: {v}"));
    }
}

#[test]
fn p1_040_p1_047_spectre_nutascii_and_nutbin_read() {
    // tests/fixtures/raw/p1-040-spectre.nutascii and p1-047-spectre.nutbin
    // (README): Spectre 25.1's op plot as settle-remote recorded it (ZERO
    // D43), its nutbin big-endian IEEE f64. Both read to the recorded names
    // and values, the binary twin exactly equal to the text one.
    let read = |f: &str| {
        let p = format!("{}/tests/fixtures/raw/{f}", env!("CARGO_MANIFEST_DIR"));
        spicerack::rawfile::parse_raw(&std::fs::read(p).expect("fixture")).expect(f)
    };
    let (text, bin) = (
        read("p1-040-spectre.nutascii"),
        read("p1-047-spectre.nutbin"),
    );
    for r in [&text, &bin] {
        assert_eq!(r.plot_name, "DC Analysis `op1'");
        let names: Vec<&str> = r.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["dummy", "in", "out", "v1:p"]);
        let vals: Vec<f64> = (0..4).map(|i| r.re(i)[0]).collect();
        assert_eq!(vals, [0.0, 2.0, 0.6666667, -6.667e-4]);
    }
    assert_eq!(text.real_data, bin.real_data);
}
