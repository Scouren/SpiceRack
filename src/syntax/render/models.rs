//! An ngspice `.model` kind, and an ngspice R, C or D card's parameters →
//! the named targets' spelling: one row per parameter name ngspice reads
//! (its aliases too), with the Spectre and the VACASK answer. A name no row
//! holds is one ngspice ignores ("unrecognized parameter - ignored",
//! `inpgmod.c`), so it is not written either. The named writer reads these
//! rows for a deck read as ngspice; another dialect's R/C models keep the
//! `RclModel` refusal (their model cards mean other equations).
//!
//! The Spectre column maps a name onto the Spectre parameter with the same
//! equation (`[CMP]`'s parameter lists), and the kind's `implied` rows write
//! the ngspice defaults Spectre's differ from. The VACASK column is
//! ngspice's own parameter: VACASK's `sp_resistor`, `sp_capacitor` and
//! `sp_diode` are ngspice-45's device code (Verilog-A Distiller), with the
//! model parameters that clash with an instance parameter renamed
//! `model_<name>` (each module's header notes).
//!
//! Cites: ngspice-45 `src/spicelib/devices/res/res.c` (`RESmPTable`,
//! `RESpTable`), `restemp.c` (`RESupdate_conduct`), `ressetup.c` (the
//! defaults), `resnoise.c`; `cap/cap.c`, `captemp.c`, `capsetup.c`;
//! `dio/dio.c` (`DIOmPTable`, `DIOpTable`), `diosetup.c` (the defaults, the
//! level-3 geometry, the knee currents), `diotemp.c`, `dioload.c`;
//! `parser/inpgmod.c` (an instance parameter on a model card is the
//! instance's default; `m` and `level` read and dropped); Spectre Circuit
//! Simulator Reference 5.0 `[CMP]` p.627-632 (`resistor`: `R = rsh*(l -
//! 2*etchl)/(w - 2*etch)`, `R(model)` before the geometry, `trise`,
//! `wdexp`…`fexp`), p.283-285 (`capacitor`), p.302-310 (`diode`: `level=3`,
//! `minr`, `dskip`, `jmax`, `allow_scaling`); Spectre Reference 19.1 p.498
//! (`model name master`); VACASK `devices/spice/{resistor,capacitor,diode}.va`.
//!
//! Contents:
//! - `pub enum Param`, `pub struct Row`, `pub enum Implied`, `pub struct Kind`
//! - `pub fn kind(spice_kind: &str, level: u32) -> Option<&'static Kind>`
//! - `pub fn instance(letter: char) -> &'static [Row]`, `pub fn row(rows: &'static [Row], name: &str) -> Option<&'static Row>`
//! - `pub const RESISTOR: Kind`, `pub const CAPACITOR: Kind`, `pub const DIODE3: Kind`
//! - `pub fn fold(text: &str) -> Option<f64>`

use Param::{Ignored, Knee, OverM, Refuse, Same, TimesM, To};

/// One parameter's answer in one target.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Param {
    /// Written under this name.
    To(&'static str),
    /// Written under its own name.
    Same,
    /// Not written: ngspice reads it without effect on the solution (a
    /// safe-operating-area limit; a coefficient of a term another row
    /// refuses unless it is zero).
    Ignored,
    /// Not written when its value folds to this number (ngspice's term
    /// vanishes there, as with the target's default); else refused.
    Off(f64),
    /// A knee current: ngspice disables it below `CKTepsmin` (1e-28,
    /// `diosetup.c` L106-L118), so it is not written there; else written
    /// under the name (`None`: refused).
    Knee(Option<&'static str>),
    /// A 0/1 switch written as this `name=no` / `name=yes`; another value
    /// is refused.
    YesNo(&'static str),
    /// A flag (a parameter with no value) written as this `name=value`.
    Flag(&'static str, &'static str),
    /// An instance's `scale`, which multiplies its capacitance: written
    /// into its `m` (`m*(scale)`).
    TimesM,
    /// An instance's `scale`, which multiplies its resistance: written
    /// into its `m` (`m/(scale)`).
    OverM,
    /// No parameter of the target has its equation: the card is refused.
    Refuse,
}

/// A parameter name ngspice reads, and its answer per named target.
pub struct Row {
    pub name: &'static str,
    pub spectre: Param,
    pub vacask: Param,
}

/// A Spectre parameter written when the card sets none of the names that
/// map onto it: ngspice's default where Spectre's differs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Implied {
    /// This text.
    Text(&'static str),
    /// The value the card gives this other Spectre parameter, else this
    /// text (ngspice's `nbv` defaults to `n`).
    Copy(&'static str, &'static str),
    /// This text, unless the card gives this other Spectre parameter.
    Unless(&'static str, &'static str),
    /// ngspice's `eg`: 1.16 when the card's `tlev` is 2, else 1.11
    /// (`diosetup.c` L129-L135).
    Eg,
}

/// One ngspice model kind's masters and rows.
pub struct Kind {
    /// The Spectre master (`model x resistor`).
    pub spectre: &'static str,
    /// The VACASK module (`load "spice/<m>.osdi"`, `model x sp_<m>`).
    pub vacask: &'static str,
    pub rows: &'static [Row],
    /// Spectre parameters written when the card leaves them out.
    pub implied: &'static [(&'static str, Implied)],
    /// Two Spectre parameters the card may not both give: ngspice and
    /// Spectre rank them in opposite orders.
    pub exclusive: Option<(&'static str, &'static str)>,
}

