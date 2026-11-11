//! Raw files -> `spicerack::rawfile::parse_raw`: a truncated header is an
//! error, not a panic; a complete ngspice ASCII raw file reads back its
//! title, variables and values. LTspice itself runs a closed-form deck in
//! `nix develop .#testing-ltspice`. `rawfile::write_raw` gives the bytes
//! ngspice's `load` read, and every recorded raw file reads back from it.
//!
//! Cites: the ngspice raw file format (manual §17.6), as a spec example.
//!
//! Contents: none (tests only).

// ---- private ----

#[test]
fn p8_160_bump_pyo3_maturin_python_support() {
    // PyO3 CHANGELOG: 0.25.0 "Support Python 3.14.0b1" (#4811), 0.27.0
    // "Test against Python 3.14 final release" (#5499); 0.24 builds for no
    // Python past 3.13. A wheel is built from the lock, so the pin is read
    // there. Checks: 1.
    let lock = include_str!("../Cargo.lock");
    let at = lock
        .find("name = \"pyo3\"\nversion = \"")
        .expect("pyo3 is locked");
    let version = lock[at..].lines().nth(1).expect("its version line");
    let mut parts = version.trim_start_matches("version = \"").split('.');
    let mut next = || parts.next().and_then(|p| p.parse::<u32>().ok());
    let (major, minor) = (next().expect("a major"), next().expect("a minor"));
    assert!(
        (major, minor) >= (0, 27),
        "pyo3 {version}: Python 3.14 needs 0.27 or later"
    );
}

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
    let ltspice = spicerack::backend::Sim::LTSPICE;
    let each = spicerack::simulation::run_each(ltspice, &deck, &kinds).expect("runs");
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

/// What a raw file holds of each plot, every value as its bits.
fn p8_101_key(plots: &[spicerack::result::RawData]) -> Vec<String> {
    let bits = |c: &Vec<f64>| c.iter().map(|x| x.to_bits()).collect::<Vec<u64>>();
    plots
        .iter()
        .map(|p| {
            let vars: Vec<String> = p
                .variables
                .iter()
                .map(|v| format!("{} {} {:?}", v.index, v.name, v.var_type))
                .collect();
            let real: Vec<Vec<u64>> = p.real_data.iter().map(bits).collect();
            let complex: Vec<Vec<[u64; 2]>> = p
                .complex_data
                .iter()
                .map(|c| c.iter().map(|x| [x.re.to_bits(), x.im.to_bits()]).collect())
                .collect();
            format!(
                "{:?} {:?} complex={} {vars:?} {real:?} {complex:?}",
                p.title, p.plot_name, p.is_complex
            )
        })
        .collect()
}

