//! A harmonic balance of one tone, read from a Xyce or an HSPICE `.hb`
//! card and written as each target's own analysis. The oracle is the
//! closed-form phasor of an RC low-pass driven by a sine, met at the tone
//! by the real simulator of each target that runs here on the written
//! deck: VACASK, ESPice, and the HSPICE text on ESPice
//! `--tokenizer=hspice`. The Xyce source's own run is a record
//! (`research/impl/evidence/S4-rf-hb-one-tone/`: Xyce prints a two-sided
//! spectrum, half the phasor at plus and minus the tone, and the harness's
//! `Xyce -r` refuses an `.HB` deck). Spectre has no binary here: its line
//! is held to its manual's. ngspice and LTspice have no harmonic balance:
//! their refusal names it.
//!
//! Unsupported fails, never skips: without `vacask` and `espice` on PATH
//! the runs fail; run the tests in `nix develop .#testing`.
//!
//! Cites: ZERO-LEDGER P5-004 (step 2), P5-007, P5-079; Xyce RG 7.10
//! §2.1.13 (`.HB`), Table 2-14 (`NUMFREQ`, default 10); HSPICE Reference
//! Manual: Commands and Control Options E-2010.12 p.115 (`.HB TONES=
//! NHARMS=`); Spectre Reference 19.1 pp.90-91 (`hb`: `fundfreqs`,
//! `maxharms`); VACASK `docs/cmd-analysis-hb.md` (`freq`, `nharm`).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{Tol, spec_example};
use spicerack::result::RawData;
use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;

/// A 1 V sine at 100 kHz through 1 kΩ into 2 nF, up to its analysis
/// cards: ωRC = 0.4π.
const RC: &str = "* t\nV1 1 0 SIN(0 1 1e5)\nR1 1 2 1k\nC1 2 0 2n\n";

/// The balance of a linear circuit is one linear solve: VACASK and ESPice
/// give the closed form to 15 digits (measured); held to 1e-9.
const LINEAR: Tol = Tol {
    rel: 1e-9,
    floor: 0.0,
};

// ---- private ----

/// The phasor (real, imaginary) of node `node` at frequency `f`, from the
/// first complex plot of `plots` that holds it (`v(node)`, or VACASK's
/// bare `node`); a plot's first variable is its frequency axis.
fn phasor(plots: &[RawData], node: &str, f: f64) -> (f64, f64) {
    let want = format!("v({node})");
    let named = |n: &str| n.eq_ignore_ascii_case(&want) || n.eq_ignore_ascii_case(node);
    for p in plots.iter().filter(|p| p.is_complex) {
        let Some(i) = p.variables.iter().position(|v| named(&v.name)) else {
            continue;
        };
        let axis = &p.complex_data[0];
        let row = axis.iter().position(|z| (z.re - f).abs() <= 1e-9 * f);
        let z = p.complex_data[i][row.expect("the tone's row")];
        return (z.re, z.im);
    }
    panic!("no complex {want} in {} plot(s)", plots.len())
}

/// ZERO-LEDGER P5-004 step 2, P5-007. Xyce's `.HB f` with `.OPTIONS HBINT
/// NUMFREQ=k` (RG 7.10 §2.1.13, Table 2-14) is HSPICE's and ESPice's `.hb
/// tones=f nharms=k` (cmdref E-2010.12 p.115), Spectre's `hb` with
/// `fundfreqs=[f] maxharms=[k]` (Reference 19.1 pp.90-91) and VACASK's
/// `analysis … hb freq=[f] nharm=k` (`docs/cmd-analysis-hb.md`). Oracle:
/// the closed form V(2) = −j/(1 + jωRC), ωRC = 0.4π (the phasor of a 1 V
/// sine through 1 kΩ into 2 nF), met at the tone by real VACASK, real
/// ESPice and the HSPICE text on ESPice. Xyce 7.10 on the source prints
/// half of it at ±100 kHz (−2.43615831e-01 − 1.93863318e-01j, recorded:
/// the harness's `Xyce -r` refuses `.HB`). Tolerance: `LINEAR`. With no
/// `NUMFREQ` Xyce computes 10 harmonics (Table 2-14; 21 rows, run).
/// ngspice-45 ("unimplemented dot command '.hb'") and LTspice (help, "Dot
/// Commands": seven analyses, none a harmonic balance) refuse it W0015.
#[test]
fn p5_004_a_xyce_one_tone_hb_is_each_targets_own_analysis() {
    let xy = Dialect::Xyce;
    let deck = format!("{RC}.hb 1e5\n.options hbint numfreq=5\n.print hb v(2)\n.end\n");
    let cite = "HSPICE cmdref E-2010.12 p.115: .HB TONES=F1 NHARMS=H1";
    for to in [Dialect::Hspice, Dialect::Espice] {
        spec_example(cite, &deck, xy, to, &[".hb tones=100k nharms=5"]);
    }
    let cite = "Spectre Reference 19.1 pp.90-91: hb fundfreqs=[...] maxharms=[...]";
    let line = "hb1 hb fundfreqs=[100k] maxharms=[5]";
    spec_example(cite, &deck, xy, Dialect::Spectre, &[line]);
    let cite = "VACASK docs/cmd-analysis-hb.md: freq, nharm";
    let line = "  analysis hb1 hb freq=[1e5] nharm=5";
    spec_example(cite, &deck, xy, Dialect::Vacask, &[line]);
    // Xyce's own deck: the card among its analyses, its count once.
    let own = oracle::written(&deck, xy, xy);
    let count = |l: &str| own.text.lines().filter(|x| *x == l).count();
    let (card, option) = (count(".hb 100k"), count(".options hbint numfreq=5"));
    assert_eq!((card, option), (1, 1), "{}", own.text);
    let noted = oracle::notes(&own);
    assert!(!noted.contains(&Code::W0027), "read, not copied: {noted:?}");
    // No count on the deck: Xyce's default.
    let bare = format!("{RC}.hb 1e5\n.print hb v(2)\n.end\n");
    let cite = "Xyce RG 7.10 Table 2-14: NUMFREQ default 10";
    let line = ".hb tones=100k nharms=10";
    spec_example(cite, &bare, xy, Dialect::Hspice, &[line]);
    let x = 0.4 * std::f64::consts::PI;
    let want = (-x / (1.0 + x * x), -1.0 / (1.0 + x * x));
    for to in [Dialect::Vacask, Dialect::Espice, Dialect::Hspice] {
        let got = phasor(&oracle::run(to, &oracle::write(&deck, xy, to)), "2", 1e5);
        assert!(
            LINEAR.within(got.0, want.0) && LINEAR.within(got.1, want.1),
            "{to:?}: V(2) at 100 kHz {got:?}, the closed form {want:?}"
        );
    }
    for to in [Dialect::Ngspice, Dialect::Ltspice] {
        let codes = oracle::refusal(&deck, xy, to);
        assert!(
            codes.contains(&Code::W0015) && !codes.contains(&Code::W0001),
            "{to:?}: {codes:?}"
        );
    }
}

