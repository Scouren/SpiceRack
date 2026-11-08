//! Testbench [`Card`]s an analysis's grammar rule read → their [`Ana`]
//! views (the old lowering's `analysis`): numbers read, sweep keywords
//! named, a `.tran`'s forms sorted; `None` when the IR cannot hold one. A new
//! analysis card is a variant of `Card` and an arm here (the match names
//! every variant).
//!
//! Cites: the old lowering's `analysis`; ngspice manual ch.15 (`.dc`, `.ac`,
//! `.tran`, `.noise`, `.tf`, `.sens`, `.pz`, `.four`).
//!
//! Contents:
//! - `Rd::analysis(&self, card: &Card) -> Option<Ana>`

use super::Rd;
use super::sweeps::DcSweep;
use super::testbench::{Ana, Card, SWEEPS, Sweep, keyword};
use crate::syntax::ast::Node;

impl Rd<'_> {
    /// An analysis card's view (the old lowering's `analysis`).
    pub(super) fn analysis(&self, card: &Card) -> Option<Ana> {
        let num = |k: u32| self.number(k);
        let sweep = |f: &[u32; 4]| -> Option<Sweep> {
            Some(Sweep {
                variation: keyword(&self.text(f[0]), SWEEPS)?,
                points: self.count(f[1])?,
                start: num(f[2])?,
                stop: num(f[3])?,
            })
        };
        Some(match card {
            Card::Op => Ana::Op,
            Card::Dc(sweeps) => Ana::Dc(
                sweeps
                    .iter()
                    .map(|s| Some((self.text(s[0]), num(s[1])?, num(s[2])?, num(s[3])?)))
                    .collect::<Option<_>>()?,
            ),
            // Xyce's `.DC LIN src a b c` is the plain linear sweep.
            Card::DcAxes(axes) => match axes
                .iter()
                .map(|a| match a.sweep {
                    DcSweep::Lin(x, y, z) => Some((a.src.clone(), x, y, z)),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
            {
                Some(lin) => Ana::Dc(lin),
                None => Ana::DcAxes(axes.clone()),
            },
            Card::Freq(false, f) => Ana::Ac(sweep(f)?),
            Card::Freq(true, f) => Ana::Sp(sweep(f)?),
            Card::Tran(nums, pair, uic, startup) => {
                let p: Vec<f64> = nums.iter().map(|&k| num(k)).collect::<Option<_>>()?;
                let start = match pair {
                    Some(pr) if self.key(*pr).eq_ignore_ascii_case("start") => {
                        Some(num(self.ast.rhs(Node(*pr)))?)
                    }
                    Some(_) => return None,
                    None => None,
                };
                let (uic, startup) = (*uic, *startup);
                match p.as_slice() {
                    [stop] => Ana::Tran {
                        step: 0.0,
                        stop: *stop,
                        start: None,
                        max_step: None,
                        uic,
                        startup,
                    },
                    [step, stop] if start.is_some() => Ana::Tran {
                        step: *step,
                        stop: *stop,
                        start,
                        max_step: None,
                        uic,
                        startup,
                    },
                    [step, stop, rest @ ..] if rest.len() <= 2 => Ana::Tran {
                        step: *step,
                        stop: *stop,
                        start: rest.first().copied(),
                        max_step: rest.get(1).copied(),
                        uic,
                        startup,
                    },
                    _ => return None,
                }
            }
            Card::Noise(nodes, src, f, pps) => Ana::Noise {
                output: nodes.first()?.clone(),
                reference: nodes.get(1).cloned().unwrap_or_else(|| "0".into()),
                source: self.text(*src),
                sweep: sweep(f)?,
                pps: pps.and_then(|k| self.text(k).parse().ok()),
            },
            Card::Tf(out, src) => Ana::Tf(out.clone(), src.clone()),
            Card::Sens(out, ac) => Ana::Sens(
                out.clone(),
                match ac {
                    Some(f) => Some(sweep(f)?),
                    None => None,
                },
            ),
            Card::Pz(w) => Ana::Pz(
                [
                    self.text(w[0]),
                    self.text(w[1]),
                    self.text(w[2]),
                    self.text(w[3]),
                ],
                keyword(&self.text(w[4]), &["vol", "cur"])?,
                keyword(&self.text(w[5]), &["pol", "zer", "pz"])?,
            ),
            Card::PzNodes(nodes, tf) => Ana::Pz(nodes.clone(), tf, "pz"),
            Card::Four(f, outs) => Ana::Four(num(*f)?, outs.clone()),
            // The testbench's other cards ([`Rd::bench_card`]).
            Card::Options
            | Card::Temp(_)
            | Card::NodeVals(_)
            | Card::Save(_)
            | Card::Probe(_)
            | Card::Print(..)
            | Card::Meas
            | Card::Step(..)
            | Card::StepList(..) => return None,
        })
    }
}
