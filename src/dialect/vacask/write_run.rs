//! Run items → statements of the one `control` … `endc` block, each in IR order: `options`
//! lines (one per `Option` or `Env` row), `analysis name kind k=v` with its `ic=`/`nodeset=`
//! lists from the `NodeSet` rows just before it, a `Dc` as its axes' `sweep` lines then an `op`,
//! a sweep `Loop` as its `sweep` line then its body, a Monte Carlo `Loop` as `mc … endmc`,
//! `alter`, `elaborate`, `save` (`write_outputs`). Nothing is reordered. The spine writes
//! `control` and `endc` around the span `control_span` gives.
//!
//! An unnamed analysis is named `<kind><n>`, n its place among the deck's analyses in listed
//! order; an unnamed sweep `sweep<n>`, n its place among the deck's sweep lines (DIALECT-VACASK
//! §4; VACASK requires both names, VD cmd-analysis, cmd-sweep).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cmd-overview, cmd-sweep, cmd-analysis-*.
//!
//! Contents: `analysis`, `control_span`, `admit` (crate-private), and for the children
//! `quote`, `name_of`, `fold_alters`, `body`

mod analyses;
mod plan;

pub(super) use super::expr_write::string as quote;
use super::read_run::{
    ARG_READS_PARAM, MC_NESTED, NAME_SPELLING, NODE_SET_PLACE, OPTION_ROW_PRIVATE, OUTPUT_FORM,
    RESET_CLASS, SOURCE_SWEEP_NAME, SOURCE_SWEEP_WAVE, TARGET_UNFOUND, identifier,
};
use super::write::Writer;
use super::{fold, write_outputs, write_structure};
use crate::ir;
use crate::ir::source::DcRule;

/// The control commands' head words (VD cmd-overview "Summary").
const COMMANDS: &[&str] = &[
    "analysis",
    "sweep",
    "save",
    "options",
    "var",
    "elaborate",
    "alter",
    "print",
    "postprocess",
    "clear",
    "mc",
    "endmc",
    "abort",
];

/// One run item, its first line at `w.depth` (a loop writes its body below it).
pub(super) fn analysis(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    match at.kind {
        ir::ItemKind::Analysis => analyses::analysis(w, at),
        ir::ItemKind::Loop => plan::loop_(w, at),
        ir::ItemKind::Alter => plan::alter(w, d.row(at.index)),
        ir::ItemKind::Option => plan::option(w, d.row(at.index)),
        ir::ItemKind::Env => plan::env(w, d.row(at.index)),
        ir::ItemKind::Elaborate => plan::elaborate(w, d.row(at.index)),
        ir::ItemKind::Save => write_outputs::save(w, at),
        // `admits` refuses every other class (W2305).
        ir::ItemKind::Reset => {
            w.line();
            w.out.push_str("clear saves\n");
        }
        // Spelled on the analysis it applies to.
        ir::ItemKind::NodeSet => {}
        _ => unreachable!("FEATURES lacks {:?}: legalize refused it", at.kind),
    }
}

/// The first and last index into `design.items(top)` of the control block's items, for the
/// spine's `control` … `endc` (VD cmd-overview: one block per file; run x1): from the first run
/// item to the last, widened over the control-command opaques next to them (a `print` before
/// the first analysis), across comments; with no run item, the first and last control-command
/// opaque of class Private, Inert or Hint (the classes only this area gives a command line). A
/// command word alone is no proof: outside a block `print …` or `save v(…)` is another area's
/// opaque (InvalidInSource P2013: run fx). So a block that holds only refused statements reads
/// back as refused statements at the top: VACASK rejects both.
pub(super) fn control_span(design: &ir::Design, top: ir::ScopeId) -> Option<(usize, usize)> {
    let items = design.items(top);
    let command = |k: usize| command_opaque(design, items[k]);
    // ponytail: every class widens, a top-level invalid command line beside the block
    // included (it reads back inside the block: VACASK rejects that deck either way); an
    // invalid statement inside a block (a refused `save (…)`) must not cut the block short.
    let ours = |k: usize| command(k).is_some();
    let comment = |k: usize| items[k].kind == ir::ItemKind::Comment;
    if let Some(mut first) = items.iter().position(|&at| run_item(at)) {
        let mut last = items.iter().rposition(|&at| run_item(at))?;
        // Widen over our command opaques, across comments between them.
        let mut k = first;
        while k > 0 && (ours(k - 1) || comment(k - 1)) {
            k -= 1;
            if ours(k) {
                first = k;
            }
        }
        let mut k = last;
        while k + 1 < items.len() && (ours(k + 1) || comment(k + 1)) {
            k += 1;
            if ours(k) {
                last = k;
            }
        }
        return Some((first, last));
    }
    let own = |k: &usize| {
        command(*k).is_some_and(|c| {
            matches!(
                c,
                ir::OpaqueClass::Private | ir::OpaqueClass::Inert | ir::OpaqueClass::Hint
            )
        })
    };
    let first = (0..items.len()).find(own)?;
    let last = (0..items.len()).rfind(own)?;
    Some((first, last))
}

