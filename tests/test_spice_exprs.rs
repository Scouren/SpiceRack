//! The SPICE-family writers' expression and value spellings across
//! dialects (`render/spice/ternary.rs`, `values.rs`, `prints.rs`,
//! `ngspice_options.rs`): Xyce's, HSPICE's and ngspice's functions,
//! outputs and defaults, each against real ngspice on the written deck or
//! the source simulator's own value (Xyce 7.10, ESPice), cited per test.
//!
//! Unsupported fails, never skips: no ngspice on PATH fails the run tests
//! (`nix develop .#testing`).
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{refusal, write};
use spicerack::backend::Dialect;
use spicerack::syntax::diag::Code;

// ---- private ----

// ── Xyce's if() and output syntax on ngspice ──

/// Xyce RG §2.3 `if(c,a,b)`; ngspice-45 has no `if` ("no such function
/// 'if'" in a B source, "Cannot compute substitute" in `{}`, run by
/// hand) but reads `((c)?(a):(b))` in both. Xyce 7.10 on this deck, by
/// hand: v(b) = 4, v(c) = 2·v(1) = 3. Real ngspice on the written deck
/// gives the same.
#[test]
fn xyce_if_is_a_ternary_on_ngspice() {
    let deck = "* t\nV1 1 0 1.5\nR1 1 0 1k\nVb b 0 {if(1>2,3,4)}\nRb b 0 1k\n\
        Bc c 0 V={IF(V(1)>0,2*V(1),0)}\nRc c 0 1k\n.op\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "b") - 4.0).abs() < 1e-9,
        "{w}"
    );
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "c") - 3.0).abs() < 1e-9,
        "{w}"
    );
}

/// Xyce RG `.PRINT` outputs on ngspice-45 (manual §15.4.1, §31.1 `@dev[param]`;
/// each run by hand): `I(R1)` is `@R1[i]` (ngspice's `i()` reads a branch:
/// "can't parse 'r1#branch'"), `IB(Q1)` is `@Q1[ib]` (5.670e-7 A, Xyce
/// 5.675e-7 with its own BJT defaults), `V(X1:N1)` is `v(X1.N1)` (1/3 V on
/// both), Xyce's parameter `R1:R` `@R1[R]` (1000 on ngspice). `PRECISION=` is a
/// print format (W0024). `DNO()`, Xyce's
/// `ONOISE` (V²/Hz, ngspice's `onoise_spectrum` V/√Hz) and arithmetic on
/// a current (ngspice prints `i(V1)` for `i(V1)*2`, nothing for
/// `@R1[i]*2`) are left out (W0023). The written deck runs on real ngspice.
#[test]
fn xyce_outputs_are_spelled_for_ngspice() {
    let deck = "* t\nV1 1 0 1\nR1 1 2 1k\nX1 2 0 half\nV5 5 0 1\nV6 6 0 0.7\nQ1 5 6 0 qn\n\
        .model qn npn\n.subckt half a b\nR9 a n1 1k\nR8 n1 b 1k\n.ends\n.tran 1u 10u\n\
        .print tran PRECISION=10 I(R1) {I(R1)*2} IB(Q1) V(X1:N1) R1:R DNO(R1)\n.end\n";
    let w = oracle::written(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(
        w.text
            .contains(".print tran @R1[i] @Q1[ib] V(X1.N1) @R1[R]\n"),
        "{}",
        w.text
    );
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&Code::W0024) && codes.contains(&Code::W0023),
        "{codes:?}"
    );
    Dialect::Ngspice
        .run(&w.text)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{}", w.text));
    let noise = "* t\nV1 in 0 dc 0 ac 1\nR1 in out 1k\nR2 out 0 1k\n\
        .noise v(out) V1 dec 1 1k 1meg\n.print noise v(out) onoise {sqrt(inoise)}\n.end\n";
    let w = write(noise, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.contains(".print noise v(out)\n"), "{w}");
}

// ── log's base ──

