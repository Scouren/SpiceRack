//! ngspice `.model` kinds Spectre and VACASK had no form for (an `r` and
//! a `c` model, a level-3 diode) and the R/C cards naming them, through
//! `render::models`: each kind's deck written for both targets, VACASK's
//! run against ngspice's on the same source, the Spectre decks a real
//! Spectre runs (`tests/spectre/run.py`, the `t_m*` entries of
//! `expected.json`) pinned byte for byte, and each refusal beside its legal
//! neighbour.
//!
//! Oracles: the closed forms in each deck's comment (by hand from
//! ngspice-45 `restemp.c` L68-L110 and `captemp.c`; the thermal noise
//! 4kTR); ngspice-45 on the source. VACASK's `sp_*` modules are ngspice's device code, so it must
//! land where ngspice does: within 1e-9 for the linear elements, within
//! the Newton tolerance (`reltol` 1e-3, both simulators' default) for the
//! diodes.
//!
//! Cites: Spectre Reference 5.0 pp.627-632 (`resistor`), pp.283-285
//! (`capacitor`), pp.302-310 (`diode`), p.859 (`resistor r=` of `v(…)`);
//! ngspice-45 `frontend/inpcom.c` L6621-L6680.
//!
//! Contents: none (tests only).

use std::path::{Path, PathBuf};
use std::process::Command;

use spicerack::rawfile::parse_raw_all;
use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::render_text;

/// 1 mA into each resistor at 60 C: v = 1e-3 * R.
/// a1 = 100*(10u-0.4u)/(2u-0.2u) * (1 + 1e-3*35 + 2e-6*35^2) = 0.5533067
/// (`tref` is no ngspice resistor parameter: tnom stays 25); a2 = 50*(1 +
/// 2e-3*(60+10-27)) = 0.0543; a3 = 1 mOhm (no value, no `r`, no `rsh`):
/// 1e-6; a4 = 1k*2/4 = 0.5 (`scale` multiplies R, `m` divides it); a5 =
/// 10 * 10u/10u = 0.01 (ngspice's default l and w); a6 = 100*(20u-0.4u)/
/// (10u-0.2u) * 1.03745 = 0.20749.
const RESISTORS: &str = "resistor models
.param wr=2u
.model rpoly r rsh=100 dw=0.1u dlr=0.2u tc1=1e-3 tc2=2e-6 tnom=25 tref=30
.model rvia r res=50 tc1=2e-3
.model rnone r
.model rdef r rsh=10
i1 0 a1 1m
r1 a1 0 rpoly w={wr} l=10u
i2 0 a2 1m
r2 a2 0 rvia dtemp=10
i3 0 a3 1m
r3 a3 0 rnone
i4 0 a4 1m
r4 a4 0 1k scale=2 m=4
i5 0 a5 1m
r5 a5 0 rdef
i6 0 a6 1m
r6 a6 0 rpoly l=20u
.temp 60
.op
.end
";

/// 1 uA into each capacitor for 1 us at 60 C: v = 1e-12 / C, less the
/// 1e12-ohm leak's t/(2RC). c1 = (2e-3*10u*20u + 1e-10*2*(10u+20u)) *
/// (1 + 1e-4*33) = 4.0734e-13: 2.45495; c2 = 3p*2: 0.166667; c3 at
/// ngspice's default width 10u: (2e-3*10u*5u + 1e-10*2*15u) * 1.0033 =
/// 1.0334e-13: 9.67676.
const CAPACITORS: &str = "capacitor models
.model cmim c cj=2e-3 cjsw=1e-10 tc1=1e-4 tnom=27 tref=25
.model cfix c cap=3p
i1 0 b1 pulse(0 1u 0 1f 1f 1 2)
c1 b1 0 cmim w=10u l=20u
rb1 b1 0 1e12
i2 0 b2 pulse(0 1u 0 1f 1f 1 2)
c2 b2 0 cfix scale=2
rb2 b2 0 1e12
i3 0 b3 pulse(0 1u 0 1f 1f 1 2)
c3 b3 0 cmim l=5u
rb3 b3 0 1e12
.temp 60
.tran 10n 1u
.end
";