/// ZERO-LEDGER P5-079. HSPICE RF's `.HB TONES=f NHARMS=k` (cmdref
/// E-2010.12 p.115) and the positional `.hb f k` of ESPice's HSPICE decks
/// (`tests/netlist/hbnoise/lti_rc_hspice.sp`; ESPice 0.3.0 runs both) are
/// one analysis: Xyce's `.HB f` with `.OPTIONS HBINT NUMFREQ=k` (RG 7.10
/// §2.1.13; Xyce 7.10 reads the options card after the `.hb` card to the
/// source's own phasor, run), VACASK's `hb`, and HSPICE's own keyed card.
/// Oracle: each manual's form, and the closed form of the test above met
/// by real VACASK on the deck written from the positional card.
#[test]
fn p5_079_an_hspice_one_tone_hb_is_read_in_both_forms() {
    let hs = Dialect::Hspice;
    let x = 0.4 * std::f64::consts::PI;
    let want = (-x / (1.0 + x * x), -1.0 / (1.0 + x * x));
    for card in [".hb tones=1e5 nharms=5", ".hb 1e5 5"] {
        let deck = format!("{RC}{card}\n.print hb v(2)\n.end\n");
        let cite = "Xyce RG 7.10 §2.1.13 .HB, Table 2-14 NUMFREQ";
        let lines = [".hb 100k", ".options hbint numfreq=5"];
        spec_example(cite, &deck, hs, Dialect::Xyce, &lines);
        let cite = "HSPICE cmdref E-2010.12 p.115: .HB TONES=F1 NHARMS=H1";
        spec_example(cite, &deck, hs, hs, &[".hb tones=100k nharms=5"]);
        let cite = "VACASK docs/cmd-analysis-hb.md: freq, nharm";
        let line = "  analysis hb1 hb freq=[1e5] nharm=5";
        let text = spec_example(cite, &deck, hs, Dialect::Vacask, &[line]);
        let got = phasor(&oracle::run(Dialect::Vacask, &text), "2", 1e5);
        assert!(
            LINEAR.within(got.0, want.0) && LINEAR.within(got.1, want.1),
            "{card}: V(2) at 100 kHz {got:?}, the closed form {want:?}"
        );
    }
}

/// A card that is no balance of one driven tone stays the source's text
/// (ZERO-LEDGER P5-007): Xyce's two-tone `.HB f1 f2`, and its oscillator
/// analysis (`HBOSC=1`: the tone is a first guess and the circuit drives
/// itself; Xyce 7.10 on its `HB/RC/lcosc.cir` finds a 0.584 V oscillation
/// with no source, where a driven balance has the zero solution). Oracle:
/// Xyce's own deck keeps each card as written, and no other target is
/// handed a one-tone analysis. A guard: it passes before the reader
/// exists, and goes red when `hb_card` drops its `hbosc` or its one-field
/// check.
#[test]
fn p5_007_a_two_tone_or_oscillator_hb_is_not_read_as_one_tone() {
    let xy = Dialect::Xyce;
    for cards in [
        ".hb 1e5 2e2\n.options hbint numfreq=5,3",
        ".hb 1e5\n.options hbint numfreq=5 hbosc=1",
    ] {
        let deck = format!("{RC}{cards}\n.print hb v(2)\n.end\n");
        let card = cards.lines().next().expect("a card");
        let own = oracle::write(&deck, xy, xy);
        assert!(own.lines().any(|l| l == card), "{card}:\n{own}");
        for to in [Dialect::Vacask, Dialect::Hspice, Dialect::Spectre] {
            let codes = oracle::refusal(&deck, xy, to);
            assert!(!codes.is_empty(), "{to:?}: {card}");
        }
    }
}
