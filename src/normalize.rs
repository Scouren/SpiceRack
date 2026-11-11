//! Variable name normalization for cross-backend consistency.
//!
//! Every backend's vector names map to one lowercase canonical form:
//! - node voltages lose their `v()` wrapper and reference node: `V(OUT,0)` -> `out`
//! - branch currents become `i(name)`: `I(V1)`, `v1#branch`, Spectre `V1:p`,
//!   VACASK `v1:flow(br)` -> `i(v1)`
//! - LTspice's and VACASK's `:` hierarchy separator becomes `.`:
//!   `V(x1:internal)`, VACASK `x1:internal` -> `x1.internal`
//! - a `.tf` result's vectors get one name each: `transfer_function`,
//!   `input_impedance`, `output_impedance` (VACASK reports one pair per
//!   source and keeps it: `transfer_function(vin)`)
//! - anything else (`time`, `frequency`, `onoise_spectrum`, bare Spectre nodes) is lowercased
//!
//! Cites: none (each simulator's raw-file vector names).
//!
//! Contents:
//! - `pub fn canonical(name: &str, backend: Option<Dialect>) -> (String, bool)`

use crate::syntax::Dialect;

/// Canonical form of a raw vector name written by `backend` (`None`: named
/// as ngspice names it), and whether it names a current. One lowercased copy
/// of `name`, plus one more for a name it rewrites.
pub fn canonical(name: &str, backend: Option<Dialect>) -> (String, bool) {
    let lower = name.to_lowercase();
    // LTspice and VACASK separate hierarchy levels with `:`.
    let colon = matches!(backend, Some(Dialect::Ltspice | Dialect::Vacask));
    let hierarchy = |s: &str| {
        if colon {
            s.replace(':', ".")
        } else {
            s.to_string()
        }
    };
    if let Some(inner) = lower.strip_prefix("i(").and_then(|s| s.strip_suffix(')')) {
        return (format!("i({})", hierarchy(inner)), true);
    }
    if let Some((dev, _)) = lower.split_once("#branch") {
        return (format!("i({dev})"), true);
    }
    // VACASK (36be560) names a branch current `<device>:flow(<branch>)`
    // (`vdd:flow(br)`); every other `:` of its names is hierarchy (`xu1:m`).
    if backend == Some(Dialect::Vacask)
        && let Some((dev, _)) = lower.split_once(":flow(")
    {
        return (format!("i({})", hierarchy(dev)), true);
    }
    // Spectre terminal currents are `<device>:<terminal>`; inside v() a `:` is hierarchy.
    if backend == Some(Dialect::Spectre)
        && !lower.starts_with("v(")
        && let Some((dev, _)) = lower.split_once(':')
    {
        return (format!("i({dev})"), true);
    }
    let wrapped = lower
        .strip_prefix("v(")
        .or_else(|| lower.strip_prefix("v ("));
    let node = match wrapped.and_then(|s| s.strip_suffix(')')) {
        Some(inner) => hierarchy(inner.split(',').next().unwrap_or(inner).trim()),
        None if colon => hierarchy(&lower),
        None => lower,
    };
    (tf_name(node, backend), false)
}

// ---- private ----

