//! SPICE `.options` → [`Opt`] views (as the old lowering applied them): the
//! portable `k=v` list, Xyce's per-package form (`.options <pkg> k=v`, `k v`,
//! comma lists), `DEVICE TEMP`/`TNOM`, and each Xyce option that is a
//! portable one under a package (`TIMEINT RELTOL` is `reltol`: the arms of
//! `Rd::options_view`).
//!
//! Cites: the old lowering; the old SPICE grammar's `Options`,
//! `oitem`; the old Xyce grammar's `Options`, `Pkg`, `xopt`, `Pair_x`, `MVal_x`,
//! `Flag_x`; Xyce Reference Guide p.96.
//!
//! Contents:
//! - `pub(in crate::syntax::render) enum Opt`
//! - `Rd::options_ok(&self, n: u32) -> bool`
//! - `Rd::options_view(&self, n: u32) -> Vec<Opt>`

use super::Rd;
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::{AnalysisForm, OptionsForm};
use crate::syntax::token::Tag as T;

/// One option a `.options` card sets.
#[derive(Clone, Debug)]
pub(in crate::syntax::render) enum Opt {
    /// A `k=v` (a flag: `v` empty) every SPICE dialect spells alike.
    Portable(String, String),
    /// Xyce's own: `pkg.key` (`pkg.` alone for a bare package; a key with
    /// no package when the card names none) and its value.
    Xyce(String, String),
    /// Xyce `.options device temp=`, or `.options temp=` with no package.
    Temp(f64),
    /// Xyce `.options device tnom=`, or `.options tnom=` with no package.
    Tnom(f64),
    /// HSPICE `.option delmax=`: the largest step, in seconds (> 0), of
    /// every transient of the deck (cmdref `.OPTION DELMAX`).
    MaxStep(f64),
    /// VACASK's own `options k=v`, its value as written.
    Vacask(String, String),
    /// `ngbehavior=<mode>`: ngspice's compatibility mode, which ngspice
    /// reads only before the deck (its `-D ngbehavior=`), so no card of
    /// any dialect sets it (`Construct::NgBehavior`).
    NgBehavior(String),
}

impl<'a> Rd<'a> {
    /// `.options` items (the old SPICE grammar's `oitem`, the old Xyce grammar's `Options`).
    pub(in crate::syntax::render) fn options_ok(&self, n: u32) -> bool {
        let items = self.fields(n);
        if self.b.options_form == OptionsForm::XycePackages && self.xyce_pkg(n).is_some() {
            return self.only_commas_between(n, items) && self.xyce_pairs(&items[1..]).is_some();
        }
        self.only_commas_between(n, items)
            && items.iter().all(|&k| {
                self.is_word(k)
                    || self.tag(k) == Tag::Assign
                        && self.is_word(self.ast.lhs(Node(k)))
                        && self.val_ok(self.ast.rhs(Node(k)))
            })
    }

