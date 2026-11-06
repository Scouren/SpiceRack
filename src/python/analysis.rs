//! The analyses Python queues: each as its ngspice card (the deck's own
//! text, which the parser reads), or, for the Spectre-only ones and the
//! RF analyses ngspice has no card for, as its Spectre statement, in the
//! deck's `simulator lang=spectre` region; a `pss` is the Spectre statement
//! on a Spectre target and the `.pss` card on any other (the reader keeps
//! a `.pss` as ngspice text, which Spectre cannot run); and each's
//! [`Kind`], which picks the backend, matches its plot and types its
//! result.
//!
//! Cites: ngspice manual ch.11 (analysis cards; §11.3.6 `.pz`, §11.3.12
//! `.pss`), §15.4.2 (`.four`, the `fourier` command, the `nfreqs`
//! variable); ngspice-45 `src/frontend/fourier.c`, `src/frontend/inp.c`;
//! LTspice help (`.FOUR`); Spectre Components `[CMP]` p.323-325 (`fourier`);
//! Spectre Reference 19.1 `[REF]` p.90-91 (`hb`), p.272-274
//! (`pss`, also the PSS base of the periodic analyses), p.174
//! (`montecarlo`), p.245-246 (`pac`), p.253-256 (`pnoise`), p.298-299
//! (`pstb`), p.303-304 (`pxf`), p.393-394 (`stb`), p.406-410 (`sweep`).
//!
//! Contents:
//! - `pub(super) enum Analysis`, `Analysis::kind(&self) -> Kind`,
//!   `Analysis::card(&self) -> Option<String>`, `Analysis::log_sweep(&self) -> bool`, `Analysis::in_spectre_region(&self, d: Dialect) -> bool`,
//!   `Analysis::fourier_count(&self, d: Dialect) -> Result<Option<Vec<String>>, String>`,
//!   `Analysis::spectre(&self) -> Result<String, String>`
//! - `pub(super) fn nfreqs(analyses: &[Analysis]) -> Result<Option<String>, String>`
//! - `pub(super) fn snum(v: f64) -> String`
//! - `pub(super) struct DcSweep`, `pub(super) struct FreqSweep`, `pub(super) struct PssBase`

use std::fmt::Write as _;

use super::keywords::{PzKind, PzTransfer, SpectreAnalysis, Sweep};
use super::model::{dec, num};
use crate::backend::Kind;
use crate::syntax::Dialect;

/// One analysis, as the Python call gave it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Analysis {
    Op,
    Dc {
        sweeps: Vec<DcSweep>,
    },
    Ac {
        sweep: FreqSweep,
    },
    Transient {
        step: f64,
        stop: f64,
        start: Option<f64>,
        max_step: Option<f64>,
        uic: bool,
    },
    Noise {
        output: String,
        reference: String,
        source: String,
        sweep: FreqSweep,
        points_per_summary: Option<u32>,
    },
    Tf {
        output: String,
        source: String,
    },
    Sensitivity {
        output: String,
        ac: Option<FreqSweep>,
    },
    PoleZero {
        nodes: [String; 4],
        tf_type: PzTransfer,
        pz_type: PzKind,
    },
    Distortion {
        sweep: FreqSweep,
        f2overf1: Option<f64>,
    },
    /// `sc_iter`, `steady_coeff`, `uic`: ngspice's `.pss … sciter
    /// steadycoeff <uic>` (manual §11.3.12).
    Pss {
        fundamental: f64,
        stabilization: f64,
        observe_node: String,
        points_per_period: u32,
        harmonics: u32,
        sc_iter: Option<u32>,
        steady_coeff: Option<f64>,
        uic: bool,
    },
    HarmonicBalance {
        frequencies: Vec<f64>,
        harmonics: Vec<u32>,
    },
    SPar {
        sweep: FreqSweep,
    },
    Stability {
        probe: String,
        sweep: FreqSweep,
    },
    /// ngspice's transient noise: a `.tran` whose noise is the deck's
    /// `TRNOISE`/`TRRANDOM` sources (manual §4.1.7); no device noise.
    TransientNoise {
        step: f64,
        stop: f64,
    },
    /// `harmonics`: how many past DC (`None`: the simulator's nine).
    Fourier {
        fundamental: f64,
        outputs: Vec<String>,
        harmonics: Option<u32>,
    },
    SpectreSweep {
        param: String,
        start: f64,
        stop: f64,
        step: f64,
        inner: String,
        inner_type: SpectreAnalysis,
    },
    SpectreMonteCarlo {
        iterations: u32,
        inner: String,
        inner_type: SpectreAnalysis,
        seed: Option<u64>,
    },
    SpectrePac {
        pss: PssBase,
        sweep: FreqSweep,
        sweep_type: String,
    },
    SpectrePnoise {
        pss: PssBase,
        output: String,
        reference: String,
        sweep: FreqSweep,
    },
    SpectrePxf {
        pss: PssBase,
        output: String,
        sweep: FreqSweep,
    },
    SpectrePstb {
        pss: PssBase,
        probe: String,
        sweep: FreqSweep,
    },
}

