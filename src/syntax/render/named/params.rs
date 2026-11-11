//! An ngspice card's parameters → the target's, through `render::models`'
//! rows: a model card's (with Spectre's implied ngspice defaults and the
//! level-3 diode's sidewall and breakdown laws), an R, C
//! or D element's (Spectre), and the voltage-dependent resistance ngspice
//! runs as a behavioural source. Each refusal anchors at the parameter it
//! is about; each parameter left out is noted.
//!
//! Cites: `render::models` (each row's clause); ngspice-45
//! `frontend/inpcom.c` L5751-L5770 (`b_transformation_wanted`: `v(`, `i(`,
//! `temper`, `hertz`, `time`), L6621-L6680 (the `B` line keeps `tc1`, `tc2`
//! and `m`); Spectre Reference 5.0 p.859 (`resistor r=100*(1+v(n1,n2)/2)`),
//! p.884 (`temp` in an expression); VACASK `docs/cmd-options-temp.md`
//! (`$temp`); ngspice-45 `dio/dioload.c` L257-L296 (the sidewall current
//! on the bottom's characteristic), `dio/diotemp.c` L113-L128 (its
//! temperature scaling), L204-L209 (`tcv`); Spectre Reference 5.0
//! p.305-306 (`ns`, `tbv1`), 19.1 p.475 (`ln`, `exp`), p.507 (`temp` and
//! `tnom` in a model parameter).
//!
//! Contents:
//! - `W::table_kind(&self, m: &Model) -> Option<&'static Kind>`, `W::model_rows(&mut self, m: &Model, node: u32, k: &'static Kind) -> Option<Vec<(String, String)>>`
//! - `W::element_rows(&mut self, letter: char, params: &[(String, String)], node: u32) -> Option<Vec<(String, String)>>`
//! - `W::diode_own_rise(&self, model: &str, params: &[(String, String)]) -> bool`
//! - `W::tran_ic(&mut self, letter: char, params: Vec<(String, String)>, node: u32) -> Vec<(String, String)>`
//! - `W::dependent(&mut self, value: &Val, model: bool, params: &[(String, String)], node: u32) -> Option<Option<Dependent>>`
//! - `type Dependent`
//! - `fn spelled_flag(source: &Behaviours, target: &Behaviours, letter: char, name: &str) -> bool`, `fn level(m: &Model) -> u32`

use super::W;
use super::checks::calls;
use crate::syntax::behaviour::{Behaviours, ModelSemantics, NamedParams};
use crate::syntax::diag::Code;
use crate::syntax::render::models::{self, Implied, Kind, Param, Row};
use crate::syntax::render::read::{Ana, Model, Val};

/// A voltage-dependent resistance's expression and the parameters kept.
pub(super) type Dependent = (String, Vec<(String, String)>);

