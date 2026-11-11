//! An analysis in a dialect region of the deck, for the analyses the
//! ngspice card does not say: a Fourier analysis's harmonic count in the
//! target's words (LTspice's `.four`, Spectre's `fourier` components) and
//! the Spectre statement of an analysis ngspice has no card for, or of a
//! `pss` on Spectre.
//!
//! Cites: LTspice help (`.FOUR`); Spectre Components `[CMP]` p.323-325
//! (`fourier`); Spectre Reference 19.1 `[REF]` p.90-91 (`hb`), p.272-274
//! (`pss`, also the PSS base of the periodic analyses), p.174
//! (`montecarlo`), p.245-246 (`pac`), p.253-256 (`pnoise`), p.298-299
//! (`pstb`), p.303-304 (`pxf`), p.393-394 (`stb`), p.406-410 (`sweep`).
//!
//! Contents:
//! - `Analysis::fourier_count(&self, d: Dialect) -> Result<Option<Vec<String>>, String>`,
//!   `Analysis::spectre(&self) -> Result<String, String>`

use super::analysis::{Analysis, PssBase, snum};
use super::model::num;
use crate::syntax::Dialect;

impl Analysis {
    /// A Fourier analysis's harmonic count in the deck for `d`, as lines
    /// for that dialect's region in place of the `.four` card: LTspice's
    /// `.four <freq> <Nharmonics> <traces>` (LTspice help, `.FOUR`),
    /// Spectre's `fourier` component's `harms` (`[CMP]` p.323-325, one per
    /// output). `None` for no count, and on ngspice, whose count is the
    /// deck's ([`nfreqs`](super::analysis::nfreqs)). `Err` for a target whose `.four` has none
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

// ---- private ----

/// A SPICE signal (`V(out)`) as the bare node Spectre names.
fn bare(s: &str) -> &str {
    let t = s.trim();
    let inner = ["V(", "v(", "I(", "i("]
        .into_iter()
        .find_map(|p| t.strip_prefix(p))
        .and_then(|r| r.strip_suffix(')'));
    inner.map_or(t, str::trim)
}