/// One `.dc` sweep: a source (or `temp`) from `start` to `stop`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DcSweep {
    pub(super) source: String,
    pub(super) start: f64,
    pub(super) stop: f64,
    pub(super) step: f64,
}

/// A frequency sweep: `points` per decade or octave, or in total (`lin`).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FreqSweep {
    pub(super) variation: Sweep,
    pub(super) points: u32,
    pub(super) start: f64,
    pub(super) stop: f64,
}

/// The periodic steady state a SpectreRF small-signal analysis linearizes
/// around.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PssBase {
    pub(super) fundamental: f64,
    pub(super) stabilization: f64,
    pub(super) harmonics: u32,
}

impl Analysis {
    /// What backend selection, plot matching and the result class read.
    pub(super) fn kind(&self) -> Kind {
        match self {
            Analysis::Op => Kind::Op,
            Analysis::Dc { .. } => Kind::Dc,
            Analysis::Ac { .. } => Kind::Ac,
            Analysis::Transient { .. } => Kind::Tran,
            Analysis::Noise { .. } => Kind::Noise,
            Analysis::Tf { .. } => Kind::Tf,
            Analysis::Sensitivity { ac: None, .. } => Kind::Sens,
            Analysis::Sensitivity { ac: Some(_), .. } => Kind::SensAc,
            Analysis::PoleZero { .. } => Kind::Pz,
            Analysis::Distortion { .. } => Kind::Disto,
            Analysis::Pss { .. } => Kind::Pss,
            Analysis::HarmonicBalance { .. } => Kind::Hb,
            Analysis::SPar { .. } => Kind::Sp,
            Analysis::Stability { .. } => Kind::Stb,
            Analysis::TransientNoise { .. } => Kind::TranNoise,
            Analysis::Fourier { .. } => Kind::Fourier,
            Analysis::SpectreSweep { .. } => Kind::SpectreSweep,
            Analysis::SpectreMonteCarlo { .. } => Kind::SpectreMonteCarlo,
            Analysis::SpectrePac { .. } => Kind::Pac,
            Analysis::SpectrePnoise { .. } => Kind::Pnoise,
            Analysis::SpectrePxf { .. } => Kind::Pxf,
            Analysis::SpectrePstb { .. } => Kind::Pstb,
        }
    }

