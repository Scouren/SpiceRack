//! A Xyce deck written for ngspice: what Xyce means by a line that ngspice
//! reads another way (`render/spice/`, the Xyce → ngspice pair rules). Each
//! test runs the Xyce source on real Xyce 7.10 and the deck written from
//! it on real ngspice-45 (`oracle::sim_pair`), or checks a refusal against
//! the ngspice clause that makes it one.
//!
//! Unsupported fails, never skips: no Xyce or ngspice on PATH fails
//! (`nix develop .#testing`).
//!
//! Cites: per test (RG = Xyce Reference Guide 7.10).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{At, Tol, sim_pair};
use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;

/// Two direct solves of one linear deck: both simulators give the closed
/// form to rounding (their RELTOL, 1e-3, bounds no iteration here).
const LINEAR: Tol = Tol {
    rel: 1e-9,
    floor: 1e-12,
};

// ---- private ----

// Ledger P3-070. Xyce's E/G `TABLE` is straight lines between its points,
// the ends held (RG 7.10 `E`; 7.10 run), so each value below is the
// table's own arithmetic: E over (-1,1) (1,-1) (2,0.5), G over (0,1) (1,2)
// into 1 Ω (v(3) = -I). ngspice's and ESPice's native `TABLE` round the
// corners (0.95 for 1.0 at -1, run), so neither deck carries it: ngspice
// takes `pwl()`, ESPice the segments as ternaries while its B source
// compiles them (64 operations: six points over one probe pass, seven
// stop with "TooManyOps", run), W0102 past that. LINEAR: every deck is
// one linear solve.
#[test]
fn p3_070_a_xyce_table_keeps_its_corners() {
    let on = [Dialect::Xyce, Dialect::Ngspice, Dialect::Espice];
    for (v1, e, g) in [
        (-1.0, 1.0, -1.0),
        (0.5, -0.5, -1.5),
        (2.0, 0.5, -2.0),
        (3.0, 0.5, -2.0),
    ] {
        let deck = format!(
            "* t\nV1 1 0 {v1}\nE1 2 0 TABLE {{V(1)}} = (-1,1) (1,-1) (2,0.5)\nR2 2 0 1k\n\
             G1 3 0 TABLE {{V(1)}} = (0,1) (1,2)\nR3 3 0 1\n.op\n.end\n"
        );
        oracle::closed_form(Dialect::Xyce, &deck, &on, "2", At::First, e, LINEAR);
        oracle::closed_form(Dialect::Xyce, &deck, &on, "3", At::First, g, LINEAR);
    }
    // Six points (five segments) at the corner (3,5): Xyce and ESPice.
    let six = "* t\nV1 1 0 3\nE1 2 0 TABLE {V(1)} = (0,1) (1,3) (2,2) (3,5) (4,4) (5,7)\n\
               R2 2 0 1k\n.op\n.end\n";
    sim_pair(Dialect::Xyce, six, Dialect::Espice, "2", At::First, LINEAR);
    // Seven points, or a control that is no single probe: past the B source.
    let w0102 = [spicerack::syntax::diag::Code::W0102];
    let seven = six.replace("(5,7)", "(5,7) (6,6)");
    assert_eq!(
        oracle::refusal(&seven, Dialect::Xyce, Dialect::Espice),
        w0102
    );
    let product = six.replace("{V(1)}", "{2*V(1)}");
    assert_eq!(
        oracle::refusal(&product, Dialect::Xyce, Dialect::Espice),
        w0102
    );
    // ngspice's `pwl()` has no such bound.
    assert!(oracle::write(&seven, Dialect::Xyce, Dialect::Ngspice).contains("value={pwl("));
}

