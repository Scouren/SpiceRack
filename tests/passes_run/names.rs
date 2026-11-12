//! The naming rows' property helpers (`names_given_name_every_statement_of_a_kind_once`): a
//! design's statement names by kind read from its rows, the rule run and checked, a planted
//! name.
//!
//! Contents: `pub fn statement_names(&ir::Design, ir::RunExtra) -> Vec<(ir::ItemRef, ir::NameId)>`,
//! `pub fn named_once(&passes::Pass, ir::RunExtra, &mut ir::Design, u64) -> Option<String>`,
//! `pub fn rename_first_named(&mut ir::Design, ir::RunExtra, &str) -> bool`

use spicerack::ir;
use spicerack::passes;

/// Every reachable statement of the kind `feature` names, with its name: an analysis, a DC
/// analysis's axis (at the analysis), a loop.
pub fn statement_names(
    design: &ir::Design,
    feature: ir::RunExtra,
) -> Vec<(ir::ItemRef, ir::NameId)> {
    let mut out = Vec::new();
    for (item, _) in design.reachable() {
        match (feature, item.kind) {
            (ir::RunExtra::AnalysisUnnamed, ir::ItemKind::Analysis) => {
                out.push((item, design.row::<ir::Analysis>(item.index).name));
            }
            (ir::RunExtra::DcAxisUnnamed, ir::ItemKind::Analysis) => {
                let a = design.row::<ir::Analysis>(item.index);
                if a.kind == ir::AnalysisKind::Dc {
                    let dc = design.detail::<ir::dc_ac::Dc>(a.detail);
                    out.extend(design.dc_axes(dc.axes).iter().map(|x| (item, x.name)));
                }
            }
            (ir::RunExtra::LoopUnnamed, ir::ItemKind::Loop) => {
                out.push((item, design.row::<ir::plan::Loop>(item.index).name));
            }
            _ => {}
        }
    }
    out
}

/// `pass` on `design`, then the property of `names_given_name_every_statement_of_a_kind_once`
/// asserted; the first name it gave.
pub fn named_once(
    pass: &passes::Pass,
    feature: ir::RunExtra,
    design: &mut ir::Design,
    seed: u64,
) -> Option<String> {
    let name = pass.name;
    let mut diags = ir::Diags::new();
    (pass.run)(design, &mut diags);
    let left: Vec<ir::ItemRef> = diags
        .rows()
        .iter()
        .filter(|d| pass.notes.contains(&d.code))
        .map(|d| d.item)
        .collect();
    let named = statement_names(design, feature);
    for (item, n) in &named {
        let unnamed = *n == ir::NameId::NONE;
        assert_eq!(unnamed, left.contains(item), "{name} seed {seed}: {item:?}");
    }
    // DC axes and loops are one set of names: both are sweeps on VACASK (`names_chain.sim`)
    let one_set = match feature {
        ir::RunExtra::AnalysisUnnamed => named.clone(),
        _ => [ir::RunExtra::DcAxisUnnamed, ir::RunExtra::LoopUnnamed]
            .iter()
            .flat_map(|&k| statement_names(design, k))
            .collect(),
    };
    let mut seen = std::collections::HashSet::new();
    for (_, n) in one_set.iter().filter(|(_, n)| *n != ir::NameId::NONE) {
        let fresh = seen.insert(design.str(*n).to_ascii_lowercase());
        assert!(fresh, "{name} seed {seed}: {} twice", design.str(*n));
    }
    let given: Vec<&ir::Diag> = diags
        .rows()
        .iter()
        .filter(|d| !pass.notes.contains(&d.code))
        .collect();
    for d in &given {
        let at = named
            .iter()
            .any(|(i, n)| *i == d.item && *n == ir::NameId(d.arg));
        assert!(at, "{name} seed {seed}: a note's name is not at its item");
    }
    given
        .first()
        .map(|d| design.str(ir::NameId(d.arg)).to_string())
}

/// Renames the first named statement of the kind `feature` names to `to`; false when none is
/// named.
pub fn rename_first_named(design: &mut ir::Design, feature: ir::RunExtra, to: &str) -> bool {
    let Some((item, _)) = statement_names(design, feature)
        .into_iter()
        .find(|(_, n)| *n != ir::NameId::NONE)
    else {
        return false;
    };
    let mut editor = design.editor();
    let name = editor.builder().name(to);
    match feature {
        ir::RunExtra::AnalysisUnnamed => {
            let mut a = editor.design().row::<ir::Analysis>(item.index);
            a.name = name;
            editor.set(item.index, a);
        }
        ir::RunExtra::DcAxisUnnamed => {
            let a = editor.design().row::<ir::Analysis>(item.index);
            let mut dc = editor.design().detail::<ir::dc_ac::Dc>(a.detail);
            let mut axes = editor.design().dc_axes(dc.axes).to_vec();
            let x = axes
                .iter_mut()
                .find(|x| x.name != ir::NameId::NONE)
                .expect("a named axis");
            x.name = name;
            dc.axes = editor.builder().dc_axes(&axes);
            editor.set_detail(a.detail, dc);
        }
        _ => {
            let mut l = editor.design().row::<ir::plan::Loop>(item.index);
            l.name = name;
            editor.set(item.index, l);
        }
    }
    true
}