/// ngspice manual §2.8.5: `log` is the natural logarithm; Xyce RG §2.3:
/// `log` is base 10, `ln` natural (Xyce 7.10 by hand: `log(100)` = 2,
/// `ln(100)` = 4.605). ngspice's `log-functions-1.sp` on Xyce read
/// 0.43136 for ln 2.7 (stream F's oracle): written `ln` for Xyce, and a
/// Xyce `log` is `log10` on ngspice, where real ngspice gives 2.
#[test]
fn log_keeps_its_base_across_ngspice_and_xyce() {
    let deck = "* t\nV1 a 0 {log(2.7)}\nR1 a 0 1k\nB1 b 0 V=log(v(a)+1)\nR2 b 0 1k\n.op\n.end\n";
    let w = write(deck, Dialect::Ngspice, Dialect::Xyce);
    assert!(
        w.contains("{ln(2.7)}") && w.contains("V={ln(v(a)+1)}"),
        "{w}"
    );
    let xy = "* t\nV1 a 0 {log(100)}\nR1 a 0 1k\n.op\n.end\n";
    let ng = write(xy, Dialect::Xyce, Dialect::Ngspice);
    assert!(
        (oracle::op(Dialect::Ngspice, &ng, "a") - 2.0).abs() < 1e-9,
        "{ng}"
    );
}

// ── R models on Xyce ──

/// ngspice manual §3.3.2: an R model's `r` (or `resistance`) is the
/// resistance of an instance that gives none (real ngspice: 2 V across
/// `r1 2 0 my` at 1 mA); Xyce 7.10's model `R` multiplies the instance's
/// value, 1k by default (by hand: 2 MΩ for the same text; stream F X2,
/// X3). Written for Xyce, the instance takes the value and the model
/// drops it: `r1 2 0 2k my` with `.model my r` is 2k on Xyce (by hand).
#[test]
fn an_r_model_value_is_the_instance_value_on_xyce() {
    let deck = "* t\nI1 0 2 1m\nr1 2 0 my\nr2 2 0 my r=1k\n.model my r(r=2k tc1=0)\n.op\n.end\n";
    assert!((oracle::op(Dialect::Ngspice, deck, "2") - 2.0 / 3.0).abs() < 1e-9);
    let w = write(deck, Dialect::Ngspice, Dialect::Xyce);
    assert!(
        w.contains("R1 2 0 2k my") && w.contains("R2 2 0 my r=1k"),
        "{w}"
    );
    assert!(w.contains(".model my r (tc1=0)"), "{w}");
    // A subcircuit's own model is the one its instance takes (ngspice
    // §2.4.3; `model-scope-5.sp`).
    let scoped = "* t\nI1 0 2 1m\nX1 2 s\n.subckt s a\n.model my r r=2k\nr1 a 0 my\n.ends\n\
        .model my r r=4k\n.op\n.end\n";
    assert!((oracle::op(Dialect::Ngspice, scoped, "2") - 2.0).abs() < 1e-9);
    let w = write(scoped, Dialect::Ngspice, Dialect::Xyce);
    assert!(w.contains("R1 a 0 2k my"), "{w}");
}

// ── nint's rounding ──

/// ngspice-45's `nint` rounds a half to even (real ngspice: `nint(2.5)`
/// is 2, in `{}` and in a B source); Xyce 7.10's rounds it away from zero
/// (3, by hand at the store path; `xpressn-3.sp` n1085, stream F X11).
/// Written for Xyce, `nint` is spelled with floor/fmod (Xyce by hand on
/// the spelling: 2, 4, -2, -2 at 2.5, 3.5, -2.5, -1.5); Xyce's `nint`
/// written for ngspice rounds away from zero there: real ngspice gives 3.
#[test]
fn nint_rounds_as_its_source() {
    let deck = "* t\nV1 a 0 {nint(2.5)}\nR1 a 0 1k\n.op\n.end\n";
    assert!((oracle::op(Dialect::Ngspice, deck, "a") - 2.0).abs() < 1e-12);
    let w = write(deck, Dialect::Ngspice, Dialect::Xyce);
    assert!(w.contains("fmod(floor((2.5)+0.5),2)"), "{w}");
    let ng = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(
        (oracle::op(Dialect::Ngspice, &ng, "a") - 3.0).abs() < 1e-12,
        "{ng}"
    );
}

// ── Xyce's M on an X line ──

