//! The run rows' by-name element references outside expressions (A32): a row (or its detail
//! row) and a pass's visitor in, every element name visited in field order, in place, out.
//! [`Names`] holds the pools a row's lists point into. A path through instances is one use per
//! segment for the kernel: a whole element path (a current, a device) is visited whole, a net
//! path (a voltage, a node set) only up to its last segment, which names a net.
//!
//! Cites: `rewrite/AMEND-A12.md` A32; `crates/ir/src/facts.rs` (`RowFacts::map_element_names`).
//!
//! Contents: `pub(crate) struct Names` (`Names::one`, `Names::each`, `Names::path`,
//! `Names::net_path`, `Names::probe`, `Names::probes`, `Names::event`, `Names::target`,
//! `Names::targets`, `Names::dc_axes`), `pub(crate) fn names`, `pub(crate) fn names_of`

use std::ops::Range;

use crate::analysis::RunDetails;
use crate::analysis_facts::DetailFacts;
use crate::dc_ac::DcAxisList;
use crate::design::Design;
use crate::id::{NameId, NameList};
use crate::measure::Event;
use crate::plan::{Target, TargetList};
use crate::probe::{Probe, ProbeKind, ProbeList};

/// A pass's visitor and the pools a run row's name lists point into.
pub(crate) struct Names<'a> {
    /// The kernel's name-list pool.
    pool: &'a mut Vec<NameId>,
    /// The run owner's pools (probes, targets, DC axes).
    run: &'a mut RunDetails,
    /// The pass's visitor.
    f: &'a mut dyn FnMut(&mut [NameId]),
}

impl Names<'_> {
    /// One element name; nothing when `NONE`.
    pub(crate) fn one(&mut self, n: &mut NameId) {
        if !n.is_none() {
            (self.f)(std::slice::from_mut(n));
        }
    }

    /// Each name of a list as its own element (Sp's ports).
    pub(crate) fn each(&mut self, list: NameList) {
        for k in span(list.start, list.len, self.pool.len()) {
            (self.f)(std::slice::from_mut(&mut self.pool[k]));
        }
    }

    /// An element path, whole; nothing when empty.
    pub(crate) fn path(&mut self, list: NameList) {
        let r = span(list.start, list.len, self.pool.len());
        if !r.is_empty() {
            (self.f)(&mut self.pool[r]);
        }
    }

    /// A net path: its instance segments, all but the last; nothing when flat.
    pub(crate) fn net_path(&mut self, list: NameList) {
        let r = span(list.start, list.len, self.pool.len());
        if r.len() > 1 {
            (self.f)(&mut self.pool[r.start..r.end - 1]);
        }
    }

    /// A probe's element names: an element path for a current, a device or a power, the
    /// instance segments of a voltage's nets; none for an expression (the kernel reads its
    /// `Expr::I`) or a category.
    pub(crate) fn probe(&mut self, p: &Probe) {
        match p.kind {
            ProbeKind::V | ProbeKind::Vdiff => {
                self.net_path(p.a);
                self.net_path(p.b);
            }
            ProbeKind::I | ProbeKind::ITerminal | ProbeKind::Param | ProbeKind::Power => {
                self.path(p.a);
            }
            ProbeKind::Expr
            | ProbeKind::AllV
            | ProbeKind::AllVTop
            | ProbeKind::AllSourceI
            | ProbeKind::AllI
            | ProbeKind::AllPinI
            | ProbeKind::OutputNoise
            | ProbeKind::InputNoise
            | ProbeKind::AllBranchI => {}
        }
    }

    /// Each probe of a pool list.
    pub(crate) fn probes(&mut self, list: ProbeList) {
        for k in span(list.start, list.len, self.run.probes.len()) {
            let p = self.run.probes[k];
            self.probe(&p);
        }
    }

    /// An event's signal.
    pub(crate) fn event(&mut self, e: &Event) {
        self.probe(&e.signal);
    }

    /// A target of an element kind (a source's DC value, an element's value, a device's
    /// parameter): its instance path and its element as one path, written back to both.
    pub(crate) fn target(&mut self, t: &mut Target) {
        if !t.kind.has_path() {
            return; // no element: a parameter, a model, the temperature, an option
        }
        let r = span(t.path.start, t.path.len, self.pool.len());
        let mut whole: Vec<NameId> = self.pool[r.clone()].to_vec();
        whole.push(t.a.name);
        (self.f)(&mut whole);
        let last = whole.pop().expect("the joined path ends in the element");
        t.a.name = last;
        self.pool[r].copy_from_slice(&whole);
    }

    /// Each target of a pool list, in place.
    pub(crate) fn targets(&mut self, list: TargetList) {
        for k in span(list.start, list.len, self.run.targets.len()) {
            let mut t = self.run.targets[k];
            self.target(&mut t);
            self.run.targets[k] = t;
        }
    }

    /// Each DC axis's target, in place.
    pub(crate) fn dc_axes(&mut self, list: DcAxisList) {
        for k in span(list.start, list.len, self.run.dc_axes.len()) {
            let mut t = self.run.dc_axes[k].target;
            self.target(&mut t);
            self.run.dc_axes[k].target = t;
        }
    }
}

/// The walk's state over `design`'s pools.
pub(crate) fn names<'a>(design: &'a mut Design, f: &'a mut dyn FnMut(&mut [NameId])) -> Names<'a> {
    Names {
        pool: &mut design.name_pool,
        run: &mut design.run,
        f,
    }
}

/// `DetailFacts::map_names` of `D`'s row at `index`, stored back; nothing when out of range.
pub(crate) fn names_of<D: DetailFacts>(
    design: &mut Design,
    index: u32,
    f: &mut dyn FnMut(&mut [NameId]),
) {
    let Some(mut d) = crate::circuit_text::get_row::<D>(design, index) else {
        return;
    };
    d.map_names(&mut names(design, f));
    D::rows_mut(design)[index as usize] = d;
}

// ---- private ----

/// The indices of a pool list, clamped to the pool.
fn span(start: u32, len: u32, pool: usize) -> Range<usize> {
    let start = start as usize;
    let end = (start + len as usize).min(pool);
    start.min(end)..end
}
