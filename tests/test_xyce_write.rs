//! The Xyce target's conformance gaps (`render/spice/`, the Xyce choices),
//! each a wrong number or a failed run real Xyce 7.10 gave on a deck written
//! from ngspice text (`tests/test_cross_sim`, the spice-bench -> Xyce cell).
//! The oracle of every run test is a closed form the deck is built to give,
//! met by real ngspice on the source and by real Xyce on the deck written
//! for it; a construct Xyce lacks is refused, its clause cited.
//!
//! Unsupported fails, never skips: no Xyce or ngspice on PATH fails
//! (`nix develop .#testing`).
//!
//! Cites: per test (RG = Xyce Reference Guide 7.10).
//!
//! Contents: none (tests only).

#[path = "test_cross_sim/sim.rs"]
#[allow(dead_code)]
mod sim;

use std::path::Path;

use sim::{Ran, Sim};
use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::{render_kit, render_text};
use spicerack::syntax::resolve::{FileId, Form, Kit, LoadError};

/// CPU seconds a run of these few-element decks may take.
const SECS: u64 = 20;

// ---- private ----

/// `text` run on real Xyce: the value its standard output gives measure
/// `name` (`D1 = 3.000000e-01 with ...`).
fn xyce_measure(text: &str, name: &str) -> f64 {
    let dir = tempfile::TempDir::new().expect("a temporary directory");
    std::fs::write(dir.path().join("t.cir"), text).expect("the deck written");
    let out = std::process::Command::new("Xyce")
        .arg("t.cir")
        .current_dir(dir.path())
        .output()
        .expect("Xyce on PATH (nix develop .#testing)");
    let log = String::from_utf8_lossy(&out.stdout);
    let want = format!("{name} = ");
    log.lines()
        .find_map(|l| {
            l.strip_prefix(&want)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
        .unwrap_or_else(|| panic!("no {name} in Xyce's output:\n{log}\n{text}"))
}

/// `deck` (ngspice text) on real ngspice and, written for Xyce, on real
/// Xyce: the same operating point `v(node)`, to 1e-3 relative. The two
/// simulators' constants differ (Xyce 7.10 `N_DEV_Const.h` k = 1.3806226e-23,
/// q = 1.6021918e-19; ngspice-45 `const.h` CODATA 2014) by 2.8e-5 in kT/q,
/// which a junction's temperature scaling (Eg/kT, about 45) amplifies; a
/// card read at another temperature moves `v` by tens of percent.
fn same_op(deck: &str, node: &str) {
    let xyce = write(deck);
    let want = op(Sim::Ngspice, deck, node);
    let got = op(Sim::Xyce, &xyce, node);
    assert!(
        (got - want).abs() <= 1e-3 * want.abs(),
        "xyce v({node}) = {got}, ngspice {want}\n{xyce}"
    );
}

/// `deck` (ngspice text) on real ngspice, and written for Xyce on real
/// Xyce: each operating point's `v(node)` is `want` (to 1e-9 relative).
fn both_op(deck: &str, node: &str, want: f64) {
    let xyce = write(deck);
    for (sim, text) in [(Sim::Ngspice, deck), (Sim::Xyce, xyce.as_str())] {
        let got = op(sim, text, node);
        assert!(
            (got - want).abs() <= 1e-9 * want.abs().max(1e-12),
            "{}: v({node}) = {got}, want {want}\n{text}",
            sim.name()
        );
    }
}

/// The ngspice deck `text` written for Xyce; panics on a refusal.
fn write(text: &str) -> String {
    render_text(text, Dialect::Ngspice, "t", Dialect::Xyce)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.messages))
        .text
}

/// The codes the refusal of the ngspice deck `text` for Xyce has.
fn refusal(text: &str) -> Vec<Code> {
    match render_text(text, Dialect::Ngspice, "t", Dialect::Xyce) {
        Ok(w) => panic!("written, not refused:\n{}", w.text),
        Err(e) => e.diags.iter().map(|d| d.code).collect(),
    }
}

