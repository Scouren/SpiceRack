//! Decks for the simulators no machine here has: one SPICE source per
//! construct row the Spectre and LTspice writers spell (`render::capability`:
//! every `Native`, `Spell` or `Lift` cell of those targets, plus the plain
//! elements and analyses), written for Spectre (`tests/spectre/p_*.scs`)
//! and LTspice (`tests/ltspice/p_*.cir`) and pinned byte for byte, with the
//! numbers a real run must give in `tests/spectre/pending.json` and
//! `tests/ltspice/pending.json` (written by this test, `"status":
//! "pending-real-run"`). `tests/spectre/run.py` and `tests/ltspice/run.py`
//! check a real run against them; until one has run, each deck is checked
//! here only for the numbers themselves: ngspice-45 on the source meets the
//! closed form beside it. A deck the writer refuses is pinned as its codes.
//!
//! `SPICERACK_BLESS=1` rewrites the decks and both JSON files.
//!
//! Oracles: the closed form in each deck's comment, by hand (the diode, BJT
//! and level-1 MOSFET solved from their model equations, met by ngspice to
//! its own RELTOL 1e-3); ngspice-45 on the SPICE source meets each.
//!
//! Cites: ngspice manual §3.2 (elements), §11.3 (analyses), §15.4
//! (`.measure`); Spectre Reference 19.1 (`bsource` p.459, `if` p.17,
//! `vsource` `pwlperiod`/`delay`); LTspice help (`PWL`, `.meas`).
//!
//! Contents: none (tests only).

use std::path::Path;
use std::process::Command;

use spicerack::rawfile::parse_raw_all;
use spicerack::syntax::Dialect;
use spicerack::syntax::render::render_text;

/// One deck: its file stem, the capability rows it covers, the SPICE
/// source, and the values a run must give, `(analysis, vector, value)`:
/// `op1` the operating point, `tran1`/`dc1` the last point.
struct Deck {
    stem: &'static str,
    rows: &'static str,
    src: &'static str,
    want: &'static [(&'static str, &'static str, f64)],
    /// The closed form holds to this relative tolerance (a model's: ngspice's RELTOL).
    rel: f64,
    /// ngspice cannot run the source (a `.param` inside `.if`, which ngspice
    /// lacks): the closed form alone is the oracle.
    hand_only: bool,
}

