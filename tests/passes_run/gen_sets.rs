//! The run rules' generation sets, shared by the laws (`pass_laws.rs` `GEN_SETS`) and the
//! rules' own tests.
//!
//! Contents: `pub const fn dc_loops(ir::RunExtra) -> ir::FeatureSet`,
//! `pub const PARAM_ARGS: ir::FeatureSet`, `pub const fn one_kind_without(ir::ElemKind, ir::Law) -> ir::FeatureSet`, `pub const fn behav_reads() -> ir::FeatureSet`,
//! `pub const fn sources_only(&[ir::WaveKind], ir::Feature) -> ir::FeatureSet` (the waves
//! rows' set, moved here from `pass_laws.rs` to keep it under 500 lines)

use spicerack::ir;

/// A deck of transients whose arguments read top-level parameters, nothing altering them: the
/// designs `analysis_param_args_to_values` writes out (over every feature an alter, a loop or a
/// statistics block always stands somewhere).
pub const PARAM_ARGS: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Item(ir::ItemKind::Title))
    .with(ir::Feature::Item(ir::ItemKind::Param))
    .with(ir::Feature::Item(ir::ItemKind::Analysis))
    .with(ir::Feature::Expr(ir::ExprTag::Num))
    .with(ir::Feature::Expr(ir::ExprTag::Param))
    .with(ir::Feature::Analysis(ir::AnalysisKind::Tran))
    .with(ir::Feature::Run(ir::RunExtra::TranCapSpan10))
    .with(ir::Feature::Run(ir::RunExtra::TranPrintStep))
    .with(ir::Feature::Run(ir::RunExtra::AnalysisUnnamed))
    .with(ir::Feature::Run(ir::RunExtra::AnalysisParamArgument));

/// Every feature but the element kinds other than the behavioural source and the items that
/// give a parameter a value the tag table cannot fix (a loop, an alter, a subcircuit's actuals,
/// a statistics block, a function's formal) or hide a read's definition (an include, a section,
/// an `if`): the designs `param_reads_to_typed` rewrites (over every feature the generator
/// reaches few behavioural sources; measured: 16 of 200 seeds changed, against 0 with the
/// element kinds alone).
pub const fn behav_reads() -> ir::FeatureSet {
    let mut set = ir::FeatureSet::ALL;
    let mut i = 0;
    while i < ir::ElemKind::ALL.len() {
        let k = ir::ElemKind::ALL[i];
        if k as u8 != ir::ElemKind::Behav as u8 {
            set = set.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Elem(k)));
        }
        i += 1;
    }
    let items = [
        ir::ItemKind::Loop,
        ir::ItemKind::Alter,
        ir::ItemKind::Subckt,
        ir::ItemKind::Statistics,
        ir::ItemKind::Func,
        ir::ItemKind::Include,
        ir::ItemKind::Section,
        ir::ItemKind::If,
    ];
    let mut j = 0;
    while j < items.len() {
        set = set.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Item(items[j])));
        j += 1;
    }
    set
}

/// Every feature but the element kinds other than `kind` and the law variant `other`: the
/// designs a stated rule of `other`'s point rewrites (the diode's `xom`: over every feature no
/// seed of 96 carried either variant, with the diode alone 5 each).
pub const fn one_kind_without(kind: ir::ElemKind, other: ir::Law) -> ir::FeatureSet {
    let mut set =
        ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Circuit(other.member())));
    let mut i = 0;
    while i < ir::ElemKind::ALL.len() {
        let k = ir::ElemKind::ALL[i];
        if k as u8 != kind as u8 {
            set = set.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Elem(k)));
        }
        i += 1;
    }
    set
}

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

/// Every feature but the element kinds other than the two independent sources, the waves not
/// in `waves`, and `without` (the competing feature): designs that hold sources of those waves
/// (pass-expr's measured construction: 66 of 96 seeds hold a SPICE pulse).
pub const fn sources_only(waves: &[ir::WaveKind], without: ir::Feature) -> ir::FeatureSet {
    let mut set = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(without));
    let mut i = 0;
    while i < ir::ElemKind::ALL.len() {
        let k = ir::ElemKind::ALL[i];
        if !matches!(k, ir::ElemKind::VSource | ir::ElemKind::ISource) {
            set = set.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Elem(k)));
        }
        i += 1;
    }
    let mut j = 0;
    while j < ir::WaveKind::ALL.len() {
        let w = ir::WaveKind::ALL[j];
        let (mut k, mut keep) = (0, false);
        while k < waves.len() {
            keep |= waves[k] as u8 == w as u8;
            k += 1;
        }
        if !keep {
            set = set.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Wave(w)));
        }
        j += 1;
    }
    set
}
