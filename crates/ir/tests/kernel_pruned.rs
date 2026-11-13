//! R100's pruning, the combinations no simulator has (`rewrite/IR-PRUNE.md`, group 3): each is a
//! `FieldDomain` row now, beside its legal neighbour one field away. A sine 0 before its delay
//! (`PreDelay::Zero` is ngspice's AM and SFFM only); an AM that is its offset before its delay
//! (ngspice 0, VACASK the t = delay value); an `if` whose condition reads the temperature or
//! the circuit (decided once from parameters everywhere, INV-semantics 11.08); a subcircuit
//! formal with no default (every row names a default); a loop over a subcircuit's default (no
//! row; Xyce's `.dc X1:…` keeps the DC axis); diffs-2's runs (f78efcca): a sine or an SFFM that
//! is its offset before its delay, and a transient with no step ceiling.
//!
//! Oracle: the inventories' rows the prune file cites, each deck read by hand.
//!
//! Contents: tests only.

use spicerack_ir::{Design, Diags, Violation, check, diag::violation_code};

// ---- private ----

/// Whether the deck is refused as a field fault: by the IR text reader (which refuses every row
/// `check` names, `text::parse`) or, read unchecked, by `check` with `FieldDomain` alone.
fn refused(lines: &str) -> bool {
    let text = format!("file \"t.sp\" deck\n{lines}\n");
    let mut d = Design::new();
    let mut diags = Diags::new();
    spicerack_ir::parse_unchecked(&text, &mut d.builder(), &mut diags);
    if !diags.is_empty() {
        return true;
    }
    spicerack_ir::link(&mut d, &mut diags);
    check(&d, &mut diags);
    let by_code = |c| Violation::ALL.into_iter().find(|v| violation_code(*v) == c);
    let found: Vec<_> = diags
        .rows()
        .iter()
        .filter_map(|r| by_code(r.code))
        .collect();
    assert!(
        found.iter().all(|&v| v == Violation::FieldDomain),
        "{lines}: {found:?}"
    );
    !found.is_empty()
}

/// A voltage source driven by `wave`.
fn source(wave: &str) -> String {
    format!("  element vsource v1 (a 0) dc=0 wave={wave}")
}

#[test]
fn each_combination_no_simulator_has_is_refused() {
    let sin = |pre: &str| {
        source(&format!(
            "sin(offset=0 amplitude=1 freq=1 delay=1 damping=0 phase_deg=0 pre={pre})"
        ))
    };
    let sffm = |pre: &str| {
        source(&format!(
            "sffm(offset=0 amplitude=1 carrier_freq=10 mod_index=2 signal_freq=1 delay=1 phase_c_deg=0 phase_m_deg=0 law=spice3 pre={pre})"
        ))
    };
    let am = |pre: &str| {
        source(&format!(
            "am(offset=0 amplitude=1 mod_offset=1 mod_index=1 mod_freq=1 carrier_freq=4 delay=0 phase_m_deg=0 phase_c_deg=90 mod_time=carrier_phase pre={pre})"
        ))
    };
    let cases = [
        ("sine zero before its delay", sin("zero"), sin("hold_delay_value")),
        // diffs-2's runs (f78efcca): every simulator holds a sine's t = delay value
        (
            "sine offset before its delay",
            sin("offset"),
            sin("hold_delay_value"),
        ),
        // ngspice and ESPice 0, VACASK the held value, Xyce no delay
        ("SFFM offset before its delay", sffm("offset"), sffm("zero")),
        // ngspice and ESPice min(print step, span/50), Xyce span/10, VACASK about span/50
        (
            "transient with no step ceiling",
            "  analysis tran stop=1u print_step=1n cap=none".to_owned(),
            "  analysis tran stop=1u print_step=1n cap=ngspice_rule".to_owned(),
        ),
        ("AM offset before its delay", am("offset"), am("zero")),
        (
            "if on the temperature",
            "  if ($temp > 30)\n    param a = 1".to_owned(),
            "  param t = 1\n  if (t > 30)\n    param a = 1".to_owned(),
        ),
        (
            "formal with no default",
            "  subckt s (a) w".to_owned(),
            "  subckt s (a) w=1".to_owned(),
        ),
        (
            "loop over a subcircuit default",
            "  subckt div (a b) r=1\n  loop sweep (subckt_param div r) (list values=[1])\n    analysis op"
                .to_owned(),
            "  param p = 1\n  loop sweep (param p) (list values=[1])\n    analysis op".to_owned(),
        ),
    ];
    for (what, bad, good) in cases {
        assert!(refused(&bad), "{what}");
        assert!(!refused(&good), "{what}: the neighbour");
    }
}
