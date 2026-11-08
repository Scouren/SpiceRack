//! ngspice, declared with the definition API: its lexicon rows (the
//! manual's citations as `lexicon/ngspice.rs` gave them, where its tests
//! still pin each rule), its spelling, its capability column and its
//! behaviours.
//!
//! Cites: ngspice manual §1.5 (comments), §1.6 (`+`), §1.7 (`.end`), §1.9
//! (quotes), §2.1.3 (scale factors), §2.2 (numparam precedence), §2.4.3
//! (`PARAMS:`).
//!
//! Contents:
//! - `pub(super) fn ngspice() -> DialectDef`

use crate::syntax::behaviour::{
    AnalysisForm, CardRules, ControlBlock, ExprEval, LoadForm, ModelSemantics, NetlistRules,
    NumberReading, SourceRules,
};
use crate::syntax::def::{Devices, DialectDef, Keywords, Operators};
use crate::syntax::diag::Code;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, End, Join, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Alt, Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::token::Tag;

/// ngspice: case-blind, `*`/`$`/`;`/`//` comments, `+` and `\\` continuation,
/// `'…'` and `{…}` expressions, `M` milli.
pub(super) fn ngspice() -> DialectDef {
    // §1.9: quotes protect comment bytes and take backslash escapes; an
    // unclosed one runs to the line's end or a `\\` break.
    let quoted = Quoting {
        escapes: true,
        protects: true,
        across_breaks: false,
    };
    let mut d = DialectDef::new("ngspice").title(true).fold_case(true);
    // §1.5: `*`, `#`, `$`, `;` are comments; the punctuation is turned into
    // one with a warning; so is a line of only a `//` comment.
    for s in [
        "*", "#", "$", ";", "=", "[", "]", "?", "(", ")", "&", "%", "\"", "!", ":", ",", "\x0c",
        "//",
    ] {
        d = d.comment_line(s);
    }
    d
        // §1.6: a `+` continues past `*`, `#`, `$` comment lines and blank lines.
        .cont_skip("*")
        .cont_skip("#")
        .cont_skip("$")
        .comment_tail(";")
        .comment_tail("//")
        .dollar(Dollar::AfterBlank)
        .plus(true)
        .backslash(Backslash::Glue)
        // §1.7: `.end` alone or before a blank is dropped, and reading goes on.
        .end(Some(End {
            col1: false,
            then_blank: true,
            stops: false,
        }))
        .stop(Some(b'@'))
        .join(Join::None)
        .quote(Quote::Expr(quoted))
        .string(quoted)
        .ident_start("$")
        .ident_continue("$.")
        .directive(Some(b'.'))
        .operators(Operators::Spice)
        .expr_braces(true)
        .caret(Caret::Pow)
        // §2.2, numparam precedence (the old ngspice grammar x_ternary .. x_unary).
        .infix(Tag::PipePipe, 2)
        .infix(Tag::AmpAmp, 3)
        .infix(Tag::EqualEqual, 4)
        .infix(Tag::BangEqual, 4)
        .infix(Tag::LessGreater, 4)
        .infix(Tag::LessEqual, 4)
        .infix(Tag::GreaterEqual, 4)
        .infix(Tag::Less, 4)
        .infix(Tag::Greater, 4)
        .infix(Tag::Plus, 5)
        .infix(Tag::Minus, 5)
        .infix(Tag::Star, 6)
        .infix(Tag::Slash, 6)
        .infix(Tag::Percent, 6)
        .infix(Tag::Backslash, 6)
        .infix(Tag::Caret, 7)
        .infix(Tag::StarStar, 7)
        .prefix(Tag::Plus)
        .prefix(Tag::Minus)
        .prefix(Tag::Bang)
        .prefix_bp(6)
        .probe("v")
        .probe("i")
        .lib_sections(true)
        // §2.1.3: the scale factors; `D` an exponent.
        .exponent("eEdD")
        .scale("meg", Scale::Exp(6))
        .scale("mil", Scale::Mil)
        .scale("t", Scale::Exp(12))
        .scale("g", Scale::Exp(9))
        .scale("k", Scale::Exp(3))
        .scale("m", Scale::Exp(-3))
        .scale("u", Scale::Exp(-6))
        .scale("n", Scale::Exp(-9))
        .scale("p", Scale::Exp(-12))
        .scale("f", Scale::Exp(-15))
        .scale("a", Scale::Exp(-18))
        .tail(Tail::Spice { dot: true })
        .arithmetic(Arithmetic::Ngspice)
        .keywords(Keywords::Spice)
        .keyword(".control", Tag::KwControl)
        .keyword(".endc", Tag::KwEndc)
        .keyword(".sp", Tag::KwSp)
        .devices(Devices::Spice)
        .devices(Devices::NgspiceOwn)
        // The spelling: §2.8.5 `{…}`, §2.4.3 `PARAMS:`.
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm::Func)
        .subckt_params(Some("PARAMS:"))
        .ngspice_numbers(true)
        // The behaviours.
        .number_reading(NumberReading::Ngspice)
        .model_semantics(ModelSemantics::Ngspice)
        .source_rules(SourceRules::Ngspice)
        .card_rules(CardRules::Ngspice)
        .load_form(LoadForm::Ngspice)
        .control_block(ControlBlock::Ngspice)
        .analysis_form(AnalysisForm::Ngspice)
        .expr_eval(ExprEval::Ngspice)
        .netlist_rules(NetlistRules::Ngspice)
        .spectre_analysis("pz")
        .spectre_analysis("sp")
        .spectre_analysis("sens")
        // The circuit temperature in an expression (manual §2.8.5).
        .temp_name("temper")
        // ngspice-45's built-in devices (`devhelp`): a model type of one of
        // these names is the built-in, never a loaded OSDI module (W0121).
        .builtin_device("URC")
        .builtin_device("ASRC")
        .builtin_device("BJT")
        .builtin_device("BSIM1")
        .builtin_device("BSIM2")
        .builtin_device("BSIM3")
        .builtin_device("BSIM3v0")
        .builtin_device("BSIM3v1")
        .builtin_device("BSIM3v32")
        .builtin_device("B4SOI")
        .builtin_device("BSIM4")
        .builtin_device("BSIM4v5")
        .builtin_device("BSIM4v6")
        .builtin_device("BSIM4v7")
        .builtin_device("B3SOIPD")
        .builtin_device("B3SOIFD")
        .builtin_device("B3SOIDD")
        .builtin_device("Capacitor")
        .builtin_device("CCCS")
        .builtin_device("CCVS")
        .builtin_device("CplLines")
        .builtin_device("CSwitch")
        .builtin_device("Diode")
        .builtin_device("HFET1")
        .builtin_device("HFET2")
        .builtin_device("hicum2")
        .builtin_device("HiSIM2")
        .builtin_device("HiSIMHV1")
        .builtin_device("HiSIMHV2")
        .builtin_device("Inductor")
        .builtin_device("mutual")
        .builtin_device("Isource")
        .builtin_device("JFET")
        .builtin_device("JFET2")
        .builtin_device("LTRA")
        .builtin_device("MES")
        .builtin_device("MESA")
        .builtin_device("Mos1")
        .builtin_device("Mos2")
        .builtin_device("Mos3")
        .builtin_device("Mos6")
        .builtin_device("Mos9")
        .builtin_device("Resistor")
        .builtin_device("SOI3")
        .builtin_device("Switch")
        .builtin_device("Tranline")
        .builtin_device("TransLine")
        .builtin_device("VBIC")
        .builtin_device("VCCS")
        .builtin_device("VCVS")
        .builtin_device("Vsource")
        .builtin_device("VDMOS")
        .builtin_device("NBJT")
        .builtin_device("NBJT2")
        .builtin_device("NUMD")
        .builtin_device("NUMD2")
        .builtin_device("NUMOS")
        // The capability column.
        .refuse(Construct::ForeignText, Code::W0001)
        .refuse(Construct::PrivateText, Code::W0039)
        .refuse(Construct::XyceSens, Code::W0015)
        .refuse(Construct::HspiceAnalysis, Code::W0015)
        .refuse(Construct::HspiceWLine, Code::W0105)
        .omit(Construct::VacaskScript, Code::W0120)
        .refuse(Construct::VacaskElaborate, Code::W0122)
        .refuse(Construct::CaretReread, Code::W0002)
        .native(Construct::StatisticalCall)
        .spell(Construct::VerilogALoad, Alt::PreOsdi)
        .spell(Construct::OsdiLoad, Alt::PreOsdi)
        .native(Construct::OsdiInstance)
        .spell(Construct::LibWholeFile, Alt::Include)
        .spell(Construct::NgBehavior, Alt::NgBehaviorLine)
        .native(Construct::LibSection)
        .native(Construct::RclModel)
        .spell(Construct::ChargeCapacitor, Alt::ChargeQuoted)
        .native(Construct::BehaviouralSource)
        .lift(Construct::Laplace, Rule::LaplaceXfer)
        .native(Construct::RfPort)
        .native(Construct::PwlRepeatV)
        .refuse(Construct::PwlRepeatI, Code::W0008)
        .native(Construct::NoiseWave)
        .native(Construct::Conditional)
        .refuse(Construct::BranchParam, Code::W0014)
        .native(Construct::Func)
        .native(Construct::LocalFunc)
        .spell(Construct::TableSource, Alt::PwlValue)
        .native(Construct::LossyLine)
        .native(Construct::AmWave)
        .refuse(Construct::Step, Code::W0016)
        .refuse(Construct::StepList, Code::W0016)
        .refuse(Construct::TranStartup, Code::W0037)
        .refuse(Construct::DcLogList, Code::W0038)
        .native(Construct::Tf)
        .native(Construct::Sens)
        .native(Construct::SensAc)
        .native(Construct::PoleZero)
        .native(Construct::SParameter)
        .native(Construct::Print)
        .native(Construct::Save)
        .native(Construct::UnmappedOption)
        .lift(Construct::Statistics, Rule::StatisticsDraws)
        .native(Construct::Measure)
    // A new construct's cell is appended below its stream's marker
    // (`dialects/mod.rs`), with its mirror in `tests/builtin_dialects.py`.
    // --- S1
    // --- S2
    // --- S3
}