impl W<'_, '_> {
    /// The rows model `m` is written through: a deck read as ngspice, a
    /// kind and level `render::models` holds.
    pub(super) fn table_kind(&self, m: &Model) -> Option<&'static Kind> {
        if self.rd.b.model_semantics != ModelSemantics::Ngspice {
            return None;
        }
        models::kind(&m.kind, level(m))
    }

    /// Model `m`'s (statement `node`'s) parameters as the target's `(name,
    /// value)`, the value as written; Spectre's implied defaults after,
    /// then its diode's laws (`diode_laws`). `None` when a row or a law
    /// refuses one.
    pub(super) fn model_rows(
        &mut self,
        m: &Model,
        node: u32,
        k: &'static Kind,
    ) -> Option<Vec<(String, String)>> {
        let mut out = self.rows(k.rows, &m.params, node)?;
        if self.b.named_params != NamedParams::Spectre {
            return Some(out);
        }
        if let Some((a, b)) = k.exclusive
            && has(&out, a)
            && has(&out, b)
        {
            self.refuse(Code::W0114, self.rd.param_token(node, a));
            return None;
        }
        for &(name, implied) in k.implied {
            if has(&out, name) {
                continue;
            }
            let value = match implied {
                Implied::Text(t) => t.to_string(),
                Implied::Copy(from, t) => value_of(&out, from).unwrap_or(t).to_string(),
                Implied::Unless(other, t) => match has(&out, other) {
                    true => continue,
                    false => t.to_string(),
                },
                Implied::Eg => {
                    let tlev = value_of(&out, "tlev").and_then(models::fold);
                    match tlev == Some(2.0) {
                        true => "1.16".to_string(),
                        false => "1.11".to_string(),
                    }
                }
            };
            out.push((name.to_string(), value));
        }
        // The one kind with laws that read several parameters.
        if k.spectre == "diode" {
            self.diode_laws(node, &mut out)?;
        }
        Some(out)
    }

    /// `params` but a C's or an L's `ic` on an ngspice card when no
    /// `.tran` of the deck has `uic`: ngspice applies it only then (manual,
    /// C and L: "the initial conditions (if any) apply only if the uic
    /// option is specified on the .tran control line"), where Spectre's
    /// `tran` applies a device `ic` and VACASK's `sp_capacitor` has none.
    /// Left out, noted W0112.
    pub(super) fn tran_ic(
        &mut self,
        letter: char,
        mut params: Vec<(String, String)>,
        node: u32,
    ) -> Vec<(String, String)> {
        let at = params
            .iter()
            .position(|(k, _)| k.eq_ignore_ascii_case("ic"));
        let Some(at) = at.filter(|_| {
            self.rd.b.model_semantics == ModelSemantics::Ngspice && matches!(letter, 'C' | 'L')
        }) else {
            return params;
        };
        let tb = self.fold();
        let uic = tb
            .plan
            .runs
            .iter()
            .any(|a| matches!(a, Ana::Tran { uic: true, .. }));
        if !uic {
            params.remove(at);
            self.note(Code::W0112, self.rd.main(node));
        }
        params
    }

    /// Element `node`'s parameters (an R, C or D card's) as Spectre's,
    /// `scale` folded into `m`; `None` when a row refuses one. A name no
    /// row holds is written as it is.
    pub(super) fn element_rows(
        &mut self,
        letter: char,
        params: &[(String, String)],
        node: u32,
    ) -> Option<Vec<(String, String)>> {
        assert!(self.b.named_params == NamedParams::Spectre);
        let rows = models::instance(letter);
        let mut out: Vec<(String, String)> = Vec::new();
        let mut scale = None;
        let mut refused = false;
        for (k, v) in params {
            let Some(row) = models::row(rows, k) else {
                out.push((k.clone(), v.clone()));
                continue;
            };
            match row.spectre {
                Param::TimesM => scale = Some(('*', v)),
                Param::OverM => scale = Some(('/', v)),
                Param::Flag(name, value) if v.is_empty() => put(&mut out, name.to_string(), value),
                _ => match self.answer(row, v, self.rd.param_token(node, k)) {
                    None => refused = true,
                    Some(None) => {}
                    Some(Some((name, v))) => put(&mut out, name, &v),
                },
            }
        }
        if refused {
            return None;
        }
        if let Some((op, s)) = scale {
            let m = out.iter().position(|(k, _)| k == "m");
            let m = m.map_or("1".to_string(), |i| out.remove(i).1);
            let (m, s) = (super::undelimited(&m), super::undelimited(s));
            out.push(("m".to_string(), format!("({m}){op}({s})")));
        }
        Some(out)
    }

    /// Whether a D card's `params` hold its own `dtemp` while its model
    /// `model` is a level-3 diode whose sidewall current `diode_laws`
    /// rescales. The rescaled `jsw` reads the circuit's `temp` and the
    /// model's `trise`, never a card's own rise, so the caller refuses the
    /// card (ruling R10 on P3-089); a card's `temp` is refused by its row.
    /// `false` for a model the deck's blocks do not hold.
    pub(super) fn diode_own_rise(&self, model: &str, params: &[(String, String)]) -> bool {
        assert!(self.b.named_params == NamedParams::Spectre, "Spectre's");
        if !params.iter().any(|(k, _)| k.eq_ignore_ascii_case("dtemp")) {
            return false;
        }
        let rescaled = |&(m, bin): &(u32, u32)| {
            let view = self.rd.model_view(m, bin);
            view.name.eq_ignore_ascii_case(model)
                && self.table_kind(&view).is_some_and(|k| k.spectre == "diode")
                && sidewall_rescaled(&view.params)
        };
        self.blocks().iter().flat_map(|b| &b.models).any(rescaled)
    }

    /// ngspice's behavioural resistor (inpcom.c L6621-L6680): when the
    /// resistance (`value`, or an `r=`) reads a node voltage or a branch
    /// current, its expression and the `tc1`, `tc2`, `m` the `B` line keeps;
    /// `Some(None)` when it reads none of `v(`, `i(`, `temper`, `hertz`,
    /// `time`. `None` when the target cannot write it (VACASK has no
    /// behavioural element, so writes only a `temper` one with no model;
    /// `hertz`, `time` have no Spectre spelling here), refused at `node`.
    /// What is left out (the `model`, any other parameter) is noted. The
    /// `B` line has no thermal noise: the caller writes the resistor
    /// noiseless.
    pub(super) fn dependent(
        &mut self,
        value: &Val,
        model: bool,
        params: &[(String, String)],
        node: u32,
    ) -> Option<Option<Dependent>> {
        let tok = self.rd.main(node);
        let texts = params.iter().map(|(_, v)| v.as_str());
        let texts: Vec<&str> = match value {
            Val::Expr(e) | Val::Raw(e) => texts.chain([e.as_str()]).collect(),
            Val::Num(_) => texts.collect(),
        };
        let reads =
            |e: &str| calls(e).any(|c| c.eq_ignore_ascii_case("v") || c.eq_ignore_ascii_case("i"));
        let word = |e: &str, ws: &[&str]| {
            e.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .any(|w| ws.iter().any(|s| w.eq_ignore_ascii_case(s)))
        };
        let (branch, temper) = (
            texts.iter().any(|e| reads(e)),
            texts.iter().any(|e| word(e, &["temper"])),
        );
        if !branch && !temper && !texts.iter().any(|e| word(e, &["hertz", "time"])) {
            return Some(None);
        }
        // `hertz` and `time` have no Spectre spelling here. VACASK has no
        // behavioural element, so it takes a resistance reading only
        // `temper` (its `$temp`), on no model: the `resistor` it names is
        // declared only for a card naming none.
        let clean = !texts.iter().any(|e| word(e, &["hertz", "time"]));
        if !clean || self.b.named_params == NamedParams::Vacask && (branch || model) {
            self.refuse(Code::W0102, tok);
            return None;
        }
        let key = |k: &str| {
            ["r", "resistance"]
                .iter()
                .any(|r| k.eq_ignore_ascii_case(r))
        };
        let expr = match value {
            Val::Expr(e) => e.clone(),
            Val::Num(_) | Val::Raw(_) => match params.iter().rev().find(|(k, _)| key(k)) {
                Some((_, v)) => super::undelimited(v).to_string(),
                None => {
                    self.refuse(Code::W0102, tok);
                    return None;
                }
            },
        };
        let keep = ["tc1", "tc2", "m"];
        let mut kept = Vec::new();
        let mut dropped = model;
        for (k, v) in params.iter().filter(|(k, _)| !key(k)) {
            let k = match k.to_ascii_lowercase().as_str() {
                "tc" => "tc1".to_string(),
                k => k.to_string(),
            };
            match keep.contains(&k.as_str()) {
                true => kept.push((k, v.clone())),
                false => dropped = true,
            }
        }
        if dropped {
            self.note(Code::W0113, tok);
        }
        // `[CMP]` p.884 ("Temperature as a parameter"): Spectre's `temp` is
        // ngspice's `temper`, re-evaluated as the temperature changes; so is
        // VACASK's `$temp` (`docs/cmd-options-temp.md`). All are Celsius.
        let temp = self.b.temp_name;
        let expr = words(&expr, |w| match w.eq_ignore_ascii_case("temper") {
            true => temp.to_string(),
            false => w.to_string(),
        });
        Some(Some((expr.replace("V(", "v(").replace("I(", "i("), kept)))
    }
}

