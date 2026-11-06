//! SPICE sweeps past the linear `.dc` and `.step`: a `.dc` axis that sweeps
//! by decade or octave or over a list of values → [`DcAxis`] rows; a
//! `.step` over a list → its name and values.
//!
//! Cites: Xyce RG `.DC` (`.DC [LIN|DEC|OCT] src start stop step|points`,
//! `.DC src LIST v…`; also `LIST src v…`, which Xyce 7.10 runs the same);
//! LTspice help `.DC` (`.dc dec src start stop points`); HSPICE `.DC`
//! (`var DEC|OCT|LIN np start stop`, `var POI np v…`); Xyce RG `.STEP`
//! (`.STEP <name> LIST <val>*`, `temp` among the names); LTspice help
//! `.STEP` (`.step param <name> list <val>*`, `.step temp list <val>*`).
//!
//! Contents:
//! - `pub(in crate::syntax::render) struct DcAxis`, `enum DcSweep`
//! - `Rd::dc_axes(&self, items: &[u32]) -> Option<Vec<DcAxis>>` (HSPICE's `type np` form too)
//! - `Rd::step_list(&self, items: &[u32]) -> Option<(String, Vec<f64>)>`
//! - `Rd::step(&self, n: u32, items: &[u32]) -> Option<Card>`

use super::Rd;
use super::testbench::{Card, Sweep};
use crate::syntax::Dialect;

/// One `.dc` axis: its source (or parameter) and how it sweeps.
#[derive(Clone, Debug)]
pub(in crate::syntax::render) struct DcAxis {
    pub(in crate::syntax::render) src: String,
    pub(in crate::syntax::render) sweep: DcSweep,
}

/// A `.dc` axis's sweep.
#[derive(Clone, Debug)]
pub(in crate::syntax::render) enum DcSweep {
    /// Start, stop, step.
    Lin(f64, f64, f64),
    /// `dec` or `oct`, its points per decade or octave, start, stop.
    Log(Sweep),
    /// The values in order.
    List(Vec<f64>),
}

