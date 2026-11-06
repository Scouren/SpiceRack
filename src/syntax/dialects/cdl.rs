//! CDL, declared with the definition API: a writer only (no reader), which
//! spells element letters and numbers from ngspice's lexicon.
//!
//! Cites: none (each cell's code cites its clause, `syntax::diag`).
//!
//! Contents:
//! - `pub(super) fn cdl() -> DialectDef`

use crate::syntax::Lang;
use crate::syntax::behaviour::{Parasitics, Writer};
use crate::syntax::def::DialectDef;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Construct, Rule};
use crate::syntax::render::spelling::FuncForm;

/// CDL for LVS: elements in the xschem layout, ngspice's cards, no analyses.
pub(super) fn cdl() -> DialectDef {
    DialectDef::new("cdl")
        .parasitics(Parasitics::Netlist)
        .writer(Writer::Cdl)
        // The spelling: ngspice's cards.
        .write_lexicon(Lang::NGSPICE)
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm::Func)
        .subckt_params(Some("PARAMS:"))
        .ngspice_numbers(true)
        // The capability column.
        .omit(Construct::ForeignText, Code::W0028)
        .copy(Construct::PrivateText, Code::W0027)
        .copy(Construct::XyceSens, Code::W0027)
        .copy(Construct::HspiceAnalysis, Code::W0027)
        .copy(Construct::VacaskScript, Code::W0027)
        .refuse(Construct::CaretReread, Code::W0002)
        .native(Construct::StatisticalCall)
        .native(Construct::VerilogALoad)
        .native(Construct::OsdiLoad)
        .native(Construct::OsdiInstance)
        .refuse(Construct::LibWholeFile, Code::W0005)
        .omit(Construct::NgBehavior, Code::W0030)
        .native(Construct::LibSection)
        .native(Construct::RclModel)
        .native(Construct::ChargeCapacitor)
        .native(Construct::BehaviouralSource)
        .lift(Construct::Laplace, Rule::LaplaceXfer)
        .native(Construct::RfPort)
        .native(Construct::PwlRepeatV)
        .refuse(Construct::PwlRepeatI, Code::W0008)
        .native(Construct::NoiseWave)
        .native(Construct::Conditional)
        .native(Construct::BranchParam)
        .native(Construct::Func)
        .native(Construct::LocalFunc)
        .native(Construct::TableSource)
        .native(Construct::LossyLine)
        .native(Construct::AmWave)
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
