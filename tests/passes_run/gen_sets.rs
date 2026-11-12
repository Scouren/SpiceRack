//! The run rules' generation sets, shared by the laws (`pass_laws.rs` `GEN_SETS`) and the
//! rules' own tests.
//!
//! Contents: `pub const fn dc_loops(ir::RunExtra) -> ir::FeatureSet`

use spicerack::ir;

/// A deck of restarting DC analyses with an axis of `axis` among source and list axes: the
/// designs `dc_axes_to_loops` converts (over every feature a competing continued sweep wins).
/// The naming members are needed: without `AnalysisNamed` or `AnalysisUnnamed` the generator
/// makes no analysis at all (`analysis_facts.rs`), so a set without them reaches nothing.
pub const fn dc_loops(axis: ir::RunExtra) -> ir::FeatureSet {
    ir::FeatureSet::EMPTY
        .with(ir::Feature::Item(ir::ItemKind::Title))
        .with(ir::Feature::Item(ir::ItemKind::Analysis))
        .with(ir::Feature::Item(ir::ItemKind::Model))
        .with(ir::Feature::Item(ir::ItemKind::Element))
        .with(ir::Feature::Elem(ir::ElemKind::Resistor))
        .with(ir::Feature::Elem(ir::ElemKind::VSource))
        .with(ir::Feature::Family(ir::ModelFamily::Resistor))
        .with(ir::Feature::Expr(ir::ExprTag::Num))
        .with(ir::Feature::Analysis(ir::AnalysisKind::Dc))
        .with(ir::Feature::Run(ir::RunExtra::DcRestart))
        .with(ir::Feature::Run(ir::RunExtra::DcNested))
        .with(ir::Feature::Run(axis))
        .with(ir::Feature::Run(ir::RunExtra::SweepDcLinStep))
        .with(ir::Feature::Run(ir::RunExtra::SweepDcList))
        .with(ir::Feature::Run(ir::RunExtra::AnalysisNamed))
        .with(ir::Feature::Run(ir::RunExtra::AnalysisUnnamed))
        .with(ir::Feature::Run(ir::RunExtra::DcAxisNamed))
        .with(ir::Feature::Run(ir::RunExtra::DcAxisUnnamed))
}
