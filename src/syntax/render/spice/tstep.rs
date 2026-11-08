//! A transient step for a deck whose source lets the simulator choose it
//! (Xyce's and LTspice's `.tran 0 T`; Xyce RG `.TRAN`), on ngspice, which
//! needs a TSTEP and caps its own step at TMAX = min(TSTEP, span/50)
//! (manual §11.3.10). The span over 50 alone lets ngspice step past a fast
//! source (`MEASURE/AverageTest.cir`: a 100 Hz SIN over 100 ms at 2 ms
//! steps, 5 a period, left the 1% tube; stream F round 4), so the step is
//! also held to a fiftieth of the fastest periodic source's period and a
//! tenth of the fastest exponential's time constant, and to span/1000
//! where a period or a time dependence is not a number (an expression, a
//! B source of `time`). PULSE and PWL corners are ngspice breakpoints.
//!
//! Contents:
//! - `W::chosen_step(&self, span: f64) -> f64`

use super::W;
use crate::syntax::render::read::{Card, Comp, Num, WaveKind};

/// Steps per period of the fastest periodic source.
const PER_PERIOD: f64 = 50.0;

impl W<'_, '_> {
    /// The TSTEP written for a zero one over `span` (see the module).
    pub(super) fn chosen_step(&self, span: f64) -> f64 {
        let mut step = span / 50.0;
        // A deck's own empty or reversed span: nothing to choose from.
        if span.is_nan() || span <= 0.0 {
            return step;
        }
        let mut cap = |t: Option<f64>| match t {
            Some(t) if t > 0.0 && t.is_finite() => step = step.min(t),
            Some(_) => {}
            None => step = step.min(span / 1000.0),
        };
        let period = |n: &Num| {
            n.num()
                .map(|f| if f > 0.0 { 1.0 / (f * PER_PERIOD) } else { 0.0 })
        };
        for b in self.blocks() {
            for &c in &b.comps {
                let Comp::Card(n) = c else { continue };
                match self.rd.card_view(n) {
                    Card::Source { wave: Some(w), .. } => match w.kind {
                        WaveKind::Sin => cap(period(&w.args[2])),
                        WaveKind::Sffm => {
                            cap(period(&w.args[2]));
                            cap(period(&w.args[4]));
                        }
                        WaveKind::Am => {
                            cap(period(&w.args[2]));
                            cap(period(&w.args[3]));
                        }
                        WaveKind::Exp => {
                            cap(w.args[3].num().map(|t| t / 10.0));
                            cap(w.args[5].num().map(|t| t / 10.0));
                        }
                        _ => {}
                    },
                    Card::Behavioural { expr, .. } if mentions_time(&expr) => cap(None),
                    _ => {}
                }
            }
        }
        // A positive step no coarser than ngspice's own TMAX cap.
        assert!(step > 0.0, "a chosen TSTEP is positive");
        assert!(step <= span / 50.0, "a chosen TSTEP is at most span/50");
        step
    }
}

// ---- private ----

/// Whether expression `e` names `time` (any case, a whole word).
fn mentions_time(e: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    e.split(|c: char| !ident(c))
        .any(|w| w.eq_ignore_ascii_case("time"))
}
