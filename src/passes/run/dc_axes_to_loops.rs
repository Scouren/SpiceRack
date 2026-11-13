//! Rule `dc_axes_to_loops`: a DC analysis becomes nested sweep loops over listed values around
//! what is left of it, where ngspice's `.dc` cannot state it. Three rows (R72, R75):
//! - `dc_axis_device_param_to_loop` (removes `DcAxisDeviceParam`) and
//!   `dc_axis_model_param_to_loop` (`DcAxisModelParam`): ngspice's `.dc` sweeps a source, an
//!   element's value or the temperature, so the axes are peeled from the outermost through the
//!   innermost axis of the row's target kind;
//! - `dc_restart_to_loops` (`DcRestart`): ngspice's `.dc` continues its innermost axis point to
//!   point, so a DC whose innermost axis restarts each point is peeled whole into loops of
//!   operating points.
//!
//! Each loop runs over its axis's values as decimal literals (`sweep_forms::values`); the
//! analysis moves into the innermost body, a DC over the axes left inside, or an operating point
//! when none is left. Requires `Item(Loop)`, a list sweep loop that keeps its last value, the
//! loop target kinds, an operating point in a loop's body and the names its loops and analysis
//! carry.
//!
//! What changes for the reader of the results: the source writes one DC plot over every axis
//! (its swept vectors named by the axes); the target writes one plot per pass of the loops,
//! outermost slowest, each an operating point (or a DC over the inner axes), and no vector of
//! a peeled axis: its value at each pass is the loop's list, in order. Around an inner DC each
//! pass still gives one DC plot, so the passes join outer-loop-major. The note `AS_LOOPS` at
//! every loop a peeled axis became says so (the results' reader takes exactly that loop's
//! plots, in run order, and its values from the loop's own sweep row); its argument is the
//! analysis's name as read.
//!
//! Exact only when:
//! - every peeled axis starts afresh (`DcAxis::afresh`, R75): a loop's pass reruns what it holds
//!   from the initial guess (ngspice-45's op after alter starts afresh, R72's run
//!   `tests/passes_run/decks/bistable_{dc,loop}.sp`: at v(in) = 0 the continued `.dc` gives
//!   -2 V and the loop of ops 0 V; and its nested `.dc` starts each outer step afresh too,
//!   `latch_desc.sp`), where an axis that carries starts from the solve before it (VACASK's
//!   default, `latch.sim`: 4.9734 against 3.490099). An axis that carries stays refused (R72);
//! - it stands in its walk's top and no analysis follows it in the walk: the loops keep their
//!   last values, which no later analysis then sees;
//! - every peeled axis has a target a loop sweeps (no temperature, option or subcircuit
//!   default) and values `sweep_forms::values` states (no log grid);
//! - the nesting stays within `ir::plan::LOOP_DEPTH_MAX`.
//!
//! Otherwise the analysis keeps the feature with a `DC_AXIS_KEPT` note.
//!
//! Cites: `crates/ir/src/{dc_ac,plan}.rs`; `rewrite/IR-RUN.md` §1; AMEND-A12 A22, A27.
//!
//! Contents: `pub fn dc_axis_device_param_to_loop(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn dc_restart_to_loops(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn dc_axis_model_param_to_loop(&mut ir::Design, &mut ir::Diags)`,
//! `pub const DC_AXIS_KEPT: ir::Code`, `pub const AS_LOOPS: ir::Code`

use super::sweep_forms::values;
use crate::ir;
use ir::dc_ac::{Dc, DcAxis};
use ir::plan::{LoopEnd, SweepLoop, TargetKind};

/// The note at a DC analysis the rule leaves as it is.
pub const DC_AXIS_KEPT: ir::Code = ir::Code::new(b'U', 415);
/// The note at each loop a DC analysis's peeled axis became: `arg` is the analysis's name.
pub const AS_LOOPS: ir::Code = ir::Code::new(b'U', 416);

/// The axis a row peels through: the innermost of a target kind (a device or model
/// parameter), or every axis of a DC whose innermost axis restarts (`DcRestart`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Through {
    Kind(TargetKind),
    Restart,
}

/// One conversion: the analysis, its scope, the peeled axes (outermost first) with their
/// values, and the axes left inside.
type Work = (
    ir::ItemRef,
    ir::ScopeId,
    Vec<(DcAxis, Vec<ir::Literal>)>,
    Vec<DcAxis>,
);

/// DC analyses with a device-parameter axis made loops.
pub fn dc_axis_device_param_to_loop(design: &mut ir::Design, diags: &mut ir::Diags) {
    convert(design, diags, Through::Kind(TargetKind::DeviceParam));
}

/// DC analyses with a model-parameter axis made loops.
pub fn dc_axis_model_param_to_loop(design: &mut ir::Design, diags: &mut ir::Diags) {
    convert(design, diags, Through::Kind(TargetKind::ModelParam));
}

/// DC analyses whose innermost axis restarts each point (`DcRestart`) made loops of
/// operating points over every axis.
pub fn dc_restart_to_loops(design: &mut ir::Design, diags: &mut ir::Diags) {
    convert(design, diags, Through::Restart);
}