/// ngspice's resistor model (`RESmPTable`), and the instance parameters a
/// model card sets as defaults (`inpgmod.c`).
pub const RESISTOR: Kind = Kind {
    spectre: "resistor",
    vacask: "resistor",
    rows: &[
        row_("rsh", To("rsh"), Same),
        // ngspice narrows by 2*narrow and shortens by 2*short (restemp.c
        // L68-L72), as Spectre's per-side etch and etchl.
        row_("narrow", To("etch"), Same),
        row_("dw", To("etch"), Same),
        row_("short", To("etchl"), Same),
        row_("dlr", To("etchl"), Same),
        row_("tc1", To("tc1"), To("model_tc1")),
        row_("tc1r", To("tc1"), Same),
        row_("tc2", To("tc2"), To("model_tc2")),
        row_("tc2r", To("tc2"), Same),
        // `pow(1.01, tce*dT)`: Spectre's resistor has no exponential term.
        row_("tce", Param::Off(0.0), To("model_tce")),
        row_("defw", To("w"), Same),
        row_("w", To("w"), To("model_w")),
        row_("l", To("l"), To("model_l")),
        // Flicker noise `kf*|I|^af / (Leff^lf * Weff^wf * f^ef)`
        // (resnoise.c L109-L111): Spectre's effective-size exponents, its
        // drawn-size ones set to 0 (`implied`).
        row_("kf", To("kf"), Same),
        row_("af", To("af"), Same),
        row_("lf", To("leexp"), Same),
        row_("wf", To("weexp"), Same),
        row_("ef", To("fexp"), Same),
        row_("tnom", To("tnom"), Same),
        row_("r", To("r"), To("model_r")),
        row_("res", To("r"), Same),
        row_("bv_max", Ignored, To("model_bv_max")),
        // Instance parameters as model defaults (`RESpTable`).
        row_("resistance", To("r"), Same),
        row_("ac", Refuse, Refuse),
        row_("temp", Refuse, Same),
        row_("dtemp", To("trise"), Same),
        row_("tc", Param::Off(0.0), Same),
        row_("scale", Param::Off(1.0), Same),
        row_("noisy", Param::Off(1.0), Same),
        row_("noise", Param::Off(1.0), Same),
        row_("m", Ignored, Ignored),
    ],
    implied: &[
        // ngspice's defaults (ressetup.c L31-L32; Spectre: l=inf, w=1u).
        ("l", Implied::Text("1e-05")),
        ("w", Implied::Text("1e-05")),
        // No value, no model `r`, no `rsh`: 1 mOhm (restemp.c L76-L79).
        ("r", Implied::Unless("rsh", "0.001")),
        ("af", Implied::Text("1")),
        ("wdexp", Implied::Text("0")),
        ("ldexp", Implied::Text("0")),
        ("leexp", Implied::Text("1")),
        ("weexp", Implied::Text("1")),
    ],
    // ngspice takes the geometry over the model's `r` when `l*w*rsh > 0`
    // (restemp.c L68-L74); Spectre takes `r(model)` first (`[CMP]` p.627).
    exclusive: Some(("r", "rsh")),
};

