//! A source's waveform as Spectre `vsource`/`isource` parameters, and an
//! ngspice RF port as Spectre's `port`; the `Spell`/`Refuse` answers for
//! PWL repeats, noise waves and ports.
//!
//! Cites: Spectre Reference 5.0 `[CMP]` pp.683-686 (`vsource` waves),
//! p.602 (`port`).
//!
//! Contents:
//! - `W::wave(&mut self, letter, w, tok) -> Option<String>`
//! - `W::port(&mut self, tok, own, port, nodes, value, ac) -> Option<String>`

use super::W;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::read::{Num, Val, Wave, WaveKind};

impl W<'_, '_> {
    /// An ngspice RF port (`portnum num z0 r`) as Spectre's `port`
    /// (`[CMP]` p.602: `dc` and `mag` are across the port terminated in
    /// `r`, half ngspice's source behind it); `None` when it is refused.
    // ponytail: a wave or an expression value is refused; Spectre's
    // `type=` waves when a deck needs one.
    pub(super) fn port(
        &mut self,
        tok: u32,
        own: &str,
        (num, r): (u32, f64),
        [np, nm]: [&str; 2],
        (value, waved): (&Val, bool),
        (mag, phase): (Option<f64>, Option<f64>),
    ) -> Option<String> {
        if !matches!(self.ask(Construct::RfPort, tok), Action::Native) {
            return None;
        }
        let (Val::Num(v), false) = (value, waved) else {
            self.refuse(Code::W0007, tok);
            return None;
        };
        let mut s = format!(
            "v{} ({} {}) port num={num} r={} dc={}",
            self.dev(own),
            self.node(np),
            self.node(nm),
            self.num(r),
            self.num(v / 2.0)
        );
        if let Some(mag) = mag {
            s.push_str(&format!(" mag={}", self.num(mag / 2.0)));
            if let Some(phase) = phase {
                s.push_str(&format!(" phase={}", self.num(phase)));
            }
        }
        Some(s)
    }

    fn wave_num(&self, n: &Num) -> String {
        match n {
            Num::Num(x) => self.num(*x),
            Num::Expr(e) => e.clone(),
        }
    }

    /// `w` as `type=…` parameters (`[CMP]` pp.683-685: `dc pulse pwl sine
    /// exp`); `None` when it is refused.
    pub(super) fn wave(&mut self, letter: char, w: &Wave, tok: u32) -> Option<String> {
        let a: Vec<String> = w.args.iter().map(|x| self.wave_num(x)).collect();
        let zero = |x: &Num| *x == Num::Num(0.0);
        Some(match w.kind {
            // `delay` is the general waveform parameter (p.683 #4).
            WaveKind::Sin => {
                let mut s = format!("type=sine sinedc={} ampl={} freq={}", a[0], a[1], a[2]);
                if !zero(&w.args[3]) {
                    s.push_str(&format!(" delay={}", a[3]));
                }
                if !zero(&w.args[4]) {
                    s.push_str(&format!(" damp={}", a[4]));
                }
                if !zero(&w.args[5]) {
                    s.push_str(&format!(" sinephase={}", a[5]));
                }
                s
            }
            WaveKind::Pulse => format!(
                "type=pulse val0={} val1={} delay={} rise={} fall={} width={} period={}",
                a[0], a[1], a[2], a[3], a[4], a[5], a[6]
            ),
            WaveKind::Pwl => {
                let period = match &w.repeat {
                    Some(r) => {
                        let c = match letter {
                            'V' => Construct::PwlRepeatV,
                            _ => Construct::PwlRepeatI,
                        };
                        let exact = (zero(r)).then(|| pwl_period(&w.args)).flatten();
                        match (self.ask(c, tok), exact) {
                            (Action::Spell(_), Some(p)) => Some(p),
                            (Action::Refuse(_), _) => return None,
                            _ => {
                                self.refuse(Code::W0008, tok);
                                return None;
                            }
                        }
                    }
                    None => None,
                };
                let pairs: Vec<String> = a.chunks(2).map(|p| p.join(" ")).collect();
                let mut s = format!("type=pwl wave=[{}]", pairs.join(" "));
                if let Some(d) = &w.delay {
                    s.push_str(&format!(" delay={}", self.wave_num(d)));
                }
                if let Some(p) = period {
                    s.push_str(&format!(" pwlperiod={}", self.num(p)));
                }
                s
            }
            // p.685: td1 tau1 td2 tau2, val0/val1 shared with pulse.
            WaveKind::Exp => format!(
                "type=exp val0={} val1={} td1={} tau1={} td2={} tau2={}",
                a[0], a[1], a[2], a[3], a[4], a[5]
            ),
            // pp.684-685: SFFM is a sine carrier with fmmodindex/fmmodfreq.
            WaveKind::Sffm => format!(
                "type=sine sinedc={} ampl={} freq={} fmmodindex={} fmmodfreq={}",
                a[0], a[1], a[2], a[3], a[4]
            ),
            WaveKind::Trnoise | WaveKind::Trrandom => {
                self.ask(Construct::NoiseWave, tok);
                return None;
            }
            // Spectre has ammodindex/ammodfreq but publishes no waveform
            // equation to map SPICE's AM() onto.
            WaveKind::Am => {
                self.refuse(Code::W0108, tok);
                return None;
            }
        })
    }
}

// ---- private ----

/// The `pwlperiod` that repeats PWL points `xs` (time, value flat) exactly
/// as ngspice's `r=0` does, when one does: numeric points from t = 0 whose
/// last value equals the first, the last segment at least the default
/// `twidth` (period/1000) long (`[CMP]` `vsource` #17, #18, p.686).
fn pwl_period(xs: &[Num]) -> Option<f64> {
    let n = xs.len();
    if n < 4 {
        return None;
    }
    let (t0, v0) = (xs[0].num()?, xs[1].num()?);
    let (tp, tn, vn) = (xs[n - 4].num()?, xs[n - 2].num()?, xs[n - 1].num()?);
    (t0 == 0.0 && vn == v0 && tn > 0.0 && tn - tp >= tn / 1000.0).then_some(tn)
}