/// Two level-3 diodes, forward through 1k from 2 V and in breakdown from
/// -20 V, at 60 C: `dj` made up, `ndiode` sky130's
/// (`continuous/models_diodes.spice`, its switches at 1).
const DIODES: &str = "level-3 diodes
.param sw_nsd_pw_cj=1 sw_func_nsd_pw_cj=1 rs_int={981*1e-12}
.model ndiode d level=3 tlevc=1 area=1e12 cj={sw_nsd_pw_cj*1e-12} mj=0.44 pb=0.729
+ cjsw={3.6001e-011*1e-6*sw_func_nsd_pw_cj} mjsw=0.0009 php=0.2 cta=0.000792 ctp=1e-005
+ tpb=0.0012287 tphp=0 js=2.75e-015 jsw=6e-016 n=1.2928 rs={rs_int} ik={1.3e-009/1e-12}
+ ikr=0 bv={11.7/sw_func_nsd_pw_cj} ibv=0.00106 trs=0 eg=1.05 xti=2 tref=30 tcv=0
+ gap1=0.000473 gap2=1110 ttt1=0 ttt2=0 tm1=0 tm2=0 lm=0 lp=0 wm=0 wp=0 xm=0 xoi=10000
+ xom=10000 xp=0 xw=0
.model dj d level=3 is=1e-14 jsw=1e-15 n=1.05 rs=10 cj=1e-3 mj=0.4 pb=0.8
+ cjsw=1e-9 mjsw=0.3 php=0.7 tlevc=1 cta=1e-3 ctp=1e-3 tpb=1e-3 tphp=1e-3
+ bv=12 ibv=1e-3 ikr=0 tref=30 xti=3 tlev=1 tcv=1e-3 tm1=0 xom=10000 xoi=10000 lm=0 wm=0
v1 1 0 2
r1 1 2 1k
d1 2 0 dj area=0.5 pj=3
v2 3 0 -20
r2 3 4 1k
d2 4 0 dj area=0.5 pj=3
v3 5 0 2
r3 5 6 1k
d3 6 0 ndiode area=0.25 pj=2
v4 7 0 -20
r4 7 8 1k
d4 8 0 ndiode area=0.25 pj=2
.temp 60
.op
.end
";

/// A resistance reading its own voltage: ngspice runs it as `B a 0
/// i=v(a)/(…)`, the model's `tc1` and the card's `w`, `l`, `dtemp` unread,
/// so v = 1e-3 * 1k * (1 + 0.1*v): v = 1/0.9 = 1.1111111.
const DEPENDENT: &str = "voltage-dependent resistor
.model resbody r tc1=1e-3 tnom=25
i1 0 a 1m
rb a 0 resbody r={1k*(1+0.1*abs(v(a)))} w=1u l=1u dtemp=5
.temp 60
.op
.end
";

/// Resistances reading only `temper`, 1 mA into each at 60 C: ngspice runs
/// each as a `B` line (`tc1` scaling the resistance, `m` dividing it).
/// a1 = 1k*1.33 * 1.33 = 1.7689; a2 = 2k/2 = 1.0; a3 = 500*(1 + 33^2*1e-4)
/// = 0.55445.
const TEMPER: &str = "temperature-dependent resistors
i1 0 a1 1m
r1 a1 0 r={1k*(1+1e-2*(temper-27))} tc1=1e-2
i2 0 a2 1m
r2 a2 0 r={2k+0*temper} m=2
i3 0 a3 1m
r3 a3 0 r={500*(1+(temper-27)*(temper-27)*1e-4)}
.temp 60
.op
.end
";

/// A dependent resistance below a 1k one, driven by an AC source: ngspice's
/// `B` line is noiseless, so the noise at `a` is the 1k's alone,
/// sqrt(4kT*1k)/2 = 2.0357e-9 V/sqrt(Hz) at 27 C.
const NOISE: &str = "dependent resistor noise
v1 in 0 dc 0 ac 1
r0 in a 1k
r1 a 0 r={1k+0*v(a)}
.noise v(a) v1 dec 1 1k 10k
.end
";

/// The decks and their pinned Spectre files.
const DECKS: [(&str, &str); 4] = [
    ("t_m1_resistors.scs", RESISTORS),
    ("t_m2_capacitors.scs", CAPACITORS),
    ("t_m3_diodes.scs", DIODES),
    ("t_m4_dependent.scs", DEPENDENT),
];

/// Nodes where VACASK's model is not ngspice-45's: upstream VACASK
/// (`36be560`) ships the diode of ngspice-47-159 (`devices/spice/diode.va`
/// header), whose reverse breakdown at 60 C is not ngspice-45's: node 4
/// (`dj`, bv=12 tcv=1e-3) -11.9024 V against -11.8876 V, node 8 (`ndiode`,
/// bv=11.7 tcv=0) -11.8877 V against -11.8273 V. A listed node that agrees
/// fails until its entry goes.
const VACASK_DIFFERS: &[(&str, &str)] = &[("d", "4"), ("d", "8")];

// ---- private ----