impl Rd<'_> {
    /// The axes of a `.dc` card's `items` when one sweeps by decade, by
    /// octave or over a list; `None` when every axis is linear (the plain
    /// `.dc` reads those) or the card does not read.
    pub(in crate::syntax::render) fn dc_axes(&self, items: &[u32]) -> Option<Vec<DcAxis>> {
        if self.d == Dialect::Hspice {
            return self.dc_axes_hspice(items);
        }
        let word = |k: u32| self.wordw(k).then(|| self.text(k).to_ascii_lowercase());
        let (mut axes, mut i) = (Vec::new(), 0);
        // Each axis takes at least two items, so the loop ends by the card's length.
        while i < items.len() {
            let kw =
                word(items[i]).filter(|w| matches!(w.as_str(), "lin" | "dec" | "oct" | "list"));
            if kw.is_some() {
                i += 1;
            }
            let &src = items.get(i)?;
            if !self.wordw(src) || self.numw(src) {
                return None;
            }
            i += 1;
            let list = kw.as_deref() == Some("list")
                || (kw.is_none() && word(*items.get(i)?).as_deref() == Some("list"));
            if list && kw.is_none() {
                i += 1;
            }
            let sweep = if list {
                let start = i;
                while i < items.len() && self.numw(items[i]) {
                    i += 1;
                }
                let values: Vec<f64> = items[start..i]
                    .iter()
                    .filter_map(|&k| self.number(k))
                    .collect();
                if values.is_empty() {
                    return None;
                }
                DcSweep::List(values)
            } else {
                let nums = items.get(i..i + 3)?;
                i += 3;
                let [a, b, c] = [nums[0], nums[1], nums[2]].map(|k| self.number(k));
                let (start, stop) = (a?, b?);
                match kw.as_deref() {
                    Some(v @ ("dec" | "oct")) => DcSweep::Log(Sweep {
                        variation: if v == "dec" { "dec" } else { "oct" },
                        points: self.count(nums[2]).filter(|&p| p > 0)?,
                        start,
                        stop,
                    }),
                    _ => DcSweep::Lin(start, stop, c?),
                }
            };
            axes.push(DcAxis {
                src: self.text(src),
                sweep,
            });
        }
        let plain = axes.iter().all(|a| matches!(a.sweep, DcSweep::Lin(..)));
        (!axes.is_empty() && !plain).then_some(axes)
    }

    /// A `.step` card's list sweep: its name (`temp`, a parameter, a
    /// device parameter) and values; `None` for another form.
    pub(in crate::syntax::render) fn step_list(&self, items: &[u32]) -> Option<(String, Vec<f64>)> {
        let is = |k: u32, w: &str| self.wordw(k) && self.text(k).eq_ignore_ascii_case(w);
        let mut it = items;
        // LTspice names a parameter after `param`, and `temp` bare.
        if self.d == Dialect::Ltspice {
            match it.first() {
                Some(&p) if is(p, "param") => it = &it[1..],
                Some(&t) if is(t, "temp") => {}
                _ => return None,
            }
        }
        let [name, list, values @ ..] = it else {
            return None;
        };
        if !self.wordw(*name) || self.numw(*name) || !is(*list, "list") || values.is_empty() {
            return None;
        }
        let v: Vec<f64> = values
            .iter()
            .map(|&k| self.number(k))
            .collect::<Option<_>>()?;
        Some((self.text(*name), v))
    }

    /// HSPICE's `.DC src DEC|OCT np start stop`, `src POI n v…` and `src
    /// LIN np start stop` axes (HSPICE `.DC`, the `type np` form), beside
    /// linear `src start stop step` ones.
    fn dc_axes_hspice(&self, items: &[u32]) -> Option<Vec<DcAxis>> {
        let word = |k: u32| self.wordw(k).then(|| self.text(k).to_ascii_lowercase());
        let (mut axes, mut i) = (Vec::new(), 0);
        // Each axis takes at least three items, so the loop ends by the card's length.
        while i < items.len() {
            let &src = items.get(i)?;
            if !self.wordw(src) || self.numw(src) {
                return None;
            }
            let kw = word(*items.get(i + 1)?)
                .filter(|w| matches!(w.as_str(), "lin" | "dec" | "oct" | "poi"));
            let sweep = match kw.as_deref() {
                None => {
                    let n = items.get(i + 1..i + 4)?;
                    i += 4;
                    let [a, b, c] = [n[0], n[1], n[2]].map(|k| self.number(k));
                    DcSweep::Lin(a?, b?, c?)
                }
                Some("poi") => {
                    let np = self.count(*items.get(i + 2)?)? as usize;
                    let v = items.get(i + 3..i + 3 + np)?;
                    i += 3 + np;
                    DcSweep::List(v.iter().map(|&k| self.number(k)).collect::<Option<_>>()?)
                }
                Some(t) => {
                    let n = items.get(i + 2..i + 5)?;
                    i += 5;
                    let np = self.count(n[0]).filter(|&p| p > 0)?;
                    let (start, stop) = (self.number(n[1])?, self.number(n[2])?);
                    match t {
                        "lin" if np > 1 => {
                            DcSweep::Lin(start, stop, (stop - start) / f64::from(np - 1))
                        }
                        "lin" => return None,
                        v => DcSweep::Log(Sweep {
                            variation: if v == "dec" { "dec" } else { "oct" },
                            points: np,
                            start,
                            stop,
                        }),
                    }
                }
            };
            axes.push(DcAxis {
                src: self.text(src),
                sweep,
            });
        }
        let plain = axes.iter().all(|a| matches!(a.sweep, DcSweep::Lin(..)));
        (!axes.is_empty() && !plain).then_some(axes)
    }

    /// A `.step` card's parts (Xyce RG `.STEP`, LTspice help `.step`).
    pub(super) fn step(&self, n: u32, items: &[u32]) -> Option<Card> {
        if matches!(self.d, Dialect::Xyce | Dialect::Ltspice)
            && self.spaced(n, items)
            && let Some((name, values)) = self.step_list(items)
        {
            return Some(Card::StepList(name, values));
        }
        // ngspice has no `.step`, so one in a deck read as ngspice is the
        // given deck's (`lexicon::NGSPICE_GIVEN`): Xyce's form (RG p.155),
        // whose `lin|dec|oct` LTspice shares, and Spectre's total-count
        // `log` (Reference 19.1 p.406-410 `sweep`), which Python's
        // `step_sweep` names.
        let kinds: &[&str] = match self.d {
            Dialect::Ngspice => &["lin", "dec", "oct", "log"],
            _ => &["lin", "dec", "oct"],
        };
        let mut it = items;
        let kind = match it.first() {
            Some(&k)
                if kinds
                    .iter()
                    .any(|w| self.wordw(k) && self.text(k).eq_ignore_ascii_case(w)) =>
            {
                it = &it[1..];
                Some(k)
            }
            _ => None,
        };
        if self.d == Dialect::Ltspice {
            let (&p, rest) = it.split_first()?;
            if !(self.wordw(p) && self.text(p).eq_ignore_ascii_case("param")) {
                return None;
            }
            it = rest;
        }
        match it {
            [name, a, b, c]
                if self.spaced(n, items)
                    && self.wordw(*name)
                    && [a, b, c].iter().all(|&&k| self.numw(k)) =>
            {
                Some(Card::Step(kind, *name, [*a, *b, *c]))
            }
            _ => None,
        }
    }
}