const DECKS: &[Deck] = &[
    // 2 V over 2k/1k: out = 2/3. 1 mA through l1 into 1k: n1 = n2 = 1.
    // e = 2*out; g: 1 mS * out into 1k; i(v1) = -2/3 mA, so f = 3*i(v1)*1
    // ohm and h = 100*i(v1).
    Deck {
        stem: "p_elements",
        rows: "R C L V I E F G H; .op",
        src: "elements
v1 in 0 2
r1 in out 2k
r2 out 0 1k
c1 out 0 1n
i1 0 n1 1m
l1 n1 n2 1u
r4 n2 0 1k
e1 e 0 out 0 2
re e 0 1k
g1 0 g out 0 1m
rg g 0 1k
f1 0 f v1 3
rf f 0 1
h1 h 0 v1 100
rh h 0 1k
.op
.end
",
        want: &[
            ("op1", "out", 2.0 / 3.0),
            ("op1", "n2", 1.0),
            ("op1", "e", 4.0 / 3.0),
            ("op1", "g", 2.0 / 3.0),
            ("op1", "f", -2e-3),
            ("op1", "h", -2.0 / 30.0),
        ],
        rel: 1e-9,
        hand_only: false,
    },
    // `.func` and `.param` expressions: out = 2 * 1k / (dbl(500) + 1k) = 1.
    Deck {
        stem: "p_func",
        rows: "Func",
        src: "a function
.param rb=1k
.func dbl(x) {2*x}
v1 in 0 2
r1 in out {dbl(500)}
r2 out 0 {rb}
.op
.end
",
        want: &[("op1", "out", 1.0)],
        rel: 1e-9,
        hand_only: false,
    },
    // The `.if` keeps r2 = 1k (sel = 1): out = 2 * 1k / 2k = 1.
    Deck {
        stem: "p_conditional",
        rows: "Conditional",
        src: "a conditional
.param sel=1
v1 in 0 2
r1 in out 1k
.if (sel == 1)
r2 out 0 1k
.else
r2 out 0 3k
.endif
.op
.end
",
        want: &[("op1", "out", 1.0)],
        rel: 1e-9,
        hand_only: false,
    },
    // A B source: b = 3 * v(out)^2 with out = 2/3, b = 4/3.
    Deck {
        stem: "p_behavioural",
        rows: "BehaviouralSource",
        src: "a behavioural source
v1 in 0 2
r1 in out 2k
r2 out 0 1k
b1 b 0 v={3*v(out)*v(out)}
rb b 0 1k
.op
.end
",
        want: &[("op1", "b", 4.0 / 3.0)],
        rel: 1e-9,
        hand_only: false,
    },
    // `.tf` of the divider: gain r2/(r1+r2) = 1/3, input resistance 3k,
    // output resistance r1 || r2 = 2k/3.
    Deck {
        stem: "p_tf",
        rows: "Tf",
        src: "a transfer function
v1 in 0 2
r1 in out 2k
r2 out 0 1k
.tf v(out) v1
.end
",
        want: &[
            ("tf1", "transfer_function", 1.0 / 3.0),
            ("tf1", "output_impedance_at_v(out)", 2e3 / 3.0),
        ],
        rel: 1e-9,
        hand_only: false,
    },
    // A `.param` set in a branch (ngspice lacks it; Spectre has it): rl = 1k,
    // out = 2 * 1k / 2k = 1.
    Deck {
        stem: "p_branch_param",
        rows: "BranchParam",
        src: "a parameter set in a branch
.param rr=2k
.if (rr > 1k)
.param rl=1k
.else
.param rl=3k
.endif
v1 in 0 2
r1 in out 1k
r2 out 0 {rl}
.op
.end
",
        want: &[("op1", "out", 1.0)],
        rel: 1e-9,
        hand_only: true,
    },
    // q = 1n * v charged by 1 uA for 1 ms from v = 0 (`uic`: the node has no
    // DC path, so there is no operating point to start from): v = 1 V.
    Deck {
        stem: "p_charge_capacitor",
        rows: "ChargeCapacitor; .tran",
        src: "a charge capacitor
iq 0 q 1u
cq q 0 q={1n*v(q)}
.tran 10u 1m uic
.end
",
        want: &[("tran1", "q", 1.0)],
        rel: 1e-3,
        hand_only: false,
    },
    // A PWL triangle 0 -> 1 -> 0 V over 2 ms, repeating from t=0 of the list
    // (r=0), delayed 0.5 ms (td): at 2.25 ms the local time is
    // (2.25-0.5) mod 2 = 1.75 ms, on the falling edge: 0.25 V.
    Deck {
        stem: "p_pwl_repeat",
        rows: "PwlRepeatV",
        src: "a repeating, delayed PWL
v1 a 0 pwl(0 0 1m 1 2m 0) r=0 td=0.5m
r1 a 0 1k
.tran 10u 2.25m
.end
",
        want: &[("tran1", "a", 0.25)],
        rel: 1e-6,
        hand_only: false,
    },
    // The divider swept to 2 V: out = 2/3 at the last point.
    Deck {
        stem: "p_dc_sweep",
        rows: ".dc",
        src: "a dc sweep
v1 in 0 0
r1 in out 2k
r2 out 0 1k
.dc v1 0 2 0.5
.end
",
        want: &[("dc1", "out", 2.0 / 3.0)],
        rel: 1e-9,
        hand_only: false,
    },
    // The models' own equations at 27 C (Vt = kT/q = 25.865 mV), solved by
    // bisection by hand: the diode (1-a)/1k = Is*(exp(a/Vt)-1), a = 0.62944;
    // the BJT's Gummel-Poon with only IS and BF, (5-Vbe)/470k = Is/BF *
    // (exp(Vbe/Vt)-1), Vbe = 0.71230, coll = 5 - 1k*Is*exp(Vbe/Vt) = 4.08772;
    // the level-1 MOSFET in saturation, Id = KP/2 * W/L * (3-1)^2 *
    // (1 + 0.05*Vds), drain = 4.6/1.02 = 4.50980.
    Deck {
        stem: "p_models",
        rows: "D Q M (level 1) models",
        src: "diode, BJT and level-1 MOSFET
.model dmod d is=1e-14 n=1.0
.model qn npn is=1e-15 bf=100
.model nm nmos level=1 vto=1.0 kp=2e-5 lambda=0.05
v1 vin 0 1
r1 vin a 1k
d1 a 0 dmod
vcc vcc 0 5
rb vcc base 470k
rc vcc coll 1k
q1 coll base 0 qn
vg gate 0 3
rd vcc drain 1k
m1 drain gate 0 0 nm w=10u l=1u
.op
.end
",
        want: &[
            ("op1", "a", 0.629441),
            ("op1", "coll", 4.087723),
            ("op1", "drain", 4.6 / 1.02),
        ],
        rel: 1e-3,
        hand_only: false,
    },
    // `.save` and an option kept; `.measure` of a 1 V sine's peak: 1 V.
    Deck {
        stem: "p_measure_save",
        rows: "Measure; Save; UnmappedOption",
        src: "measure, save, options
v1 a 0 sin(0 1 1k)
r1 a 0 1k
.save v(a)
.options reltol=1e-4
.tran 1u 1m
.measure tran vmax max v(a)
.end
",
        want: &[("measure", "vmax", 1.0)],
        rel: 1e-3,
        hand_only: false,
    },
];

// ---- private ----

/// The written deck for `to`, or `refused <codes>` when the writer refuses.
fn written(src: &str, to: Dialect) -> String {
    match render_text(src, Dialect::Ngspice, "t", to) {
        Ok(w) => w.text + "\n",
        Err(e) => {
            let mut codes: Vec<String> = e.diags.iter().map(|d| format!("{:?}", d.code)).collect();
            codes.sort();
            codes.dedup();
            format!("refused {}\n", codes.join(" "))
        }
    }
}