/// `text` on `sim`: the operating point's `v(node)`.
fn op(sim: Sim, text: &str, node: &str) -> f64 {
    let plots = match sim::run(sim, Path::new("/nonexistent/t.cir"), text, &[], SECS) {
        Ran::Plots(p) => p,
        Ran::Timeout => panic!("{}: past {SECS} s\n{text}", sim.name()),
        Ran::Failed(e) => panic!("{}: {e}\n{text}", sim.name()),
    };
    let want = format!("v({node})");
    for p in &plots {
        if let Some(i) = p
            .variables
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(&want) || v.name.eq_ignore_ascii_case(node))
        {
            return p.real_data[i][0];
        }
    }
    panic!("{}: no {want} in {text}", sim.name())
}

/// RG `.SUBCKT` (`PARAMS:`): a subcircuit default is a
/// number or a `{}` expression; Xyce 7.10 reads a bare name there as a
/// string ("Cannot convert 'RG' to double for expression R0"). ngspice reads
/// it as the parameter (manual §2.4.3). 1 mA into the 2k default: 2 V.
#[test]
fn a_bare_name_default_is_the_parameter() {
    let deck = "* t\n.param rg=2k\n.subckt s a b r0=rg\nr1 a b {r0}\n.ends\n\
        i1 0 1 1m\nx1 1 0 s\n.op\n.end\n";
    both_op(deck, "1", 2.0);
}

/// RG §2.2 (`{}` expressions): a MOSFET's `w=w l=l` in a subcircuit,
/// unbraced, selects no bin of a binned model on Xyce 7.10 ("Model is
/// required for device M0 and no valid model card found"; the gf180
/// `nmos_3p3` decks); braced it selects the bin a flat `M1 ... w=0.6u
/// l=1u` does. The bins differ in `vth0` only, so the wrong bin moves the
/// drain current.
#[test]
fn a_binned_model_in_a_subcircuit_selects_its_bin() {
    let deck = "* t\n\
        .model nch.0 nmos level=54 lmin=0.1u lmax=1.5u wmin=0.1u wmax=10u vth0=0.4\n\
        .model nch.1 nmos level=54 lmin=1.5u lmax=10u wmin=0.1u wmax=10u vth0=0.5\n\
        .subckt s d g sb w=1u l=1u\nm0 d g sb sb nch w=w l=l\n.ends\n\
        vg g 0 1\nrs d2 sx 1k\nvd2 d2 0 1\nx1 sx g 0 s w=0.6u l=1u\n\
        .op\n.end\n";
    same_op(deck, "sx");
}

/// RG `D` (levels 1, 2, 200, 2002): ngspice's level-3 diode (ngspice-45
/// `diosetup.c`: level 1 plus the area from `w`, `l` and the metal and poly
/// capacitors) is Xyce's level 1 when neither is given; ngspice's `tref`
/// alias (`dio.c`) is Xyce's `TNOM` (Xyce lacks `TREF`, so 7.10 runs the
/// card at TNOM 27). 1 mA forward through area 2, the card's parameters
/// given at -40 C.
#[test]
fn a_level_3_diode_is_xyce_level_1() {
    let deck = "* t\n.model dm d level=3 is=1e-14 n=1.05 rs=10 tref=-40\n\
        i1 0 1 1m\nd1 1 0 dm area=2\n.op\n.end\n";
    same_op(deck, "1");
}

/// RG `Q` (level 1): ngspice's BJT `tref` (`bjt.c`, alias of `tnom`) is
/// Xyce's `TNOM`; Xyce 7.10 ignores `TREF` (the gf180 vnpn decks: 1.24e-10
/// vs 1.55e-5 A). A diode-connected npn fed 10 uA, its card at -40 C.
#[test]
fn a_bjt_tref_is_xyce_tnom() {
    let deck = "* t\n.model qn npn is=1e-16 bf=100 tref=-40\n\
        i1 0 b 10u\nq1 b b 0 qn\n.op\n.end\n";
    same_op(deck, "b");
}