/// Appends a refusal for each row of this area VACASK cannot spell for a reason no feature
/// states (R21): an option row of a private key (W2304; VACASK's own options are Hint or
/// Private opaques); a sweep of a waveform source's DC value (W2307, a feature candidate) or
/// of a source VACASK would not find by its name from the top (W2309); a
/// node set no analysis line that takes its list follows (W2308: VACASK states `ic=` and
/// `nodeset=` on an analysis only); an `mc` loop inside another (W2310: VD cmd-analysis-mc, no
/// nesting); a transfer or noise analysis whose output is no node or node pair (W2311); a
/// device or top-level parameter target VACASK would not find (W2312); an analysis argument or
/// sweep range reading a parameter or the temperature (W2313, R68: goes with A42's feature and
/// invariant).
pub(super) fn admit(
    design: &ir::Design,
    at: ir::ItemRef,
    scope: ir::ScopeId,
    diags: &mut ir::Diags,
) {
    if args_read_params(design, at) {
        diags.push_at(ARG_READS_PARAM, design, at, 0);
    }
    if output_unspelled(design, at) {
        diags.push_at(OUTPUT_FORM, design, at, 0);
    }
    if is_mc(design, at) && within_mc(design, scope) {
        diags.push_at(MC_NESTED, design, at, 0);
    }
    if at.kind == ir::ItemKind::NodeSet && !on_an_analysis(design, scope, at) {
        diags.push_at(NODE_SET_PLACE, design, at, 0);
    }
    if at.kind == ir::ItemKind::Option
        && design.row::<ir::OptionRow>(at.index).key == ir::OptionKey::Private
    {
        diags.push_at(OPTION_ROW_PRIVATE, design, at, 0);
    }
    if !names_spelled(design, at) {
        diags.push_at(NAME_SPELLING, design, at, 0);
    }
    for t in swept_targets(design, at) {
        match t.kind {
            ir::TargetKind::SourceDc => match source_of(design, t) {
                None => diags.push_at(SOURCE_SWEEP_NAME, design, at, 0),
                Some(src) if src.dc_rule == DcRule::WaveAtZero && !src.wave.is_none() => {
                    diags.push_at(SOURCE_SWEEP_WAVE, design, at, 0);
                }
                Some(_) => {}
            },
            ir::TargetKind::DeviceParam if element_of(design, t).is_none() => {
                diags.push_at(TARGET_UNFOUND, design, at, 0);
            }
            ir::TargetKind::Param if !top_param(design, t) => {
                diags.push_at(TARGET_UNFOUND, design, at, 0);
            }
            _ => {}
        }
    }
    let saves = ir::ResetSet::of(&[ir::Restorable::Saves]);
    if at.kind == ir::ItemKind::Reset && design.row::<ir::Reset>(at.index).restores != saves {
        diags.push_at(RESET_CLASS, design, at, 0);
    }
}

/// The hierarchical name of `path` (`x1:r1`).
pub(super) fn name_of(design: &ir::Design, path: ir::NameList) -> String {
    let names: Vec<&str> = design.names(path).iter().map(|&n| design.str(n)).collect();
    names.join(":")
}