    /// The ngspice card; `None` for an analysis ngspice has no card for
    /// ([`Self::spectre`] spells those).
    pub(super) fn card(&self) -> Option<String> {
        let sweep = |f: &FreqSweep| {
            let v = f.variation.name();
            format!("{v} {} {} {}", f.points, num(f.start), num(f.stop))
        };
        Some(match self {
            Analysis::Op => ".op".into(),
            Analysis::Dc { sweeps } => {
                let mut s = String::from(".dc");
                for sw in sweeps {
                    let (a, b, c) = (num(sw.start), num(sw.stop), num(sw.step));
                    w(&mut s, format_args!(" {} {a} {b} {c}", sw.source));
                }
                s
            }
            Analysis::Ac { sweep: f } => format!(".ac {}", sweep(f)),
            Analysis::Transient {
                step,
                stop,
                start,
                max_step,
                uic,
            } => {
                let mut s = format!(".tran {} {}", num(*step), num(*stop));
                // A step cap needs a start before it.
                match (start, max_step) {
                    (Some(st), _) => w(&mut s, format_args!(" {}", num(*st))),
                    (None, Some(_)) => s.push_str(" 0"),
                    (None, None) => {}
                }
                if let Some(ms) = max_step {
                    w(&mut s, format_args!(" {}", num(*ms)));
                }
                if *uic {
                    s.push_str(" UIC");
                }
                s
            }
            Analysis::Noise {
                output,
                reference,
                source,
                sweep: f,
                points_per_summary,
            } => {
                let out = match reference.as_str() {
                    "" | "0" => format!("V({output})"),
                    r => format!("V({output},{r})"),
                };
                let mut s = format!(".noise {out} {source} {}", sweep(f));
                if let Some(pps) = points_per_summary {
                    w(&mut s, format_args!(" {pps}"));
                }
                s
            }
            Analysis::Tf { output, source } => format!(".tf {output} {source}"),
            Analysis::Sensitivity { output, ac } => match ac {
                Some(f) => format!(".sens {output} AC {}", sweep(f)),
                None => format!(".sens {output}"),
            },
            Analysis::PoleZero {
                nodes,
                tf_type,
                pz_type,
            } => format!(
                ".pz {} {} {}",
                nodes.join(" "),
                tf_type.name(),
                pz_type.name()
            ),
            Analysis::Distortion { sweep: f, f2overf1 } => {
                let mut s = format!(".disto {}", sweep(f));
                if let Some(r) = f2overf1 {
                    w(&mut s, format_args!(" {}", dec(*r)));
                }
                s
            }
            Analysis::Pss {
                fundamental,
                stabilization,
                observe_node,
                points_per_period,
                harmonics,
                sc_iter,
                steady_coeff,
                uic,
            } => {
                let (f, t) = (dec(*fundamental), dec(*stabilization));
                let mut s = format!(".pss {f} {t} {observe_node} {points_per_period} {harmonics}");
                // SCITER and STEADYCOEFF are positional (manual §11.3.12):
                // one given, or `uic`, writes both, the other at ngspice's
                // default.
                if sc_iter.is_some() || steady_coeff.is_some() || *uic {
                    let c = dec(steady_coeff.unwrap_or(1e-3));
                    w(&mut s, format_args!(" {} {c}", sc_iter.unwrap_or(50)));
                }
                if *uic {
                    s.push_str(" uic");
                }
                s
            }
            Analysis::TransientNoise { step, stop } => {
                format!(".tran {} {}", num(*step), num(*stop))
            }
            Analysis::SPar { sweep: f } => {
                let v = f.variation.name();
                format!(".sp {v} {} {} {}", f.points, dec(f.start), dec(f.stop))
            }
            // A harmonic count is no field of the card (manual §15.4.2:
            // the `nfreqs` variable; [`Self::fourier_count`]).
            Analysis::Fourier {
                fundamental,
                outputs,
                ..
            } => format!(".four {} {}", num(*fundamental), outputs.join(" ")),
            Analysis::HarmonicBalance { .. }
            | Analysis::Stability { .. }
            | Analysis::SpectreSweep { .. }
            | Analysis::SpectreMonteCarlo { .. }
            | Analysis::SpectrePac { .. }
            | Analysis::SpectrePnoise { .. }
            | Analysis::SpectrePxf { .. }
            | Analysis::SpectrePstb { .. } => return None,
        })
    }

    /// Whether a frequency sweep of the analysis is log-spaced over N points
    /// in total (Spectre's `log=`): no SPICE target, nor VACASK, has that spacing
    /// (ngspice manual §11.3.1, `.ac dec|oct|lin`).
    pub(super) fn log_sweep(&self) -> bool {
        let f = match self {
            Analysis::Ac { sweep }
            | Analysis::Noise { sweep, .. }
            | Analysis::Distortion { sweep, .. }
            | Analysis::SPar { sweep }
            | Analysis::Stability { sweep, .. }
            | Analysis::Sensitivity {
                ac: Some(sweep), ..
            }
            | Analysis::SpectrePac { sweep, .. }
            | Analysis::SpectrePnoise { sweep, .. }
            | Analysis::SpectrePxf { sweep, .. }
            | Analysis::SpectrePstb { sweep, .. } => sweep,
            _ => return false,
        };
        f.variation == Sweep::Log
    }