/// ngspice's capacitor model (`CAPmPTable`) and its instance defaults.
pub const CAPACITOR: Kind = Kind {
    spectre: "capacitor",
    vacask: "capacitor",
    rows: &[
        row_("cap", To("c"), To("model_cap")),
        row_("cj", To("cj"), Same),
        row_("cox", To("cj"), Same),
        row_("cjsw", To("cjsw"), Same),
        row_("capsw", To("cjsw"), Same),
        row_("defw", To("w"), Same),
        row_("w", To("w"), To("model_w")),
        row_("defl", To("l"), Same),
        row_("l", To("l"), To("model_l")),
        // `(W - narrow)*(L - short)` (captemp.c): Spectre etches both
        // sides of both by one `etch`.
        row_("narrow", Param::Off(0.0), Same),
        row_("short", Param::Off(0.0), Same),
        row_("del", Param::Off(0.0), Same),
        row_("tc1", To("tc1"), To("model_tc1")),
        row_("tc2", To("tc2"), To("model_tc2")),
        row_("tnom", To("tnom"), Same),
        // `cj = di*eps0/thick` when cj is not given: no Spectre form.
        row_("di", Refuse, Same),
        row_("thick", Refuse, Same),
        row_("bv_max", Ignored, To("model_bv_max")),
        row_("c", Ignored, Ignored),
        // Instance parameters as model defaults (`CAPpTable`).
        row_("capacitance", To("c"), Same),
        row_("ic", Refuse, Same),
        row_("temp", Refuse, Same),
        row_("dtemp", To("trise"), Same),
        row_("scale", Param::Off(1.0), Same),
        row_("m", Ignored, Ignored),
    ],
    // ngspice's default width (capsetup.c L37-L39; Spectre: w=0).
    implied: &[("w", Implied::Text("1e-05"))],
    exclusive: None,
};

