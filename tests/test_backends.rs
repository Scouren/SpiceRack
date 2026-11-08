//! Raw files -> `spicerack::rawfile::parse_raw`: a truncated header is an
//! error, not a panic; a complete ngspice ASCII raw file reads back its
//! title, variables and values.
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