/// R42's framing before analysis `at` (a top-level one): for each `Given` source with a wave
/// (`fold::given`, item order), `alter instance("<name>") type="<word>"` before a transient
/// and `type="dc"` before a DC-kind analysis, when the side differs from the previous top-level
/// analysis's (the line starts at dc). Contract: S4-AREAS.md "dc plus type alters".
pub(super) fn fold_alters(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let Some(&scope) = w.scopes.get(&at) else {
        return;
    };
    let top = d.file(d.scope(scope).file).top;
    if scope != top {
        return;
    }
    let sources = fold::given(d, top);
    if sources.is_empty() {
        return;
    }
    let wave = |a: ir::ItemRef| d.row::<ir::Analysis>(a.index).kind == ir::AnalysisKind::Tran;
    let items = d.items(top);
    let k = items
        .iter()
        .position(|&i| i == at)
        .expect("listed in its scope");
    let before = items[..k]
        .iter()
        .rev()
        .find(|i| i.kind == ir::ItemKind::Analysis)
        .is_some_and(|&a| wave(a));
    let now = wave(at);
    if before == now {
        return;
    }
    for (source, word) in sources {
        let row: ir::Element = d.row(source.index);
        w.line();
        w.out.push_str("alter instance(");
        quote(d.str(row.name), w.out);
        w.out.push_str(") type=");
        quote(if now { word } else { "dc" }, w.out);
        w.out.push('\n');
    }
}

/// Every item of `scope` (a loop's body), one level deeper.
pub(super) fn body(w: &mut Writer<'_, '_>, scope: ir::ScopeId) {
    let d = w.design;
    w.depth += 1;
    for &at in d.items(scope) {
        match at.kind {
            ir::ItemKind::Comment => write_structure::comment(w, d.row(at.index)),
            ir::ItemKind::Opaque => write_structure::opaque(w, d.row(at.index)),
            _ => analysis(w, at),
        }
    }
    w.depth -= 1;
}

// ---- private ----

/// Whether analysis `at`'s own fields (a DC analysis's axes included) or sweep loop `at`'s range
/// read a parameter, `$temp` or `$tnom`, which VACASK's analysis arguments cannot see (R68).
fn args_read_params(design: &ir::Design, at: ir::ItemRef) -> bool {
    let sweep_loop = at.kind == ir::ItemKind::Loop
        && design.row::<ir::Loop>(at.index).kind == ir::LoopKind::Sweep;
    if at.kind != ir::ItemKind::Analysis && !sweep_loop {
        return false;
    }
    let mut handles = Vec::new();
    design.item_handles(at, &mut handles);
    let mut todo: Vec<ir::ExprId> = Vec::new();
    for h in handles {
        match h {
            ir::Handle::Expr(e) | ir::Handle::ExprDyn(e) => todo.push(e),
            ir::Handle::Exprs(list) => todo.extend_from_slice(design.exprs(list)),
            _ => {}
        }
    }
    // Bounded: each expression row is taken once per parent that reads it (a tree).
    while let Some(e) = todo.pop() {
        if e.is_none() {
            continue;
        }
        match design.expr(e) {
            ir::Expr::Param(_) | ir::Expr::Temp | ir::Expr::Tnom => return true,
            ir::Expr::Unary(_, x) => todo.push(x),
            ir::Expr::Binary(_, a, b) => todo.extend([a, b]),
            ir::Expr::Ternary(c, a, b) => todo.extend([c, a, b]),
            ir::Expr::Call(_, args)
            | ir::Expr::UserCall(_, args)
            | ir::Expr::Draw(_, _, _, args)
            | ir::Expr::List(args) => todo.extend_from_slice(design.exprs(args)),
            _ => {}
        }
    }
    false
}

/// Whether analysis `at` is a `dcxf`, `acxf` or `noise` whose output is neither a node nor a
/// node pair (VD cmd-analysis-dcxf, -acxf, -noise: `out` names one node or two).
fn output_unspelled(design: &ir::Design, at: ir::ItemRef) -> bool {
    if at.kind != ir::ItemKind::Analysis {
        return false;
    }
    let row: ir::Analysis = design.row(at.index);
    let probe = match row.kind {
        ir::AnalysisKind::DcXf => design.detail::<ir::dc_ac::DcXf>(row.detail).output,
        ir::AnalysisKind::Xf => design.detail::<ir::dc_ac::Xf>(row.detail).output,
        ir::AnalysisKind::Noise => design.detail::<ir::dc_ac::Noise>(row.detail).output,
        _ => return false,
    };
    !matches!(probe.kind, ir::ProbeKind::V | ir::ProbeKind::Vdiff)
}