/// ngspice's diode at `level=3` (`DIOmPTable`) and its instance defaults:
/// Spectre's `diode` at `level=3` ("junction + additional metal and
/// polysilicon capacitances", `[CMP]` p.304), VACASK's `sp_diode`.
pub const DIODE3: Kind = Kind {
    spectre: "diode",
    vacask: "diode",
    rows: &[
        row_("level", To("level"), Same),
        row_("is", To("js"), Same),
        row_("js", To("js"), Same),
        row_("jsw", To("jsw"), Same),
        row_("isw", To("jsw"), Same),
        row_("tnom", To("tnom"), Same),
        row_("tref", To("tnom"), Same),
        row_("rs", To("rs"), Same),
        row_("trs", To("trs"), Same),
        row_("trs1", To("trs"), Same),
        row_("trs2", To("trs2"), Same),
        row_("n", To("n"), Same),
        row_("ns", To("ns"), Same),
        row_("tt", To("tt"), Same),
        row_("ttt1", Param::Off(0.0), Same),
        row_("ttt2", Param::Off(0.0), Same),
        row_("cjo", To("cjo"), Same),
        row_("cj0", To("cjo"), Same),
        row_("cj", To("cjo"), Same),
        row_("vj", To("vj"), Same),
        row_("pb", To("vj"), Same),
        row_("m", To("m"), Same),
        row_("mj", To("m"), Same),
        row_("tm1", Param::Off(0.0), Same),
        row_("tm2", Param::Off(0.0), Same),
        row_("cjp", To("cjsw"), Same),
        row_("cjsw", To("cjsw"), Same),
        row_("php", To("vjsw"), Same),
        row_("vjsw", To("vjsw"), Same),
        row_("mjsw", To("mjsw"), Same),
        row_("ikf", Knee(Some("ik")), Same),
        row_("ik", Knee(Some("ik")), Same),
        // Spectre reads `ikr` with `hcomp=1` only (`[CMP]` p.305).
        row_("ikr", Knee(None), Same),
        row_("nbv", To("nz"), Same),
        row_("nz", To("nz"), Same),
        row_("area", To("area"), To("model_area")),
        row_("pj", To("perim"), To("model_pj")),
        row_("tlev", To("tlev"), Same),
        row_("tlevc", To("tlevc"), Same),
        row_("eg", To("eg"), Same),
        row_("gap1", To("gap1"), Same),
        row_("gap2", To("gap2"), Same),
        row_("xti", To("xti"), Same),
        row_("cta", To("cta"), Same),
        row_("ctc", To("cta"), Same),
        row_("ctp", To("ctp"), Same),
        row_("tpb", To("pta"), Same),
        row_("tvj", To("pta"), Same),
        row_("tphp", To("ptp"), Same),
        // Tunnelling and recombination currents: no Spectre term. Their
        // exponents matter only where the current does.
        row_("jtun", Param::Off(0.0), Same),
        row_("jtunsw", Param::Off(0.0), Same),
        row_("ntun", Ignored, Same),
        row_("xtitun", Ignored, Same),
        row_("keg", Ignored, Same),
        row_("isr", Param::Off(0.0), Same),
        row_("nr", Ignored, Same),
        row_("kf", To("kf"), Same),
        row_("af", To("af"), Same),
        row_("fc", To("fc"), Same),
        row_("fcs", To("fcs"), Same),
        row_("bv", To("bv"), Same),
        row_("vb", To("bv"), Same),
        row_("vrb", To("bv"), Same),
        row_("var", To("bv"), Same),
        row_("ibv", To("ibv"), Same),
        row_("ib", To("ibv"), Same),
        row_("tcv", To("tbv1"), Same),
        row_("tbv1", To("tbv1"), Same),
        // Safe-operating-area limits: ngspice warns, the solution is the same.
        row_("fv_max", Ignored, Same),
        row_("bv_max", Ignored, Same),
        row_("id_max", Ignored, Same),
        row_("te_max", Ignored, Same),
        row_("pd_max", Ignored, Same),
        // Self-heating: neither Spectre's diode nor VACASK's build has it.
        row_("rth0", Param::Off(0.0), Param::Off(0.0)),
        row_("cth0", Ignored, Ignored),
        row_("lm", To("lm"), To("model_lm")),
        row_("lp", To("lp"), To("model_lp")),
        row_("wm", To("wm"), To("model_wm")),
        row_("wp", To("wp"), To("model_wp")),
        // Angstroms in both (diompar.c L265-L272; `[CMP]` p.305).
        row_("xom", To("xom"), Same),
        row_("xoi", To("xoi"), Same),
        row_("xm", To("xm"), Same),
        row_("xp", To("xp"), Same),
        row_("xw", To("xw"), Same),
        row_("d", Ignored, Ignored),
        // Instance parameters as model defaults (`DIOpTable`).
        row_("off", Refuse, Refuse),
        row_("temp", Refuse, Same),
        row_("dtemp", To("trise"), Same),
        row_("ic", Refuse, Refuse),
        row_("perim", Refuse, Same),
        row_("w", Refuse, Same),
        row_("l", Refuse, Same),
        row_("thermal", Refuse, Refuse),
    ],
    implied: &[
        // ngspice's defaults where Spectre's differ (diosetup.c).
        ("eg", Implied::Eg),
        ("nz", Implied::Copy("n", "1")),
        ("fcs", Implied::Text("0.5")),
        ("xom", Implied::Text("10000")),
        ("xoi", Implied::Text("10000")),
        // What ngspice does not do: linearize below 0.1*iabstol, limit the
        // current to `jmax*area` (default 1 A). Spectre also drops a series
        // resistance below `minr` (default 0.1 ohm), which ngspice keeps:
        // `minr` is not written, Spectre 25.1 stops on `minr=0` (CMI-2049,
        // "should be greater than zero").
        ("dskip", Implied::Text("no")),
        ("jmax", Implied::Text("1e+20")),
    ],
    exclusive: None,
};

/// ngspice's resistor card in Spectre (`[CMP]` p.629).
const R_INSTANCE: &[Row] = &[
    row_("resistance", To("r"), Same),
    row_("r", To("r"), Same),
    row_("ac", Refuse, Refuse),
    row_("temp", Refuse, Same),
    row_("dtemp", To("trise"), Same),
    row_("l", To("l"), Same),
    row_("w", To("w"), Same),
    row_("m", To("m"), Same),
    row_("tc", To("tc1"), Same),
    row_("tc1", To("tc1"), Same),
    row_("tc2", To("tc2"), Same),
    row_("tce", Param::Off(0.0), Same),
    row_("bv_max", Ignored, Same),
    // ngspice divides the conductance by `scale` (restemp.c L110);
    // Spectre's `scale` scales the geometry.
    row_("scale", OverM, Same),
    row_("noisy", Param::YesNo("isnoisy"), Same),
    row_("noise", Param::YesNo("isnoisy"), Same),
];

