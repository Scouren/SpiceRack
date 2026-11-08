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
//! - `DcSweep::values(&self) -> Option<Vec<f64>>`
//! - `Rd::dc_axes(&self, items: &[u32]) -> Option<Vec<DcAxis>>` (HSPICE's `type np` form too)
//! - `Rd::step_list(&self, items: &[u32]) -> Option<(String, Vec<f64>)>`
//! - `Rd::step(&self, n: u32, items: &[u32]) -> Option<Card>`, `Rd::xyce_lin(&self, n: u32) -> Option<Card>`,
//!   `Rd::top_ac(&self) -> Option<[u32; 4]>`,
//!   `Rd::noise(&self, n: u32, items: &[u32]) -> Option<Card>`, `Rd::hspice_pz(&self, n: u32) -> Option<Card>`

use super::Rd;
use super::testbench::{Card, Sweep};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::CardRules;

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

/// The most points a log axis expands to; a longer grid is refused.
const LOG_POINTS_MAX: usize = 100_000;

impl DcSweep {
    /// A log or list axis's values in sweep order: `start·f^(k/n)` (`f` 10
    /// or 2, `n` points per decade or octave) up to `stop` (Xyce RG `.DC`
    /// `DEC`; Xyce 7.10 ran 1, 1.585, …, 10 for `DEC 5` over 1..10), or the
    /// list as written; `None` for a linear axis, a log axis not over
    /// `0 < start <= stop`, or more than `LOG_POINTS_MAX` points.
    pub(in crate::syntax::render) fn values(&self) -> Option<Vec<f64>> {
        let sw = match self {
            DcSweep::Lin(..) => return None,
            DcSweep::List(v) => return Some(v.clone()),
            DcSweep::Log(sw) => sw,
        };
        if !(sw.start > 0.0 && sw.start <= sw.stop) {
            return None;
        }
        let f: f64 = if sw.variation == "dec" { 10.0 } else { 2.0 };
        let n = f64::from(sw.points);
        let end = sw.stop * (1.0 + 1e-9);
        let grid: Vec<f64> = (0..=LOG_POINTS_MAX)
            .map(|k| sw.start * f.powf(k as f64 / n))
            .take_while(|&x| x <= end)
            .collect();
        (grid.len() <= LOG_POINTS_MAX).then_some(grid)
    }
}