/// Xyce RG `X`: `M` on a subcircuit instance is its multiplicity even
/// where the subcircuit declares `M` (Xyce_Regression ISSUE_267
/// `test_a3.cir`: V(2) = 1/6 V, five copies of 0.5 Ω in each half);
/// ngspice-45 takes it as the declared parameter (0.5 V), and as the
/// multiplicity only where none is declared (run by hand). Written for
/// ngspice, the parameter is renamed in its declaration and its body, and
/// real ngspice gives 1/6 V.
#[test]
fn xyce_m_on_an_instance_is_its_multiplicity_on_ngspice() {
    let deck = "* t\nVin 1 0 1\nRin 1 2 1\nXtest1 2 3 test M=5\nXtest2 3 0 test M=5\n\
        .subckt test A B PARAMS: M=2\nR1 A B {M/4}\n.ends\n.op\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.contains("PARAMS: m_1=2") && w.contains("{m_1/4}"), "{w}");
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "2") - 1.0 / 6.0).abs() < 1e-9,
        "{w}"
    );
}

/// ESPice ignores `m` on an `X` line (run by hand: `Xtest1 2 3 test m=5`
/// gives 0.5 V where ngspice gives 1/6 V), so an instance the source
/// multiplies is refused there (W0047).
#[test]
fn espice_refuses_a_subcircuit_multiplicity() {
    let deck = "* t\nVin 1 0 1\nRin 1 2 1\nXtest1 2 3 test m=5\nXtest2 3 0 test m=5\n\
        .subckt test A B\nR1 A B 0.5\n.ends\n.op\n.end\n";
    assert!((oracle::op(Dialect::Ngspice, deck, "2") - 1.0 / 6.0).abs() < 1e-9);
    let codes = refusal(deck, Dialect::Ngspice, Dialect::Espice);
    assert!(codes.contains(&Code::W0047), "{codes:?}");
}

// ── HSPICE's defaults and options on ngspice ──

/// HSPICE's TNOM and TEMP default to 25 °C, ngspice's to 27 (manual
/// §11.1.1): ESPice's HSPICE reading of `tnom_default.sp` (ESPice
/// conformance corpus) gives v(a) = 0.785904, which ngspice-45 reproduces
/// with both at 25; written as read it gave 0.791142. HSPICE's GSHUNT is
/// ngspice's RSHUNT, its inverse (§11.1.2): 1 mA into 1k ∥ 1k is 0.5 V.
/// GMINDC has no ngspice option (ngspice ignored it: 1.01e-12 A for the
/// deck's 1e-3): refused (W0017).
#[test]
fn hspice_defaults_and_options_keep_their_numbers_on_ngspice() {
    let tnom =
        "* t\ni1 0 a 1m\nd1 a 0 dmod\n.model dmod d is=1e-14 n=1.2 rs=5 eg=1.11 xti=3\n.op\n.end\n";
    let w = write(tnom, Dialect::Hspice, Dialect::Ngspice);
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "a") - 0.785904).abs() < 1e-6,
        "{w}"
    );
    let gshunt = "* t\n.option gshunt=1m post\ni1 0 a 1m\nr1 a 0 1k\n.op\n.end\n";
    let w = write(gshunt, Dialect::Hspice, Dialect::Ngspice);
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "a") - 0.5).abs() < 1e-9,
        "{w}"
    );
    let gmindc =
        "* t\n.option gmindc=1e-3\nv1 0 a 1\nd1 a 0 dmod\n.model dmod d(is=1e-14)\n.op\n.end\n";
    let codes = refusal(gmindc, Dialect::Hspice, Dialect::Ngspice);
    assert!(codes.contains(&Code::W0017), "{codes:?}");
}