    /// What `.options` card `n` sets, in order (the old lowering).
    pub(in crate::syntax::render) fn options_view(&self, n: u32) -> Vec<Opt> {
        let items = self.fields(n);
        match (self.b.options_form, self.xyce_pkg(n)) {
            (OptionsForm::XycePackages, Some(pkg)) => {
                let pkg = self.text(pkg).to_ascii_lowercase();
                let pairs = self.xyce_pairs(&items[1..]).expect("options_ok read it");
                let mut out = Vec::new();
                if pairs.is_empty() {
                    out.push(Opt::Xyce(format!("{pkg}."), String::new()));
                }
                for (k, v, x) in pairs {
                    out.push(match (pkg.as_str(), k.to_ascii_lowercase().as_str(), x) {
                        ("device", "temp", Some(x)) => Opt::Temp(x),
                        ("device", "tnom", Some(x)) => Opt::Tnom(x),
                        // RG `.OPTIONS PARSER` SCALE: the scale of every
                        // L and W, SPICE's SCALE.
                        ("parser", "scale", Some(_)) => Opt::Portable("scale".into(), v),
                        // RG Table 2-2 (DEVICE) GMIN: the minimum
                        // conductance, SPICE's GMIN.
                        ("device", "gmin", Some(_)) => Opt::Portable("gmin".into(), v),
                        // RG Table 2-5: METHOD `gear` or 8 is the Gear
                        // method and MAXORD the highest order tried,
                        // SPICE's `method=gear` and `maxord`.
                        ("timeint", "method", _) if v.eq_ignore_ascii_case("gear") || v == "8" => {
                            Opt::Portable("method".into(), "gear".into())
                        }
                        ("timeint", "maxord", Some(_)) => Opt::Portable("maxord".into(), v),
                        // RG `.OPTIONS TIMEINT` RELTOL: the integrator's
                        // relative tolerance, SPICE's RELTOL.
                        ("timeint", "reltol", Some(_)) => Opt::Portable("reltol".into(), v),
                        _ => Opt::Xyce(format!("{pkg}.{}", k.to_ascii_lowercase()), v),
                    });
                }
                out
            }
            // No package word: Xyce reads `temp=` and `tnom=` as DEVICE's
            // (7.10 by hand: `.options temp=100`, `.option Temp=100.0C` and
            // `.options device temp=100` give one answer).
            (OptionsForm::XycePackages, None) => {
                let pairs = self.params_of(items, false);
                assert_eq!(pairs.len(), items.len(), "options_ok: words and `k=v` only");
                let num = |i: u32| match self.tag(i) {
                    Tag::Assign => self.number(self.ast.rhs(Node(i))),
                    _ => None,
                };
                pairs
                    .into_iter()
                    .zip(items)
                    .map(
                        |((k, v), &i)| match (k.to_ascii_lowercase().as_str(), num(i)) {
                            ("temp", Some(x)) => Opt::Temp(x),
                            ("tnom", Some(x)) => Opt::Tnom(x),
                            _ => Opt::Xyce(k, v),
                        },
                    )
                    .collect()
            }
            _ => {
                // HSPICE's `.OPTION DELMAX` (cmdref p.443) is the step cap its
                // writer spells that way (`spice/analyses.rs`); ngspice-45
                // has no such option and reads the card without a word. A
                // value that is no positive number stays an option.
                let hspice = self.b.analysis_form == AnalysisForm::Hspice;
                let lex = self.t.lexicon_at(self.main(n));
                self.params_of(items, false)
                    .into_iter()
                    .map(|(k, v)| match k.to_ascii_lowercase().as_str() {
                        "ngbehavior" => Opt::NgBehavior(v),
                        "delmax" if hspice => match self.number_text(&v, lex) {
                            Some(x) if x > 0.0 => Opt::MaxStep(x),
                            _ => Opt::Portable(k, v),
                        },
                        _ => Opt::Portable(k, v),
                    })
                    .collect()
            }
        }
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// quirk: whether the only tokens between the card's items (and after them)
    /// are commas: the old SPICE grammar's `oitem` reads no stray `( ) =`.
    fn only_commas_between(&self, n: u32, items: &[u32]) -> bool {
        let mut prev = self.main(n);
        for &k in items.iter() {
            if !(prev + 1..self.first(k)).all(|x| self.ttag(x) == T::Comma) {
                return false;
            }
            prev = self.last(k);
        }
        (prev + 1..=self.line_last(self.main(n))).all(|x| self.ttag(x) == T::Comma)
    }

    /// quirk: the old Xyce grammar's `xopt`: `Pair_x` (`k=v` or `k v`, the
    /// value run on
    /// by comma-separated words or blank-separated numbers, `MVal_x`) or
    /// `Flag_x` (a word or a brace): `(key, value text, its number)`.
    fn xyce_pairs(&self, rest: &[u32]) -> Option<Vec<(String, String, Option<f64>)>> {
        let mut out = Vec::new();
        let mut i = 0;
        let numberish = |k: u32| {
            self.is_word(k)
                && self
                    .text(k)
                    .starts_with(|c: char| c.is_ascii_digit() || "+-.".contains(c))
        };
        while i < rest.len() {
            let k = rest[i];
            let (key, v) = match self.tag(k) {
                Tag::Assign if self.is_word(self.ast.lhs(Node(k))) => {
                    (self.key(k), self.ast.rhs(Node(k)))
                }
                Tag::Word
                    if rest
                        .get(i + 1)
                        .is_some_and(|&x| numberish(x) && !self.comma_between(k, x))
                        && self
                            .text(k)
                            .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') =>
                {
                    i += 1;
                    (self.text(k), rest[i])
                }
                Tag::Word | Tag::Delimited => {
                    out.push((self.text(k), String::new(), None));
                    i += 1;
                    continue;
                }
                _ => return None,
            };
            if v == NONE
                || !matches!(self.tag(v), Tag::Word | Tag::Str | Tag::Delimited)
                    && self.nests_delimiter(v)
            {
                return None;
            }
            let mut text = self.text(v).trim().to_string();
            let mut prev = v;
            let mut more = false;
            while let Some(&x) = rest.get(i + 1)
                && self.is_word(x)
                && (self.comma_between(prev, x) || numberish(x))
            {
                text.push_str(if self.comma_between(prev, x) {
                    ","
                } else {
                    " "
                });
                text.push_str(&self.text(x));
                (prev, more) = (x, true);
                i += 1;
            }
            let num = if more { None } else { self.number(v) };
            out.push((key, text, num));
            i += 1;
        }
        Some(out)
    }

    /// Whether a comma stands between items `a` and `b`.
    fn comma_between(&self, a: u32, b: u32) -> bool {
        (self.last(a) + 1..self.first(b)).any(|k| self.ttag(k) == T::Comma)
    }

    /// the old Xyce grammar's `Pkg`: a first word that is no `key=`.
    fn xyce_pkg(&self, n: u32) -> Option<u32> {
        self.fields(n).first().copied().filter(|&k| self.is_word(k))
    }
}