/// `src` (ngspice) written for `to`, which must not refuse it.
fn written(src: &str, to: Dialect) -> String {
    render_text(src, Dialect::Ngspice, "t", to)
        .unwrap_or_else(|e| panic!("{to:?}: {}\n{src}", e.messages))
        .text
}

/// The writer's refusal codes for `src` in `to`.
fn refusal(src: &str, to: Dialect) -> Vec<Code> {
    let e = render_text(src, Dialect::Ngspice, "t", to).expect_err(src);
    let mut codes: Vec<Code> = e
        .diags
        .iter()
        .map(|d| d.code)
        .filter(|&c| {
            spicerack::syntax::diag::info(c).kind == spicerack::syntax::diag::Kind::Refusal
        })
        .collect();
    codes.dedup();
    codes
}

/// The last value of each vector ngspice writes for `src`, run in `dir`.
fn ngspice(dir: &Path, src: &str) -> Vec<(String, f64)> {
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
    last_values(&[raw])
}

/// The last value of each vector VACASK writes for `deck`, run in `dir`.
fn vacask(dir: &Path, deck: &str) -> Vec<(String, f64)> {
    std::fs::write(dir.join("deck.sim"), deck).unwrap();
    let out = Command::new("vacask")
        .arg("deck.sim")
        .current_dir(dir)
        .output()
        .expect("vacask not on PATH: run the tests in `nix develop .#testing`");
    let raws: Vec<PathBuf> = ["op1.raw", "tran1.raw"]
        .iter()
        .map(|f| dir.join(f))
        .filter(|p| p.exists())
        .collect();
    assert!(
        !raws.is_empty(),
        "{}\n{deck}",
        String::from_utf8_lossy(&out.stdout)
    );
    last_values(&raws)
}

/// Each real vector's last value across `raws`, its name lowercased.
fn last_values(raws: &[PathBuf]) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    for raw in raws {
        for plot in parse_raw_all(&std::fs::read(raw).unwrap()).unwrap() {
            for (v, col) in plot.variables.iter().zip(&plot.real_data) {
                if let Some(&x) = col.last() {
                    out.push((v.name.to_ascii_lowercase(), x));
                }
            }
        }
    }
    out
}

#[test]
fn spectre_decks_are_written_as_pinned() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/spectre");
    let bless = std::env::var_os("SPICERACK_BLESS").is_some();
    for (file, src) in DECKS {
        let mut text = written(src, Dialect::Spectre);
        text.push('\n');
        if bless {
            std::fs::write(dir.join(file), &text).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(dir.join(file))
            .unwrap_or_else(|_| panic!("{file}: SPICERACK_BLESS=1 writes it"));
        assert_eq!(text, want, "{file}");
    }
}

#[test]
fn spectre_maps_each_parameter_onto_its_equation() {
    let r = written(RESISTORS, Dialect::Spectre);
    for want in [
        // dw/dlr narrow and shorten per side; ngspice's defaults written.
        "model rpoly resistor rsh=100 etch=100n etchl=200n tc1=1m tc2=2u tnom=25 l=10u w=10u",
        "model rnone resistor l=10u w=10u r=1m",
        "r2 (a2 0) rvia trise=10",
        "r4 (a4 0) resistor r=1k m=(4)/(2)",
    ] {
        assert!(r.contains(want), "no `{want}` in\n{r}");
    }
    assert!(!r.contains("tref"), "{r}");
    let c = written(CAPACITORS, Dialect::Spectre);
    for want in [
        "model cmim capacitor cj=2m cjsw=100p tc1=100u tnom=27 w=10u",
        "c2 (b2 0) cfix m=(1)*(2)",
    ] {
        assert!(c.contains(want), "no `{want}` in\n{c}");
    }
    let d = written(DIODES, Dialect::Spectre);
    for want in [
        " level=3 ",
        " js=10f ",
        " cjo=1m m=400m vj=800m ",
        " pta=1m ptp=1m ",
        " tnom=30 ",
        " tbv1=-1m ",
        " eg=1.11 nz=1.05 fcs=500m dskip=no jmax=",
        "d1 (2 0) dj area=500m perim=3",
    ] {
        assert!(d.contains(want), "no `{want}` in\n{d}");
    }
    // ngspice disables a knee current below 1e-28; `tm1=0` adds nothing.
    assert!(!d.contains("ikr") && !d.contains("tm1"), "{d}");
}

