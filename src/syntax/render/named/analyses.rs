//! A testbench analysis → its Spectre statement: `dc` (a `sweep` around it
//! per further SPICE axis, a log or list axis as `values=[…]`), `ac`,
//! `tran`, `noise`, `xf`, `sens`, `sp`, `fourier`, `pz`, `montecarlo`, and
//! a Spectre analysis as read.
//!
//! Cites: Spectre Reference 19.1 `[REF]` p.64 (`dc`), p.43 (`ac`),
//! pp.416-417 (`tran`), pp.181-182 (`noise`), pp.438-439 (`xf`), p.524
//! (`sens`), pp.387-389 (`sp`), pp.406-410 (`sweep`, `values=`), p.174
//! (`montecarlo`); Spectre Reference 5.0 `[CMP]` pp.323-325 (`fourier`),
//! p.790 (`pz`).
//!
//! Contents:
//! - `W::analysis(&mut self, a: &Ana, tok: u32) -> Option<String>`
//! - `W::values(&self, v: &[f64]) -> String`

use super::W;
use super::bench::{Signal, bare_node, signal_of};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::read::{Ana, Card, Comp, DcSweep, Sweep};
use crate::syntax::render::swept::dc_value;

/// The most points an `oct` sweep's `values` list holds.
const OCT_POINTS_MAX: usize = 100_000;

