//! One element → its VACASK instance statement (`r1 (a b) resistor
//! r=1e3`), named by the model its module's `model` line declares, and its
//! source waveform; the refusals for what VACASK has no element or
//! waveform for.
//!
//! Cites: VACASK `share/doc/vacask/{demo,test}/*.sim` (as the old VACASK writer
//! probed them), `docs/input-identifiers.md`.
//!
//! Contents:
//! - `W::vacask_card(&mut self, n: u32) -> Option<String>`
//! - `W::dc_alters(&self, swept: &[String]) -> Vec<(String, &'static str)>`

use super::W;
use super::vacask::{num, q};
use crate::syntax::behaviour::ModelSemantics;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::Construct;
use crate::syntax::render::read::{Card, Comp, Num, Val, Wave, WaveKind};

impl W<'_, '_> {
    /// Element `n`'s statement, with no line end; `None` when it is
    /// refused.
    pub(super) fn vacask_card(&mut self, n: u32) -> Option<String> {
        let tok = self.rd.main(n);
        // The declaration's name after its type letter (the name map's).
        let own = &self.names.decl(n)[1..].to_string();
        Some(match self.rd.card_view(n) {
            Card::Rcl {
                letter,
                name: _,
                n1,
                n2,
                value,
                model,
                params,
            } => {
                // A model card read as ngspice maps through
                // `render::models` (`sp_resistor`, `sp_capacitor`).
                let ngspice = self.rd.b.model_semantics == ModelSemantics::Ngspice;
                if model.is_some() && !(ngspice && matches!(letter, 'R' | 'C')) {
                    self.ask(Construct::RclModel, tok);
                    return None;
                }
                if ngspice
                    && letter == 'R'
                    && let Some((r, kept)) = self.dependent(&value, model.is_some(), &params, n)?
                {
                    // ngspice's `B` line is noiseless (`sp_resistor`'s
                    // `noisy`, `devices/spice/resistor.va`).
                    return Some(format!(
                        "r{} ({} {}) resistor r={}{} noisy=0",
                        q(own),
                        q(&n1),
                        q(&n2),
                        self.expr(&r),
                        self.inst_params(&kept)
                    ));
                }
                let mut params = self.tran_ic(letter, params, n);
                let extra = self.parasitics(letter, n, &n1, &n2, &mut params);
                let n1 = extra.as_ref().map_or(n1, |(at, _)| at.clone());
                let (l, m) = match letter {
                    'R' => ('r', "resistor"),
                    'C' => ('c', "capacitor"),
                    _ => ('l', "inductor"),
                };
                let value = match &value {
                    Val::Raw(t) if t.is_empty() => String::new(),
                    v => format!(" {l}={}", self.vval(v)),
                };
                format!(
                    "{l}{} ({} {}) {}{value}{}{}",
                    q(own),
                    q(&n1),
                    q(&n2),
                    model.map_or(m.to_string(), |m| q(&m)),
                    self.inst_params(&params),
                    self.part_lines(extra)
                )
            }
            // One `mutual` per pair, the first keeping the name.
            Card::Coupling { ls, k, .. } => crate::syntax::render::spice::pairs(&ls)
                .enumerate()
                .map(|(i, (a, b))| {
                    let suffix = crate::syntax::render::spice::pair_suffix(i);
                    format!(
                        "k{}{suffix} () mutual k={} ind1=\"l{}\" ind2=\"l{}\"",
                        q(own),
                        num(k),
                        q(a),
                        q(b)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Card::Source {
                letter,
                name: _,
                np,
                nm,
                value,
                ac_mag,
                ac_phase,
                wave,
                port,
                mut parasitics,
            } => {
                if port.is_some() {
                    self.ask(Construct::RfPort, tok);
                    return None;
                }
                let extra = self.parasitics(letter, n, &np, &nm, &mut parasitics);
                let np = extra.as_ref().map_or(np, |(at, _)| at.clone());
                let (l, m) = match letter {
                    'V' => ('v', "vsource"),
                    _ => ('i', "isource"),
                };
                let wave = match &wave {
                    Some(w) => Some(self.vacask_wave(w, tok)?),
                    None => None,
                };
                let mut s = format!(
                    "{l}{} ({} {}) {m} dc={}",
                    q(own),
                    q(&np),
                    q(&nm),
                    self.vval(&value)
                );
                if let Some(v) = ac_mag {
                    s.push_str(&format!(" mag={}", num(v)));
                }
                if let Some(v) = ac_phase {
                    s.push_str(&format!(" phase={}", num(v)));
                }
                if let Some(w) = wave {
                    s.push(' ');
                    s.push_str(&w);
                }
                s.push_str(&self.part_lines(extra));
                s
            }
            Card::Linear {
                letter,
                name: _,
                nodes,
                gain,
            } => {
                // E, G: two output and two controlling nodes; F, H: two
                // output nodes and the sensing source.
                let held = if matches!(letter, 'E' | 'G') { 4 } else { 3 };
                assert_eq!(nodes.len(), held, "a linear {letter} card's nodes");
                let ns: Vec<String> = nodes.iter().map(|x| q(x)).collect();
                let name = q(own);
                let g = num(gain);
                match letter {
                    'E' => format!("e{name} ({}) vcvs gain={g}", ns.join(" ")),
                    'G' => format!("g{name} ({}) vccs gain={g}", ns.join(" ")),
                    'F' => format!(
                        "f{name} ({} {}) cccs ctlinst=\"{}\" gain={g}",
                        ns[0], ns[1], ns[2]
                    ),
                    _ => format!(
                        "h{name} ({} {}) ccvs ctlinst=\"{}\" gain={g}",
                        ns[0], ns[1], ns[2]
                    ),
                }
            }
            Card::Semi {
                letter,
                name: _,
                nodes,
                model,
                params,
            } => {
                if letter == 'M' && nodes.len() != 4 || letter == 'O' {
                    self.refuse(Code::W0105, tok);
                    return None;
                }
                let ns: Vec<String> = nodes.iter().map(|x| q(x)).collect();
                // A loaded module's instance keeps its name as read.
                let letter = match letter {
                    'N' => String::new(),
                    l => l.to_ascii_lowercase().to_string(),
                };
                format!(
                    "{}{} ({}) {}{}",
                    letter,
                    q(own),
                    ns.join(" "),
                    q(&model),
                    self.inst_params(&params)
                )
            }
            Card::Inst {
                name: _,
                ports,
                master,
                params,
            } => {
                let ps: Vec<String> = ports.iter().map(|p| q(p)).collect();
                format!(
                    "x{} ({}) {}{}",
                    q(own),
                    ps.join(" "),
                    q(&master),
                    self.inst_params(&params)
                )
            }
            Card::Charge { .. } => {
                self.ask(Construct::ChargeCapacitor, tok);
                return None;
            }
            // VACASK's source line, `b1 (p n) v=<expr>`: the tree through
            // `bsource.rs` (`docs/dev-builtin-behavioral.md`).
            Card::Behavioural {
                current,
                np,
                nm,
                expr,
                value,
                ..
            } => self.bsource_card(tok, own, current, [&np, &nm], &expr, value)?,
            Card::Table { .. } => {
                self.ask(Construct::TableSource, tok);
                return None;
            }
            // `tline_ideal` (`devices/tline_ideal.va`): port 1 `(p1 n1)`,
            // port 2 `(p2 n2)`, as SPICE's `T`. `td` is always written:
            // without it the module's delay is `nl/f`, 1 ms.
            Card::Line {
                name: _,
                nodes,
                z0,
                td,
            } => {
                let ns: Vec<String> = nodes.iter().map(|x| q(x)).collect();
                format!(
                    "t{} ({}) tline_ideal z0={} td={}",
                    q(own),
                    ns.join(" "),
                    num(z0),
                    num(td)
                )
            }
            // No polynomial builtin, no switch.
            Card::Poly { .. } | Card::Switch { .. } => {
                self.refuse(Code::W0105, tok);
                return None;
            }
        })
    }

    /// The top's waveform sources the DC analyses must see as `type="dc"`,
    /// each instance name and its waveform's `type`: VACASK evaluates the
    /// waveform in every analysis (`lib/devvisrc.cpp` `sourceCompute`),
    /// ngspice gives op and `.dc` the card's DC value (run by hand: "dc
    /// value used for op instead of transient time=0 value"). Only those
    /// whose DC value is not the waveform's at time zero, or that a `.dc`
    /// in `swept` sweeps.
    pub(super) fn dc_alters(&self, swept: &[String]) -> Vec<(String, &'static str)> {
        let mut out = Vec::new();
        for &c in &self.deck.top.comps {
            let Comp::Card(n) = c else { continue };
            let Card::Source {
                letter,
                value,
                wave: Some(w),
                ..
            } = self.rd.card_view(n)
            else {
                continue;
            };
            let kind = match w.kind {
                WaveKind::Sin => "sine",
                WaveKind::Pulse => "pulse",
                WaveKind::Exp => "exp",
                WaveKind::Sffm => "fm",
                WaveKind::Am => "am",
                WaveKind::Pwl => "pwl",
                // Not a VACASK waveform (NoiseWave).
                WaveKind::Trnoise | WaveKind::Trrandom => continue,
            };
            let name = format!(
                "{}{}",
                letter.to_ascii_lowercase(),
                q(&self.names.decl(n)[1..])
            );
            if value != w.at_zero() || swept.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                out.push((name, kind));
            }
        }
        out
    }

    /// `w` as `type="…"` parameters; `None` when it is refused.
    fn vacask_wave(&mut self, w: &Wave, tok: u32) -> Option<String> {
        super::sources::assert_held(w);
        let a: Vec<String> = w.args.iter().map(|x| self.vacask_num(x)).collect();
        Some(match w.kind {
            WaveKind::Sin => {
                // VACASK accepts a phase and ignores it.
                if w.args[5] != Num::Num(0.0) {
                    self.refuse(Code::W0108, tok);
                    return None;
                }
                format!(
                    "type=\"sine\" sinedc={} ampl={} freq={} delay={} theta={}",
                    a[0], a[1], a[2], a[3], a[4]
                )
            }
            WaveKind::Pulse => {
                // A period no longer than the pulse is no repeat (period 0).
                let nums: Option<Vec<f64>> =
                    [6, 3, 4, 5].iter().map(|&i| w.args[i].num()).collect();
                let period = match nums.as_deref() {
                    Some(&[p, r, f, wd]) if p > 0.0 && p <= r + f + wd => num(0.0),
                    _ => a[6].clone(),
                };
                format!(
                    "type=\"pulse\" val0={} val1={} delay={} rise={} fall={} width={} period={period}",
                    a[0], a[1], a[2], a[3], a[4], a[5]
                )
            }
            WaveKind::Exp => {
                // VACASK counts td2 from delay.
                let (Some(rd), Some(fd)) = (w.args[2].num(), w.args[4].num()) else {
                    self.refuse(Code::W0108, tok);
                    return None;
                };
                if fd < rd {
                    self.refuse(Code::W0108, tok);
                    return None;
                }
                format!(
                    "type=\"exp\" val0={} val1={} delay={} tau1={} td2={} tau2={}",
                    a[0],
                    a[1],
                    a[2],
                    a[3],
                    num(fd - rd),
                    a[5]
                )
            }
            WaveKind::Sffm => format!(
                "type=\"fm\" sinedc={} ampl={} freq={} modindex={} modfreq={}",
                a[0], a[1], a[2], a[3], a[4]
            ),
            // ngspice-45's order (`Wave::args`) as VACASK's `type="am"`
            // (`lib/devvisrc.cpp`): modindex VMA/VMO; VACASK holds the
            // value at `delay` before it, ngspice 0, so a delayed AM is
            // written only with no offset; no carrier phase (the pinned
            // VACASK runs `phase=60` as 0: run, 1.3 V off).
            WaveKind::Am => {
                let x: Vec<Option<f64>> = w.args.iter().map(Num::num).collect();
                let [Some(vo), Some(vmo), Some(vma), _, _, Some(td), _, Some(pc)] = x[..] else {
                    self.refuse(Code::W0108, tok);
                    return None;
                };
                if vmo == 0.0 || pc != 0.0 || td != 0.0 && vo != 0.0 {
                    self.refuse(Code::W0108, tok);
                    return None;
                }
                format!(
                    "type=\"am\" sinedc={} ampl={} modindex={} modfreq={} freq={} delay={} modphase={}",
                    a[0],
                    a[1],
                    num(vma / vmo),
                    a[3],
                    a[4],
                    a[5],
                    a[6]
                )
            }
            // `type="pwl"` holds the first value before the first point
            // and the last after, as ngspice does; `delay` is `td=`
            // (`docs/dev-builtin-src.md`, `lib/devvisrc.cpp` L910 at
            // 36be560). Its `period` repeats from time 0 and wants the
            // last value equal to the first: only that `r=0` has a form.
            WaveKind::Pwl => {
                let n = w.args.len();
                let whole = n >= 4
                    && w.args[0] == Num::Num(0.0)
                    && w.args[1] == w.args[n - 1]
                    && w.repeat == Some(Num::Num(0.0));
                if w.repeat.is_some() && !whole {
                    self.refuse(Code::W0108, tok);
                    return None;
                }
                let mut s = format!("type=\"pwl\" wave=[{}]", a.join(", "));
                if whole {
                    s.push_str(&format!(" period={}", a[n - 2]));
                }
                if let Some(d) = &w.delay {
                    s.push_str(&format!(" delay={}", self.vacask_num(d)));
                }
                s
            }
            WaveKind::Trnoise | WaveKind::Trrandom => {
                self.ask(Construct::NoiseWave, tok);
                return None;
            }
        })
    }
}