/// The four plots of `tests/fixtures/raw/p8-101-hand*.raw`, value for value
/// as `research/impl/evidence/P8-101/mkraw.py` lists them.
fn p8_101_hand_plots() -> Vec<spicerack::result::RawData> {
    use num_complex::Complex64 as C;
    use spicerack::result::{RawData, VarInfo, VarType};
    let var = |index: usize, name: &str, var_type: VarType| VarInfo {
        index,
        name: name.to_string(),
        var_type,
    };
    vec![
        RawData {
            title: "p8-101 hand".into(),
            plot_name: "Operating Point".into(),
            variables: vec![
                var(0, "v(in)", VarType::Voltage),
                var(1, "v(out)", VarType::Voltage),
                var(2, "i(v1)", VarType::Current),
            ],
            real_data: vec![vec![2.0], vec![4.0 / 3.0], vec![-6.666666666666668e-4]],
            ..RawData::default()
        },
        RawData {
            title: "p8-101 hand".into(),
            plot_name: "Transient Analysis".into(),
            variables: vec![
                var(0, "time", VarType::Time),
                var(1, "v(out)", VarType::Voltage),
                var(2, "i(v1)", VarType::Current),
            ],
            real_data: vec![
                vec![0.0, 1e-11, 2.5e-9, 1e-6],
                vec![0.1, 1.9999700004499943e-7, -0.0, f64::MAX],
                vec![5e-324, -1.25, 0.30000000000000004, 123456789.12345679],
            ],
            ..RawData::default()
        },
        RawData {
            title: "p8-101 hand".into(),
            plot_name: "AC Analysis".into(),
            variables: vec![
                var(0, "frequency", VarType::Frequency),
                var(1, "v(out)", VarType::Voltage),
            ],
            complex_data: vec![
                vec![C::new(1e3, 0.0), C::new(3.1622776601683795e3, 0.0)],
                vec![
                    C::new(0.6666549695629838, -2.792477806477596e-3),
                    C::new(0.1, -0.2),
                ],
            ],
            is_complex: true,
            ..RawData::default()
        },
        RawData {
            plot_name: "DC Analysis `op1'".into(),
            variables: vec![
                var(0, "r", VarType::Param),
                var(1, "out", VarType::Voltage),
                var(2, "gain", VarType::Other),
            ],
            real_data: vec![
                vec![1000.0, 2000.0],
                vec![0.6666667, 0.5],
                vec![0.25, 1.0 / 3.0],
            ],
            ..RawData::default()
        },
    ]
}

#[test]
fn p8_101_writer_gives_the_bytes_ngspice_loaded() {
    // Oracle: ngspice-45's own `load`. The two fixtures were assembled by
    // a Python script, not by this crate; ngspice loaded both and wrote
    // every one of their 25 values back bit for bit (tests/fixtures/raw/
    // README; research/impl/evidence/P8-101). The writer must give exactly
    // those bytes for the same plots. Checks: 2 files, byte for byte.
    use spicerack::rawfile::{RawFormat, write_raw};
    let plots = p8_101_hand_plots();
    for (format, file) in [
        (RawFormat::Binary, "p8-101-hand.raw"),
        (RawFormat::Ascii, "p8-101-hand-ascii.raw"),
    ] {
        let p = format!("{}/tests/fixtures/raw/{file}", env!("CARGO_MANIFEST_DIR"));
        let want = std::fs::read(p).expect("fixture");
        let got = write_raw(&plots, format);
        assert!(
            got == want,
            "{file}: not the bytes ngspice loaded; the writer gave\n{}",
            String::from_utf8_lossy(&got)
        );
    }
}