impl W<'_, '_> {
    /// `a`'s statement (several lines `\n`-joined where Spectre needs a
    /// companion analysis, or one statement per output); `None` when it is
    /// refused.
    pub(super) fn analysis(&mut self, a: &Ana, tok: u32) -> Option<String> {
        let n = |w: &Self, v: f64| w.num(v);
        Some(match a {
            // `[REF]`: `dc1 dc`, a `dc` with no sweep is the operating point.
            Ana::Op => "op1 dc".into(),
            // SPICE's first sweep is the inner one; each further one a
            // `sweep` around it (`[REF]` pp.406-409: `dev`/`param`,
            // `start`/`stop`/`step` or `values=[…]`, analyses in braces).
            Ana::Dc(sweeps) => {
                let axes = sweeps
                    .iter()
                    .map(|(src, a, b, c)| {
                        let iv = format!(
                            "start={} stop={} step={}",
                            n(self, *a),
                            n(self, *b),
                            n(self, *c)
                        );
                        (self.dc_selector(src), iv)
                    })
                    .collect();
                nest_dc(axes)
            }
            // A log or list axis is its values (`DcSweep::values`).
            Ana::DcAxes(axes) => {
                if !matches!(self.ask(Construct::DcLogList, tok), Action::Native) {
                    return None;
                }
                let mut out = Vec::new();
                for ax in axes {
                    let iv = match (&ax.sweep, ax.sweep.values()) {
                        (DcSweep::Lin(a, b, c), _) => {
                            format!(
                                "start={} stop={} step={}",
                                n(self, *a),
                                n(self, *b),
                                n(self, *c)
                            )
                        }
                        (_, Some(v)) => self.values(&v),
                        (_, None) => {
                            self.refuse(Code::W0015, tok);
                            return None;
                        }
                    };
                    out.push((self.dc_selector(&ax.src), iv));
                }
                nest_dc(out)
            }
            Ana::Ac(sw) => format!("ac1 ac {}", self.points(sw, tok)?),
            // `step` (#9) is the closest analogue of SPICE's tstep.
            Ana::Tran {
                step,
                stop,
                start,
                max_step,
                uic,
                ..
            } => {
                let mut s = format!("tran1 tran step={} stop={}", n(self, *step), n(self, *stop));
                if let Some(st) = start {
                    s.push_str(&format!(" start={}", n(self, *st)));
                }
                if let Some(ms) = max_step {
                    s.push_str(&format!(" maxstep={}", n(self, *ms)));
                }
                if *uic {
                    s.push_str(" skipdc=yes");
                }
                s
            }
            // The output is the terminal list; `iprobe` names the input
            // source, a component.
            Ana::Noise {
                output,
                reference,
                source,
                sweep,
                ..
            } => {
                let neg = match reference.is_empty() {
                    true => "0",
                    false => bare_node(reference),
                };
                format!(
                    "noise1 ({} {neg}) noise {} iprobe={}",
                    bare_node(output),
                    self.points(sweep, tok)?,
                    self.inst_ref(source)
                )
            }
            // `[REF]` p.438: xf is the small-signal transfer function from
            // every source to the output (`V(out)/V(src)`), here at the one
            // frequency 0, SPICE's `.tf` at the operating point (p.439
            // `values`). It has no input or output resistance: noted.
            Ana::Tf(out, _) => {
                self.note(Code::W0125, tok);
                match signal_of(out) {
                    Some(Signal::Current(dev)) => {
                        format!("xf1 xf probe={} values=[0]", self.inst_ref(dev))
                    }
                    Some(Signal::Nodes(ns)) if ns.len() == 2 => {
                        format!(
                            "xf1 ({} {}) xf values=[0]",
                            self.path(ns[0]),
                            self.path(ns[1])
                        )
                    }
                    _ => format!("xf1 ({} 0) xf values=[0]", self.path(bare_node(out))),
                }
            }
            // `sens` is a control statement over a named analysis, which is
            // written too.
            Ana::Sens(out, Some(sw)) => format!(
                "sensac1 ac {}\nsens ({}) for (sensac1)",
                self.points(sw, tok)?,
                bare_node(out)
            ),
            Ana::Sens(out, None) => format!("sensdc1 dc\nsens ({}) for (sensdc1)", bare_node(out)),
            Ana::Sp(sw) => format!("sp1 sp {}", self.points(sw, tok)?),
            // `fourier` is a component, one per output.
            Ana::Four(f, outs) => outs
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    format!(
                        "four{} ({} 0) fourier fund={}",
                        i + 1,
                        bare_node(o),
                        n(self, *f)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            // Spectre Reference 5.0 p.790: poles and zeros from the
            // `iprobe` source to the output pair; SPICE's input node pair
            // is that source where one of the transfer's kind (`vol` a V,
            // `cur` an I) stands across it.
            Ana::Pz([i1, i2, o1, o2], tf, kind) => {
                let src = (*kind == "pz").then(|| self.pz_source(i1, i2, tf));
                let Some(Some(src)) = src else {
                    self.refuse(Code::W0015, tok);
                    return None;
                };
                if !matches!(self.ask(Construct::PoleZero, tok), Action::Native) {
                    return None;
                }
                format!(
                    "pz1 ({} {}) pz iprobe={src}",
                    self.path(bare_node(o1)),
                    self.path(bare_node(o2))
                )
            }
            // `[REF]` p.174: every parameter precedes the brace; the names
            // as written.
            Ana::MonteCarlo {
                name,
                runs,
                seed,
                variations,
                inner: (inner, kind),
            } => {
                let mut s = format!("{name} montecarlo numruns={runs}");
                if let Some(seed) = seed {
                    s.push_str(&format!(" seed={seed}"));
                }
                if let Some(v) = variations {
                    s.push_str(&format!(" variations={v}"));
                }
                format!("{s} {{\n  {inner} {kind}\n}}")
            }
            // Spectre's own analysis, as read.
            Ana::Spectre { node, .. } => self.rd.lines(*node).join("\n"),
        })
    }

    /// `values=[v1 v2 …]` in Spectre's numbers.
    pub(super) fn values(&self, v: &[f64]) -> String {
        let v: Vec<String> = v.iter().map(|&x| self.num(x)).collect();
        format!("values=[{}]", v.join(" "))
    }

    /// The top's source across `a b` of `tf`'s kind (`vol` a V, `cur`
    /// an I), as its declaration is written.
    fn pz_source(&self, a: &str, b: &str, tf: &str) -> Option<String> {
        let want = if tf == "vol" { 'V' } else { 'I' };
        let same = |x: &str, y: &str| match self.keep_case {
            true => x == y,
            false => x.eq_ignore_ascii_case(y),
        };
        self.deck.top.comps.iter().find_map(|c| {
            let Comp::Card(n) = *c else { return None };
            let Card::Source { letter, np, nm, .. } = self.rd.card_view(n) else {
                return None;
            };
            let across = same(&np, a) && same(&nm, b);
            (letter == want && across).then(|| {
                let l = letter.to_ascii_lowercase();
                format!("{l}{}", self.dev(&self.names.decl(n)[1..]))
            })
        })
    }

    /// `start=1 stop=1M dec=10`: a sweep's interval. `lin=` and `log=`
    /// count steps, one fewer than the points (`[REF]` p.43), so one point
    /// is `values=[start]`. Spectre has no `oct` (`[REF]` p.43):
    /// its points are `values=[…]`, ngspice's grid (`acan.c`: `f *=
    /// 2^(1/n)` from `start` while `f <= stop + 2^(1/n)*stop*reltol`).
    // ponytail: reltol is ngspice's default 1e-3, not the deck's; it moves
    // only whether a point a hair past `stop` is kept.
    fn points(&mut self, sw: &Sweep, tok: u32) -> Option<String> {
        let (a, b) = (self.num(sw.start), self.num(sw.stop));
        match (sw.variation, sw.points) {
            ("oct", _) => {}
            ("dec", n) => return Some(format!("start={a} stop={b} dec={n}")),
            (_, 1) => return Some(format!("values=[{a}]")),
            (v, n) => return Some(format!("start={a} stop={b} {v}={}", n.saturating_sub(1))),
        }
        let delta = 2f64.powf(1.0 / f64::from(sw.points));
        let end = sw.stop + delta * sw.stop * 1e-3;
        let mut grid = Vec::new();
        let mut f = sw.start;
        while f <= end && grid.len() <= OCT_POINTS_MAX && sw.start > 0.0 {
            grid.push(self.num(f));
            f *= delta;
        }
        if grid.is_empty() || grid.len() > OCT_POINTS_MAX {
            self.refuse(Code::W0015, tok);
            return None;
        }
        assert!(
            sw.start > 0.0 && delta > 1.0,
            "an octave grid rises from start"
        );
        Some(format!("values=[{}]", grid.join(" ")))
    }
}

// ---- private ----

/// `dc1 dc <axis 1>` inside one `dcsweep<k> sweep <axis k> {` per further
/// axis (`[REF]` pp.406-409); `dc1 dc` for none.
fn nest_dc(axes: Vec<(String, String)>) -> String {
    let Some((sel, iv)) = axes.first() else {
        return "dc1 dc".into();
    };
    let mut s = format!("dc1 dc {sel} {iv}");
    for (k, (sel, iv)) in axes.iter().enumerate().skip(1) {
        let inner = s.replace('\n', "\n  ");
        s = format!("dcsweep{k} sweep {sel} {iv} {{\n  {inner}\n}}");
    }
    s
}

impl W<'_, '_> {
    /// SPICE `.dc Vin 0 5 0.1` sweeps a source's dc value: `dev=vin
    /// param=dc`; `temp` is `param=temp`; a top-level R, C or L with no
    /// model its own value ([`dc_value`]): `dev=rl param=r` (`[REF]`
    /// p.408: `dev`, `param`); anything else a netlist parameter (`[REF]`
    /// p.64).
    fn dc_selector(&self, source: &str) -> String {
        let lower = source.to_lowercase();
        if lower == "temp" {
            return "param=temp".into();
        }
        if let Some(p) = dc_value(self.rd, self.deck, &self.names, source) {
            return format!("dev={lower} param={p}");
        }
        match lower.as_bytes().first() {
            Some(b'v' | b'i') => format!("dev={lower} param=dc"),
            _ => format!("param={source}"),
        }
    }
}
