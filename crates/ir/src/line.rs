//! Transmission lines' typed instance fields (lossless, LTRA, URC, TXL, CPL): the electrical
//! length of a lossless line as written, and the physical length and lumping of the lossy ones,
//! whose per-unit-length data is the model's (`ModelFamily::Ltra`, `Txl`, `Cpl`, `Urc`).
//! Builder pushes in, rows out through `design.line(&e)`.
//!
//! A lossless line keeps the form its card gave: `z0` + `td`, `freq` + `nl`, or `length` +
//! `vel`; a reader never converts one into another (IR-CIRCUIT §2), and fills the simulator's
//! default `nl` it implies (sp 0.25, va 1; INV-semantics 6.24).
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §2 (lines); `rewrite/INV-constructs.md` D31–D35, D71–D73,
//! D84, D134; `rewrite/INV-semantics.md` 6.20–6.24.
//!
//! Contents: `pub struct Line`, `Design::line(&self, &Element) -> Line`

use crate::design::Design;
use crate::element::{DetailKind, Element};
use crate::id::ExprId;

/// The detail row of a line element. Which fields a kind may hold: `LosslessLine` `z0` and one
/// of (`td`), (`freq`, `nl`), (`length`, `vel`); `LossyLine`, `SingleLossyLine`,
/// `CoupledLines` `length`; `RcLine` `length`, `lumps`. Every other field is `NONE`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// Characteristic impedance (Ω) (ng/hs/xy `Z0=`, lt `Zo=`, sp `z0=`).
    pub z0: ExprId,
    /// One-way delay (s) (`TD=`, lt `Td=`).
    pub td: ExprId,
    /// The frequency at which the line is `nl` wavelengths long (Hz) (`F=`, sp `f=`).
    pub freq: ExprId,
    /// Normalised length in wavelengths at `freq` (`NL=`); present exactly when `freq` is.
    pub nl: ExprId,
    /// Physical length (m) (sp `len=`; LTRA/TXL/CPL/URC instance `len=`/`l=`). `NONE` on a
    /// lossy line: the model's length.
    pub length: ExprId,
    /// Propagation velocity relative to c (sp `vel=`); present exactly when a lossless line
    /// is given by `length`.
    pub vel: ExprId,
    /// Lumps of a URC expansion (ng `n=`). `NONE`: the model's rule from fmax and k (6.23).
    pub lumps: ExprId,
}

const _: () = assert!(size_of::<Line>() == 28);

impl Design {
    /// The row of a line element. Asserts the kind.
    pub fn line(&self, element: &Element) -> Line {
        assert!(
            element.kind.detail() == DetailKind::Line,
            "{:?} has no Line row",
            element.kind
        );
        self.detail(element.detail)
    }
}