/// HSPICE and Xyce evaluate a statistical call to its nominal outside a
/// Monte Carlo run (ESPice's HSPICE reading of `param_distributions.sp`:
/// i(v1) = -2 mA; Xyce 7.10 by hand: agauss, gauss, unif, aunif, limit at
/// the nominal, `rand()` 0.5); ngspice draws one (-2.178 mA as written
/// before). Written for ngspice, each call is its nominal: real ngspice
/// gives 1 V / 1k ∥ 1k = 2 mA.
#[test]
fn a_draw_outside_monte_carlo_is_its_nominal_on_ngspice() {
    let deck = "* t\n.param rv=agauss(1k,100,3) gv='limit(1k, 500)'\nv1 a 0 1\n\
        r1 a 0 rv\nr2 a 0 gv\n.op\n.end\n";
    let w = write(deck, Dialect::Hspice, Dialect::Ngspice);
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("{e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("i(v1)"));
    let i = raw.real_data[i.expect("i(v1)")][0];
    assert!((i + 2e-3).abs() < 1e-12, "{i}\n{w}");
    let xy = "* t\n.param pc={2.0*rand()} pa={agauss(1,1)}\nV1 a 0 {pc+pa}\nR1 a 0 1\n.op\n.end\n";
    let ng = write(xy, Dialect::Xyce, Dialect::Ngspice);
    assert!(
        (oracle::op(Dialect::Ngspice, &ng, "a") - 2.0).abs() < 1e-12,
        "{ng}"
    );
}

/// Xyce RG §2.3: `limit(x, lo, hi)` clamps x (Xyce 7.10 by hand:
/// `limit(3,0,2)` is 2); ngspice-45's `limit` is a statistical call (5 in
/// `{}`, 0 in a B source, run by hand). Written `min(max(…))`, real
/// ngspice gives 2 in both.
#[test]
fn a_xyce_clamp_is_min_of_max_on_ngspice() {
    let deck = "* t\nV1 a 0 {limit(3,0,2)}\nR1 a 0 1k\nV3 c 0 1\nB1 d 0 V=limit(v(c)*3,0,2)\nR3 d 0 1k\n.op\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "a") - 2.0).abs() < 1e-12,
        "{w}"
    );
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "d") - 2.0).abs() < 1e-12,
        "{w}"
    );
}

// ── SFFM's argument order ──