#[test]
fn a_dependent_resistance_keeps_what_ngspice_keeps() {
    let w = render_text(DEPENDENT, Dialect::Ngspice, "t", Dialect::Spectre).unwrap();
    assert!(
        w.text
            .contains("rb (a 0) resistor r=1k*(1+0.1*abs(v(a))) isnoisy=no\n"),
        "{}",
        w.text
    );
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert!(codes.contains(&Code::W0113), "{codes:?}");
    // `temper` is Spectre's `temp` (Reference 5.0 p.884).
    let t = written(
        "t\nr1 a 0 r={1k*(1+1e-3*(temper-27))}\n.end\n",
        Dialect::Spectre,
    );
    assert!(t.contains("r=1k*(1+1e-3*(temp-27))"), "{t}");
    // VACASK has no behavioural element: a resistance reading only
    // `temper` is its `$temp`, and none naming a model.
    assert_eq!(refusal(DEPENDENT, Dialect::Vacask), vec![Code::W0102]);
    let v = written(TEMPER, Dialect::Vacask);
    assert!(
        v.contains("r1 (a1 0) resistor r=1k*(1+1e-2*($temp-27)) tc1=1e-2 noisy=0"),
        "{v}"
    );
    let named = "t\n.model rt r\nr1 a 0 rt r={1k+temper}\n.end\n";
    assert_eq!(refusal(named, Dialect::Vacask), vec![Code::W0102]);
}

#[test]
fn ngspice_runs_a_dependent_resistance_noiseless() {
    let tmp = tempfile::tempdir().unwrap();
    let out = ngspice(tmp.path(), NOISE);
    let (_, v) = out
        .iter()
        .find(|(k, _)| k == "onoise_spectrum")
        .unwrap_or_else(|| panic!("{out:?}"));
    let want = (4.0 * 1.380649e-23 * 300.15 * 1e3_f64).sqrt() / 2.0;
    assert!((v - want).abs() / want < 1e-4, "{v} against {want}");
}

#[test]
fn each_refusal_has_a_legal_neighbour() {
    // A tunnelling current, an `ikr` Spectre reads with `hcomp=1` only, an
    // exponential temperature coefficient, a model `r` beside an `rsh`
    // (ngspice takes the geometry first, Spectre the `r`), a resistor's
    // `ac` value: no Spectre parameter computes them. At their off values
    // (or alone) they are written.
    let diode = |p: &str| format!("t\n.model dt d level=3 is=1e-14 {p}\nd1 a 0 dt\n.end\n");
    let res = |p: &str| format!("t\n.model rt r {p}\nr1 a 0 rt\n.end\n");
    for (bad, good) in [
        (diode("jtun=1e-5"), diode("jtun=0")),
        (diode("ikr=4e8"), diode("ikr=1e-30")),
        // P3-089: a sidewall current on the bottom's characteristic scales
        // by `tlev=2`'s law, a `tcv` by a `tlev` that is no constant; with
        // its own `ns`, or a `tlev` given, each is written.
        (
            diode("jsw=1e-15 n=1.05 tlev=2"),
            diode("jsw=1e-15 n=1.05 ns=1.05 tlev=2"),
        ),
        (
            "t\n.param tl=1\n.model dt d level=3 is=1e-14 bv=12 tcv=1e-3 tlev={tl}\nd1 a 0 dt\n.end\n"
                .to_string(),
            diode("bv=12 tcv=1e-3 tlev=1"),
        ),
        // R10: a card's own `dtemp` is not in the rescaled `jsw`; with the
        // model's own `ns` nothing is rescaled and the card is written.
        (
            diode("jsw=1e-15 n=1.05").replace("d1 a 0 dt", "d1 a 0 dt dtemp=10"),
            diode("jsw=1e-15 n=1.05 ns=1.05").replace("d1 a 0 dt", "d1 a 0 dt dtemp=10"),
        ),
        (res("rsh=10 tce=1e-3"), res("rsh=10 tce=0")),
        (res("rsh=10 r=5"), res("r=5")),
        (
            "t\nr1 a 0 1k ac=2k\n.end\n".to_string(),
            "t\nr1 a 0 1k\n.end\n".to_string(),
        ),
    ] {
        assert_eq!(refusal(&bad, Dialect::Spectre), vec![Code::W0114], "{bad}");
        written(&good, Dialect::Spectre);
    }
    // VACASK's diode has no self-heating.
    assert_eq!(
        refusal(&diode("rth0=100"), Dialect::Vacask),
        vec![Code::W0114]
    );
    written(&diode("rth0=0"), Dialect::Vacask);
    // Another dialect's R/C model card means other equations.
    let hspice = "t\n.model rt r rsh=10\nr1 a 0 rt l=1u w=1u\n.end\n";
    let refused = render_text(hspice, Dialect::Hspice, "t", Dialect::Spectre).unwrap_err();
    let codes: Vec<Code> = refused.diags.iter().map(|d| d.code).collect();
    assert!(codes.contains(&Code::W0006), "{codes:?}");
}