/// Whether flag `name` on an element of type `letter` is written as a
/// `name=value` in the target (Spectre's `region=off`), so is no flag.
pub(super) fn spelled_flag(
    source: &Behaviours,
    target: &Behaviours,
    letter: char,
    name: &str,
) -> bool {
    let row = models::row(models::instance(letter), name);
    source.model_semantics == ModelSemantics::Ngspice
        && target.named_params == NamedParams::Spectre
        && row.is_some_and(|r| matches!(r.spectre, Param::Flag(..)))
}

/// A model's `level`, 1 when it gives none or no number.
pub(super) fn level(m: &Model) -> u32 {
    m.params
        .iter()
        .rev()
        .find(|(k, _)| k.eq_ignore_ascii_case("level"))
        .and_then(|(_, v)| models::fold(v))
        .map_or(1, |l| l as u32)
}

// ---- private ----

impl W<'_, '_> {
    /// `params` through `rows` in the target's column, in card order; a
    /// later name mapping onto an earlier one's target replaces it (ngspice
    /// keeps the last). `None` when a row refuses one.
    fn rows(
        &mut self,
        rows: &'static [Row],
        params: &[(String, String)],
        node: u32,
    ) -> Option<Vec<(String, String)>> {
        let mut out: Vec<(String, String)> = Vec::new();
        let mut refused = false;
        for (k, v) in params {
            let at = self.rd.param_token(node, k);
            let Some(row) = models::row(rows, k) else {
                self.note(Code::W0112, at);
                continue;
            };
            match self.answer(row, v, at) {
                None => refused = true,
                Some(None) => {}
                Some(Some((name, v))) => put(&mut out, name, &v),
            }
        }
        (!refused).then_some(out)
    }