    /// Whether the deck for `d` holds the analysis as its Spectre
    /// statement ([`Self::spectre`]) in its `simulator lang=spectre` region
    /// rather than as its ngspice [`Self::card`]: one with no card, and
    /// `pss` on Spectre, which runs no ngspice `.pss`.
    pub(super) fn in_spectre_region(&self, d: Dialect) -> bool {
        self.card().is_none() || (d == Dialect::Spectre && matches!(self, Analysis::Pss { .. }))
    }

    /// A Fourier analysis's harmonic count in the deck for `d`, as lines
    /// for that dialect's region in place of the `.four` card: LTspice's
    /// `.four <freq> <Nharmonics> <traces>` (LTspice help, `.FOUR`),
    /// Spectre's `fourier` component's `harms` (`[CMP]` p.323-325, one per
    /// output). `None` for no count, and on ngspice, whose count is the
    /// deck's ([`nfreqs`]). `Err` for a target whose `.four` has none
    /// (HSPICE; Xyce, which counts only `.measure … four … numfreq=`).
    /// VACASK, which has no Fourier analysis, CDL and Verilog, which write
    /// none, take the card as it is.
    ///
    /// # Panics
    /// When `self` is no Fourier analysis.
    pub(super) fn fourier_count(&self, d: Dialect) -> Result<Option<Vec<String>>, String> {
        let Analysis::Fourier {
            fundamental,
            outputs,
            harmonics,
        } = self
        else {
            panic!("{self:?} is no Fourier analysis");
        };
        let Some(n) = *harmonics else {
            return Ok(None);
        };
        Ok(match d.family() {
            Dialect::Ngspice => None,
            Dialect::Ltspice => {
                let f = num(*fundamental);
                Some(vec![format!(".four {f} {n} {}", outputs.join(" "))])
            }
            // Named apart from the `four<k>` an uncounted `.four` is written
            // as on Spectre.
            Dialect::Spectre => Some(
                outputs
                    .iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let f = snum(*fundamental);
                        format!("fourh{} ({} 0) fourier fund={f} harms={n}", i + 1, bare(o))
                    })
                    .collect(),
            ),
            // VACASK has no Fourier analysis: the renderer refuses the
            // `.four` card (W0015), counted or not.
            Dialect::Vacask => None,
            Dialect::Hspice | Dialect::Xyce => {
                return Err(format!(
                    "fourier num_harmonics={n} ({} writes no harmonic count)",
                    d.name()
                ));
            }
            Dialect::Espice => unreachable!("ESPice's family is ngspice"),
            // Structure only: the writer keeps no analysis, counted or not.
            Dialect::Cdl | Dialect::Verilog => None,
        })
    }

    /// The Spectre statement of an analysis [`Self::in_spectre_region`]
    /// holds in the region; `Err` naming what Spectre cannot run.
    ///
    /// # Panics
    /// When `self` has an ngspice card and is no `pss`.
    pub(super) fn spectre(&self) -> Result<String, String> {
        let pss = |p: &PssBase| {
            let (f, t) = (snum(p.fundamental), snum(p.stabilization));
            format!("pss1 pss fund={f} tstab={t} harms={}\n", p.harmonics)
        };
        let range = super::spectre::range;
        Ok(match self {
            // `[REF]` p.272-274: `fund` (#2), `harms` (#4), `tstab` (#6), and
            // for an autonomous circuit the observed node as the terminals;
            // there is no `ppv` and no `probe`. ngspice's shooting controls
            // have no Spectre parameter.
            Analysis::Pss {
                sc_iter,
                steady_coeff,
                uic,
                ..
            } if sc_iter.is_some() || steady_coeff.is_some() || *uic => {
                return Err("pss sciter/steadycoeff/uic (ngspice shooting controls)".into());
            }
            Analysis::Pss {
                fundamental,
                stabilization,
                observe_node,
                harmonics,
                ..
            } => {
                let terms = match observe_node.as_str() {
                    "" => String::new(),
                    n => format!("({} 0) ", bare(n)),
                };
                let (f, t) = (snum(*fundamental), snum(*stabilization));
                format!("pss1 {terms}pss fund={f} tstab={t} harms={harmonics}")
            }
            // `[REF]` p.90-91: `fundfreqs=[...]`, `maxharms=[...]`.
            Analysis::HarmonicBalance {
                frequencies,
                harmonics,
            } => {
                let f: Vec<String> = frequencies.iter().map(|f| snum(*f)).collect();
                let h: Vec<String> = harmonics.iter().map(u32::to_string).collect();
                format!(
                    "hb1 hb fundfreqs=[{}] maxharms=[{}]",
                    f.join(" "),
                    h.join(" ")
                )
            }
            // `[REF]` p.393-394: `probe` is the instance the loop gain is
            // taken around.
            Analysis::Stability { probe, sweep } => {
                format!("stb1 stb {} probe={}", range(sweep)?, probe.to_lowercase())
            }
            // `[REF]` p.406-410.
            Analysis::SpectreSweep {
                param,
                start,
                stop,
                step,
                inner,
                inner_type,
            } => format!(
                "sweep1 sweep param={param} start={} stop={} step={} {{\n  {inner} {}\n}}",
                snum(*start),
                snum(*stop),
                snum(*step),
                inner_type.name()
            ),
            // `[REF]` p.174: every parameter precedes the brace.
            Analysis::SpectreMonteCarlo {
                iterations,
                inner,
                inner_type,
                seed,
            } => {
                let seed = seed.map(|s| format!(" seed={s}")).unwrap_or_default();
                format!(
                    "mc1 montecarlo numruns={iterations}{seed} {{\n  {inner} {}\n}}",
                    inner_type.name()
                )
            }
            // `[REF]` p.245-246; PSS is its prerequisite (p.272).
            Analysis::SpectrePac {
                pss: p,
                sweep,
                sweep_type,
            } => format!(
                "{}pac1 pac {} sweeptype={sweep_type}",
                pss(p),
                range(sweep)?
            ),
            // `[REF]` p.253-256: the reference node is a terminal.
            Analysis::SpectrePnoise {
                pss: p,
                output,
                reference,
                sweep,
            } => {
                let neg = if reference.is_empty() {
                    "0"
                } else {
                    bare(reference)
                };
                format!(
                    "{}pnoise1 ({} {neg}) pnoise {}",
                    pss(p),
                    bare(output),
                    range(sweep)?
                )
            }
            // `[REF]` p.303-304: `probe` is the output; no input source.
            Analysis::SpectrePxf {
                pss: p,
                output,
                sweep,
            } => format!("{}pxf1 ({} 0) pxf {}", pss(p), bare(output), range(sweep)?),
            // `[REF]` p.298-299.
            Analysis::SpectrePstb {
                pss: p,
                probe,
                sweep,
            } => format!(
                "{}pstb1 pstb {} probe={}",
                pss(p),
                range(sweep)?,
                probe.to_lowercase()
            ),
            a => unreachable!("{a:?} has an ngspice card and is no pss"),
        })
    }
}