/// `name` (lowercase, out of its `v()`) under the one name a `.tf` vector
/// has on every backend, or `name` itself when it is none. The names each
/// simulator writes:
/// - ngspice-45 (and LTspice, SPICE3's names): `transfer_function`,
///   `<source>#input_impedance`, `output_impedance_at_<output>`
/// - ESPice 0.3.0: `transfer_function`, `input_resistance`,
///   `output_resistance`
/// - VACASK 36be560 (`lib/core.cpp:183-185`): `tf(<source>)` and
///   `zin(<source>)` for every source of the deck, which keep it
///   (`transfer_function(vin)`), and no output impedance
fn tf_name(name: String, backend: Option<Dialect>) -> String {
    debug_assert!(!name.chars().any(char::is_uppercase), "{name}");
    let espice = backend == Some(Dialect::Espice);
    let vacask = backend == Some(Dialect::Vacask);
    if name.ends_with("#input_impedance") || (espice && name == "input_resistance") {
        return "input_impedance".to_string();
    }
    if name.starts_with("output_impedance_at_") || (espice && name == "output_resistance") {
        return "output_impedance".to_string();
    }
    if vacask && let Some(source) = name.strip_prefix("tf(") {
        return format!("transfer_function({source}");
    }
    if vacask && let Some(source) = name.strip_prefix("zin(") {
        return format!("input_impedance({source}");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(name: &str, backend: &str) -> String {
        canonical(name, Some(backend.parse().unwrap())).0
    }

    fn cur(name: &str, backend: &str) -> bool {
        canonical(name, Some(backend.parse().unwrap())).1
    }

    #[test]
    fn no_backend_reads_ngspice_names() {
        for name in ["v(x1:a)", "I(V1)", "v1#branch", "V1:p", "time"] {
            assert_eq!(
                canonical(name, None),
                canonical(name, Some(Dialect::Ngspice)),
                "{name}"
            );
        }
        assert_eq!(canonical("v1#branch", None), ("i(v1)".to_string(), true));
        assert_eq!(canonical("V1:p", None), ("v1:p".to_string(), false));
    }

    // ── ngspice ──

    #[test]
    fn test_ngspice_voltage() {
        assert_eq!(norm("v(out)", "ngspice"), "out");
        assert_eq!(norm("V(OUT)", "ngspice"), "out");
    }

    #[test]
    fn test_ngspice_voltage_with_ref() {
        assert_eq!(norm("v(out,0)", "ngspice"), "out");
    }

    #[test]
    fn test_ngspice_current() {
        assert_eq!(norm("i(v1)", "ngspice"), "i(v1)");
        assert_eq!(norm("I(V1)", "ngspice"), "i(v1)");
    }

    #[test]
    fn test_ngspice_hierarchy() {
        assert_eq!(norm("v(x1.internal)", "ngspice"), "x1.internal");
        assert_eq!(norm("V(X1.INTERNAL)", "ngspice"), "x1.internal");
    }

    // ── LTspice ──

    #[test]
    fn test_ltspice_voltage() {
        assert_eq!(norm("V(out)", "ltspice"), "out");
    }

    #[test]
    fn test_ltspice_hierarchy() {
        assert_eq!(norm("V(x1:internal)", "ltspice"), "x1.internal");
    }

    #[test]
    fn test_ltspice_current() {
        assert_eq!(norm("I(V1)", "ltspice"), "i(v1)");
    }

    // ── Spectre ──

    #[test]
    fn test_spectre_bare_node() {
        assert_eq!(norm("out", "spectre"), "out");
        assert_eq!(norm("OUT", "spectre"), "out");
    }

    #[test]
    fn test_spectre_current() {
        // Spectre uses "V1:p" for current through V1 positive terminal
        assert_eq!(norm("V1:p", "spectre"), "i(v1)");
    }

    #[test]
    fn test_spectre_current_isrc() {
        assert_eq!(norm("I0:src", "spectre"), "i(i0)");
    }

    #[test]
    fn test_spectre_hierarchy() {
        // Spectre uses `.` for hierarchy (same as canonical)
        assert_eq!(norm("x1.internal", "spectre"), "x1.internal");
    }

    // ── Vacask ──

    #[test]
    fn test_vacask_bare_node() {
        assert_eq!(norm("out", "vacask"), "out");
    }

    #[test]
    fn test_vacask_current() {
        // VACASK 36be560's own name for the branch current of `vdd`.
        assert_eq!(norm("vdd:flow(br)", "vacask"), "i(vdd)");
        assert!(cur("vdd:flow(br)", "vacask"));
    }

    #[test]
    fn p3_032_a_vacask_subcircuit_node_is_a_voltage() {
        // Oracle: the names VACASK 36be560 wrote for a 1k/1k/2k chain inside
        // instance xu1, and inside xu2's instance xin
        // (research/impl/evidence/P3-032/op1.raw): nodes, at 1.125 V and 0.75 V.
        for (raw, name) in [
            ("xu1:m", "xu1.m"),
            ("xu1:q", "xu1.q"),
            ("xu2:xin:m", "xu2.xin.m"),
            ("d1:a_int", "d1.a_int"),
        ] {
            assert_eq!(
                canonical(raw, Some(Dialect::Vacask)),
                (name.to_string(), false),
                "{raw}"
            );
        }
        // A current inside an instance keeps its path (evidence inner-op1.raw).
        assert_eq!(
            canonical("xu1:v1:flow(br)", Some(Dialect::Vacask)),
            ("i(xu1.v1)".to_string(), true)
        );
    }

    // ── Sweep variables ──

    #[test]
    fn test_sweep_vars_unchanged() {
        assert_eq!(norm("time", "ngspice"), "time");
        assert_eq!(norm("frequency", "ngspice"), "frequency");
    }

    // ── Noise spectra ──

    #[test]
    fn test_noise_vars_preserved() {
        assert_eq!(norm("inoise_spectrum", "ngspice"), "inoise_spectrum");
        assert_eq!(norm("onoise_spectrum", "ngspice"), "onoise_spectrum");
    }

    // ── .tf ──

    #[test]
    fn p3_095_tf_vectors_have_one_name_on_every_backend() {
        // Oracle: the names each simulator wrote for `.tf V(out) Vin` of a
        // 1k/3k divider (research/impl/evidence/P3-095/RESULT.txt: ngspice-45,
        // ESPice 0.3.0, VACASK 36be560).
        for (raw, backend, name) in [
            ("v(Transfer_function)", "ngspice", "transfer_function"),
            ("v(vin#Input_impedance)", "ngspice", "input_impedance"),
            (
                "v(output_impedance_at_V(out))",
                "ngspice",
                "output_impedance",
            ),
            ("transfer_function", "espice", "transfer_function"),
            ("input_resistance", "espice", "input_impedance"),
            ("output_resistance", "espice", "output_impedance"),
            ("tf(vin)", "vacask", "transfer_function(vin)"),
            ("zin(vin)", "vacask", "input_impedance(vin)"),
            ("tf(xu1:v1)", "vacask", "transfer_function(xu1.v1)"),
        ] {
            assert_eq!(norm(raw, backend), name, "{raw} on {backend}");
            assert!(!cur(raw, backend), "{raw} on {backend}");
        }
        // Another simulator's node of that name is a node.
        assert_eq!(norm("v(input_resistance)", "ngspice"), "input_resistance");
        assert_eq!(norm("v(tf)", "vacask"), "tf");
    }

    // ── is_current_name ──

    #[test]
    fn test_is_current_explicit() {
        assert!(cur("i(v1)", "ngspice"));
    }

    #[test]
    fn test_is_current_spectre_terminal() {
        assert!(cur("V1:p", "spectre"));
        assert!(cur("I0:src", "spectre"));
        assert!(!cur("I0:src", "vacask"));
    }

    #[test]
    fn test_is_not_current_voltage() {
        assert!(!cur("v(out)", "ngspice"));
    }

    #[test]
    fn test_is_not_current_spectre_voltage_wrapper() {
        // v(x1:internal) in Spectre context -- the `:` is inside v(), so
        // it's hierarchy, not a current
        assert!(!cur("v(x1:internal)", "spectre"));
    }

    // ── Issue 06: Cross-backend result parity ──

    #[test]
    fn test_same_circuit_same_normalized_keys() {
        let names_ng = ["time", "v(out)", "v(in)", "i(V1)"];
        let names_lt = ["TIME", "V(OUT)", "V(IN)", "I(V1)"];
        let names_sp = ["time", "out", "in", "V1:p"];

        let norm_ng: Vec<String> = names_ng.iter().map(|n| norm(n, "ngspice")).collect();
        let norm_lt: Vec<String> = names_lt.iter().map(|n| norm(n, "ltspice")).collect();
        let norm_sp: Vec<String> = names_sp.iter().map(|n| norm(n, "spectre")).collect();

        assert_eq!(norm_ng, norm_lt, "ngspice vs ltspice");
        assert_eq!(norm_ng, norm_sp, "ngspice vs spectre");
    }
}