/// ngspice's capacitor card in Spectre (`[CMP]` p.284).
const C_INSTANCE: &[Row] = &[
    row_("capacitance", To("c"), Same),
    row_("cap", To("c"), Same),
    row_("c", To("c"), Same),
    row_("ic", To("ic"), Same),
    row_("temp", Refuse, Same),
    row_("dtemp", To("trise"), Same),
    row_("w", To("w"), Same),
    row_("l", To("l"), Same),
    row_("m", To("m"), Same),
    row_("tc1", To("tc1"), Same),
    row_("tc2", To("tc2"), Same),
    row_("bv_max", Ignored, Same),
    // ngspice multiplies the capacitance by `scale` (captemp.c).
    row_("scale", TimesM, Same),
];

/// ngspice's diode card in Spectre (`[CMP]` p.303): the level-3 area from
/// `w` and `l` scales with `.option scale` in ngspice, with
/// `allow_scaling` in Spectre, which scales a given area too.
const D_INSTANCE: &[Row] = &[
    row_("area", To("area"), Same),
    row_("pj", To("perim"), Same),
    row_("perim", To("perim"), Same),
    row_("w", Refuse, Same),
    row_("l", Refuse, Same),
    row_("m", To("m"), Same),
    row_("lm", To("lm"), Same),
    row_("lp", To("lp"), Same),
    row_("wm", To("wm"), Same),
    row_("wp", To("wp"), Same),
    row_("temp", Refuse, Same),
    row_("dtemp", To("trise"), Same),
    row_("ic", Refuse, Refuse),
    // ngspice's `off` and Spectre's `region=off` are the first guess of
    // the operating point only (`[CMP]` p.303 #7); VACASK's build has none.
    row_("off", Param::Flag("region", "off"), Refuse),
    row_("thermal", Refuse, Refuse),
];

/// The constant folder's cursor; `depth` bounds the parentheses.
struct Fold<'a> {
    b: &'a [u8],
    i: usize,
    depth: u32,
}

/// Parentheses deeper than this are no constant the folder reads.
const FOLD_DEPTH_MAX: u32 = 64;

/// The kind ngspice's `.model x <spice_kind>` at `level` is in these rows.
pub fn kind(spice_kind: &str, level: u32) -> Option<&'static Kind> {
    match (spice_kind.to_ascii_lowercase().as_str(), level) {
        ("r" | "res", _) => Some(&RESISTOR),
        ("c" | "cap", _) => Some(&CAPACITOR),
        ("d", 3) => Some(&DIODE3),
        _ => None,
    }
}

/// An ngspice R, C or D card's own parameters (`RESpTable`, `CAPpTable`,
/// `DIOpTable`) in Spectre; VACASK's modules take ngspice's names.
pub fn instance(letter: char) -> &'static [Row] {
    match letter.to_ascii_uppercase() {
        'R' => R_INSTANCE,
        'C' => C_INSTANCE,
        'D' => D_INSTANCE,
        _ => &[],
    }
}

