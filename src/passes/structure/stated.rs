//! The **stated** rule body (R92, R90): a law point whose variant names the value an absent
//! parameter means. Toward a target that lacks the design's variant, each carrier gets that
//! value written as the parameter, after which nothing carries the point; `legalize` then
//! restates the design's laws as the target's (R90a). One registry row per stated variant (the
//! shape of `run/names_dropped.rs`): `stated_xom_1e4`, `stated_xom_1e14`, `stated_kp_35`,
//! `stated_kp_20`, each removing its variant's `Circuit` member and requiring nothing (a model
//! parameter is no feature).
//!
//! Exact: under the variant an absent parameter means `Stated::absent(model)`, the model's own
//! row's value (R92); written out, the parameter means that value under every law. A carrier
//! that is no model row (no stated point has one today) keeps the member with `STATED_KEPT`.
//!
//! Cites: R90, R90a, R92; `crates/ir/src/laws.rs` (`Rewrite::Stated`, `Stated`),
//! `laws_table.rs`.
//!
//! Contents: `pub fn stated_xom_1e4`, `pub fn stated_xom_1e14`, `pub fn stated_kp_35`,
//! `pub fn stated_kp_20` (each `(&mut ir::Design, &mut ir::Diags)`),
//! `pub const fn row(ir::Law, &'static str, fn(&mut ir::Design, &mut ir::Diags)) -> Pass`,
//! `pub const STATED_KEPT: ir::Code`

use crate::ir;
use crate::passes::Pass;

/// The note at a carrier of a stated point that is no model row.
pub const STATED_KEPT: ir::Code = ir::Code::new(b'U', 315);

/// F21, ngspice's `xom`/`xoi` absent as 1e4.
pub fn stated_xom_1e4(design: &mut ir::Design, diags: &mut ir::Diags) {
    stated(design, diags, ir::Law::DiodeXomAbsent1e4);
}

/// F21, the VACASK module's `xom`/`xoi` absent as 1e14.
pub fn stated_xom_1e14(design: &mut ir::Design, diags: &mut ir::Diags) {
    stated(design, diags, ir::Law::DiodeXomAbsent1e14);
}

/// F24, ngspice's VDMOS `kp` absent as 35 (n) or 15 (p).
pub fn stated_kp_35(design: &mut ir::Design, diags: &mut ir::Diags) {
    stated(design, diags, ir::Law::VdmosKpAbsent35);
}

/// F24, the VACASK module's VDMOS `kp` absent as 20 (n) or 10 (p).
pub fn stated_kp_20(design: &mut ir::Design, diags: &mut ir::Diags) {
    stated(design, diags, ir::Law::VdmosKpAbsent20);
}

/// The registry row of the stated rule `run` for `law` (asserted to be a stated variant).
pub const fn row(
    law: ir::Law,
    name: &'static str,
    run: fn(&mut ir::Design, &mut ir::Diags),
) -> Pass {
    assert!(
        matches!(law.rewrite(), ir::Rewrite::Stated(_)),
        "a stated row of a stated variant"
    );
    Pass {
        name,
        removes: ir::Feature::Circuit(law.member()),
        requires: ir::FeatureSet::EMPTY,
        notes: &[STATED_KEPT],
        run,
    }
}

// ---- private ----

/// Writes every absent stated parameter of `law` on each model that carries its member.
fn stated(design: &mut ir::Design, diags: &mut ir::Diags, law: ir::Law) {
    let ir::Rewrite::Stated(params) = law.rewrite() else {
        unreachable!("row() admits stated variants only")
    };
    for item in carrying(design, ir::Feature::Circuit(law.member())) {
        if item.kind != ir::ItemKind::Model {
            diags.push_at(STATED_KEPT, design, item, 0);
            continue;
        }
        let model = design.row::<ir::Model>(item.index);
        let mut list: Vec<(String, ir::ExprId)> = design
            .params(model.params)
            .iter()
            .map(|&(n, v)| (design.str(n).to_string(), v))
            .collect();
        let given = |list: &[(String, ir::ExprId)], names: &[&str]| {
            list.iter().any(|(n, _)| names.contains(&n.as_str()))
        };
        let mut editor = design.editor();
        for s in params {
            if given(&list, s.names) {
                continue;
            }
            let value = editor.builder().num((s.absent)(&model));
            list.push((s.names[0].to_string(), value));
        }
        let pairs: Vec<(&str, ir::ExprId)> = list.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        let params = editor.builder().params(&pairs);
        editor.set(item.index, ir::Model { params, ..model });
    }
}

/// The items that carry `feature` (`ir::check_legal` against every feature but it).
fn carrying(design: &ir::Design, feature: ir::Feature) -> Vec<ir::ItemRef> {
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    let rows = rows.rows().iter();
    rows.filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == index)
        .map(|d| d.item)
        .collect()
}