    /// `row`'s answer for value `v` in the target: the name and value it
    /// is written as, `Some(None)` when it is left out (noted at `at`
    /// unless its value is the off one), `None` when refused at `at`.
    fn answer(&mut self, row: &Row, v: &str, at: u32) -> Option<Option<(String, String)>> {
        let answer = match self.b.named_params {
            NamedParams::Spectre => row.spectre,
            NamedParams::Vacask | NamedParams::None => row.vacask,
        };
        let named = |n: &str| Some(Some((n.to_string(), v.to_string())));
        match answer {
            Param::To(t) => named(t),
            Param::Same => named(row.name),
            Param::Ignored => {
                self.note(Code::W0112, at);
                Some(None)
            }
            Param::Off(x) if models::fold(v) == Some(x) => Some(None),
            Param::Knee(_) if models::fold(v).is_some_and(|x| x.abs() < 1e-28) => Some(None),
            Param::Knee(Some(t)) => named(t),
            Param::YesNo(t) if matches!(models::fold(v), Some(0.0 | 1.0)) => {
                let yes = models::fold(v) == Some(1.0);
                Some(Some((
                    t.to_string(),
                    if yes { "yes" } else { "no" }.to_string(),
                )))
            }
            Param::Off(_)
            | Param::Knee(None)
            | Param::YesNo(_)
            | Param::Flag(..)
            | Param::TimesM
            | Param::OverM
            | Param::Refuse => {
                self.refuse(Code::W0114, at);
                None
            }
        }
    }