// Ledger P3-081. Xyce sets a parameter an `X` line passes even where the
// `.subckt` line does not declare it, over the top-level `.param` of that
// name (7.10: XYCE_CORPUS `Certification_Tests/BUG_1558/bug_1558_3.cir`,
// I(V1) = -5e-5; here v(2) = 2.5, the 100k/100k divider, where the
// `.param`'s 1k would give 0.0495); ngspice-45 stops on the line
// ("Mismatch: 0 formal but 1 actual params"), so the head declares it.
// A `.param` of the body is a declaration on both (the second deck, after
// XYCE_CORPUS `BUGS_ISSUE_54/multiSubcktPar.cir`: X1 computes with 1, X3
// with the body's 10: 10/11 V). An `X` line with no parameter after one
// that passed an undeclared one computes on Xyce with that line's value
// (7.10: 100k, not the `.param`'s 1k): no head default says that (W0140).
#[test]
fn p3_081_an_x_line_parameter_the_subckt_does_not_declare() {
    let deck = "* t\n.param R=1k\nV1 1 0 5\nR0 1 2 100k\nX1 2 0 simple PARAMS: R=100k\n\
                .subckt simple a b\nR1 a b {R}\n.ends\n.op\n.end\n";
    let (xyce, _) = sim_pair(
        Dialect::Xyce,
        deck,
        Dialect::Ngspice,
        "2",
        At::First,
        LINEAR,
    );
    assert!(LINEAR.within(xyce, 2.5), "Xyce v(2) = {xyce}");
    let local = "* t\nV1 1 0 1\nX1 1 2 sub1 rvalue=1.0\nX3 2 0 sub1 bogus=3\n\
                 .subckt sub1 a b\n.param rvalue=10.0 bogus=1.0\nR1 a b {rvalue}\n.ends\n\
                 .op\n.end\n";
    let (xyce, _) = sim_pair(
        Dialect::Xyce,
        local,
        Dialect::Ngspice,
        "2",
        At::First,
        LINEAR,
    );
    assert!(LINEAR.within(xyce, 10.0 / 11.0), "Xyce v(2) = {xyce}");
    let stale = deck.replace("PARAMS: R=100k\n", "PARAMS: R=100k\nX4 2 0 simple\n");
    assert_eq!(
        oracle::refusal(&stale, Dialect::Xyce, Dialect::Ngspice),
        [spicerack::syntax::diag::Code::W0140]
    );
}

// Ledger P3-080. Xyce holds a BSIM3 instance's `IC=Vds,Vgs` in its DCOP
// (RG 7.10 `M`, `IC`; Xyce 7.10: V(3) = 0.5, V(2) = 1.2 at t = 0, and
// 1.96126 / 2.0 with the IC removed); ngspice reads a MOS `ic=` only under
// UIC (manual §7.6.1), so the deck carries `.ic V(3)=0.5` and `.ic
// V(2)=1.2`. The corpus decks: XYCE_CORPUS `INIT_CONDS/comparatorIC.cir`,
// `IC_AND_NODESET/ring_osc_bsim3.cir`. t = 0: both simulators hold the
// node at the given value to 15 digits (LINEAR). t = 20 ns: the two BSIM3
// codes and integrators agree to 2.2e-4 (1.79610881 against 1.795716):
// 1e-3, each run's RELTOL.
#[test]
fn p3_080_xyce_bsim3_instance_ic_holds_in_the_op() {
    let deck = "* t\nvdd 1 0 2\nr1 1 3 10k\nc1 3 0 1p\nr2 1 2 10k\nc2 2 0 1p\n\
                mn1 3 2 0 0 n1 l=0.35u w=5u IC=0.5,1.2\n.model n1 nmos level=9\n\
                .tran 10p 20n\n.end\n";
    for (node, held) in [("3", 0.5), ("2", 1.2)] {
        let (xyce, ngspice) = sim_pair(
            Dialect::Xyce,
            deck,
            Dialect::Ngspice,
            node,
            At::First,
            LINEAR,
        );
        assert!(LINEAR.within(xyce, held), "Xyce v({node}) = {xyce}");
        assert!(
            LINEAR.within(ngspice, held),
            "ngspice v({node}) = {ngspice}"
        );
    }
    let late = Tol {
        rel: 1e-3,
        floor: 1e-12,
    };
    sim_pair(Dialect::Xyce, deck, Dialect::Ngspice, "3", At::Last, late);
    // An `.op` deck: ngspice applies no `.ic` there, so the line stays refused.
    let op = deck.replace(".tran 10p 20n", ".op");
    assert_eq!(
        oracle::refusal(&op, Dialect::Xyce, Dialect::Ngspice),
        [spicerack::syntax::diag::Code::W0049]
    );
}

