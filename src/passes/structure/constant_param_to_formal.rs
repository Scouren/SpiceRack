//! Pass `constant_param_to_formal`: a design in, the same design out with every `Param` listed
//! directly in a subcircuit's body whose value is a constant made a formal of that subcircuit,
//! its value the default, appended to the head in body order. It removes
//! `Extra::SubcktConstantParam` (VACASK reads `parameters x=1` in a body as a formal, ngspice's
//! `.param` there is none: A25, R48) and requires nothing: the default is a constant.
//!
//! Exact while no instance passes an actual of that name (none can override a local today);
//! a subcircuit an instance passes such a name to keeps the parameter with a `PASSED_LOCAL`
//! note, since the formal would take the actual.
//!
//! Cites: `rewrite/AMEND-A12.md` A25; R48.
//!
//! Contents: `pub fn constant_param_to_formal(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const PASSED_LOCAL: ir::Code`

use crate::ir;

/// The note at a subcircuit-body parameter an instance passes an actual for.
pub const PASSED_LOCAL: ir::Code = ir::Code::new(b'U', 310);

/// Makes each constant body parameter a formal of its subcircuit.
pub fn constant_param_to_formal(design: &mut ir::Design, diags: &mut ir::Diags) {
    let feature = ir::Feature::Extra(ir::Extra::SubcktConstantParam);
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    let items: Vec<ir::ItemRef> = rows
        .rows()
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == index)
        .map(|d| d.item)
        .collect();
    if items.is_empty() {
        return;
    }
    let reachable = design.reachable();
    for item in items {
        let body = reachable
            .iter()
            .find(|p| p.0 == item)
            .map(|p| p.1)
            .expect("listed");
        let subckt = design.scope(body).owner;
        assert!(
            subckt.kind == ir::ItemKind::Subckt,
            "a constant parameter of a body"
        );
        let p = design.row::<ir::item::Param>(item.index);
        let def = design.lookup(body, ir::DefKind::Param, p.name);
        assert!(
            design.def(def).item == item,
            "the parameter defines its name"
        );
        let passed = reachable.iter().any(|&(i, _)| {
            i.kind == ir::ItemKind::Element && {
                let e = design.row::<ir::Element>(i.index);
                e.model.is_resolved()
                    && design.def(e.model.def).item == subckt
                    && design.params(e.params).iter().any(|a| a.0 == p.name)
            }
        });
        if passed {
            diags.push_at(PASSED_LOCAL, design, item, 0);
            continue;
        }
        let mut s = design.row::<ir::item::Subckt>(subckt.index);
        let mut head: Vec<(String, ir::ExprId)> = design
            .params(s.params)
            .iter()
            .map(|&(n, v)| (design.str(n).to_string(), v))
            .collect();
        head.push((design.str(p.name).to_string(), p.value));
        let mut editor = design.editor();
        let pairs: Vec<(&str, ir::ExprId)> = head.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        s.params = editor.builder().params(&pairs);
        editor.set(subckt.index, s);
        editor.redefine(def, subckt);
        editor.remove_item(body, item); // its definition stays: its item is the subcircuit now
    }
}
