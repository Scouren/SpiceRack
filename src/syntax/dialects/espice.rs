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
        // No `pwl()` in a B source, and its own `TABLE` rounds the corners
        // (0.95 where Xyce gives 1.0, run): the segments as ternaries.
        .spell(Construct::TableSource, Alt::TernaryValue)
        // ESPice 0.3.0 runs ngspice's AM but no VO: "AM with a nonzero VO
        // offset" (run).
        .spell(Construct::AmWave, Alt::AmNoOffset)
        // A new construct's cell is appended below its stream's marker
        // (`dialects/mod.rs`), with its mirror in `tests/builtin_dialects.py`.
        // --- S1
        // --- S2
        // ESPice 0.3.0 runs no `.control` command ("command 'alterparam'
        // ignored", run): one circuit a deck.
        .refuse(Construct::Epochs, Code::W0130)
        // So ngspice's loop is not ESPice's: a `.step` stays refused until
        // its own `.step` card is written (ZERO-LEDGER P4-029).
        .refuse(Construct::Step, Code::W0016)
        .refuse(Construct::StepList, Code::W0016)
        // Nor is a log or list `.dc` axis: refused until ESPice's own
        // `dec`/`poi` card is written (ZERO-LEDGER P4-099).
        .refuse(Construct::DcLogList, Code::W0038)
    // --- S3
}