/// Whether item `at` is a Monte Carlo loop.
fn is_mc(design: &ir::Design, at: ir::ItemRef) -> bool {
    at.kind == ir::ItemKind::Loop
        && design.row::<ir::Loop>(at.index).kind == ir::LoopKind::MonteCarlo
}

/// Whether `scope` lies in the body of a Monte Carlo loop.
fn within_mc(design: &ir::Design, scope: ir::ScopeId) -> bool {
    let mut at = scope;
    // Bounded: each step moves to a parent; the scope tree is acyclic (`ir::check`).
    for _ in 0..1 << 16 {
        if at.is_none() {
            return false;
        }
        let s = design.scope(at);
        if s.kind == ir::ScopeKind::Loop && is_mc(design, s.owner) {
            return true;
        }
        at = s.parent;
    }
    false
}

/// Whether node set `at` stands in the run of node sets just before an analysis whose line
/// takes its list (`analyses::lists`; VD cmd-analysis-*: `ic=` on `tran`, `nodeset=` on `op`,
/// its sweeps, `dcinc`, `dcxf` and `tran`).
fn on_an_analysis(design: &ir::Design, scope: ir::ScopeId, at: ir::ItemRef) -> bool {
    use ir::AnalysisKind as A;
    let items = design.items(scope);
    let Some(k) = items.iter().position(|&i| i == at) else {
        return false;
    };
    let Some(next) = items[k..].iter().find(|i| i.kind != ir::ItemKind::NodeSet) else {
        return false;
    };
    if next.kind != ir::ItemKind::Analysis {
        return false;
    }
    let kind = design.row::<ir::Analysis>(next.index).kind;
    match design.row::<ir::NodeSet>(at.index).kind {
        ir::NodeSetKind::Ic => kind == A::Tran,
        ir::NodeSetKind::Nodeset => matches!(kind, A::Op | A::Dc | A::DcInc | A::DcXf | A::Tran),
        ir::NodeSetKind::Device => false,
    }
}

