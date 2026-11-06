//! Each analysis -> its documented Spectre statement (dc, ac, tran, noise,
//! xf, sens, sp, fourier), SI scale factors, and the analyses Spectre
//! refuses.
//!
//! Cites: `[REF]`, `[CMP]` and `[KUN]` (see `main.rs`), per test.
//!
//! Contents: none (tests only).

use super::*;

// ---- private ----

/// `[REF]` p.20143 verbatim `dc1 dc` (a `dc` without a sweep parameter is the
/// operating point).
/// `[REF]` p.64: temperature is `param=temp`, a netlist parameter is a bare
/// `param=<name>`, and a device parameter needs `dev=<instance>` alongside
/// `param=<parameter>`. SPICE `.dc Vin ...` sweeps the source's `dc` value
/// (`[CMP]` p.683 vsource parameter #1).
#[test]
fn spectre_dc_sweeps_pick_the_documented_selector() {
    assert_eq!(analysis(".op"), "op1 dc");
    let dc = |src: &str| analysis(&format!(".dc {src} 0 5 0.1"));
    assert_eq!(
        dc("Vin"),
        "dc1 dc dev=vin param=dc start=0 stop=5 step=100m"
    );
    assert_eq!(
        dc("Iin"),
        "dc1 dc dev=iin param=dc start=0 stop=5 step=100m"
    );
    assert_eq!(dc("temp"), "dc1 dc param=temp start=0 stop=5 step=100m");
    assert_eq!(dc("rload"), "dc1 dc param=rload start=0 stop=5 step=100m");
}

/// `[REF]` p.43-44 (ac) and p.416-417 (tran): `Name ac ...` /
/// `Name tran ...`. tran parameters `stop` (#1), `start` (#3),
/// `maxstep` (#8), `step` (#9). Verbatim `[REF]` p.433
/// `tran1 tran stop=0.5u noisefmax=10G noiseseed=1`.
#[test]
fn spectre_ac_and_tran_match_documented_parameters() {
    assert_eq!(
        analysis(".ac dec 100 1 1g"),
        "ac1 ac start=1 stop=1G dec=100"
    );
    assert_eq!(
        analysis(".tran 1n 1u 100n 5n"),
        "tran1 tran step=1n stop=1u start=100n maxstep=5n",
    );
}

/// `[REF]` "Sweep interval parameters" (p.44, p.182, p.389, p.394, p.439, ...)
/// list `step lin dec log values valuesfile` only: Spectre has no octave
/// sweep, so SPICE's `oct` is its grid as `values=[…]`. The grid is
/// ngspice-45's, recorded by running it: `.ac oct 3 1 10` gives 10
/// points, 1 to 8 (2^(10/3) = 10.08 is past `stop` plus its 1.26e-2
/// tolerance); `.ac oct 250 100 1meg` gives 3323, the last 1.000199e6.
#[test]
fn spectre_octave_sweeps_are_ngspices_grid_as_values() {
    let s = analysis(".ac oct 3 1 10");
    let v: Vec<f64> = s
        .strip_prefix("ac1 ac values=[")
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or_else(|| panic!("{s}"))
        .split(' ')
        .map(|x| x.parse().unwrap())
        .collect();
    let want = [
        1.0, 1.259921, 1.587401, 2.0, 2.519842, 3.174802, 4.0, 5.039684, 6.349604, 8.0,
    ];
    assert_eq!(v.len(), want.len(), "{s}");
    for (g, w) in v.iter().zip(want) {
        assert!((g - w).abs() < 1e-6 * w, "{s}");
    }
    let s = analysis(".ac oct 250 100 1meg");
    let v = s.split_once("values=[").unwrap().1.trim_end_matches(']');
    let v: Vec<&str> = v.split(' ').collect();
    assert_eq!(v.len(), 3323, "{s}");
    let last: f64 = v[v.len() - 1].parse().unwrap();
    assert!((last / 1.000199e6 - 1.0).abs() < 1e-6, "{s}");
}

/// `[REF]` pp.406-409 `sweep`: "Within a sweep statement, you can specify
/// analyses statements ... bound within braces"; `dev`/`param` select the
/// swept value. SPICE's `.dc` sweeps its first source innermost.
#[test]
fn a_nested_dc_is_a_spectre_sweep_around_the_inner_dc() {
    assert_eq!(
        analysis(".dc Vgs 0 1 0.5 Vbs 0 -3 -1.5"),
        "dcsweep1 sweep dev=vbs param=dc start=0 stop=-3 step=-1.5 {\n  \
         dc1 dc dev=vgs param=dc start=0 stop=1 step=500m\n}"
    );
}

/// `[REF]` p.181-182 noise: `Name [p] [n] noise parameter=value ...`,
/// "The optional terminals (p and n) specify the output of the circuit",
/// `oprobe` (#16) and `iprobe` (#17) name *components*: an output is a
/// terminal pair, never `oprobe=V(out)`.
#[test]
fn spectre_noise_puts_the_output_in_the_terminal_list() {
    assert_eq!(
        analysis(".noise V(out) Vin dec 10 1 1meg"),
        "noise1 (out 0) noise start=1 stop=1M dec=10 iprobe=vin",
    );
}

