//! An ngspice `.model` card → Xyce's: the device's level as Xyce numbers
//! it, each parameter checked against the names Xyce's device reads.
//! Xyce 7.10 ignores a name its device lacks with only a warning
//! ("Netlist warning … unrecognized parameter"), so a parameter ngspice
//! reads and Xyce lacks is a silent wrong number: it is refused (W0050)
//! unless its value is the one at which ngspice's equation is Xyce's, then
//! left out. A name ngspice spells as an alias Xyce lacks is written as
//! Xyce's. Both name tables are generated, one line per device (`<device>
//! <name> <name> …`, lowercase): `xyce_params.txt` from Xyce 7.10
//! `doc/Reference_Guide/<D|Q>_<level>_Device_Model_Params.tex` (first
//! column), and `render/named/ngspice_params.txt` from ngspice-45.
//!
//! Cites: Xyce RG 7.10 `D` (levels 1, 2, 200, 2002: level 1 is SPICE3f5's
//! diode), `Q` (level 1 Gummel-Poon, 234 HICUM/L2), `M` (level 70 BSIM-SOI
//! 4, level 10 BSIM-SOI 3.2; ngspice's B4SOI is 10 and 58,
//! `inpdomod.c:347-348`), Table 2-36 (BSIM3 is
//! level 49, 8 rejected); ngspice-45 `devices/dio/dio.c`, `diosetup.c`
//! (level 3: the area and perimeter from `w`, `l`, the metal and poly
//! capacitances, the only level-3 terms; `diotemp.c` L210 its breakdown
//! current times the area, as Xyce's level 1), `devices/bjt/bjt.c`,
//! `bjtsetup.c` (the defaults), `devices/hicum2/hicum2.c` (level 8, its
//! aliases), `devices/mos6/mos6set.c:93-97` (level 6: `lambda0`, by default
//! `lambda`), `devices/mes/mes.c`, `messetup.c` (MESFET level 1: `vt0`,
//! -2 V by default; Xyce RG `Z`: `VTO`, 0 V);
//! Xyce 7.10 `N_DEV_MOSFET6.C:3055-3062` (`LAMBDA` read only
//! beside `LAMBDA0`). The levels with no Xyce device (`NO_XYCE_DEVICE`):
//! Xyce 7.10 and ngspice-45 run on a one-device deck per type and level
//! (ZERO-LEDGER P1-066, research/impl/evidence/P1-066-xyce-model-levels/).
//!
//! Contents:
//! - `pub(super) enum Shape`, `pub(super) fn xyce_shape_of(kind: &str, params: &[(String, String)]) -> Shape`
//! - `pub(super) fn xyce_model(ngspice: bool, kind: &str, params: &[(String, String)]) -> Result<Option<Vec<(String, String)>>, Code>`

use crate::syntax::diag::Code;
use crate::syntax::render::models::fold;

/// What an ngspice card asks of the instance line Xyce reads it on.
#[derive(Clone, PartialEq, Debug)]
pub(super) enum Shape {
    /// The line as written.
    Plain,
    /// A diode: no `w`/`l` (Xyce's `D` has none, RG 7.10 Table 2-46); at
    /// level 3 (`Some`, the card's mask offset `xw`) they are its area and
    /// perimeter (ngspice-45 `diosetup.c:281-306`).
    Diode(Option<String>),
    /// HICUM/L2: five nodes, as Xyce's level 234 takes (RG 7.10 `Q`).
    Hicum,
}

const XYCE: &str = include_str!("xyce_params.txt");
const NGSPICE: &str = include_str!("../named/ngspice_params.txt");

/// An ngspice parameter Xyce's device lacks: the value at which leaving it
/// out keeps ngspice's equation.
#[derive(Clone, Copy)]
enum Off {
    At(f64),
    /// No value changes a number Xyce computes: a safe-operating-area
    /// limit (a warning in ngspice), a type flag, a term only another
    /// parameter that is refused turns on.
    Any,
}

/// One ngspice device as Xyce's: its key in both name tables, the level
/// Xyce gives it, ngspice's names Xyce spells another way, and the ngspice
/// names Xyce lacks with their off values (every other such name refused).
struct Device {
    key: &'static str,
    level: &'static str,
    renames: &'static [(&'static str, &'static str)],
    off: &'static [(&'static str, Off)],
    /// Names both devices read under other laws: refused off 0.
    differ: &'static [&'static str],
    /// Names whose default is another value on Xyce: ngspice's, written
    /// when the card gives none.
    defaults: &'static [(&'static str, &'static str)],
}