/// The `.options nfreqs=` card of an ngspice deck whose Fourier analyses
/// count their harmonics: the count of frequencies, DC the first
/// (ngspice-45 `src/frontend/fourier.c`: `nfreqs`, default 10, read from
/// the deck's options, which `src/frontend/inp.c` sets as the circuit's
/// variables). `None` when no analysis counts; `Err` when two count apart,
/// as the count is the deck's (one uncounted counts nine).
pub(super) fn nfreqs(analyses: &[Analysis]) -> Result<Option<String>, String> {
    let mut counted = false;
    let mut counts = std::collections::BTreeSet::new();
    for a in analyses {
        if let Analysis::Fourier { harmonics, .. } = a {
            counted |= harmonics.is_some();
            counts.insert(harmonics.unwrap_or(9));
        }
    }
    match (counted, counts.first()) {
        (true, Some(n)) if counts.len() == 1 => Ok(Some(format!(".options nfreqs={}", n + 1))),
        (true, _) => Err(format!(
            "fourier num_harmonics {counts:?} on ngspice (its nfreqs is the deck's)"
        )),
        (false, _) => Ok(None),
    }
}

/// A number in Spectre's scale factors (`M` mega, `G`, `T`), which differ
/// from SPICE's `meg`, `g`, `t`.
pub(super) fn snum(v: f64) -> String {
    let s = crate::unit::format_spice_number(v);
    for (spice, spectre) in [("meg", "M"), ("g", "G"), ("t", "T")] {
        if let Some(head) = s.strip_suffix(spice) {
            return format!("{head}{spectre}");
        }
    }
    s
}