/// The value ngspice gives for `(analysis, vector)` of `src`.
fn ngspice(dir: &Path, src: &str, analysis: &str, vector: &str) -> f64 {
    if analysis == "measure" {
        let deck = dir.join("deck.sp");
        std::fs::write(&deck, src).unwrap();
        let out = Command::new("ngspice")
            .arg("-b")
            .arg(&deck)
            .output()
            .expect("ngspice not on PATH: run the tests in `nix develop .#testing`");
        let text = String::from_utf8_lossy(&out.stdout);
        let line = text
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{vector} ")))
            .unwrap_or_else(|| panic!("no .measure {vector}:\n{text}"));
        let value = line
            .split('=')
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap();
        return value.parse().unwrap();
    }
    let deck = dir.join("deck.sp");
    std::fs::write(&deck, src).unwrap();
    let raw = dir.join("ngspice.raw");
    let out = Command::new("ngspice")
        .arg("-b")
        .arg("-r")
        .arg(&raw)
        .arg(&deck)
        .output()
        .expect("ngspice not on PATH: run the tests in `nix develop .#testing`");
    assert!(raw.exists(), "{}", String::from_utf8_lossy(&out.stdout));
    let plots = parse_raw_all(&std::fs::read(&raw).unwrap()).unwrap();
    for p in plots {
        for (v, col) in p.variables.iter().zip(&p.real_data) {
            if v.name.eq_ignore_ascii_case(&format!("v({vector})"))
                || v.name.eq_ignore_ascii_case(vector)
            {
                return *col.last().unwrap();
            }
        }
    }
    panic!("ngspice wrote no {vector}");
}

/// `pending.json` for `target`: each deck's file, rows, status and values.
fn pending(ext: &str, target: Dialect) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"_provenance\": \"Written by tests/test_pending_real_run.rs from its SPICE sources; the values are the closed forms there, which ngspice-45 meets on the source (hand_only: ngspice cannot run it). status pending-real-run: no {} run has checked these decks yet.\",\n",
        target.name()
    ));
    for (i, d) in DECKS.iter().enumerate() {
        let file = format!("{}.{ext}", d.stem);
        let deck = written(d.src, target);
        let status = match deck.strip_prefix("refused ") {
            Some(codes) => format!("refused {}", codes.trim()),
            None => "pending-real-run".into(),
        };
        // Spectre's `xf` gives transfer functions only, no impedance
        // (Reference 19.1 pp.438-439; the writer notes W0125).
        let values: Vec<String> = d
            .want
            .iter()
            .filter(|(_, v, _)| !(target == Dialect::Spectre && v.contains("impedance")))
            .map(|(a, v, x)| format!("\"{a}:{v}\": {x:e}"))
            .collect();
        out.push_str(&format!(
            "  \"{file}\": {{ \"rows\": \"{}\", \"status\": \"{status}\", \"rel_tol\": {:e}, \"values\": {{ {} }} }}{}\n",
            d.rows,
            d.rel,
            values.join(", "),
            if i + 1 < DECKS.len() { "," } else { "" }
        ));
    }
    out.push_str("}\n");
    out
}

#[test]
fn every_deck_is_written_as_pinned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let bless = std::env::var_os("SPICERACK_BLESS").is_some();
    for (dir, ext, target) in [
        ("spectre", "scs", Dialect::Spectre),
        ("ltspice", "cir", Dialect::Ltspice),
    ] {
        let mut files: Vec<(String, String)> = DECKS
            .iter()
            .map(|d| (format!("{}.{ext}", d.stem), written(d.src, target)))
            .collect();
        files.push(("pending.json".into(), pending(ext, target)));
        for (file, text) in files {
            let path = root.join(dir).join(&file);
            if bless {
                std::fs::write(&path, &text).unwrap();
                continue;
            }
            let want = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("{dir}/{file}: SPICERACK_BLESS=1 writes it"));
            assert_eq!(text, want, "{dir}/{file}: SPICERACK_BLESS=1 rewrites it");
        }
    }
}

#[test]
fn ngspice_meets_every_closed_form() {
    let tmp = tempfile::TempDir::new().unwrap();
    for d in DECKS.iter().filter(|d| !d.hand_only) {
        for &(a, v, x) in d.want {
            let got = ngspice(tmp.path(), d.src, a, v);
            assert!(
                (got - x).abs() <= d.rel * x.abs(),
                "{} {a}:{v}: ngspice {got} vs {x}",
                d.stem
            );
        }
    }
}

#[test]
fn ngspice_refuses_a_branch_parameter() {
    // The hand-only deck's premise: ngspice reads no `.param` in an `.if`
    // branch, so no ngspice run can be its oracle.
    let d = DECKS.iter().find(|d| d.hand_only).unwrap();
    assert!(render_text(d.src, Dialect::Ngspice, "t", Dialect::Ngspice).is_err());
}