/// Ledger P3-089: the level-3 diode's two laws on Spectre.
///
/// `tbv1`: Spectre 25.1's breakdown voltage is `bv*(1 + tbv1*dT)` (the
/// recorded runs of settle-remote: `t_m3_diodes` at `tlev=1`, −12.589478
/// for today's `tbv1=+tcv`; P4-097), ngspice's at `tlev=1` `bv*(1 -
/// tcv*dT)` (`diotemp.c` L204-L209): `tbv1=-1m` for `tcv=1e-3`.
///
/// The sidewall current: the oracle is ngspice-45 on the equivalent deck.
/// With an `ns`, ngspice runs the sidewall on its own characteristic,
/// scaled with `ns` (`dioload.c` L259-L261): the law the recorded Spectre
/// run fits to 1e-6 (`research/impl/evidence/P3-089`). So the written
/// `ns` and `jsw` (Spectre's `temp` is ngspice's `temper`), put on the
/// source's cards, must give the source's own values, at 60 C and away
/// from it. Tolerance: ngspice's RELTOL (1e-3), each run's own accuracy
/// (the two differ by 2e-6; `ns = n` alone by 3.3e-3 and 2.6e-2 at 60 C).
/// No Spectre run of this text exists: UNRUN_SPECTRE_HSPICE (ZERO D43).
#[test]
fn p3_089_spectre_diode_mapping_wrong_t() {
    let text = written(DIODES, Dialect::Spectre);
    assert!(text.contains(" tlev=1 tbv1=-1m "), "{text}");
    let mut eq = DIODES.to_string();
    for (model, jsw, n) in [
        ("ndiode", "jsw=6e-016", "1.2928"),
        ("dj", "jsw=1e-15", "1.05"),
    ] {
        let head = format!("model {model} diode ");
        let line = text
            .lines()
            .find(|l| l.starts_with(&head))
            .unwrap_or_else(|| panic!("no {model} in\n{text}"));
        let word = |key: &str| -> String {
            let found = line.split(' ').find_map(|w| w.strip_prefix(key));
            found
                .unwrap_or_else(|| panic!("no `{key}` in {line}"))
                .to_string()
        };
        assert_eq!(word("ns="), n, "{line}");
        let scaled = word("jsw=").replace("temp", "temper");
        assert_eq!(eq.matches(jsw).count(), 1, "{jsw}");
        eq = eq.replace(jsw, &format!("jsw={{{scaled}}} ns={n}"));
    }
    let tmp = tempfile::tempdir().unwrap();
    for temp in ["60", "100"] {
        let run = |deck: &str, name: &str| {
            let dir = tmp.path().join(format!("{name}{temp}"));
            std::fs::create_dir_all(&dir).unwrap();
            ngspice(&dir, &deck.replace(".temp 60", &format!(".temp {temp}")))
        };
        let (want, got) = (run(DIODES, "src"), run(&eq, "eq"));
        for node in ["v(2)", "v(4)", "v(6)", "v(8)"] {
            let at = |out: &[(String, f64)]| {
                let found = out.iter().find(|(k, _)| k == node);
                found.unwrap_or_else(|| panic!("{node}: {out:?}")).1
            };
            let (w, g) = (at(&want), at(&got));
            assert!(
                (g - w).abs() <= 1e-3 * w.abs(),
                "{temp} C {node}: the written card {g}, the source {w}\n{eq}"
            );
        }
    }
}

#[test]
fn vacask_lands_where_ngspice_does() {
    let tmp = tempfile::tempdir().unwrap();
    for (name, src, tol) in [
        ("r", RESISTORS, 1e-9),
        ("c", CAPACITORS, 1e-9),
        ("d", DIODES, 1e-3),
        ("t", TEMPER, 1e-9),
    ] {
        let dir = tmp.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let ngspice = ngspice(&dir, src);
        let vacask = vacask(&dir, &written(src, Dialect::Vacask));
        let mut seen = 0;
        for (node, v) in &vacask {
            let Some(want) = ngspice.iter().find(|(k, _)| *k == format!("v({node})")) else {
                continue;
            };
            seen += 1;
            let rel = (v - want.1).abs() / want.1.abs().max(1e-30);
            let known = VACASK_DIFFERS.contains(&(name, node.as_str()));
            assert_eq!(
                rel <= tol,
                !known,
                "{name}: {node}: vacask {v} ngspice {} (listed: {known})",
                want.1
            );
        }
        assert!(
            seen >= 3,
            "{name}: {seen} nodes compared\n{vacask:?}\n{ngspice:?}"
        );
    }
}
