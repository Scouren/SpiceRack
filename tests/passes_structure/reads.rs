//! What the structure passes' tests read from a design through the IR's own doors: each
//! item's references and parameter reads, the instances of a subcircuit, the pair decks'
//! `proves =` lines.
//!
//! Contents: `pub fn feature`, `pub fn refs`, `pub fn param_reads`, `pub fn instances_pass`,
//! `pub fn elements`, `pub fn added_library`, `pub fn model_row`, `pub fn proven_by_pair_decks`,
//! `pub type Run`, `pub fn instance_actual(&str, Run, &str, &str) -> Result<f64, ir::Domain>`

use std::path::Path;

use spicerack::ir;

/// A pass's run function.
pub type Run = fn(&mut ir::Design, &mut ir::Diags);

pub fn feature(x: ir::CircuitExtra) -> ir::Feature {
    ir::Feature::Circuit(x)
}

/// Every reachable item with the references it holds.
pub fn refs(design: &ir::Design) -> Vec<(ir::ItemRef, Vec<(ir::Ref, ir::DefKind)>)> {
    let reachable = design.reachable().into_iter();
    reachable
        .map(|(item, _)| {
            let mut out = Vec::new();
            design.item_refs(item, &mut out);
            (item, out)
        })
        .collect()
}

/// Every bound parameter read of every reachable item's expressions, in item and tree order:
/// (item, definition).
pub fn param_reads(design: &ir::Design) -> Vec<(ir::ItemRef, ir::DefId)> {
    let mut out = Vec::new();
    let mut handles = Vec::new();
    for (item, _) in design.reachable() {
        handles.clear();
        design.item_handles(item, &mut handles);
        let mut stack: Vec<ir::ExprId> = Vec::new();
        for h in &handles {
            match *h {
                ir::Handle::Expr(e) | ir::Handle::ExprDyn(e) => stack.push(e),
                ir::Handle::Exprs(l) => stack.extend_from_slice(design.exprs(l)),
                ir::Handle::Params(l) => stack.extend(design.params(l).iter().map(|p| p.1)),
                _ => {}
            }
        }
        stack.retain(|e| !e.is_none());
        stack.reverse();
        // bounded: a tree's rows lie below its root, each visited once per path
        while let Some(e) = stack.pop() {
            match design.expr(e) {
                ir::Expr::Param(r) if r.is_resolved() => out.push((item, r.def)),
                ir::Expr::Call(_, l)
                | ir::Expr::UserCall(_, l)
                | ir::Expr::Draw(_, _, _, l)
                | ir::Expr::List(l) => stack.extend(design.exprs(l).iter().rev()),
                ir::Expr::Unary(_, a) => stack.push(a),
                ir::Expr::Binary(_, a, b) => stack.extend([b, a]),
                ir::Expr::Ternary(c, a, b) => stack.extend([b, a, c]),
                _ => {}
            }
        }
    }
    out
}

/// Whether every reachable instance of the subcircuit `subckt` passes `name` as a read of `old`.
pub fn instances_pass(
    design: &ir::Design,
    subckt: ir::ItemRef,
    name: ir::NameId,
    old: ir::DefId,
) -> bool {
    elements(design).all(|i| {
        let e = design.row::<ir::Element>(i.index);
        let of = e.kind == ir::ElemKind::Instance
            && e.model.is_resolved()
            && design.def(e.model.def).item == subckt;
        !of || design.params(e.params).iter().any(|&(n, v)| {
            n == name && matches!(design.expr(v), ir::Expr::Param(r) if r.def == old)
        })
    })
}

pub fn elements(design: &ir::Design) -> impl Iterator<Item = ir::ItemRef> + '_ {
    let reachable = design.reachable().into_iter();
    reachable
        .map(|(i, _)| i)
        .filter(|i| i.kind == ir::ItemKind::Element)
}

/// Whether `item` is a library the pass added (no item of `before`).
pub fn added_library(
    before: &[(ir::ItemRef, Vec<(ir::Ref, ir::DefKind)>)],
    item: ir::ItemRef,
) -> bool {
    item.kind == ir::ItemKind::DeviceLibrary && !before.iter().any(|(i, _)| *i == item)
}

pub fn model_row(
    name: ir::NameId,
    family: ir::ModelFamily,
    polarity: ir::Polarity,
    module: ir::Ref,
    word: Option<ir::NameId>,
) -> ir::Model {
    ir::Model {
        name,
        params: ir::ParamList::EMPTY,
        bin_of: ir::NameId::NONE,
        ako: ir::Ref::unresolved(ir::NameId::NONE),
        module,
        version: ir::ExprId::NONE,
        word: word.unwrap_or(ir::NameId::NONE),
        family,
        polarity,
    }
}

/// The passes some pair deck says it proves (`proves = <pass>` in a `.pair` file).
pub fn proven_by_pair_decks() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    let mut out = Vec::new();
    let cells = std::fs::read_dir(root).into_iter().flatten().flatten();
    for cell in cells.filter(|c| c.path().is_dir()) {
        for deck in std::fs::read_dir(cell.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if deck.path().extension().is_some_and(|e| e == "pair") {
                let text = std::fs::read_to_string(deck.path()).unwrap_or_default();
                for line in text.lines() {
                    let line = line.split('#').next().unwrap_or("");
                    if let Some((k, v)) = line.split_once('=')
                        && k.trim() == "proves"
                    {
                        out.push(v.trim().to_string());
                    }
                }
            }
        }
    }
    out
}

/// The fixture `tests/fixtures/passes/structure/<case>.ir` read and passed by `run`: the
/// actual instance `inst` passes for `formal`, evaluated with every read bound by its
/// definition.
///
/// # Panics
/// When the case does not read clean, or `inst` passes no `formal`.
pub fn instance_actual(case: &str, run: Run, inst: &str, formal: &str) -> Result<f64, ir::Domain> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/passes/structure/{case}.ir"));
    let text = std::fs::read_to_string(path).expect(case);
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(&text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{case}: {:?}", diags.rows());
    run(&mut design, &mut diags);
    let e = elements(&design)
        .map(|i| design.row::<ir::Element>(i.index))
        .find(|e| design.str(e.name) == inst)
        .expect("the instance");
    let actual = design
        .params(e.params)
        .iter()
        .find(|p| design.str(p.0) == formal);
    let actual = actual
        .unwrap_or_else(|| panic!("{case}: {inst} passes no {formal}"))
        .1;
    ir::eval(&design, actual, &crate::bindings::ByDefinition(&design))
}