/// Ledger P3-099. Xyce RG §2.3.4: `$G_A` and `$G_AB` are two global nodes,
/// and `G_AB` a third, plain one. Xyce 7.10 by hand on this deck:
/// V($G_A) = 1, V($G_AB) = 2 (2 mA into 1 kΩ), V(G_AB) = 3 (3 mA into
/// 1 kΩ). ngspice reads `$` as a comment, so each global is renamed and
/// declared `.global` (manual §2.4.4): whole names only, or `$G_AB` lands
/// on `G_AB` and ngspice gives 2.5 V there (5 mA into 500 Ω).
#[test]
fn p3_099_a_global_that_heads_another_keeps_its_own_node() {
    let deck = "* t\nV1 $G_A 0 1\nR1 $G_AB 0 1k\nI1 0 $G_AB 2m\n\
        R2 G_AB 0 1k\nI2 0 G_AB 3m\n.op\n.end\n";
    let (xyce, _) = sim_pair(
        Dialect::Xyce,
        deck,
        Dialect::Ngspice,
        "G_AB",
        At::First,
        LINEAR,
    );
    assert!(LINEAR.within(xyce, 3.0), "Xyce v(G_AB) = {xyce}");
    let w = oracle::write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.lines().any(|l| l == ".global G_A G_AB_1"), "{w}");
    assert!(w.lines().any(|l| l.starts_with("R1 G_AB_1 0 ")), "{w}");
}

/// ZERO-LEDGER P3-056 (`O-TF-NAME`: "DC Transfer Function" is ngspice's
/// name for `.dc`). Xyce RG §2.1.3: `.DC` sweeps a device parameter
/// (`R1:R`) and a `.param`; ngspice manual §11.3.2: a source, a resistor
/// or `temp`. Xyce_Regression `Certification_Tests/BUG_806_SON/
/// dc_baseline.cir` and `PARAM_REFACTOR/paramDc1.cir`: ngspice-45 "named
/// "r1:r" is not in the circuit" before. The resistor is swept by its
/// name; a parameter is refused for ngspice, which would sweep a source
/// of the parameter's name where Xyce and ESPice sweep the parameter.
#[test]
fn p3_056_o_tf_name_tf_naming() {
    let (xy, ng, es) = (Dialect::Xyce, Dialect::Ngspice, Dialect::Espice);
    // Linear dividers: the simulators agree to their printed digits.
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    let deck = "* t\nVT1 4 0 10\nR1 4 5 10\nR2 5 0 5\n.DC R1:R 20 40 20\n.print dc v(5)\n.end\n";
    // 10 V · 5 / (R1 + 5): 2 V at R1 = 20, 10/9 V at 40 (3.33 V unswept).
    let last = 10.0 / 9.0;
    oracle::closed_form(xy, deck, &[xy, ng, es], "5", oracle::At::First, 2.0, tol);
    oracle::closed_form(xy, deck, &[xy, ng, es], "5", oracle::At::Last, last, tol);
    let cite = "ngspice manual §11.3.2: srcnam is a source, a resistor or temp";
    let param = "* t\n.param v1=5\nV1 a 0 1\nR1 a 0 1k\nV2 b 0 {v1}\nR2 b 0 1k\n\
        .dc V1 1 3 1\n.print dc v(a) v(b)\n.end\n";
    oracle::spec_refusal(cite, param, xy, ng, &[Code::W0015]);
    // The parameter is swept, not the source V1: v(b) = 3 V at the last
    // point (5 V with the source swept, as ngspice-45 does).
    oracle::closed_form(xy, param, &[xy, es], "b", oracle::At::Last, 3.0, tol);
    let cap = "* t\nV1 a 0 1\nC1 a 0 1p\n.dc C1:C 1p 2p 1p\n.end\n";
    oracle::spec_refusal(cite, cap, xy, ng, &[Code::W0015]);
}
