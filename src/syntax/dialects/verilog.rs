//! Verilog, declared with the definition API: a writer only (no reader), which
//! spells element letters and numbers from ngspice's lexicon.
//!
//! Cites: none (each cell's code cites its clause, `syntax::diag`).
//!
//! Contents:
//! - `pub(super) fn verilog() -> DialectDef`

use crate::syntax::Lang;
use crate::syntax::behaviour::{Parasitics, Writer};
use crate::syntax::def::DialectDef;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::Construct;
use crate::syntax::render::spelling::FuncForm;

/// Structural Verilog: one module per subcircuit, its instances.
pub(super) fn verilog() -> DialectDef {
    DialectDef::new("verilog")
        .parasitics(Parasitics::Netlist)
        .writer(Writer::Verilog)
        // The spelling: ngspice's cards.
        .write_lexicon(Lang::NGSPICE)
        .comment("//")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm::Func)
        .subckt_params(Some("PARAMS:"))
        .ngspice_numbers(true)
        // The capability column.
        .refuse(Construct::ForeignText, Code::W0001)
        .refuse(Construct::PrivateText, Code::W0039)
        .refuse(Construct::XyceSens, Code::W0001)
        .refuse(Construct::HspiceAnalysis, Code::W0001)
        .refuse(Construct::CaretReread, Code::W0002)
        .native(Construct::StatisticalCall)
        .native(Construct::VerilogALoad)
        .native(Construct::OsdiLoad)
        .refuse(Construct::LibWholeFile, Code::W0005)
        .omit(Construct::NgBehavior, Code::W0030)
        .native(Construct::LibSection)
        .native(Construct::RclModel)
        .native(Construct::ChargeCapacitor)
        .native(Construct::BehaviouralSource)
        .refuse(Construct::Laplace, Code::W0011)
        .native(Construct::RfPort)
        .refuse(Construct::PwlRepeatV, Code::W0008)
        .refuse(Construct::PwlRepeatI, Code::W0008)
        .refuse(Construct::NoiseWave, Code::W0009)
        .native(Construct::Conditional)
        .native(Construct::BranchParam)
        .native(Construct::Func)
        .native(Construct::LocalFunc)
        .native(Construct::TableSource)
        .native(Construct::LossyLine)
        .native(Construct::Step)
        .native(Construct::StepList)
        .native(Construct::TranStartup)
        .native(Construct::DcLogList)
        .native(Construct::Tf)
        .native(Construct::Sens)
        .native(Construct::SensAc)
        .native(Construct::PoleZero)
        .native(Construct::SParameter)
        .native(Construct::Print)
        .native(Construct::Save)
        .native(Construct::UnmappedOption)
        .native(Construct::Statistics)
        .native(Construct::Measure)
}