/// ngspice-45's SFFM is (VO VA FM MDI FC): the modulation frequency third,
/// the carrier fifth (`src/spicelib/devices/vsrc/vsrcload.c`); HSPICE's,
/// Xyce's, LTspice's is (VO VA FC MDI FS). By hand: `sffm(0 1 10meg 0
/// 1meg)` at 25 ns is 1.000 on Xyce 7.10 (a 10 MHz carrier) and 0.156 on
/// ngspice. Written across, the two swap: real ngspice on the written
/// Xyce deck gives sin(π/2) = 1.
#[test]
fn sffm_keeps_its_carrier_across_dialects() {
    let deck = "* t\nV1 a 0 sffm(0 1 10meg 0 1meg)\nR1 a 0 1k\n.tran 1n 25n\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.contains("SFFM(0 1 1000000 0 10000000)"), "{w}");
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("{e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("v(a)"));
    let v = *raw.real_data[i.expect("v(a)")].last().unwrap();
    assert!((v - 1.0).abs() < 1e-3, "{v}\n{w}");
    let back = write(&w, Dialect::Ngspice, Dialect::Xyce);
    assert!(back.contains("SFFM(0 1 10000000 0 1000000)"), "{back}");
}

/// HSPICE reads `.param t2="a^3"` as an expression (ESPice's HSPICE
/// reading of `hspice_expressions.sp`: r2 = 1k·2³ = 8k); ngspice-45 reads
/// the quotes as a string ("Undefined parameter [t2]"): written in braces,
/// real ngspice gives 1 V / 8k. HSPICE's `.option search=` has no ngspice
/// option (manual §11.1): refused W0017.
#[test]
fn hspice_quoted_params_and_search_on_ngspice() {
    let deck = "* t\n.param a=2\n.param t2=\"a^3\"\nv2 n2 0 1\nr2 n2 0 '1k*t2'\n.op\n.end\n";
    let w = write(deck, Dialect::Hspice, Dialect::Ngspice);
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("{e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("i(v2)"));
    assert!(
        (raw.real_data[i.expect("i(v2)")][0] + 1.0 / 8e3).abs() < 1e-12,
        "{w}"
    );
    let search = "* t\n.option search='dir'\nv1 a 0 1\nr1 a 0 1k\n.op\n.end\n";
    assert!(refusal(search, Dialect::Hspice, Dialect::Ngspice).contains(&Code::W0017));
}

// ── Xyce's global nodes ──

/// Xyce RG §2.3.4: a node named `$G_…` is global (every subcircuit sees
/// it); ngspice-45 reads `$` as a comment (`VDD $G_VDDNODE 0 5`: "Not
/// enough parameters", `BUG_294_SON/bjt_baseline.cir`). Written for
/// ngspice as a `.global` node `G_VDD`, the subcircuit's resistor reaches
/// it: real ngspice gives the divider's 0.5·5 V.
#[test]
fn xyce_global_nodes_are_ngspice_globals() {
    let deck = "* t\nVDD $G_VDD 0 5\nX1 mid half\nR2 mid 0 1k\n.subckt half m\nR1 $G_VDD m 1k\n.ends\n.op\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.contains(".global G_VDD\n") && !w.contains('$'), "{w}");
    assert!(
        (oracle::op(Dialect::Ngspice, &w, "mid") - 2.5).abs() < 1e-9,
        "{w}"
    );
}

// ── measures ngspice cannot take ──

/// ngspice manual §15.4: `.meas` measures DC, AC, TRAN and SP; ngspice-45
/// on a Xyce `.measure noise …`: "unrecognized analysis type 'noise'"
/// (run by hand). Xyce's `RFC_LEVEL` (RG `.MEASURE`) has no ngspice
/// form. Each is refused W0048.
#[test]
fn ngspice_refuses_a_measure_it_cannot_take() {
    let noise = "* t\nV1 in 0 dc 0 ac 1\nR1 in out 1k\nR2 out 0 1k\n.noise v(out) V1 dec 1 1k 1meg\n\
        .measure noise onmax max onoise\n.end\n";
    assert!(refusal(noise, Dialect::Xyce, Dialect::Ngspice).contains(&Code::W0048));
    let rfc = "* t\nV1 1 0 sin(0 1 1k)\nR1 1 0 1k\n.tran 1u 2m\n\
        .measure tran m max v(1) RFC_LEVEL=0.5 RISE=1\n.end\n";
    assert!(refusal(rfc, Dialect::Xyce, Dialect::Ngspice).contains(&Code::W0048));
}

// ── a parameter defined twice ──

/// Xyce keeps a parameter's first definition (Xyce_Regression
/// `ISSUE_202/first_last.cir`: `.param foobar=1000` … `=1`, R1 = 1 kΩ,
/// I(V1) = -1 mA); ngspice-45 keeps the last (-1 A, stream F round 4).
/// Written for ngspice with the first alone, real ngspice gives -1 mA.
#[test]
fn a_xyce_parameter_keeps_its_first_definition_on_ngspice() {
    let deck = "* t\n.param foobar=1000\n.param foobar=1\nR1 1 0 {foobar}\nV1 1 0 1\n.op\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("{e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("i(v1)"));
    assert!(
        (raw.real_data[i.expect("i(v1)")][0] + 1e-3).abs() < 1e-12,
        "{w}"
    );
}

// ── a device IC= without UIC ──

/// Xyce holds a capacitor's `IC=` in its operating point (Xyce_Regression
/// `DIODE_ANALYTIC/forward_model.cir`: V(2)(0) = 0, stream F Y4);
/// ngspice-45 applies a device IC only with UIC (manual §3.3.5) and gave
/// V(2)(0) = 0.2, its own operating point. A grounded capacitor's IC is an
/// `.ic` there: real ngspice's operating point holds V(2) = 0. An
/// inductor's IC (a branch current `.ic` cannot hold) is refused W0049.
#[test]
fn a_xyce_device_ic_holds_in_ngspices_operating_point() {
    let deck = "* t\nV1 1 0 0.2\nR1 1 2 1k\nC1 2 0 1p IC=0\n.tran 1u 10u\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.contains(".ic V(2)=0"), "{w}");
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("{e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("v(2)"));
    assert!(raw.real_data[i.expect("v(2)")][0].abs() < 1e-9, "{w}");
    let l = deck.replace("C1 2 0 1p IC=0", "L1 2 0 1m IC=1m");
    assert!(refusal(&l, Dialect::Xyce, Dialect::Ngspice).contains(&Code::W0049));
    let uic = deck.replace(".tran 1u 10u", ".tran 1u 10u UIC");
    assert!(!write(&uic, Dialect::Xyce, Dialect::Ngspice).contains(".ic"));
}

/// Xyce's (and PSpice's) `TC=a,b` on a resistor is ngspice's `tc1=a
/// tc2=b` (manual §3.3.1; ngspice-45 on `r_r1 n 0 10 tc=0,0`: "unknown
/// parameter (0)", stream F). Real ngspice runs the written card: 1 mA at
/// TNOM.
#[test]
fn a_resistor_tc_pair_is_tc1_and_tc2_on_ngspice() {
    let deck = "* t\nV1 1 0 1\nR1 1 0 1k tc=0.01,0\n.op\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.contains("tc1=0.01 tc2=0"), "{w}");
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("{e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("i(v1)"));
    let i = raw.real_data[i.expect("i(v1)")][0];
    assert!((i + 1e-3).abs() < 1e-9, "{i}\n{w}");
}

// ── Xyce instance parameters ngspice's devices lack (ledger P3-067) ──

/// Xyce RG 7.10 `C` ("If AGE is given, then the capacitance is
/// C[1 − D log(AGE)]"; Xyce 7.10: v(a) 0.677124 with `age=87600`, 0.632197
/// without or at `age=0`), `M` ("IC1 and IC2 may therefore be specified
/// compactly as IC=<ic1>,<ic2>"), BSIM-SOI's instance `shmod` (Xyce:
/// 0.563876 against 0.383470). ngspice-45: "unknown parameter (age)",
/// "(ic1)", "(shmod)" (`cap.c`, `b4soi.c` instance tables; manual ch.11:
/// an M's `ic=vds,vgs,vbs`). An aging C and an instance `shmod` are
/// refused; `age=0` leaves with its `d`; `IC1`/`IC2` are the `ic=` vector.
/// XYCE_CORPUS `AGECAP/agecap.cir`, `INIT_CONDS/comparatorICnonVector.cir`,
/// `B4SOI_461/test1_shmod1.cir`.
#[test]
fn p3_067_o_dev_params_instance_model() {
    let cite = "ngspice-45 cap.c, b4soi.c (instance tables): no age, no shmod";
    let rc = |c: &str| format!("* t\nv1 1 0 1\nr1 1 a 1k\n{c}\n.tran 10u 4m uic\n.end\n");
    let (xy, ng) = (Dialect::Xyce, Dialect::Ngspice);
    oracle::spec_refusal(cite, &rc("c1 a 0 4u age=87600"), xy, ng, &[Code::W0114]);
    let w = write(&rc("c1 a 0 4u age=0 d=0.05"), xy, ng).to_ascii_lowercase();
    assert!(!w.contains("age") && !w.contains("d=0.05"), "{w}");
    let mos = |p: &str| {
        format!(
            "* t\nvd d 0 1\nvg g 0 3\nm1 d g 0 0 nm w=10u l=1u {p}\n\
             .model nm nmos level=1 vto=1 kp=2e-5\n.tran 1n 10n uic\n.end\n"
        )
    };
    let w = write(&mos("ic1=2 ic2=3"), xy, ng).to_ascii_lowercase();
    assert!(w.contains("ic=2,3") && !w.contains("ic1"), "{w}");
    oracle::spec_refusal(cite, &mos("shmod=1"), xy, ng, &[Code::W0114]);
}

// ── a level-6 MOSFET's lambda on Xyce and ngspice (ledger P3-073) ──

/// Xyce 7.10 `N_DEV_MOSFET6.C:3055-3062` (`if(lambda0Given)`, the test of
/// ngspice-45 `mos6set.c:93-97` inverted): λ is LAMBDA when LAMBDA0 is
/// given too, else 0; ngspice's is `lambda0`, by default `lambda`.
/// Saturation, Id = KC·(W/L)·(Vgs − VTO)^NC·(1 + λ·Vds) = 1 mA·(1 + λ·Vds)
/// (the defaults KC 5e-5, NC 1) through 1k from 5 V: v(d) = 4/(1 + λ).
/// Xyce 7.10: 4 V with `lambda=0.2` alone and with `lambda0=0.2` alone,
/// 3.3333 V with `lambda=0.2 lambda0=0.05`; ngspice-45 on the copied
/// cards: 3.3333, 3.3333, 3.8095. XYCE_CORPUS `MOS6/invert_mos6.cir`.
#[test]
fn p3_073_xyce_mos_level_xyce_drops() {
    let cards = [
        ("lambda=0.2", 0.0),
        ("lambda0=0.2", 0.0),
        ("lambda=0.2 lambda0=0.05", 0.2),
    ];
    for (lambdas, lambda) in cards {
        let deck = format!(
            "* t\nvd dd 0 5\nrd dd d 1k\nvg g 0 3\nm1 d g 0 0 nm l=1u w=10u\n\
             .model nm nmos level=6 vto=1 {lambdas}\n.op\n.end\n"
        );
        let (on, at) = ([Dialect::Xyce, Dialect::Ngspice], oracle::At::First);
        let tol = oracle::Tol {
            rel: 1e-6,
            floor: 0.0,
        };
        oracle::closed_form(
            Dialect::Xyce,
            &deck,
            &on,
            "d",
            at,
            4.0 / (1.0 + lambda),
            tol,
        );
    }
}
