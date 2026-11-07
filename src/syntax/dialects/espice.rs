//! ESPice, declared with the definition API: ngspice's reader and spelling
//! (it reads ngspice decks, and compiles Verilog-A itself), its own load
//! form, analysis forms and capability cells.
//!
//! Cites: none (each cell's code cites its clause, `syntax::diag`).
//!
//! Contents:
//! - `pub(super) fn espice() -> DialectDef`

use crate::syntax::Lang;
use crate::syntax::behaviour::{
    AnalysisForm, CardRules, ControlBlock, LoadForm, ModelSemantics, NumberReading, SourceRules,
};
use crate::syntax::def::DialectDef;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Alt, Construct, Rule};

/// ESPice: ngspice text with Verilog-A loaded as `.hdl` instead of
/// `pre_osdi`.
pub(super) fn espice() -> DialectDef {
    DialectDef::new("espice")
        .extends(Lang::NGSPICE)
        .text_family(Lang::NGSPICE)
        // The behaviours: ESPice is never read as itself (its text is
        // ngspice's), so its reading rows are the plain SPICE ones.
        .number_reading(NumberReading::Decimal)
        .model_semantics(ModelSemantics::Other)
        .source_rules(SourceRules::Spice)
        .card_rules(CardRules::Spice)
        .load_form(LoadForm::Espice)
        .control_block(ControlBlock::None)
        .analysis_form(AnalysisForm::Espice)
        // ESPice ignores `m` on an `X` line (run by hand: `m=5` gives
        // one copy's 0.5 V where ngspice gives 1/6 V).
        .subckt_multiplicity(false)
        // The capability cells it answers otherwise than ngspice.
        .spell(Construct::VerilogALoad, Alt::Hdl)
        .refuse(Construct::OsdiLoad, Code::W0004)
        .refuse(Construct::OsdiInstance, Code::W0004)
        .omit(Construct::NgBehavior, Code::W0030)
        .lift(Construct::Laplace, Rule::LaplaceTable)
        .native(Construct::PwlRepeatI)
        .refuse(Construct::NoiseWave, Code::W0009)
        .refuse(Construct::TableSource, Code::W0102)
        // ESPice 0.3.0 runs ngspice's AM but no VO: "AM with a nonzero VO
        // offset" (run).
        .spell(Construct::AmWave, Alt::AmNoOffset)
}
