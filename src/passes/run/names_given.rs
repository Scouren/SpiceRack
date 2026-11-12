//! Rule `names_given`: a name given to every unnamed analysis, DC axis and loop, each with a
//! `NAME_GIVEN` note at its item whose argument is the new name's id (the rename map as data,
//! as `names_dropped` keeps it: the run side gives the results back under the source's
//! addressing). The mirror of `names_dropped`. Three rows, each removing its unnamed feature
//! and requiring the named twin (A35, R59):
//! - `analysis_name_given` (`AnalysisUnnamed`): `Analysis::name` set to the kind's IR word and a
//!   count (`op1`, `tran1`, `dc_xf1`);
//! - `dc_axis_name_given` (`DcAxisUnnamed`): every unnamed axis of a DC analysis named
//!   `axis<n>` (one note per axis, at the analysis);
//! - `loop_name_given` (`LoopUnnamed`): `Loop::name` set to `loop<n>`, unless the loop's body
//!   holds an opaque statement, which may read a name the new one shadows (`LOOP_NAME_UNREAD`).
//!
//! The count runs from 1 per word over the design in listed order and skips every name the
//! design already gives a statement of that kind, compared without case: no two analyses
//! share a name (VACASK refuses that: settle `dup_name.sim`), and no DC axis or loop shares
//! one with another axis or loop: VACASK writes both as sweeps and refuses one name twice in a
//! sweep chain (settle `names_chain.sim`), while an analysis and a sweep may share one
//! (`names.sim`). VACASK's names are case-sensitive, ngspice's are not.
//!
//! Exact: a name labels a result, never a number; no row of the IR reads an analysis, axis or
//! loop by its name.
//!
//! Cites: `rewrite/AMEND-A12.md` A16, A22, A35; R59.
//!
//! Contents: `pub fn analysis_name_given(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn dc_axis_name_given(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn loop_name_given(&mut ir::Design, &mut ir::Diags)`,
//! `pub const NAME_GIVEN: ir::Code`, `pub const LOOP_NAME_UNREAD: ir::Code`

use std::collections::{HashMap, HashSet};

use super::names_dropped::{inside, of_kind};
use crate::ir;

/// The note at an item given a name: `arg` is the new name's `NameId`.
pub const NAME_GIVEN: ir::Code = ir::Code::new(b'U', 410);
/// The note at an unnamed loop whose body may read a name.
pub const LOOP_NAME_UNREAD: ir::Code = ir::Code::new(b'U', 411);

/// The names one kind of statement holds (lower-cased) and the next count per word.
struct Names {
    taken: HashSet<String>,
    count: HashMap<String, u32>,
}

/// Names every reachable unnamed analysis.
pub fn analysis_name_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    let items = of_kind(design, ir::ItemKind::Analysis);
    let rows: Vec<ir::Analysis> = items.iter().map(|i| design.row(i.index)).collect();
    let mut names = Names::new(design, rows.iter().map(|a| a.name));
    for (item, mut a) in items.into_iter().zip(rows) {
        if a.name != ir::NameId::NONE {
            continue;
        }
        a.name = names.next(design, &snake(a.kind));
        diags.push_at(NAME_GIVEN, design, item, a.name.0);
        design.editor().set(item.index, a);
    }
}

/// Names every unnamed axis of every reachable DC analysis.
pub fn dc_axis_name_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    let dcs: Vec<(ir::ItemRef, ir::Analysis)> = of_kind(design, ir::ItemKind::Analysis)
        .into_iter()
        .map(|i| (i, design.row::<ir::Analysis>(i.index)))
        .filter(|(_, a)| a.kind == ir::AnalysisKind::Dc)
        .collect();
    let mut names = Names::new(design, sweep_names(design).into_iter());
    for (item, a) in dcs {
        let mut axes = axes_of(design, &a);
        if axes.iter().all(|x| x.name != ir::NameId::NONE) {
            continue;
        }
        for x in axes.iter_mut().filter(|x| x.name == ir::NameId::NONE) {
            x.name = names.next(design, "axis");
            diags.push_at(NAME_GIVEN, design, item, x.name.0);
        }
        let mut dc = design.detail::<ir::dc_ac::Dc>(a.detail);
        let mut editor = design.editor();
        dc.axes = editor.builder().dc_axes(&axes);
        editor.set_detail(a.detail, dc);
    }
}

/// Names every reachable unnamed loop, unless its body holds an opaque statement.
pub fn loop_name_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    let reachable = design.reachable();
    let items = of_kind(design, ir::ItemKind::Loop);
    let rows: Vec<ir::plan::Loop> = items.iter().map(|i| design.row(i.index)).collect();
    let mut names = Names::new(design, sweep_names(design).into_iter());
    for (item, mut l) in items.into_iter().zip(rows) {
        if l.name != ir::NameId::NONE {
            continue;
        }
        let opaque = reachable
            .iter()
            .any(|&(i, s)| i.kind == ir::ItemKind::Opaque && inside(design, s, l.body));
        if opaque {
            diags.push_at(LOOP_NAME_UNREAD, design, item, 0);
            continue;
        }
        l.name = names.next(design, "loop");
        diags.push_at(NAME_GIVEN, design, item, l.name.0);
        design.editor().set(item.index, l);
    }
}

// ---- private ----

impl Names {
    fn new(design: &ir::Design, held: impl Iterator<Item = ir::NameId>) -> Names {
        let taken = held
            .filter(|&n| n != ir::NameId::NONE)
            .map(|n| design.str(n).to_ascii_lowercase())
            .collect();
        Names {
            taken,
            count: Default::default(),
        }
    }

    /// `word` and the first count from its last one whose name is not taken, interned and
    /// taken.
    fn next(&mut self, design: &mut ir::Design, word: &str) -> ir::NameId {
        assert!(!word.is_empty(), "a name has a word");
        let count = self.count.entry(word.to_string()).or_insert(0);
        // bounded: each step past the first skips one taken name
        let name = loop {
            *count += 1;
            assert!(
                *count as usize <= self.taken.len() + 1,
                "a free count exists"
            );
            let name = format!("{word}{count}");
            if !self.taken.contains(&name) {
                break name;
            }
        };
        let fresh = self.taken.insert(name.clone());
        assert!(fresh, "a given name is new among its kind");
        design.editor().builder().name(&name)
    }
}

/// An analysis kind's word in IR text: its member's name in snake case (`DcXf` → `dc_xf`).
fn snake(kind: ir::AnalysisKind) -> String {
    let mut out = String::new();
    for (i, c) in format!("{kind:?}").chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// The names of every reachable DC axis and loop: one set, as both are sweeps on VACASK.
fn sweep_names(design: &ir::Design) -> Vec<ir::NameId> {
    let mut out = Vec::new();
    for item in of_kind(design, ir::ItemKind::Analysis) {
        let a = design.row::<ir::Analysis>(item.index);
        if a.kind == ir::AnalysisKind::Dc {
            out.extend(axes_of(design, &a).iter().map(|x| x.name));
        }
    }
    for item in of_kind(design, ir::ItemKind::Loop) {
        out.push(design.row::<ir::plan::Loop>(item.index).name);
    }
    out
}

/// The axes of a DC analysis, innermost first.
fn axes_of(design: &ir::Design, a: &ir::Analysis) -> Vec<ir::dc_ac::DcAxis> {
    let dc = design.detail::<ir::dc_ac::Dc>(a.detail);
    design.dc_axes(dc.axes).to_vec()
}