#[test]
fn p8_101_written_raws_read_back_bit_for_bit() {
    // Oracle: the round trip. Every recorded raw file (README), read,
    // written in both formats and read again, holds the same plots bit for
    // bit: ngspice's, Xyce's, LTspice's (float32 widened, a stepped run one
    // plot a step), Spectre's nutascii and big-endian nutbin (written
    // little-endian under the same plot name), and the two hand-assembled
    // files. A recording added to the folder is covered with no edit here.
    use spicerack::rawfile::{RawFormat, parse_raw_all, write_raw};
    use spicerack::result::{RawData, VarInfo, VarType};
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/raw");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the fixtures")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| {
            [".raw", ".nutbin", ".nutascii"]
                .iter()
                .any(|x| n.ends_with(*x))
        })
        .collect();
    names.sort();
    for known in [
        "p1-001.raw",
        "p1-002-op.raw",
        "p1-002-tran.raw",
        "p1-006-ac.raw",
        "p1-006-dc.raw",
        "p1-006-noise.raw",
        "p1-006-op.raw",
        "p1-006-tf.raw",
        "p1-006-tran.raw",
        "p1-040-spectre.nutascii",
        "p1-047-spectre.nutbin",
        "p8-101-hand-ascii.raw",
        "p8-101-hand.raw",
        "p8-101.raw",
    ] {
        assert!(names.contains(&known.to_string()), "{known} is recorded");
    }
    let both = [RawFormat::Binary, RawFormat::Ascii];
    for name in &names {
        let data = std::fs::read(format!("{dir}/{name}")).expect("fixture");
        let read = parse_raw_all(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        for format in both {
            let back = parse_raw_all(&write_raw(&read, format))
                .unwrap_or_else(|e| panic!("{name} {format:?}: {e}"));
            assert_eq!(p8_101_key(&back), p8_101_key(&read), "{name} {format:?}");
        }
    }

    // What no recording holds: a plot with no points before another plot, a
    // plot with no variables, the infinities.
    let var = |name: &str| VarInfo {
        index: 0,
        name: name.to_string(),
        var_type: VarType::Voltage,
    };
    let edges = vec![
        RawData {
            plot_name: "no points".into(),
            variables: vec![var("a")],
            real_data: vec![Vec::new()],
            ..RawData::default()
        },
        RawData {
            plot_name: "no variables".into(),
            ..RawData::default()
        },
        RawData {
            title: "t".into(),
            plot_name: "infinite".into(),
            variables: vec![var("a")],
            real_data: vec![vec![f64::INFINITY, f64::NEG_INFINITY, f64::MIN_POSITIVE]],
            ..RawData::default()
        },
    ];
    for format in both {
        let back = parse_raw_all(&write_raw(&edges, format)).expect("reads back");
        assert_eq!(p8_101_key(&back), p8_101_key(&edges), "{format:?}");
    }

    // A complex plot's real-valued column reads back complex, imaginary
    // part 0; a NaN reads back as a NaN in both formats.
    let mixed = RawData {
        plot_name: "mixed".into(),
        variables: vec![var("a")],
        real_data: vec![vec![1.5, f64::NAN]],
        is_complex: true,
        ..RawData::default()
    };
    for format in both {
        let back = parse_raw_all(&write_raw(std::slice::from_ref(&mixed), format)).expect("reads");
        let col = back[0].complex(0).expect("a complex column");
        assert!(
            back[0].is_complex && back[0].real_data.is_empty(),
            "{format:?}"
        );
        assert_eq!(
            (col[0].re, col[0].im, col.len()),
            (1.5, 0.0, 2),
            "{format:?}"
        );
        assert!(col[1].re.is_nan() && col[1].im == 0.0, "{format:?}");
    }
    assert!(write_raw(&[], RawFormat::Binary).is_empty());
}

#[cfg(feature = "backend")]
#[test]
fn p1_049_spectre_hb_format_nutascii_segfaults() {
    // settle-remote P1-040 (Spectre 25.1.0.470.isr13, 2026-11-07): `hb`
    // under `-format nutascii` stops on FATAL SPECTRE-18 after "Output for
    // `td.pss_hb' is not supported in nutmeg format", and no PSF file of an
    // hb run was kept to read against (ZERO D43: EVIDENCE-BLOCKED). So the
    // backend refuses the deck by name before Spectre starts, the same
    // with and without Spectre installed. `hb` as a comment's or an
    // instance's word is no analysis: that deck gets past the refusal.
    use spicerack::syntax::Dialect;
    let rc = "simulator lang=spectre\n\
              v1 (in 0) vsource type=sine ampl=1 freq=1k fundname=\"f1\"\n\
              r1 (in out) resistor r=1k\n\
              c1 (out 0) capacitor c=159.15494309189535n\n";
    let deck = format!("{rc}hb1 hb funds=[\"f1\"] maxharms=[3]\n");
    let Err(why) = Dialect::Spectre.run_all(&deck) else {
        panic!("an hb deck is refused");
    };
    let why = why.to_string();
    assert!(why.contains("`hb1`") && why.contains("SPECTRE-18"), "{why}");
    let plain = format!("{rc}// hb1 hb\nhb (out 0) resistor r=1k\ntran1 tran stop=1m\n");
    if let Err(e) = Dialect::Spectre.run_all(&plain) {
        assert!(!e.to_string().contains("SPECTRE-18"), "{e}");
    }
}