/// RG `Q` (level 234, HICUM/L2): ngspice's HICUM/L2 is level 8
/// (`hicum2.c`), with older names for six parameters (`ceox` is `cbepar`,
/// `alhc` `ahc`, ...) and `tref` for `tnom`; Xyce 7.10 finds no model card at
/// level 8 ("Model is required for device Q1"). Xyce's HICUM takes the
/// substrate and thermal nodes (RG `Q`), both given here.
#[test]
fn a_hicum_level_8_is_xyce_level_234() {
    let deck = "* t\n.model qh npn level=8 c10=9.074e-30 qp0=1.008e-13 \
        cjei0=8.869e-15 ceox=1e-15 alhc=0.2 tref=-40\n\
        i1 0 b 10u\nq1 b b 0 0 t qh\n.op\n.end\n";
    same_op(deck, "b");
}

/// RG `D`, `Q` model tables: Xyce's level-1 diode and BJT have none of
/// ngspice's `tlev`/`tlevc` temperature coefficients (ngspice-45 `dio.c`
/// `cta`, `bjt.c` `tbf1`), and Xyce 7.10 runs a card setting one with a
/// warning only. The instance using such a card is refused (W0050), the
/// card left out (W0053); a card no instance uses only noted, the deck
/// written. At its off value the parameter is left out of the card.
#[test]
fn a_parameter_xyce_lacks_is_refused_where_it_is_used() {
    for (card, inst) in [
        (".model dm d level=3 is=1e-14 tlevc=1 cta=1e-3", "d1 1 0 dm"),
        (".model qn npn is=1e-16 tbf1=4e-3", "q1 1 1 0 qn"),
    ] {
        let deck = format!("* t\n{card}\nv1 1 0 1\n{inst}\n.op\n.end\n");
        assert_eq!(refusal(&deck), [Code::W0053, Code::W0050], "{card}");
        let unused = format!("* t\n{card}\nv1 1 0 1\nr1 1 0 1k\n.op\n.end\n");
        let w = render_text(&unused, Dialect::Ngspice, "t", Dialect::Xyce).expect("written");
        let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
        assert_eq!(codes, [Code::W0053], "{card}");
        assert!(!w.text.contains(".model"), "{}", w.text);
    }
    let off = "* t\n.model dm d level=3 is=1e-14 tlevc=0 cta=0 bv_max=5\n\
        i1 0 1 1m\nd1 1 0 dm\n.op\n.end\n";
    assert!(!write(off).contains("cta"), "{}", write(off));
    same_op(off, "1");
}

/// The gf180 shape: a library section holding a card Xyce lacks
/// parameters for and one it computes, called by `.lib`. The deck using
/// the good card is written (and the library, the bad card left out,
/// W0053); the deck using the bad card is refused at its instance (W0050),
/// the card judged in the library's own file.
#[test]
fn a_library_card_is_judged_at_the_deck_that_uses_it() {
    let lib = "* lib\n.lib dio\n.model good d is=1e-14\n\
        .model bad d level=3 is=1e-14 tlevc=1 cta=1e-3\n.endl dio\n";
    for (inst, refused) in [("d1 1 0 good", false), ("d1 1 0 bad", true)] {
        let deck = format!("* t\n.lib 'lib.sp' dio\ni1 0 1 1m\n{inst}\n.op\n.end\n");
        let files = [("/k/t.sp", deck.as_str()), ("/k/lib.sp", lib)];
        let kit = Kit::load(&["/k/t.sp".into()], Form::Deck, Dialect::Ngspice, &mut |p| {
            files
                .iter()
                .find(|(n, _)| Path::new(n) == p)
                .map(|(_, t)| t.to_string())
                .ok_or(LoadError::NotFound)
        });
        let top = render_kit(&kit, FileId(0), "t", Dialect::Xyce);
        match (top, refused) {
            (Err(e), true) => {
                let codes: Vec<Code> = e.diags.iter().map(|d| d.code).collect();
                assert_eq!(codes, [Code::W0050], "{inst}");
            }
            (Ok(_), false) => {}
            (r, _) => panic!("{inst}: {:?}", r.map(|w| w.text).map_err(|e| e.messages)),
        }
        let w = render_kit(&kit, FileId(1), "t", Dialect::Xyce).expect("the library written");
        assert!(w.text.contains(".model good") && !w.text.contains(".model bad"), "{}", w.text);
    }
}