/// The ngspice `.model` types (lowercase) and LEVELs ngspice-45 has a
/// device for and Xyce 7.10 none: Xyce stops on an instance of one ("Model
/// is required for device M1 and no valid model card found"). At MOSFET
/// level 9 it runs another model with no message (ngspice's MOS9, Xyce's
/// BSIM3: one default card at Vds = Vgs = 1 V, 1.036e-5 A against
/// 1.010e-12 A). In order: BSIM1, BSIM2, MOS9, BSIM3SOI FD and DD, HiSIM2,
/// HiSIM_HV; VBIC (Xyce's are levels 11 and 12, another parameter set);
/// MESA (Xyce's `Z` has level 1 alone); HFET1 and HFET2 (no such type).
const NO_XYCE_DEVICE: [(&str, &[u32]); 8] = [
    ("nmos", &[4, 5, 9, 55, 56, 68, 73]),
    ("pmos", &[4, 5, 9, 55, 56, 68, 73]),
    ("npn", &[4, 9]),
    ("pnp", &[4, 9]),
    ("nmf", &[2, 3, 4]),
    ("pmf", &[2, 3, 4]),
    ("nhfet", &[5, 6]),
    ("phfet", &[5, 6]),
];

/// The safe-operating-area limits ngspice reads only for a warning.
const SOA: &[(&str, Off)] = &[
    ("bv_max", Off::Any),
    ("fv_max", Off::Any),
    ("id_max", Off::Any),
    ("ib_max", Off::Any),
    ("ic_max", Off::Any),
    ("pd_max", Off::Any),
    ("te_max", Off::Any),
    ("vbe_max", Off::Any),
    ("vbc_max", Off::Any),
    ("vce_max", Off::Any),
];

/// ngspice's diode (levels 1 and 3) as Xyce's level 1. `tcv` (alias
/// `tbv1`) is ngspice's `bv - tcv*dt`, Xyce's `TBV1` the relative
/// `bv*(1 + tbv1*dt)`: no rename; but `tlev=1` makes ngspice's relative,
/// `bv*(1 - tcv*dt)`, its only difference from `tlev=0` (`diotemp.c`), so
/// there it is Xyce's `TBV1=-tcv` and `tlev` is left out (`breakdown`). ngspice's knee current `ik`/`ikf` is
/// cd/(1+sqrt(cd/ikf)) (`dioload.c:382-386`), Xyce's `IKF` the PSpice
/// cd·sqrt(ikf/(ikf+cd)) (`N_DEV_Diode.C:2396-2401`): no rename either, so
/// Xyce's same name is refused off 0 (`differ`). Level 3's mask offset `xw`
/// is the instance's geometry (`xyce_card`), not a card parameter.
const DIODE: Device = Device {
    key: "diode",
    level: "1",
    renames: &[
        ("isw", "jsw"),
        ("pb", "vj"),
        ("mj", "m"),
        ("vrb", "bv"),
        ("var", "bv"),
        ("ib", "ibv"),
        ("nz", "nbv"),
        ("tref", "tnom"),
    ],
    off: &[
        ("d", Off::Any),
        ("tlevc", Off::At(0.0)),
        ("cta", Off::At(0.0)),
        ("ctc", Off::At(0.0)),
        ("ctp", Off::At(0.0)),
        ("tpb", Off::At(0.0)),
        ("tvj", Off::At(0.0)),
        ("tphp", Off::At(0.0)),
        ("jtun", Off::At(0.0)),
        ("jtunsw", Off::At(0.0)),
        ("ntun", Off::At(30.0)),
        ("xtitun", Off::At(3.0)),
        ("keg", Off::At(1.0)),
        ("ikr", Off::At(0.0)),
        ("tm1", Off::At(0.0)),
        ("tm2", Off::At(0.0)),
        ("ttt1", Off::At(0.0)),
        ("ttt2", Off::At(0.0)),
        ("gap1", Off::At(7.02e-4)),
        ("gap2", Off::At(1108.0)),
        ("rth0", Off::At(0.0)),
        ("cth0", Off::Any),
        ("sens_area", Off::Any),
        // An instance's default on the card (`inpgmod.c`).
        ("area", Off::At(1.0)),
        ("pj", Off::At(0.0)),
        ("perim", Off::At(0.0)),
        ("dtemp", Off::At(0.0)),
        // Level 3's metal and poly capacitors, and its mask offset.
        ("lm", Off::At(0.0)),
        ("lp", Off::At(0.0)),
        ("wm", Off::At(0.0)),
        ("wp", Off::At(0.0)),
        ("xm", Off::At(0.0)),
        ("xp", Off::At(0.0)),
        ("xom", Off::Any),
        ("xoi", Off::Any),
        ("xw", Off::Any),
    ],
    differ: &["ik", "ikf"],
    defaults: &[],
};