/// `name`'s row in `rows`, ignoring case; `None` when ngspice reads no
/// such parameter.
pub fn row(rows: &'static [Row], name: &str) -> Option<&'static Row> {
    rows.iter().find(|r| r.name.eq_ignore_ascii_case(name))
}

/// `text`'s value when it is a constant: numbers (SPICE suffixes), `+ - *
/// /`, parentheses, inside `{…}` or `'…'`; `None` for anything else.
pub fn fold(text: &str) -> Option<f64> {
    let t = text.trim();
    let t = t
        .strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .or_else(|| t.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
        .unwrap_or(t);
    let mut p = Fold {
        b: t.as_bytes(),
        i: 0,
        depth: 0,
    };
    let v = p.sum()?;
    p.blank();
    (p.i == p.b.len()).then_some(v)
}

// ---- private ----

const fn row_(name: &'static str, spectre: Param, vacask: Param) -> Row {
    Row {
        name,
        spectre,
        vacask,
    }
}

impl Fold<'_> {
    fn blank(&mut self) {
        while self.b.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }

    fn sum(&mut self) -> Option<f64> {
        let mut v = self.product()?;
        loop {
            self.blank();
            match self.b.get(self.i) {
                Some(b'+') => {
                    self.i += 1;
                    v += self.product()?;
                }
                Some(b'-') => {
                    self.i += 1;
                    v -= self.product()?;
                }
                _ => return Some(v),
            }
        }
    }

    fn product(&mut self) -> Option<f64> {
        let mut v = self.atom()?;
        loop {
            self.blank();
            match self.b.get(self.i) {
                Some(b'*') if self.b.get(self.i + 1) != Some(&b'*') => {
                    self.i += 1;
                    v *= self.atom()?;
                }
                Some(b'/') => {
                    self.i += 1;
                    v /= self.atom()?;
                }
                _ => return Some(v),
            }
        }
    }

    fn atom(&mut self) -> Option<f64> {
        self.blank();
        match self.b.get(self.i)? {
            b'-' => {
                self.i += 1;
                Some(-self.atom()?)
            }
            b'+' => {
                self.i += 1;
                self.atom()
            }
            b'(' => {
                assert!(self.depth <= FOLD_DEPTH_MAX);
                if self.depth == FOLD_DEPTH_MAX {
                    return None;
                }
                self.i += 1;
                self.depth += 1;
                let v = self.sum()?;
                self.blank();
                (self.b.get(self.i) == Some(&b')')).then_some(())?;
                self.i += 1;
                self.depth -= 1;
                Some(v)
            }
            _ => {
                let rest = std::str::from_utf8(&self.b[self.i..]).ok()?;
                let (v, len, _) = crate::unit::parse_spice_number(rest)?;
                // A suffix's letters run on (`1meg`, `2.5uF`).
                let tail = rest[len..]
                    .bytes()
                    .take_while(u8::is_ascii_alphabetic)
                    .count();
                self.i += len + tail;
                Some(v)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_reads_constants_and_nothing_else() {
        assert_eq!(fold("{0/1e-12}"), Some(0.0));
        assert_eq!(fold("'0/1e-12'"), Some(0.0));
        assert_eq!(fold("1e-030"), Some(1e-30));
        assert_eq!(fold("-(2+3)*1k"), Some(-5000.0));
        assert_eq!(fold("1.0"), Some(1.0));
        assert_eq!(fold("sw_rdn*0.95"), None);
        assert_eq!(fold("2**3"), None);
        assert_eq!(fold("(1"), None);
        assert_eq!(fold(&"(".repeat(100)), None);
    }

    #[test]
    fn every_kind_names_each_parameter_once() {
        // A name twice would make the first row win silently.
        for k in [&RESISTOR, &CAPACITOR, &DIODE3] {
            for (i, r) in k.rows.iter().enumerate() {
                assert!(
                    r.name
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                );
                assert!(k.rows[..i].iter().all(|s| s.name != r.name), "{}", r.name);
            }
        }
        for l in ['R', 'C', 'D'] {
            let rows = instance(l);
            assert!(!rows.is_empty());
            for (i, r) in rows.iter().enumerate() {
                assert!(rows[..i].iter().all(|s| s.name != r.name), "{}", r.name);
            }
        }
        assert!(instance('L').is_empty());
    }

    #[test]
    fn kinds_by_spice_type_and_level() {
        assert_eq!(kind("R", 1).map(|k| k.spectre), Some("resistor"));
        assert_eq!(kind("cap", 1).map(|k| k.spectre), Some("capacitor"));
        assert_eq!(kind("d", 3).map(|k| k.spectre), Some("diode"));
        // Level 1 keeps the older name-for-name diode path.
        assert!(kind("d", 1).is_none());
        assert!(kind("nmos", 3).is_none());
        assert_eq!(row(RESISTOR.rows, "TREF").map(|r| r.name), None);
        assert_eq!(
            row(DIODE3.rows, "TREF").map(|r| r.spectre),
            Some(To("tnom"))
        );
    }
}