/// RG capacitor and inductor model `C`, `L` (multipliers): ngspice's model
/// `c=`, `l=` is the value of an element that gives none (ngspice-45
/// `cap.c`, `ind.c`, `inpgmod.c`); Xyce multiplies the element's value by
/// it: refused. (The resistor's `r=` is lifted onto its instances by
/// stream B's writer.)
#[test]
fn a_c_or_l_model_value_is_refused() {
    for (card, inst) in [(".model my c c=1p", "c1 1 0 my"), (".model my l l=1u", "l1 1 0 my")] {
        let deck = format!("* t\n{card}\nv1 1 0 1\n{inst}\n.op\n.end\n");
        assert_eq!(refusal(&deck), [Code::W0053, Code::W0051], "{card}");
    }
}

/// The outputs Xyce writes no vector for are noted, the deck written: an
/// `.op` beside another analysis (RG `.OP`: printed only; spice-bench
/// `circuitsim90/BJT/vreg.sp` lost its op plot), the integrated noise (RG
/// `.NOISE`: to stdout), an ngspice `@dev[param]` vector (RG `.PRINT`).
#[test]
fn outputs_xyce_writes_no_vector_for_are_noted() {
    let rc = "v1 1 0 dc 1 ac 1\nr1 1 2 1k\nr2 2 0 1k\n";
    for (bench, code) in [
        (".op\n.dc v1 0 1 0.5", Code::W0052),
        (".noise v(2) v1 dec 10 1 1k", Code::W0052),
        (".save @r1[i]\n.op", Code::W0052),
    ] {
        let deck = format!("* t\n{rc}{bench}\n.end\n");
        let w = render_text(&deck, Dialect::Ngspice, "t", Dialect::Xyce)
            .unwrap_or_else(|e| panic!("{}\n{deck}", e.messages));
        let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
        assert!(codes.contains(&code), "{bench}: {codes:?}\n{}", w.text);
    }
}

/// RG `.MEASURE` (`TRIG <variable>=<value>`): Xyce 7.10 stops on
/// ngspice's `TRIG v(1) VAL=0.5` (manual §11.4) with "has invalid MEASURE
/// line" (spice-bench `examples/measure/mos-meas-dc.sp`). A 1 V/s ramp from
/// 0.5 V to 0.8 V: 0.3 s.
#[test]
fn a_trig_targ_measure_runs_on_xyce() {
    let deck = "* t\nvs 1 0 pwl 0 0 1 1\nr1 1 0 1k\n.tran 10m 1\n\
        .meas tran d1 trig v(1) val=0.5 rise=1 targ v(1) val=0.8 rise=1\n.end\n";
    let d1 = xyce_measure(&write(deck), "D1");
    assert!((d1 - 0.3).abs() <= 1e-9, "D1 = {d1}");
}

/// RG `M` (level 18 VDMOS, no `vdmos` model type; Xyce 7.10: "Parameter
/// pchan not formatted correctly", spice-bench
/// `examples/vdmos/ro_11_vdmos.sp`): ngspice's VDMOS is refused.
#[test]
fn an_ngspice_vdmos_is_refused() {
    let deck = "* t\n.model n1 vdmos vto=1 kp=1e-5\nvd d 0 1\nm1 d d 0 n1\n.op\n.end\n";
    assert_eq!(refusal(deck), [Code::W0053, Code::W0050]);
}