// ---- private ----

/// The rows' body: `through` names the axis peeled through.
fn convert(design: &mut ir::Design, diags: &mut ir::Diags, through: Through) {
    assert!(matches!(
        through,
        Through::Restart | Through::Kind(TargetKind::DeviceParam | TargetKind::ModelParam)
    ));
    let mut work: Vec<Work> = Vec::new();
    for (top, order) in ir::run_walks(design) {
        for (k, &(item, scope)) in order.iter().enumerate() {
            if item.kind != ir::ItemKind::Analysis {
                continue;
            }
            let a = design.row::<ir::Analysis>(item.index);
            if a.kind != ir::AnalysisKind::Dc {
                continue;
            }
            let dc = design.detail::<Dc>(a.detail);
            let axes = design.dc_axes(dc.axes);
            let inner = match through {
                Through::Restart => axes.first().filter(|x| x.afresh).map(|_| 0),
                Through::Kind(kind) => axes.iter().position(|x| x.target.kind == kind),
            };
            let Some(inner) = inner else {
                continue;
            };
            let later = order[k + 1..]
                .iter()
                .any(|(i, _)| i.kind == ir::ItemKind::Analysis);
            // R75: a loop's points start afresh, so every peeled axis must (`DcAxis::afresh`)
            let afresh = axes[inner..].iter().all(|x| x.afresh);
            let peeled = (afresh && scope == top && !later)
                .then(|| peel(design, &axes[inner..]))
                .flatten();
            match peeled {
                Some(p) => work.push((item, scope, p, axes[..inner].to_vec())),
                None => diags.push_at(DC_AXIS_KEPT, design, item, 0),
            }
        }
    }
    for w in work {
        build(design, diags, w);
    }
}

/// The peeled axes, outermost first, with their values; `None` when one cannot be a loop.
fn peel(design: &ir::Design, axes: &[DcAxis]) -> Option<Vec<(DcAxis, Vec<ir::Literal>)>> {
    if axes.len() > ir::plan::LOOP_DEPTH_MAX {
        return None;
    }
    let loopable = |t: TargetKind| {
        matches!(
            t,
            TargetKind::Param
                | TargetKind::SourceDc
                | TargetKind::ElementValue
                | TargetKind::DeviceParam
                | TargetKind::ModelParam
        )
    };
    axes.iter()
        .rev()
        .map(|x| {
            loopable(x.target.kind)
                .then(|| values(design, &x.sweep))
                .flatten()
                .map(|v| (*x, v))
        })
        .collect()
}

/// Builds the loops where the analysis stood and moves it into the innermost body.
fn build(design: &mut ir::Design, diags: &mut ir::Diags, (item, scope, peeled, inner): Work) {
    assert!(!peeled.is_empty(), "a conversion peels an axis");
    let names: Vec<Option<String>> = peeled
        .iter()
        .map(|(x, _)| (x.name != ir::NameId::NONE).then(|| design.str(x.name).to_string()))
        .collect();
    let mut a = design.row::<ir::Analysis>(item.index);
    for l in place(design, (item, scope), &peeled, &names, &inner, &mut a) {
        diags.push_at(AS_LOOPS, design, l, a.name.0);
    }
}

/// The loops pushed where `at.0` stood, `a` rewritten (an operating point, or a DC over
/// `inner`) and moved into the innermost body; the loops, outermost first.
fn place(
    design: &mut ir::Design,
    (item, scope): (ir::ItemRef, ir::ScopeId),
    peeled: &[(DcAxis, Vec<ir::Literal>)],
    names: &[Option<String>],
    inner: &[DcAxis],
    a: &mut ir::Analysis,
) -> Vec<ir::ItemRef> {
    let mut editor = design.editor();
    let at = editor
        .design()
        .items(scope)
        .iter()
        .position(|&i| i == item)
        .expect("listed");
    let mut parent = scope;
    let mut loops = Vec::with_capacity(peeled.len());
    for ((axis, vals), name) in peeled.iter().zip(names) {
        let mut b = editor.builder();
        let ids: Vec<ir::ExprId> = vals.iter().map(|&l| b.literal(l)).collect();
        let list = b.exprs(&ids);
        let row = SweepLoop {
            target: axis.target,
            sweep: ir::SweepSpec::list(list),
        };
        let (l, body) = b.loop_(
            parent,
            name.as_deref(),
            row,
            LoopEnd::KeepLast,
            ir::Span::NONE,
        );
        if loops.is_empty() {
            let items = editor.items_mut(scope);
            assert!(items.last() == Some(&l), "a new loop is pushed last");
            items.pop();
            items.insert(at, l);
        }
        loops.push(l);
        parent = body;
    }
    if inner.is_empty() {
        a.kind = ir::AnalysisKind::Op;
        a.detail = u32::MAX;
    } else {
        let mut dc = editor.design().detail::<Dc>(a.detail);
        dc.axes = editor.builder().dc_axes(inner);
        editor.set_detail(a.detail, dc);
    }
    editor.set(item.index, *a);
    editor.move_item(scope, item, parent, 0);
    assert!(loops.len() == peeled.len(), "a loop an axis");
    loops
}
