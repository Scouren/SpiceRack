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
