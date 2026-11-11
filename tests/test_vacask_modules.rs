//! A device model VACASK ships as a module in a file of its own
//! (`DialectDef::model_load`), written from the simulator that has it as a
//! `.model` LEVEL. Each test runs the source deck on its own simulator and
//! the deck written from it on real VACASK 36be560, one operating point to
//! VACASK's Newton tolerance; a card the module cannot take is refused,
//! VACASK's own message cited.
//!
//! Unsupported fails, never skips: no Xyce, ESPice or VACASK on PATH fails
//! (`nix develop .#testing`).
//!
//! Cites: per test (RG = Xyce Reference Guide 7.10).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{At, Tol};
use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;

// ---- private ----

/// ZERO-LEDGER P4-013. PSP 103 is Xyce's `M` level 103 (RG 7.10
/// §2.3.20.12, version 103.4), ESPice's level 1040 (its CMC QA decks, read
/// as ngspice text) and VACASK 36be560's `psp103va` of `psp103v4.osdi`
/// (`devices/psp103v4/psp103.va` L46). Oracle: real Xyce and real ESPice,
/// each on its own card, against real VACASK on the written deck: the
/// drains of an NMOS and a PMOS stage at one operating point, `M=2` on
/// each. Tolerance: VACASK's Newton `reltol`, 1e-3 (measured 1.4e-4; at
/// `reltol=1e-9` VACASK gives both sources' nine digits:
/// research/impl/evidence/P4-013). VACASK stops on a name the module
/// lacks ("Parameter 'dta' not found."): an instance `DTA`, which Xyce
/// and ESPice read (v(dn) 0.8136 for 0.8305), and a model name are
/// refused, the second on a card read as ngspice too (ngspice-45 has no
/// device at 1040 to speak for). Xyce's level 1031 is the self-heating
/// build (`psp103t.va`), which VACASK does not ship.
#[test]
fn p4_013_psp103_is_vacasks_psp103va() {
    const P: &str = "TOXO=1.5e-9 VFBO=-0.9 NSUBO=4e23 UO=0.04 THESATO=1.2 RSW1=60 \
        TOXOVO=1.5e-9 LOV=10e-9 DTA=5";
    const G: &str = "W=10u L=0.1u AS=5p AD=5p PS=21u PD=21u M=2";
    let deck = |level: u32, inst: &str, card: &str| {
        format!(
            "* t\nVdd vdd 0 1.2\nVgn gn 0 0.5\nVgp gp 0 0.7\nRn vdd dn 1k\n\
             M1 dn gn 0 0 nm {G}{inst}\nRp dp 0 1k\nM2 dp gp vdd vdd pm {G}\n\
             .model nm NMOS (LEVEL={level} {P}{card})\n\
             .model pm PMOS (LEVEL={level} {P})\n.op\n.end\n"
        )
    };
    let tol = Tol {
        rel: 1e-3,
        floor: 0.0,
    };
    // (read as, run on, the simulator's level).
    for (from, on, level) in [
        (Dialect::Xyce, Dialect::Xyce, 103),
        (Dialect::Ngspice, Dialect::Espice, 1040),
    ] {
        let src = deck(level, "", "");
        let va = oracle::write(&src, from, Dialect::Vacask);
        for line in [
            "load \"psp103v4.osdi\"\n",
            "model nm psp103va (type=1 toxo=1.5e-9 ",
            "model pm psp103va (type=-1 toxo=1.5e-9 ",
            "\nm1 (dn gn 0 0) nm w=10u l=0.1u as=5p ad=5p ps=21u pd=21u $mfactor=2\n",
        ] {
            assert!(va.contains(line), "{on:?}: {line}\n{va}");
        }
        let (s, v) = (oracle::run(on, &src), oracle::run(Dialect::Vacask, &va));
        for node in ["dn", "dp"] {
            let want = oracle::value(&s, node, At::First);
            let got = oracle::value(&v, node, At::First);
            assert!(tol.within(got, want), "{node}: VACASK {got}, {on:?} {want}");
        }
        let codes = oracle::refusal(&deck(level, " DTA=10", ""), from, Dialect::Vacask);
        assert_eq!(codes, [Code::W0129], "{on:?}: an instance DTA");
        let codes = oracle::refusal(&deck(level, "", " CGOVO=2e-10"), from, Dialect::Vacask);
        assert_eq!(codes, [Code::W0129], "{on:?}: a name the module lacks");
    }
    let codes = oracle::refusal(&deck(1031, "", ""), Dialect::Xyce, Dialect::Vacask);
    assert_eq!(codes, [Code::W0104, Code::W0104], "Xyce's level 1031");
}