    /// The level-3 diode's two laws no row states, applied to `out`
    /// (Spectre's names, the implied defaults in); `None` when one has no
    /// Spectre form, refused at its parameter of statement `node`.
    ///
    /// Breakdown: ngspice's voltage is `bv - tcv*dT` at `tlev=0` and
    /// `bv*(1 - tcv*dT)` otherwise (`diotemp.c` L204-L209), Spectre's
    /// `bv*(1 + tbv1*dT)` at either (Spectre 25.1: settle-remote
    /// `t_m3_diodes`, P4-097). So `tbv1` is `-tcv/bv` or `-tcv`.
    ///
    /// Sidewall: with no `ns`, ngspice adds `jsw*pj` to the bottom's
    /// saturation current, so it conducts with `n`, yet scales it over
    /// temperature with `ns = 1` (`dioload.c` L257-L296, `diotemp.c`
    /// L113-L128). Spectre's sidewall diode conducts and scales with its
    /// `ns` (default 1). So `ns` is `n`, and `jsw` carries the scaling
    /// `ns = n` loses: `exp((1 - 1/n)*A)`, `A = (T/Tnom - 1)*eg/(k/q*T) +
    /// xti*ln(T/Tnom)`, in Spectre's `temp` and `tnom` (`[REF]` p.507);
    /// `k/q` is ngspice-45's (`CONSTboltz/CHARGE`).
    // The expression reads the circuit's `temp` and the model's `trise`;
    // a card's own `dtemp` (its `trise`) is not in it: that card is
    // refused (`diode_own_rise`, ruling R10).
    fn diode_laws(&mut self, node: u32, out: &mut Vec<(String, String)>) -> Option<()> {
        assert!(self.b.named_params == NamedParams::Spectre, "Spectre's");
        assert!(has(out, "eg"), "the implied `eg` comes before the laws");
        let raw = |name: &str| value_of(out, name).map(str::to_string);
        let (tcv, bv, jsw, n) = (raw("tbv1"), raw("bv"), raw("jsw"), raw("n"));
        let (eg, xti, tnom, rise) = (raw("eg"), raw("xti"), raw("tnom"), raw("trise"));
        let own = has(out, "ns");
        // `None`: no constant, so the law it selects is not known here.
        let tlev = raw("tlev").map_or(Some(0.0), |v| models::fold(&v));
        let folds = |v: &str, x: f64| models::fold(v) == Some(x);
        if let Some(tcv) = tcv.filter(|v| !folds(v, 0.0)) {
            if tlev.is_none() {
                self.refuse(Code::W0114, self.rd.param_token(node, "tcv"));
                return None;
            }
            let neg = match models::fold(&tcv) {
                Some(x) => (-x).to_string(),
                None => format!("-({})", self.value_text(&tcv)),
            };
            match (tlev == Some(0.0), &bv) {
                // ngspice reads `tcv` only beside a `bv` (`diotemp.c` L204).
                (true, None) => {}
                (true, Some(bv)) => {
                    replace(out, "tbv1", format!("{neg}/({})", self.value_text(bv)));
                }
                (false, _) => replace(out, "tbv1", neg),
            }
        }
        if let (Some(jsw), Some(n), Some(eg), false) = (&jsw, &n, &eg, own)
            && !folds(jsw, 0.0)
            && !folds(n, 1.0)
        {
            // `tlev=2` scales with the gap's own temperature law.
            if !matches!(tlev, Some(0.0 | 1.0)) {
                self.refuse(Code::W0114, self.rd.param_token(node, "jsw"));
                return None;
            }
            let temp = self.b.temp_name;
            let t = match &rise {
                Some(r) => format!("({temp}+({})+273.15)", self.value_text(r)),
                None => format!("({temp}+273.15)"),
            };
            let tnom = tnom
                .as_deref()
                .map_or("tnom".to_string(), |v| format!("({})", self.value_text(v)));
            let xti = xti
                .as_deref()
                .map_or("3".to_string(), |v| self.value_text(v));
            let ratio = format!("{t}/({tnom}+273.15)");
            let a = format!(
                "({ratio}-1)*({})/(8.6173303e-5*{t})+({xti})*ln({ratio})",
                self.value_text(eg)
            );
            let scaled = format!(
                "({})*exp((1-1/({}))*({a}))",
                self.value_text(jsw),
                self.value_text(n)
            );
            replace(out, "jsw", scaled);
            out.push(("ns".to_string(), n.clone()));
        }
        Some(())
    }
}

/// `out` with `name=v`, replacing an earlier `name`.
fn put(out: &mut Vec<(String, String)>, name: String, v: &str) {
    out.retain(|(n, _)| *n != name);
    out.push((name, v.to_string()));
}

/// `out`'s `name` given the value `v`, in its place.
fn replace(out: &mut [(String, String)], name: &str, v: String) {
    let at = out.iter().position(|(n, _)| n == name);
    assert!(at.is_some(), "`{name}` is a row of the card");
    if let Some(i) = at {
        out[i].1 = v;
    }
}

/// `e` with each identifier passed through `f`.
fn words(e: &str, f: impl Fn(&str) -> String) -> String {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(e.len());
    let mut rest = e;
    while let Some(i) = rest.find(ident) {
        out.push_str(&rest[..i]);
        let n = rest[i..]
            .find(|c: char| !ident(c))
            .map_or(rest.len() - i, |n| n);
        out.push_str(&f(&rest[i..i + n]));
        rest = &rest[i + n..];
    }
    out.push_str(rest);
    out
}

fn has(out: &[(String, String)], name: &str) -> bool {
    out.iter().any(|(n, _)| n == name)
}

fn value_of<'a>(out: &'a [(String, String)], name: &str) -> Option<&'a str> {
    out.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

/// Whether a level-3 diode card's `params` (ngspice's names) get
/// `diode_laws`' sidewall rewrite: a `jsw` (or `isw`) that is not 0 and an
/// `n` that is not 1, with no `ns` of its own.
fn sidewall_rescaled(params: &[(String, String)]) -> bool {
    let last = |names: &[&str]| {
        let named = |k: &str| names.iter().any(|n| k.eq_ignore_ascii_case(n));
        let found = params.iter().rev().find(|(k, _)| named(k));
        found.map(|(_, v)| v.as_str())
    };
    let folds = |v: &str, x: f64| models::fold(v) == Some(x);
    let (jsw, n) = (last(&["jsw", "isw"]), last(&["n"]));
    last(&["ns"]).is_none()
        && jsw.is_some_and(|v| !folds(v, 0.0))
        && n.is_some_and(|v| !folds(v, 1.0))
}
