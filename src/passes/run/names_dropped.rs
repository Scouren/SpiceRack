//! Rule `names_dropped`: statement names a target cannot spell dropped, each with a
//! `NAME_DROPPED` note at its item whose argument is the old name's id (the rename map as data,
//! A32: the run side gives the results back under the source's names). Three rows, each
//! removing its feature and requiring its unnamed twin (`AnalysisUnnamed`, `DcAxisUnnamed`,
//! `LoopUnnamed`: A35, R59):
//! - `analysis_name_dropped` (`AnalysisNamed`, A16): `Analysis::name` cleared;
//! - `dc_axis_name_dropped` (`DcAxisNamed`, A22): every axis's `DcAxis::name` of a DC analysis
//!   cleared (one note per named axis, at the analysis);
//! - `loop_name_dropped` (`LoopNamed`, A22, A27): `Loop::name` cleared, unless the loop's body
//!   holds an opaque statement, which may read the name as ngspice's `foreach` variable
//!   (`LOOP_NAME_READ`).
//!
//! Exact: a name labels a result, never a number; no row of the IR reads an analysis, axis
//! or loop by its name.
//!
//! Cites: `rewrite/AMEND-A12.md` A16, A22, A27, A32; `crates/ir/src/{analysis,dc_ac,plan}.rs`.
//!
//! Contents: `pub fn analysis_name_dropped(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn dc_axis_name_dropped(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn loop_name_dropped(&mut ir::Design, &mut ir::Diags)`,
//! `pub const NAME_DROPPED: ir::Code`, `pub const LOOP_NAME_READ: ir::Code`,
//! `pub(super) fn of_kind(&ir::Design, ir::ItemKind) -> Vec<ir::ItemRef>`,
//! `pub(super) fn inside(&ir::Design, ir::ScopeId, ir::ScopeId) -> bool`

use crate::ir;

/// The note at an item whose name was dropped: `arg` is the old name's `NameId`.
pub const NAME_DROPPED: ir::Code = ir::Code::new(b'U', 407);
/// The note at a named loop whose body may read its name.
pub const LOOP_NAME_READ: ir::Code = ir::Code::new(b'U', 408);

/// Clears every reachable analysis's name.
pub fn analysis_name_dropped(design: &mut ir::Design, diags: &mut ir::Diags) {
    for item in of_kind(design, ir::ItemKind::Analysis) {
        let mut a = design.row::<ir::Analysis>(item.index);
        if a.name == ir::NameId::NONE {
            continue;
        }
        diags.push_at(NAME_DROPPED, design, item, a.name.0);
        a.name = ir::NameId::NONE;
        design.editor().set(item.index, a);
    }
}

/// Clears every reachable DC analysis's axis names.
pub fn dc_axis_name_dropped(design: &mut ir::Design, diags: &mut ir::Diags) {
    for item in of_kind(design, ir::ItemKind::Analysis) {
        let a = design.row::<ir::Analysis>(item.index);
        if a.kind != ir::AnalysisKind::Dc {
            continue;
        }
        let mut dc = design.detail::<ir::dc_ac::Dc>(a.detail);
        let mut axes: Vec<ir::dc_ac::DcAxis> = design.dc_axes(dc.axes).to_vec();
        if axes.iter().all(|x| x.name == ir::NameId::NONE) {
            continue;
        }
        for x in axes.iter_mut().filter(|x| x.name != ir::NameId::NONE) {
            diags.push_at(NAME_DROPPED, design, item, x.name.0);
            x.name = ir::NameId::NONE;
        }
        let mut editor = design.editor();
        dc.axes = editor.builder().dc_axes(&axes);
        editor.set_detail(a.detail, dc);
    }
}

/// Clears every reachable loop's name, unless its body holds an opaque statement.
pub fn loop_name_dropped(design: &mut ir::Design, diags: &mut ir::Diags) {
    let reachable = design.reachable();
    for item in of_kind(design, ir::ItemKind::Loop) {
        let mut l = design.row::<ir::plan::Loop>(item.index);
        if l.name == ir::NameId::NONE {
            continue;
        }
        let opaque = reachable
            .iter()
            .any(|&(i, s)| i.kind == ir::ItemKind::Opaque && inside(design, s, l.body));
        if opaque {
            diags.push_at(LOOP_NAME_READ, design, item, 0);
            continue;
        }
        diags.push_at(NAME_DROPPED, design, item, l.name.0);
        l.name = ir::NameId::NONE;
        design.editor().set(item.index, l);
    }
}

/// The reachable items of `kind`, in listed order.
pub(super) fn of_kind(design: &ir::Design, kind: ir::ItemKind) -> Vec<ir::ItemRef> {
    let reachable = design.reachable().into_iter();
    reachable
        .map(|(i, _)| i)
        .filter(|i| i.kind == kind)
        .collect()
}

/// Whether `scope` is `body` or lies inside it.
pub(super) fn inside(design: &ir::Design, scope: ir::ScopeId, body: ir::ScopeId) -> bool {
    let mut at = scope;
    // bounded by the scope tree's depth
    while !at.is_none() {
        if at == body {
            return true;
        }
        at = design.scope(at).parent;
    }
    false
}
