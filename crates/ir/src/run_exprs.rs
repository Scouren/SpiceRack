//! The run rows' walk over their expression roots (A23): a row (or its detail row) and a pass's
//! visitor in, every root visited in `handles`' order and stored back out. [`Roots`] holds the
//! pools a row's lists point into, so each row's part is one call per field in field order.
//!
//! Cites: `rewrite/AMEND-A12.md` A23; `crates/ir/src/facts.rs` (`RowFacts::map_exprs`).
//!
//! Contents: `pub(crate) struct Roots` (`Roots::expr`, `Roots::exprs`, `Roots::sweep`,
//! `Roots::probe`, `Roots::probes`, `Roots::event`, `Roots::dc_axes`, `Roots::varies`,
//! `Roots::correlations`), `pub(crate) fn roots`, `pub(crate) fn exprs_of`

use std::ops::Range;

use crate::analysis::RunDetails;
use crate::analysis_facts::DetailFacts;
use crate::dc_ac::DcAxisList;
use crate::design::Design;
use crate::facts::ExprUse;
use crate::id::{ExprId, ExprList};
use crate::measure::Event;
use crate::probe::{Probe, ProbeList};
use crate::stats::{CorrelationList, VaryList};
use crate::sweep::SweepSpec;

/// A pass's visitor and the pools a run row's lists point into. Each method visits what the
/// matching `*_handles` function lists, in its order, `NONE` roots included.
pub(crate) struct Roots<'a> {
    /// The kernel's expression-list pool (`Handle::Exprs` elements).
    pool: &'a mut Vec<ExprId>,
    /// The run owner's pools (probes, DC axes, varies, correlations).
    run: &'a mut RunDetails,
    /// The pass's visitor.
    f: &'a mut dyn FnMut(&mut ExprId, ExprUse),
}

impl Roots<'_> {
    /// A `Handle::Expr` field.
    pub(crate) fn expr(&mut self, e: &mut ExprId) {
        (self.f)(e, ExprUse::Constant);
    }

    /// A `Handle::Exprs` list: each element, in place.
    pub(crate) fn exprs(&mut self, list: ExprList) {
        let r = span(list.start, list.len, self.pool.len());
        for e in &mut self.pool[r] {
            (self.f)(e, ExprUse::Constant);
        }
    }

    /// A sweep's roots (`sweep_handles`).
    pub(crate) fn sweep(&mut self, s: &mut SweepSpec) {
        for e in [&mut s.start, &mut s.stop, &mut s.step, &mut s.points] {
            self.expr(e);
        }
        self.exprs(s.values);
    }

    /// A probe's root (`probe_handles`): its expression, which may read the circuit.
    pub(crate) fn probe(&mut self, p: &mut Probe) {
        (self.f)(&mut p.expr, ExprUse::Circuit);
    }

    /// Each probe of a pool list, in place.
    pub(crate) fn probes(&mut self, list: ProbeList) {
        for k in span(list.start, list.len, self.run.probes.len()) {
            let mut p = self.run.probes[k];
            self.probe(&mut p);
            self.run.probes[k] = p;
        }
    }

    /// An event's roots (`event_handles`).
    pub(crate) fn event(&mut self, e: &mut Event) {
        self.probe(&mut e.signal);
        (self.f)(&mut e.value, ExprUse::Circuit);
        for x in [&mut e.count, &mut e.delay, &mut e.until] {
            self.expr(x);
        }
    }

    /// Each DC axis's sweep, in place (a target holds no expression).
    pub(crate) fn dc_axes(&mut self, list: DcAxisList) {
        for k in span(list.start, list.len, self.run.dc_axes.len()) {
            let mut s = self.run.dc_axes[k].sweep;
            self.sweep(&mut s);
            self.run.dc_axes[k].sweep = s;
        }
    }

    /// Each vary's mean and spread, in place.
    pub(crate) fn varies(&mut self, list: VaryList) {
        for k in span(list.start, list.len, self.run.varies.len()) {
            let mut v = self.run.varies[k];
            self.expr(&mut v.mean);
            self.expr(&mut v.spread);
            self.run.varies[k] = v;
        }
    }

    /// Each correlation's coefficient, in place.
    pub(crate) fn correlations(&mut self, list: CorrelationList) {
        for k in span(list.start, list.len, self.run.correlations.len()) {
            let mut c = self.run.correlations[k];
            self.expr(&mut c.cc);
            self.run.correlations[k] = c;
        }
    }
}

/// The walk's state over `design`'s pools. Asserts nothing; a list past its pool visits what
/// the pool holds (`check` reports it).
pub(crate) fn roots<'a>(
    design: &'a mut Design,
    f: &'a mut dyn FnMut(&mut ExprId, ExprUse),
) -> Roots<'a> {
    Roots {
        pool: &mut design.expr_pool,
        run: &mut design.run,
        f,
    }
}

/// `DetailFacts::map_exprs` of `D`'s row at `index`, stored back; nothing when out of range.
pub(crate) fn exprs_of<D: DetailFacts>(
    design: &mut Design,
    index: u32,
    f: &mut dyn FnMut(&mut ExprId, ExprUse),
) {
    let Some(mut d) = crate::circuit_text::get_row::<D>(design, index) else {
        return;
    };
    d.map_exprs(&mut roots(design, f));
    D::rows_mut(design)[index as usize] = d;
}

// ---- private ----

/// The indices of a pool list, clamped to the pool.
fn span(start: u32, len: u32, pool: usize) -> Range<usize> {
    let start = start as usize;
    let end = (start + len as usize).min(pool);
    start.min(end)..end
}