/// ngspice's Gummel-Poon BJT (level 1) as Xyce's level 1: Xyce has none of
/// ngspice's `tlev` temperature coefficients (`t<p>1`, `t<p>2`), the
/// substrate diode (`iss`, `ns`) or the quasi-saturation model.
const BJT: Device = Device {
    key: "bjt",
    level: "1",
    renames: &[("tref", "tnom")],
    off: &[
        ("npn", Off::Any),
        ("pnp", Off::Any),
        ("subs", Off::At(1.0)),
        ("tlev", Off::At(0.0)),
        ("tlevc", Off::At(0.0)),
        ("cte", Off::At(0.0)),
        ("ctc", Off::At(0.0)),
        ("cts", Off::At(0.0)),
        ("tvje", Off::At(0.0)),
        ("tvjc", Off::At(0.0)),
        ("tvjs", Off::At(0.0)),
        // NE·(1 + tne1·dT + tne2·dT²) (`bjttemp.c:126`). `tbf1`, `tbf2`
        // have no row: given, at 0 too, they replace the XTB law (L236).
        ("tne1", Off::At(0.0)),
        ("tne2", Off::At(0.0)),
        ("iss", Off::At(0.0)),
        ("ns", Off::At(1.0)),
        ("ibe", Off::At(0.0)),
        ("ibc", Off::At(0.0)),
        ("quasimod", Off::At(0.0)),
        ("rco", Off::Any),
        ("vo", Off::Any),
        ("gamma", Off::Any),
        ("qco", Off::Any),
        ("vg", Off::Any),
        ("cn", Off::Any),
        ("d", Off::Any),
        ("rth0", Off::At(0.0)),
        ("sens_area", Off::Any),
        ("area", Off::At(1.0)),
        ("areab", Off::At(1.0)),
        ("areac", Off::At(1.0)),
        ("m", Off::At(1.0)),
        ("dtemp", Off::At(0.0)),
    ],
    differ: &[],
    defaults: &[],
};

/// ngspice's HICUM/L2 (level 8, `hicum2.c`) as Xyce's (level 234): the
/// same parameters, six under ngspice's older names.
const HICUM: Device = Device {
    key: "hicum",
    level: "234",
    renames: &[
        ("alhc", "ahc"),
        ("aljei", "ajei"),
        ("aljep", "ajep"),
        ("ccox", "cbcpar"),
        ("ceox", "cbepar"),
        ("fbc", "fbcpar"),
        ("fbe", "fbepar"),
        ("tref", "tnom"),
    ],
    off: &[
        ("npn", Off::Any),
        ("pnp", Off::Any),
        ("version", Off::Any),
        // Instance outputs (`HICUM_QUEST_*`).
        ("dtsh", Off::Any),
        ("it", Off::Any),
        ("tk", Off::Any),
        ("area", Off::At(1.0)),
        ("m", Off::At(1.0)),
    ],
    differ: &[],
    defaults: &[],
};

/// ngspice's MESFET (level 1, Statz; `devices/mes/mes.c`) as Xyce's `Z`
/// level 1 (RG 7.10 `Z`): the same parameters, the threshold under
/// ngspice's `vt0`, which Xyce ignores with a warning ("No model parameter
/// VT0 found"), and by default -2 V (`messetup.c`) where Xyce's `VTO` is
/// 0 V (7.10.0 runs: off where ngspice conducts; with `vto=` the two agree
/// to 1.8e-9, research/impl/evidence/P1-066-xyce-mes/runs.txt).
const MES: Device = Device {
    key: "mes1",
    level: "1",
    renames: &[("vt0", "vto")],
    off: &[
        ("nmf", Off::Any),
        ("pmf", Off::Any),
        ("area", Off::At(1.0)),
        ("m", Off::At(1.0)),
    ],
    differ: &[],
    defaults: &[("vto", "-2")],
};

/// The [`Shape`] ngspice's card `kind` with `params` gives its instances.
pub(super) fn xyce_shape_of(kind: &str, params: &[(String, String)]) -> Shape {
    let get = |n: &str| {
        params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(n))
            .map(|(_, v)| v.trim())
    };
    match (kind.to_ascii_lowercase().as_str(), get("level")) {
        ("d", Some("3")) => Shape::Diode(Some(get("xw").unwrap_or("0").to_string())),
        ("d", _) => Shape::Diode(None),
        ("npn" | "pnp", Some("8")) => Shape::Hicum,
        _ => Shape::Plain,
    }
}