// ---- private ----

fn w(s: &mut String, args: std::fmt::Arguments<'_>) {
    s.write_fmt(args).expect("a String takes any write");
}

/// A SPICE signal (`V(out)`) as the bare node Spectre names.
fn bare(s: &str) -> &str {
    let t = s.trim();
    let inner = ["V(", "v(", "I(", "i("]
        .into_iter()
        .find_map(|p| t.strip_prefix(p))
        .and_then(|r| r.strip_suffix(')'));
    inner.map_or(t, str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pss_base() -> PssBase {
        PssBase {
            fundamental: 1e9,
            stabilization: 1e-7,
            harmonics: 10,
        }
    }

    fn dec(points: u32, start: f64, stop: f64) -> FreqSweep {
        FreqSweep {
            variation: Sweep::Dec,
            points,
            start,
            stop,
        }
    }

    /// `[REF]` p.272-274 pss, p.90-91 hb (`fundfreqs`, `maxharms`, not
    /// `toneN`), p.393 stb (`probe`, the instance lowercased as every
    /// instance is written).
    #[test]
    fn spectre_rf_large_signal_analyses_match_documented_parameters() {
        let pss = |sc_iter| Analysis::Pss {
            fundamental: 1e9,
            stabilization: 1e-7,
            observe_node: "out".into(),
            points_per_period: 128,
            harmonics: 10,
            sc_iter,
            steady_coeff: None,
            uic: false,
        };
        assert_eq!(
            pss(None).spectre().unwrap(),
            "pss1 (out 0) pss fund=1G tstab=100n harms=10"
        );
        assert!(pss(Some(50)).spectre().is_err(), "a shooting control");
        assert!(pss(None).in_spectre_region(Dialect::Spectre));
        assert!(!pss(None).in_spectre_region(Dialect::Ngspice));
        let hb = Analysis::HarmonicBalance {
            frequencies: vec![1e9, 1.01e9],
            harmonics: vec![5, 3],
        };
        assert_eq!(
            hb.spectre().unwrap(),
            "hb1 hb fundfreqs=[1G 1.01G] maxharms=[5 3]"
        );
        let stb = Analysis::Stability {
            probe: "IPRB0".into(),
            sweep: dec(20, 1.0, 1e9),
        };
        assert_eq!(
            stb.spectre().unwrap(),
            "stb1 stb start=1 stop=1G dec=20 probe=iprb0"
        );
    }

    /// `[REF]` p.245-246 (pac, `sweeptype`), p.253-256 (pnoise: the
    /// reference node is a terminal, there is no `refprobe`), p.303-304
    /// (pxf: the only probe is the output, there is no `isrc`), p.298-299
    /// (pstb, `probe`); each after the pss it linearizes around (p.272).
    #[test]
    fn spectre_periodic_small_signal_analyses_drop_invented_parameters() {
        let pss = "pss1 pss fund=1G tstab=100n harms=10\n";
        let pac = Analysis::SpectrePac {
            pss: pss_base(),
            sweep: dec(100, 1.0, 1e9),
            sweep_type: "relative".into(),
        };
        let pac_text = "pac1 pac start=1 stop=1G dec=100 sweeptype=relative";
        assert_eq!(pac.spectre().unwrap(), format!("{pss}{pac_text}"));
        let pnoise = Analysis::SpectrePnoise {
            pss: pss_base(),
            output: "out".into(),
            reference: "0".into(),
            sweep: dec(10, 1.0, 1e6),
        };
        let pnoise_text = "pnoise1 (out 0) pnoise start=1 stop=1M dec=10";
        assert_eq!(pnoise.spectre().unwrap(), format!("{pss}{pnoise_text}"));
        let pxf = Analysis::SpectrePxf {
            pss: pss_base(),
            output: "out".into(),
            sweep: dec(10, 1.0, 1e6),
        };
        let pxf_text = "pxf1 (out 0) pxf start=1 stop=1M dec=10";
        assert_eq!(pxf.spectre().unwrap(), format!("{pss}{pxf_text}"));
        let pstb = Analysis::SpectrePstb {
            pss: pss_base(),
            probe: "IPRB0".into(),
            sweep: dec(10, 1.0, 1e6),
        };
        let pstb_text = "pstb1 pstb start=1 stop=1M dec=10 probe=iprb0";
        assert_eq!(pstb.spectre().unwrap(), format!("{pss}{pstb_text}"));
    }

    /// `[REF]` p.410 sweep (`swp sweep param=temp … { … }`), p.174
    /// montecarlo (every parameter before the brace); ngspice manual
    /// §4.1.7: transient noise is a `.tran` over `TRNOISE` sources.
    #[test]
    fn spectre_sweep_and_montecarlo_blocks_match_documented_forms() {
        let sweep = Analysis::SpectreSweep {
            param: "rload".into(),
            start: 1e3,
            stop: 10e3,
            step: 1e3,
            inner: "ac1".into(),
            inner_type: SpectreAnalysis::Ac,
        };
        let want = "sweep1 sweep param=rload start=1k stop=10k step=1k {\n  ac1 ac\n}";
        assert_eq!(sweep.spectre().unwrap(), want);
        let mc = |seed| Analysis::SpectreMonteCarlo {
            iterations: 200,
            inner: "tran1".into(),
            inner_type: SpectreAnalysis::Tran,
            seed,
        };
        let want = "mc1 montecarlo numruns=200 seed=1234 {\n  tran1 tran\n}";
        assert_eq!(mc(Some(1234)).spectre().unwrap(), want);
        let want = "mc1 montecarlo numruns=200 {\n  tran1 tran\n}";
        assert_eq!(mc(None).spectre().unwrap(), want);
        let noise = Analysis::TransientNoise {
            step: 1e-9,
            stop: 1e-6,
        };
        assert_eq!(noise.card().unwrap(), ".tran 1n 1u");
    }

    /// ngspice manual §11.3.12: `.pss gfreq tstab oname psspoints harms
    /// [sciter steadycoeff] [uic]`, the two positional controls written
    /// together, the missing one at ngspice's default (50, 1e-3).
    #[test]
    fn pss_card_writes_both_shooting_controls_or_neither() {
        let pss = |sc_iter, steady_coeff, uic| Analysis::Pss {
            fundamental: 1e6,
            stabilization: 1e-5,
            observe_node: "out".into(),
            points_per_period: 128,
            harmonics: 10,
            sc_iter,
            steady_coeff,
            uic,
        };
        let card = |a: Analysis| a.card().unwrap();
        assert_eq!(
            card(pss(None, None, false)),
            ".pss 1000000 0.00001 out 128 10"
        );
        assert_eq!(
            card(pss(Some(20), None, false)),
            ".pss 1000000 0.00001 out 128 10 20 0.001"
        );
        assert_eq!(
            card(pss(None, Some(1e-4), false)),
            ".pss 1000000 0.00001 out 128 10 50 0.0001"
        );
        assert_eq!(
            card(pss(None, None, true)),
            ".pss 1000000 0.00001 out 128 10 50 0.001 uic"
        );
    }

    #[test]
    fn every_analysis_has_exactly_one_spelling() {
        let f = FreqSweep {
            variation: Sweep::Dec,
            points: 10,
            start: 1.0,
            stop: 1e6,
        };
        assert_eq!(
            Analysis::Ac { sweep: f.clone() }.card().unwrap(),
            ".ac dec 10 1 1meg"
        );
        let stb = Analysis::Stability {
            probe: "Iprb".into(),
            sweep: f,
        };
        assert!(stb.card().is_none());
        assert_eq!(
            stb.spectre().unwrap(),
            "stb1 stb start=1 stop=1M dec=10 probe=iprb"
        );
    }
}
