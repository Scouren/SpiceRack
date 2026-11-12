//! Statistics blocks: the table form of statistical variation (Spectre `statistics { process
//! { vary … } mismatch { vary … } correlate … truncate … }`), as one item with ranges into the
//! run owner's vary and correlation pools. A reader's block in, rows out.
//!
//! The call form (`.param p=agauss(1k,100,1)`) is not here: it is the kernel's `Draw` node,
//! whose `DrawWhen`/`DrawPer` state when and how often it is drawn (Addendum A2). The two are
//! different native meanings; converting one to the other is a pass (`DESIGN-mc-lowering.md`
//! §2). A block varies only inside a Monte Carlo loop whose `Variations` selects its scope; a
//! `process` variation is drawn once per sample, a `mismatch` one once per subcircuit instance
//! per sample (`INV-semantics.md` 12.03, 12.05).
//!
//! Cites: `rewrite/IR-RUN.md` §5, §6; `rewrite/INV-constructs.md` J1–J5, J15, J17, J18;
//! `rewrite/INV-semantics.md` 12.03, 12.05, 12.07.
//!
//! Contents: `pub struct Statistics`, `pub struct Vary`, `pub enum VaryDist`,
//! `pub struct Correlation`, `pub type VaryList`, `pub type CorrelationList`;
//! `VaryDist::ALL`, `VaryDist::COUNT`; `Design::varies(&self, VaryList) -> &[Vary]`,
//! `Design::correlations(&self, CorrelationList) -> &[Correlation]`; `RowFacts` for
//! `Statistics` (`todo_facts!` placeholder, A2)

use crate::design::Design;
use crate::id::{ExprId, List, Ref};

/// A statistics block: the variations of a Monte Carlo loop, by scope. At the top or in a
/// library section (a block inside a subcircuit or a branch is refused by its reader, W0018).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Statistics {
    /// Drawn once per sample.
    pub process: VaryList,
    /// Drawn once per subcircuit instance per sample.
    pub mismatch: VaryList,
    /// Correlations among `process` parameters.
    pub process_correlations: CorrelationList,
    /// Correlations among `mismatch` parameters.
    pub mismatch_correlations: CorrelationList,
    /// Truncation of every draw of the block, in standard deviations (`truncate tr=` at the
    /// block's top), or `NONE` for none.
    pub truncate: ExprId,
    /// Truncation of the `process` draws, overriding `truncate`, or `NONE`.
    pub process_truncate: ExprId,
    /// Truncation of the `mismatch` draws, overriding `truncate`, or `NONE`.
    pub mismatch_truncate: ExprId,
}

/// One varied parameter (`vary p dist=gauss std=s`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vary {
    /// The parameter varied (a `Param` definition the linker binds).
    pub param: Ref,
    /// Its distribution.
    pub dist: VaryDist,
    /// Whether `spread` is a percentage of the parameter's nominal value (`percent=yes`)
    /// instead of an absolute amount in the parameter's unit.
    pub percent: bool,
    /// The distribution's centre, or `NONE` for the parameter's own value.
    pub mean: ExprId,
    /// The spread [`VaryDist`] states; never `NONE`.
    pub spread: ExprId,
}

/// A varied parameter's distribution. Each member states what `spread` is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VaryDist {
    /// Normal, `spread` = one standard deviation (Spectre `dist=gauss std=`).
    Gauss,
    /// Uniform on `mean ± spread` (Spectre `dist=unif N=`).
    Unif,
    /// Log-normal: the value's natural logarithm is normal around the logarithm of `mean`,
    /// with standard deviation `spread` (Spectre `dist=lnorm std=`; per the Spectre Reference,
    /// not verified by a run, `INV-semantics.md` B4).
    LogNormal,
}

/// The correlation coefficient of two varied parameters of one scope (`correlate param=[a b]
/// cc=0.8`; a list of n parameters is its n·(n−1)/2 pairs, each with the same coefficient).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Correlation {
    /// The first parameter (a `Param` definition the linker binds).
    pub a: Ref,
    /// The second parameter; differs from `a`.
    pub b: Ref,
    /// The coefficient, in [−1, 1]; never `NONE`.
    pub cc: ExprId,
}

/// An ordered list of variations, a range into the run owner's vary pool.
pub type VaryList = List<Vary>;
/// An ordered list of correlations, a range into the run owner's correlation pool.
pub type CorrelationList = List<Correlation>;

const _: () = assert!(size_of::<Statistics>() == 44);
const _: () = assert!(size_of::<Vary>() == 20);
const _: () = assert!(size_of::<Correlation>() == 20);

impl VaryDist {
    /// Every distribution, in declaration order.
    pub const ALL: [VaryDist; 3] = [VaryDist::Gauss, VaryDist::Unif, VaryDist::LogNormal];
    /// Distribution count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The variations of a list. Asserts the range is inside the vary pool.
    pub fn varies(&self, list: VaryList) -> &[Vary] {
        todo!()
    }

    /// The correlations of a list. Asserts the range is inside the correlation pool.
    pub fn correlations(&self, list: CorrelationList) -> &[Correlation] {
        todo!()
    }
}

// run: implementation phase — replace this line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(Statistics);