/// The card of a `.model x <kind>` with `params` as Xyce reads it (a
/// BSIM3 level from any dialect, the devices from a source `ngspice`'s
/// equations mean):
/// `Ok(None)` unchanged, `Ok(Some(..))` its parameters rewritten (the
/// level Xyce's, aliases renamed, off parameters left out).
///
/// # Errors
/// W0050: a parameter ngspice reads, Xyce's device lacks, and set to
/// another value than its off one, or a model kind or level no Xyce
/// device is ([`NO_XYCE_DEVICE`]);
/// W0051: a C or L model giving the element's value.
pub(super) fn xyce_model(
    ngspice: bool,
    kind: &str,
    params: &[(String, String)],
) -> Result<Option<Vec<(String, String)>>, Code> {
    let level = params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("level"))
        .map(|(_, v)| v.trim());
    let device = match (kind.to_ascii_lowercase().as_str(), level) {
        // Xyce's BSIM3 is level 49 and it rejects 8 (RG Table 2-36).
        ("nmos" | "pmos", Some("8")) => return Ok(Some(leveled(params, "49"))),
        _ if !ngspice => return Ok(None),
        // ngspice's level 6 computes with `lambda0`, by default `lambda`;
        // Xyce's with `LAMBDA`, and only beside `LAMBDA0` (`mos6_lambda`).
        ("nmos" | "pmos", Some("6")) => return Ok(mos6_lambda(params)),
        // ngspice's 10 and 58 are B4SOI (`inpdomod.c:347-348`), Xyce's 70;
        // Xyce's 10 is BSIM-SOI 3.2 and it has no 58 (RG `M` level table).
        ("nmos" | "pmos", Some("10" | "58")) => return Ok(Some(leveled(params, "70"))),
        ("d", None | Some("1" | "3")) => &DIODE,
        ("npn" | "pnp", None | Some("1")) => &BJT,
        ("npn" | "pnp", Some("8")) => &HICUM,
        ("nmf" | "pmf", None | Some("1")) => &MES,
        (k @ ("c" | "l"), _) => return default_value(k, params),
        // ngspice's VDMOS (`devices/vdmos`): Xyce has no `vdmos` model
        // type, and its level-18 VDMOS is another model (RG `M` level 18).
        ("vdmos", _) => return Err(Code::W0050),
        (k, l) if no_xyce_device(k, l) => return Err(Code::W0050),
        _ => return Ok(None),
    };
    let xyce = names(XYCE, device.key);
    let ngspice = names(NGSPICE, device.key);
    debug_assert!(!xyce.is_empty() && !ngspice.is_empty(), "{}", device.key);
    let tlev = params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("tlev"))
        .map_or(Some(0.0), |(_, v)| fold(v));
    let mut out = Vec::with_capacity(params.len());
    for (k, v) in params {
        let low = k.to_ascii_lowercase();
        if device.key == DIODE.key {
            match breakdown(&low, v, tlev) {
                Some(Ok(Some(p))) => out.push(p),
                Some(Ok(None)) => {}
                Some(Err(c)) => return Err(c),
                None => {}
            }
            if matches!(low.as_str(), "tlev" | "tcv" | "tbv1") {
                continue;
            }
        }
        if low == "level" {
            out.push((k.clone(), device.level.to_string()));
            continue;
        }
        if device.differ.contains(&low.as_str()) {
            match fold(v) == Some(0.0) {
                true => continue,
                false => return Err(Code::W0050),
            }
        }
        let name = device
            .renames
            .iter()
            .find(|(from, _)| *from == low)
            .map_or(low.as_str(), |(_, to)| to);
        // Xyce's own name, or one neither simulator reads: as written.
        if xyce.contains(&name) || !ngspice.contains(&low.as_str()) {
            let k = if name == low {
                k.clone()
            } else {
                name.to_string()
            };
            out.push((k, v.clone()));
            continue;
        }
        let off = device.off.iter().chain(SOA).find(|(n, _)| *n == low);
        match off.map(|(_, o)| *o) {
            Some(Off::Any) => {}
            Some(Off::At(x)) if fold(v) == Some(x) => {}
            _ => return Err(Code::W0050),
        }
    }
    for (name, value) in device.defaults {
        if !out.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)) {
            out.push((name.to_string(), value.to_string()));
        }
    }
    debug_assert!(out.len() <= params.len() + device.defaults.len());
    Ok(Some(out))
}

// ---- private ----

