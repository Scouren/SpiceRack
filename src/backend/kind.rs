//! The analysis kinds a backend runs: one [`Kind`] each, and its
//! [`Spec`] row (`Kind::spec`): the dialects that run it, the workaround
//! without one, and the plot a run writes for it. A new kind is a variant
//! and its arm in `Kind::spec`; every other site reads the row.
//!
//! Cites: ngspice manual §4.1.7 (`TRNOISE`); the plot names are ngspice's
//! and ESPice's `Plotname` lines.
//!
//! Contents:
//! - `pub enum Kind`, `Kind::name(self) -> &'static str`, `Kind::spec(self) -> Spec`
//! - `pub(crate) struct Spec`, `pub(crate) enum Plot`

use super::RUNNABLE;
use crate::syntax::Dialect;

/// An analysis's kind: what picks the simulator that runs it
/// ([`select`]), and the plot a run writes for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Op,
    Dc,
    Ac,
    Tran,
    Noise,
    Tf,
    /// `.sens` at DC.
    Sens,
    /// `.sens … AC …`.
    SensAc,
    Pz,
    Disto,
    Pss,
    Hb,
    Sp,
    Stb,
    TranNoise,
    /// `.four`: a table on stdout, no plot.
    Fourier,
    SpectreSweep,
    SpectreMonteCarlo,
    Pac,
    Pnoise,
    Pxf,
    Pstb,
}

/// What a run of one [`Kind`] needs and writes (`Kind::spec`).
pub(crate) struct Spec {
    /// The short name errors and [`check`] use.
    pub(crate) name: &'static str,
    /// Which plot of a multi-analysis run is this analysis's.
    pub(crate) plot: Plot,
    /// The dialects that run it, in preference order.
    pub(super) route: &'static [Dialect],
    /// The workaround [`select`] suggests when none is installed.
    pub(super) alternative: &'static str,
}

/// How `simulation::run_each` finds an analysis's plot among a run's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Plot {
    /// A plot whose `Plotname` starts with this (ngspice's and ESPice's
    /// names); of several, the one the analysis's place among those of
    /// its kind picks.
    Named(&'static str),
    /// As `Named`, and the plot the run wrote right after it when that
    /// one's `Plotname` starts with the second name: both are the
    /// analysis's (ngspice's and ESPice's `Integrated Noise`, one point
    /// behind a noise run's spectra).
    NamedWith(&'static str, &'static str),
    /// A table on stdout, no plot (`.four`).
    Table,
    /// Not matched beside other analyses; the name the Python API gives it.
    Unmatched(&'static str),
}

impl Kind {
    /// The analysis's short name, as errors and `check` name it.
    pub fn name(self) -> &'static str {
        self.spec().name
    }

    /// Everything a run of this kind depends on, in one row: a new kind is
    /// one arm here.
    pub(crate) const fn spec(self) -> Spec {
        use Dialect::*;
        const fn row(
            name: &'static str,
            plot: Plot,
            route: &'static [Dialect],
            alternative: &'static str,
        ) -> Spec {
            Spec {
                name,
                plot,
                route,
                alternative,
            }
        }
        // `tf` runs everywhere, so its old ".dc() with small-signal sweep"
        // workaround could never be shown. `fourier`, `sweep` and
        // `montecarlo` wrap a transient here, so they are named `tran`.
        match self {
            Kind::Op => row("op", Plot::Named("Operating Point"), &RUNNABLE, "N/A"),
            Kind::Dc => row(
                "dc",
                Plot::Named("DC transfer characteristic"),
                &RUNNABLE,
                "N/A",
            ),
            Kind::Ac => row("ac", Plot::Named("AC Analysis"), &RUNNABLE, "N/A"),
            Kind::Tran => row("tran", Plot::Named("Transient Analysis"), &RUNNABLE, "N/A"),
            Kind::Noise => row(
                "noise",
                Plot::NamedWith("Noise Spectral Density Curves", "Integrated Noise"),
                &RUNNABLE,
                "N/A",
            ),
            Kind::Tf => row("tf", Plot::Named("Transfer Function"), &RUNNABLE, "N/A"),
            Kind::Sens => row(
                "sens",
                Plot::Named("Sensitivity Analysis"),
                &[Ngspice, Spectre, Espice],
                "N/A",
            ),
            Kind::SensAc => row(
                "sens_ac",
                Plot::Named("Sensitivity Analysis"),
                &[Ngspice, Spectre],
                "N/A",
            ),
            Kind::Pz => row(
                "pz",
                Plot::Named("Pole-Zero Analysis"),
                &[Ngspice],
                ".ac() + post-process for poles/zeros",
            ),
            Kind::Disto => row(
                "disto",
                Plot::Named("DISTORTION"),
                &[Ngspice],
                ".transient() + FFT post-processing",
            ),
            Kind::Pss => row(
                "pss",
                Plot::Unmatched("Pss"),
                &[Spectre, Ngspice],
                "long .tran() + discard startup",
            ),
            Kind::Hb => row(
                "hb",
                Plot::Unmatched("HarmonicBalance"),
                &[Vacask, Spectre],
                "long .tran() + discard startup + FFT",
            ),
            // VACASK's analyses (the pinned build's test decks): op, ac, acxf,
            // dcinc, dcxf, noise, tran, hb; no `sp`, no `stb`.
            Kind::Sp => row("sp", Plot::Named("SP Analysis"), &[Spectre, Ngspice], "N/A"),
            Kind::Stb => row(
                "stb",
                Plot::Unmatched("Stability"),
                &[Spectre],
                ".ac() + Middlebrook method",
            ),
            // ngspice's `TRNOISE` sources (manual §4.1.7) are the noise.
            Kind::TranNoise => row(
                "trannoise",
                Plot::Named("Transient Analysis"),
                &[Ngspice],
                "N/A",
            ),
            Kind::Fourier => row("tran", Plot::Table, &RUNNABLE, "N/A"),
            Kind::SpectreSweep => row("tran", Plot::Unmatched("SpectreSweep"), &RUNNABLE, "N/A"),
            Kind::SpectreMonteCarlo => row(
                "tran",
                Plot::Unmatched("SpectreMonteCarlo"),
                &RUNNABLE,
                "N/A",
            ),
            Kind::Pac => row("pac", Plot::Unmatched("SpectrePac"), &[Spectre], "N/A"),
            Kind::Pnoise => row(
                "pnoise",
                Plot::Unmatched("SpectrePnoise"),
                &[Spectre],
                "N/A",
            ),
            Kind::Pxf => row("pxf", Plot::Unmatched("SpectrePxf"), &[Spectre], "N/A"),
            Kind::Pstb => row("pstb", Plot::Unmatched("SpectrePstb"), &[Spectre], "N/A"),
        }
    }
}