impl Rd<'_> {
    /// The axes of a `.dc` card's `items` when one sweeps by decade, by
    /// octave or over a list; `None` when every axis is linear (the plain
    /// `.dc` reads those) or the card does not read.
    pub(in crate::syntax::render) fn dc_axes(&self, items: &[u32]) -> Option<Vec<DcAxis>> {
        if self.b.card_rules == CardRules::Hspice {
            return self.dc_axes_hspice(items);
        }
        let word = |k: u32| self.is_word(k).then(|| self.text(k).to_ascii_lowercase());
        let (mut axes, mut i, mut lin_kw) = (Vec::new(), 0, false);
        // Each axis takes at least two items, so the loop ends by the card's length.
        while i < items.len() {
            let kw =
                word(items[i]).filter(|w| matches!(w.as_str(), "lin" | "dec" | "oct" | "list"));
            if kw.is_some() {
                lin_kw |= kw.as_deref() == Some("lin");
                i += 1;
            }
            let &src = items.get(i)?;
            if !self.is_word(src) || self.numw(src) {
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
        // A linear sweep read here only with Xyce's `LIN` keyword (Xyce
        // RG `.DC`); the plain `.dc` reads the others.
        let plain = !lin_kw && axes.iter().all(|a| matches!(a.sweep, DcSweep::Lin(..)));
        (!axes.is_empty() && !plain).then_some(axes)
    }

    /// A `.step` card's list sweep: its name (`temp`, a parameter, a
    /// device parameter) and values; `None` for another form.
    pub(in crate::syntax::render) fn step_list(&self, items: &[u32]) -> Option<(String, Vec<f64>)> {
        let is = |k: u32, w: &str| self.is_word(k) && self.text(k).eq_ignore_ascii_case(w);
        let mut it = items;
        // LTspice names a parameter after `param`, and `temp` bare.
        if self.b.card_rules == CardRules::Ltspice {
            match it.first() {
                Some(&p) if is(p, "param") => it = &it[1..],
                Some(&t) if is(t, "temp") => {}
                _ => return None,
            }
        }
        let [name, list, values @ ..] = it else {
            return None;
        };
        if !self.is_word(*name) || self.numw(*name) || !is(*list, "list") || values.is_empty() {
            return None;
        }
        let v: Vec<f64> = values
            .iter()
            .map(|&k| self.number(k))
            .collect::<Option<_>>()?;
        Some((self.text(*name), v))
    }

    /// Xyce's `.LIN [k=v]…` (Xyce RG `.LIN`): the S (and Y, Z) parameters
    /// over the deck's `.AC` sweep, ngspice's `.sp` of that sweep (ngspice
    /// manual `.sp`; Xyce 7.10 and ngspice-45 gave the same S11, S21, S22,
    /// run); `None` with `SPARCALC=0` (no S computed) or no `.AC`. Its
    /// other keys choose the output file's form only.
    pub(super) fn xyce_lin(&self, n: u32) -> Option<Card> {
        let pairs = self.fields(n);
        let off = pairs.iter().any(|&a| {
            self.tag(a) == Tag::Assign
                && self.key(a).eq_ignore_ascii_case("sparcalc")
                && self.number(self.ast.rhs(Node(a))) == Some(0.0)
        });
        if off || !pairs.iter().all(|&a| self.tag(a) == Tag::Assign) {
            return None;
        }
        Some(Card::Freq(true, self.top_ac()?))
    }

    /// The deck's first top `.ac` card's `variation points start stop`
    /// items, by that card's own rule (testbench `read_card`).
    pub(super) fn top_ac(&self) -> Option<[u32; 4]> {
        let ac = self
            .ast
            .fields(self.ast.root())
            .iter()
            .copied()
            .find(|&s| self.tag(s) == Tag::Analysis && self.head_kind(s) == "ac")?;
        match self.fields(ac) {
            &[v, a, b, c] if self.is_word(v) && [a, b, c].iter().all(|&k| self.numw(k)) => {
                Some([v, a, b, c])
            }
            _ => None,
        }
    }

    /// A `.noise` card's parts: `v(out[,ref]) src variation points start
    /// stop [pts_per_summary]`, or HSPICE's `out src [interval]` over the
    /// deck's `.ac`.
    pub(super) fn noise(&self, n: u32, items: &[u32]) -> Option<Card> {
        // HSPICE `.NOISE out src [interval]` sweeps the deck's `.AC`.
        let (items, spaced) = match items {
            [g, src, rest @ ..] if self.b.card_rules == CardRules::Hspice && rest.len() <= 1 => {
                let ac = self.top_ac()?;
                let v: Vec<u32> = [*g, *src]
                    .into_iter()
                    .chain(ac)
                    .chain(rest.iter().copied())
                    .collect();
                (v, self.spaced(n, items))
            }
            _ => (items.to_vec(), self.spaced(n, items)),
        };
        let [g, src, var, a, b, c, rest @ ..] = &items[..] else {
            return None;
        };
        if rest.len() > 1 || self.tag(*g) != Tag::Group || !self.group_closed(*g) || !spaced {
            return None;
        }
        let gn = self.ast.lhs(Node(*g));
        if gn == NONE || !self.text(gn).eq_ignore_ascii_case("v") {
            return None;
        }
        let nodes = self.group_items(*g);
        if nodes.is_empty() || nodes.len() > 2 || !nodes.iter().all(|&k| self.is_word(k)) {
            return None;
        }
        if !self.is_word(*src)
            || !self.is_word(*var)
            || ![a, b, c].iter().all(|&&k| self.numw(k))
            || rest.iter().any(|&k| !self.numw(k))
        {
            return None;
        }
        Some(Card::Noise(
            nodes.iter().map(|&k| self.text(k)).collect(),
            *src,
            [*var, *a, *b, *c],
            rest.first().copied(),
        ))
    }

    /// HSPICE's `.PZ v(out[,ref]) src` (HSPICE UG `.PZ`: the poles and
    /// zeros from source `src` to the output) as ngspice's `.pz in+ in-
    /// out+ out- vol|cur pz` over `src`'s nodes (`vol` for a V source,
    /// `cur` for an I; the ESPice corpus's pz_source.expected.json takes
    /// `.pz in 0 out 0 vol pz` as its twin); `None` for another form.
    pub(super) fn hspice_pz(&self, n: u32) -> Option<Card> {
        let rest = self.rest_text(n);
        let (out, after) = self.one_out(&rest)?;
        let src = after.trim();
        let inner = out
            .strip_prefix(['v', 'V'])?
            .strip_prefix('(')?
            .strip_suffix(')')?;
        let (op, om) = match inner.split_once(',') {
            Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
            None => (inner.trim().to_string(), "0".to_string()),
        };
        if src.is_empty() || src.contains(char::is_whitespace) {
            return None;
        }
        let card = self
            .ast
            .fields(self.ast.root())
            .iter()
            .copied()
            .find(|&s| {
                self.tag(s) == Tag::Instance
                    && self
                        .text(self.ast.header(Node(s)).name)
                        .eq_ignore_ascii_case(src)
            })?;
        let tf = match src.chars().next()?.to_ascii_lowercase() {
            'v' => "vol",
            'i' => "cur",
            _ => return None,
        };
        let items = self.items(card);
        let (ip, im) = (self.text(*items.first()?), self.text(*items.get(1)?));
        Some(Card::PzNodes([ip, im, op, om], tf))
    }

    /// HSPICE's `.DC src DEC|OCT np start stop`, `src POI n v…` and `src
    /// LIN np start stop` axes (HSPICE `.DC`, the `type np` form), beside
    /// linear `src start stop step` ones.
    fn dc_axes_hspice(&self, items: &[u32]) -> Option<Vec<DcAxis>> {
        let word = |k: u32| self.is_word(k).then(|| self.text(k).to_ascii_lowercase());
        let (mut axes, mut i) = (Vec::new(), 0);
        // Each axis takes at least three items, so the loop ends by the card's length.
        while i < items.len() {
            let &src = items.get(i)?;
            if !self.is_word(src) || self.numw(src) {
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
        if matches!(self.b.card_rules, CardRules::Xyce | CardRules::Ltspice)
            && self.spaced(n, items)
            && let Some((name, values)) = self.step_list(items)
        {
            return Some(Card::StepList(name, values));
        }
        // ngspice has no `.step`, so one in a deck read as ngspice is the
        // given deck's (`Lang::NGSPICE_GIVEN`): Xyce's form (RG p.155),
        // whose `lin|dec|oct` LTspice shares, and Spectre's total-count
        // `log` (Reference 19.1 p.406-410 `sweep`), which Python's
        // `step_sweep` names.
        let kinds: &[&str] = match self.b.card_rules {
            CardRules::Ngspice => &["lin", "dec", "oct", "log"],
            _ => &["lin", "dec", "oct"],
        };
        let mut it = items;
        let kind = match it.first() {
            Some(&k)
                if kinds
                    .iter()
                    .any(|w| self.is_word(k) && self.text(k).eq_ignore_ascii_case(w)) =>
            {
                it = &it[1..];
                Some(k)
            }
            _ => None,
        };
        if self.b.card_rules == CardRules::Ltspice {
            let (&p, rest) = it.split_first()?;
            if !(self.is_word(p) && self.text(p).eq_ignore_ascii_case("param")) {
                return None;
            }
            it = rest;
        }
        match it {
            [name, a, b, c]
                if self.spaced(n, items)
                    && self.is_word(*name)
                    && [a, b, c].iter().all(|&&k| self.numw(k)) =>
            {
                Some(Card::Step(kind, *name, [*a, *b, *c]))
            }
            _ => None,
        }
    }
}