/// `[REF]` p.438-439 xf: "you can simply specify a voltage to be the output by
/// giving a pair of nodes on the xf analysis statement". xf computes the
/// transfer function from *every* independent source (`V(out)/V(src)`), so
/// there is no `source=`; `values=[0]` (p.439) is the one frequency 0,
/// SPICE's DC `.tf`. A current output is a probe (#16). The `.tf`'s input
/// and output resistances have no xf output: W0125.
#[test]
fn spectre_xf_has_no_source_parameter() {
    let s = analysis(".tf V(out) Vin");
    assert_eq!(s, "xf1 (out 0) xf values=[0]");
    assert_eq!(analysis(".tf i(Vout) Vin"), "xf1 xf probe=vout values=[0]");
    let w = spicerack::syntax::render::render_text(
        "t\n.tf v(a,b) vin\n.end\n",
        Dialect::Ngspice,
        "t",
        Dialect::Spectre,
    )
    .unwrap();
    assert!(w.text.contains("xf1 (a b) xf values=[0]"), "{}", w.text);
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0125]);
}

/// `[REF]` p.524 verbatim: `sens (1 n2 7) for (analAC)` — `sens` is a control
/// statement naming an analysis, spelled
/// `sens (outputs) to (params) for (analyses)`.
#[test]
fn spectre_sens_matches_documented_control_statement() {
    assert_eq!(
        analysis(".sens V(out)"),
        "sensdc1 dc\nsens (out) for (sensdc1)"
    );
    assert_eq!(
        analysis(".sens V(out) AC dec 10 1 1meg"),
        "sensac1 ac start=1 stop=1M dec=10\nsens (out) for (sensac1)",
    );
}

/// `[REF]` p.387 sp: `Name sp parameter=value ...` with the sweep interval
/// parameters.
#[test]
fn spectre_sp_matches_documented_parameters() {
    assert_eq!(
        analysis(".sp dec 20 1meg 1g"),
        "sp1 sp start=1M stop=1G dec=20"
    );
}

/// `[CMP]` p.323-325: Spectre's `fourier` is a *component* placed on a node
/// pair — `Name [p] [n] [pr] [nr] fourier parameter=value ...`, verbatim
/// `four1 (1 0) fourmod harms=50`, with `fund` as instance parameter #1. It is
/// not an analysis statement and has no `signal=` parameter. (ngspice's
/// `.four` names no harmonic count, so none is written.)
#[test]
fn spectre_fourier_is_emitted_as_a_component() {
    assert_eq!(
        analysis(".tran 1u 1m\n.four 1k V(out) mid"),
        "tran1 tran step=1u stop=1m\nfour1 (out 0) fourier fund=1k\nfour2 (mid 0) fourier fund=1k",
    );
}

/// Analyses belonging to other simulators' dialects must be refused:
/// Spectre's `pz` (`[REF]` p.312) does not take SPICE's node quadruple.
#[test]
fn spectre_refuses_foreign_analyses() {
    let r = try_analysis(".pz 1 0 3 0 vol pz");
    assert!(r.is_err(), "{r:?}");
}

/// Kundert Appendix B, Table B.1 (SI, used by Spectre) vs Table B.2 (SPICE):
/// SPICE's `meg`/`g`/`t` are not Spectre scale factors; SI spells them
/// `M`/`G`/`T`. `k m u n p f` mean the same in both.
#[test]
fn spectre_numbers_use_si_scale_factors() {
    let r = |v: &str| component(&format!("R1 a b {v}"));
    assert!(r("1meg").ends_with("r=1M"), "{}", r("1meg"));
    assert!(r("1g").ends_with("r=1G"), "{}", r("1g"));
    assert!(r("1t").ends_with("r=1T"), "{}", r("1t"));
    assert!(r("1k").ends_with("r=1k"), "{}", r("1k"));
    for v in ["1meg", "1g", "1t"] {
        let s = r(v);
        assert!(
            !s.contains("meg") && !s.ends_with('g') && !s.ends_with('t'),
            "{s}"
        );
    }
}

/// `[REF]` pp.406-409: `dc` and `sweep` take `values=[…]` for their
/// points, and a `sweep` around analyses in braces. Xyce RG `.DC DEC`,
/// `LIST` and `.STEP … LIST`: a decade axis is start·10^(k/n) to stop, a
/// list its values, the first `.dc` axis the inner one.
#[test]
fn spectre_dc_dec_and_list_and_a_list_step_are_values() {
    use spicerack::syntax::render::render_text;
    let xyce = |deck: &str| {
        render_text(deck, Dialect::Xyce, "t", Dialect::Spectre)
            .unwrap_or_else(|e| panic!("{}", e.messages))
            .text
    };
    let sp =
        xyce("t\nV1 1 0 1\nV2 2 0 1\nR1 1 0 1k\nR2 2 0 1k\n.DC DEC V1 1 10 2 V2 LIST 0 1\n.end\n");
    let want = "dcsweep1 sweep dev=v2 param=dc values=[0 1] {\n  \
                dc1 dc dev=v1 param=dc values=[1 3.1622776601683795 10]\n}";
    assert!(sp.contains(want), "{sp}");
    let sp = xyce("t\n.param r=1k\nR1 1 0 {r}\nI1 0 1 1m\n.step r list 1k 2k\n.op\n.end\n");
    let want = "sweep1_r sweep param=r values=[1k 2k] {\n  op1 dc\n}";
    assert!(sp.contains(want), "{sp}");
}