/// Whether ngspice's `.model` type `kind` (lowercase) at `level` (the
/// card's text; none is level 1) is a row of [`NO_XYCE_DEVICE`]. A level
/// that is no whole number (an expression, `4.5`) is no row: the card is
/// copied and Xyce reports it.
fn no_xyce_device(kind: &str, level: Option<&str>) -> bool {
    debug_assert!(!kind.bytes().any(|b| b.is_ascii_uppercase()));
    let level = match level.map(fold) {
        None => 1.0,
        Some(Some(l)) => l,
        Some(None) => return false,
    };
    NO_XYCE_DEVICE
        .iter()
        .any(|(k, levels)| *k == kind && levels.iter().any(|&l| f64::from(l) == level))
}

/// An ngspice C or L model whose card gives the element's value (`c=1p`):
/// ngspice's model `cap`/`ind` and an instance parameter on the card are
/// the instance's default (ngspice-45 `cap.c`, `ind.c`, `inpgmod.c`);
/// Xyce's model `C`, `L` multiply the instance's value (RG C, L model
/// tables): refused (W0051). (An R model's value is lifted onto its
/// instances by stream B's writer, `render::spice::values`.) Any other card
/// is unchanged.
fn default_value(
    kind: &str,
    params: &[(String, String)],
) -> Result<Option<Vec<(String, String)>>, Code> {
    let values: &[&str] = match kind {
        "c" => &["c", "cap", "capacitance"],
        _ => &["l", "ind", "inductance"],
    };
    match params
        .iter()
        .any(|(k, _)| values.iter().any(|v| k.eq_ignore_ascii_case(v)))
    {
        true => Err(Code::W0051),
        false => Ok(None),
    }
}

/// `device`'s line of `table`: its names.
fn names(table: &'static str, device: &str) -> Vec<&'static str> {
    table
        .lines()
        .find_map(|l| l.strip_prefix(device)?.strip_prefix(' '))
        .map_or_else(Vec::new, |rest| rest.split(' ').collect())
}

/// An ngspice level-6 MOSFET card as Xyce computes the same λ from:
/// ngspice's is `lambda0`, by default `lambda` (`mos6set.c:93-97`); Xyce's
/// is `LAMBDA`, and only when `LAMBDA0` is given too
/// (`N_DEV_MOSFET6.C:3055-3062`), so both are written with ngspice's
/// value. `None`: the card names neither (0 in both).
fn mos6_lambda(params: &[(String, String)]) -> Option<Vec<(String, String)>> {
    let is = |k: &str, n: &str| k.eq_ignore_ascii_case(n);
    let get = |n: &str| {
        params
            .iter()
            .find(|(k, _)| is(k, n))
            .map(|(_, v)| v.clone())
    };
    let value = get("lambda0").or_else(|| get("lambda"))?;
    let mut out: Vec<(String, String)> = params
        .iter()
        .filter(|(k, _)| !is(k, "lambda") && !is(k, "lambda0"))
        .cloned()
        .collect();
    out.push(("lambda".to_string(), value.clone()));
    out.push(("lambda0".to_string(), value));
    debug_assert!(out.len() <= params.len() + 1, "one name may join");
    Some(out)
}

/// `params` with the level `level`.
fn leveled(params: &[(String, String)], level: &str) -> Vec<(String, String)> {
    let mut out = params.to_vec();
    for (k, v) in &mut out {
        if k.eq_ignore_ascii_case("level") {
            *v = level.to_string();
        }
    }
    out
}

/// ngspice's diode `tlev` and breakdown coefficient `tcv` (alias `tbv1`,
/// `dio.c:118-119`) as Xyce's, for parameter `low` = `v` under `tlev`:
/// `tlev` 0 or 1 is left out (`diotemp.c`: the two differ only in the
/// breakdown law); `tcv` at 0 left out, under `tlev=1` (`bv*(1 -
/// tcv*dt)`) Xyce's `TBV1=-tcv` (`bv*(1 + tbv1*dt)`, RG 7.10 `D`), under
/// `tlev=0` (`bv - tcv*dt`) refused. `None`: not one of these names.
fn breakdown(
    low: &str,
    v: &str,
    tlev: Option<f64>,
) -> Option<Result<Option<(String, String)>, Code>> {
    Some(match low {
        "tlev" if matches!(tlev, Some(0.0 | 1.0)) => Ok(None),
        "tlev" => Err(Code::W0050),
        "tcv" | "tbv1" if fold(v) == Some(0.0) => Ok(None),
        "tcv" | "tbv1" if tlev == Some(1.0) => {
            let neg = fold(v).map_or_else(|| format!("{{-({v})}}"), |x| format!("{}", -x));
            Ok(Some(("tbv1".to_string(), neg)))
        }
        "tcv" | "tbv1" => Err(Code::W0050),
        _ => return None,
    })
}