/// The targets a DC analysis's axes, a sweep loop or an alter set; none for any other item.
fn swept_targets(design: &ir::Design, at: ir::ItemRef) -> Vec<ir::Target> {
    match at.kind {
        ir::ItemKind::Analysis => {
            let row: ir::Analysis = design.row(at.index);
            if row.kind != ir::AnalysisKind::Dc {
                return Vec::new();
            }
            let dc: ir::dc_ac::Dc = design.detail(row.detail);
            design.dc_axes(dc.axes).iter().map(|a| a.target).collect()
        }
        ir::ItemKind::Alter => vec![design.row::<ir::Alter>(at.index).target],
        ir::ItemKind::Loop => {
            let row: ir::Loop = design.row(at.index);
            match row.kind {
                ir::LoopKind::Sweep => {
                    vec![design.detail::<ir::plan::SweepLoop>(row.detail).target]
                }
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// The element a target names, found from the top level (every file's top) through each
/// instance's bound
/// subcircuit, as VACASK's `instance="x1:v1"` finds it (the reader's
/// `masters::Declared::has_instance`); `None` when a name on the way is no element.
fn element_of(design: &ir::Design, t: ir::Target) -> Option<ir::Element> {
    let named = |scope: ir::ScopeId, name: ir::NameId| {
        design.items(scope).iter().copied().find(|i| {
            i.kind == ir::ItemKind::Element && design.row::<ir::Element>(i.index).name == name
        })
    };
    // VACASK's include is textual: the top of every file is the one top level.
    let mut scopes: Vec<ir::ScopeId> = design.files().map(|f| design.file(f).top).collect();
    // An elaborated subcircuit's instances stand at the top level too (VD cir-elaboration).
    for (at, _) in design.reachable() {
        if at.kind == ir::ItemKind::Elaborate {
            let row: ir::Elaborate = design.row(at.index);
            for r in design.refs(row.circuits) {
                if !r.def.is_none() && design.def(r.def).kind == ir::DefKind::Subckt {
                    let sub: ir::item::Subckt = design.row(design.def(r.def).item.index);
                    scopes.push(sub.body);
                }
            }
        }
    }
    // Bounded by the path's length.
    for &inst in design.names(t.path) {
        let at = scopes.iter().find_map(|&sc| named(sc, inst))?;
        let model = design.row::<ir::Element>(at.index).model.def;
        if model.is_none() || design.def(model).kind != ir::DefKind::Subckt {
            return None;
        }
        scopes = vec![
            design
                .row::<ir::item::Subckt>(design.def(model).item.index)
                .body,
        ];
    }
    let at = scopes.iter().find_map(|&sc| named(sc, t.a.name))?;
    Some(design.row(at.index))
}

/// The independent source a source-DC target names (`element_of`), or `None`.
fn source_of(design: &ir::Design, t: ir::Target) -> Option<ir::source::Source> {
    let row = element_of(design, t)?;
    matches!(row.kind, ir::ElemKind::VSource | ir::ElemKind::ISource).then(|| design.source(&row))
}

/// Whether parameter target `t` is bound to a parameter of a file's top level, which
/// VACASK's `__topinst__` names (R70; settle run/top_nosuch: "Parameter 'zz' not found.").
fn top_param(design: &ir::Design, t: ir::Target) -> bool {
    !t.a.def.is_none() && design.scope(design.def(t.a.def).scope).kind == ir::ScopeKind::File
}

/// Whether `at` is a run item.
fn run_item(at: ir::ItemRef) -> bool {
    matches!(
        at.kind,
        ir::ItemKind::Analysis
            | ir::ItemKind::Loop
            | ir::ItemKind::Alter
            | ir::ItemKind::Option
            | ir::ItemKind::Env
            | ir::ItemKind::NodeSet
            | ir::ItemKind::Save
            | ir::ItemKind::Print
            | ir::ItemKind::Measure
            | ir::ItemKind::Elaborate
            | ir::ItemKind::Reset
    )
}

/// The class of `at` when it is a `vacask` opaque whose head word is a control command.
fn command_opaque(design: &ir::Design, at: ir::ItemRef) -> Option<ir::OpaqueClass> {
    if at.kind != ir::ItemKind::Opaque {
        return None;
    }
    let row: ir::item::Opaque = design.row(at.index);
    let text = design.text(row.text);
    let head = text
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("");
    (design.str(row.dialect) == "vacask" && COMMANDS.contains(&head)).then_some(row.class)
}

/// Whether `at`'s own names spell (settle names, names_quoted, names_chain): an analysis's, a
/// loop's and each DC axis's name, when set, is an identifier; the sweep lines of one chain (a
/// `Dc`'s axes, a loop and the loops and `Dc` that are its whole body) have distinct names.
fn names_spelled(design: &ir::Design, at: ir::ItemRef) -> bool {
    let ok = |n: ir::NameId| n.is_none() || identifier(design.str(n));
    match at.kind {
        ir::ItemKind::Analysis => {
            let row: ir::Analysis = design.row(at.index);
            if row.kind != ir::AnalysisKind::Dc {
                return ok(row.name);
            }
            let dc: ir::dc_ac::Dc = design.detail(row.detail);
            let names: Vec<ir::NameId> = design.dc_axes(dc.axes).iter().map(|a| a.name).collect();
            let distinct = names
                .iter()
                .enumerate()
                .all(|(k, n)| n.is_none() || !names[..k].contains(n));
            ok(row.name) && names.iter().all(|&n| ok(n)) && distinct
        }
        ir::ItemKind::Loop => {
            // The chain of sweep lines this loop heads: the loops whose body is one loop, then
            // a `Dc`'s axes; every name in it distinct (bounded by the loop depth, LOOP_DEPTH_MAX).
            let mut names = Vec::new();
            let mut item = at;
            for _ in 0..=ir::plan::LOOP_DEPTH_MAX {
                match item.kind {
                    ir::ItemKind::Loop => {
                        let row: ir::Loop = design.row(item.index);
                        names.push(row.name);
                        match design.items(row.body) {
                            [only] => item = *only,
                            _ => break,
                        }
                    }
                    ir::ItemKind::Analysis => {
                        let row: ir::Analysis = design.row(item.index);
                        if row.kind == ir::AnalysisKind::Dc {
                            let dc: ir::dc_ac::Dc = design.detail(row.detail);
                            names.extend(design.dc_axes(dc.axes).iter().map(|a| a.name));
                        }
                        break;
                    }
                    _ => break,
                }
            }
            let distinct = names
                .iter()
                .enumerate()
                .all(|(k, n)| n.is_none() || !names[..k].contains(n));
            ok(design.row::<ir::Loop>(at.index).name) && distinct
        }
        _ => true,
    }
}
