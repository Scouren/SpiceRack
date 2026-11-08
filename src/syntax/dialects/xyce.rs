//! Xyce, declared with the definition API: its lexicon rows (each
//! rule's citation as `lexicon/xyce.rs` gave it, where its tests still pin
//! it), its spelling, its behaviours and its capability column.
//!
//! Cites (as the old Xyce grammar did; RG = Xyce Reference Guide 7.9, checked
//! on Xyce 7.10): §2.1.39 p.163 (line 1 the title); §2.1.39.1 (`*` comment
//! lines, a blank-led line is a comment); §2.1.39.2 (`;` anywhere, quotes
//! and braces included); §2.1.39.3 (`+` continuation); §2.1.6 (`.END`);
//! §3.1 p.837 and Table 6-2 p.852 (scale factors, `X` mega, no atto); §2.2
//! p.164 (`{expr}` and `'expr'`); Table 2-32 (operators); p.165 (`:` in
//! names).
//!
//! Contents:
//! - `pub(super) fn xyce() -> DialectDef`

use crate::syntax::behaviour::{
    AnalysisForm, CardRules, Coupling, ExprEval, IncludeScope, LoadForm, NetlistRules, OptionsForm,
    ParamForm,
};
use crate::syntax::def::{Devices, DialectDef, Keywords, Operators};
use crate::syntax::diag::Code;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, End, Join, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Alt, Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::token::Tag;

/// §2.1.39.2: a `;` inside quotes still starts a comment.
const UNPROTECTED: Quoting = Quoting {
    escapes: false,
    protects: false,
    across_breaks: false,
};

const SCALES: &[(&str, Scale)] = &[
    ("meg", Scale::Exp(6)),
    ("mil", Scale::Mil),
    ("x", Scale::Exp(6)),
    ("t", Scale::Exp(12)),
    ("g", Scale::Exp(9)),
    ("k", Scale::Exp(3)),
    ("m", Scale::Exp(-3)),
    ("u", Scale::Exp(-6)),
    ("n", Scale::Exp(-9)),
    ("p", Scale::Exp(-12)),
    ("f", Scale::Exp(-15)),
];

/// RG p.49: `.incl` is `.include`.
/// `.global_param` (Xyce RG `.GLOBAL_PARAM`) is a `.param` Xyce
/// re-evaluates as the run goes; Xyce 7.10 runs a constant, a swept and a
/// stepped `.param` the same as the `.global_param` (checked by running
/// both); one reading a swept variable is kept as text by the reader
/// (`Rd::global_swept`), and Xyce writes the rest back as `.global_param`
/// (`Rd::global_param`).
const OWN_KEYWORDS: &[(&str, Tag)] = &[
    (".incl", Tag::KwInclude),
    (".step", Tag::KwStep),
    (".global_param", Tag::KwParam),
];

/// Xyce: case-blind, `*` and blank-led comment lines, `;` comments that no
/// quote protects, `+` continuation, `.END` in column 1.
pub(super) fn xyce() -> DialectDef {
    let mut d = DialectDef::new("xyce");
    d = d.title(true);
    d = d.fold_case(true);
    d = d.comment_line("*");
    d = d.blank_led_comment(true);
    d = d.cont_skip("*");
    d = d.comment_tail(";");
    d = d.dollar(Dollar::Text);
    d = d.plus(true);
    d = d.backslash(Backslash::None);
    d = d.end(Some(End {
        col1: true,
        then_blank: true,
        stops: true,
    }));
    d = d.stop(None);
    d = d.join(Join::None);
    d = d.quote(Quote::Expr(UNPROTECTED));
    d = d.string(UNPROTECTED);
    d = d.ident_start("$");
    d = d.ident_continue("$.:");
    d = d.directive(Some(b'.'));
    d = d.operators(Operators::Spice);
    d = d.expr_braces(true);
    d = d.caret(Caret::Xor);
    // Table 2-32 (RG p.164), the levels measured on Xyce 7.10.
    d = d.infix(Tag::PipePipe, 2);
    d = d.infix(Tag::Pipe, 2);
    d = d.infix(Tag::AmpAmp, 2);
    d = d.infix(Tag::Amp, 2);
    d = d.infix(Tag::Caret, 3);
    d = d.infix(Tag::EqualEqual, 4);
    d = d.infix(Tag::BangEqual, 4);
    d = d.infix(Tag::LessGreater, 4);
    d = d.infix(Tag::LessEqual, 4);
    d = d.infix(Tag::GreaterEqual, 4);
    d = d.infix(Tag::Less, 4);
    d = d.infix(Tag::Greater, 4);
    d = d.infix(Tag::Plus, 5);
    d = d.infix(Tag::Minus, 5);
    d = d.infix(Tag::Percent, 6);
    d = d.infix(Tag::Star, 7);
    d = d.infix(Tag::Slash, 7);
    d = d.infix(Tag::StarStar, 9);
    d = d.right(Tag::StarStar);
    d = d.prefix(Tag::Plus);
    d = d.prefix(Tag::Minus);
    d = d.prefix(Tag::Tilde);
    d = d.prefix_bp(8);
    // RG §2.1.31 pp.130-133.
    d = d.probe("v");
    d = d.probe("vr");
    d = d.probe("vi");
    d = d.probe("vm");
    d = d.probe("vp");
    d = d.probe("vdb");
    d = d.probe("i");
    d = d.probe("ir");
    d = d.probe("ii");
    d = d.probe("im");
    d = d.probe("ip");
    d = d.probe("idb");
    d = d.probe("n");
    d = d.probe("p");
    d = d.probe("w");
    d = d.lib_sections(true);
    d = d.exponent("eE");
    for &(s, k) in SCALES {
        d = d.scale(s, k);
    }
    d = d.tail(Tail::Spice { dot: false });
    d = d.arithmetic(Arithmetic::Decimal);
    d = d.keywords(Keywords::Spice);
    for &(s, t) in OWN_KEYWORDS {
        d = d.keyword(s, t);
    }
    d = d.devices(Devices::Spice);
    d = d.devices(Devices::LossyLine);
    d = d.devices(Devices::XycePort);
    d
        // The spelling.
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm::Func)
        .subckt_params(Some("PARAMS:"))
        // The behaviours.
        .caret_power(false)
        .hier_sep(':')
        .card_rules(CardRules::Xyce)
        .options_form(OptionsForm::XycePackages)
        .load_form(LoadForm::Xyce)
        .analysis_form(AnalysisForm::Xyce)
        .coupling(Coupling::OneCard)
        .param_form(ParamForm::Xyce)
        .expr_eval(ExprEval::Xyce)
        .netlist_rules(NetlistRules::Xyce)
        .include_scope(IncludeScope::Global)
        .spectre_analysis("hb")
        .spectre_analysis("sp")
        .spectre_analysis("sens")
        .spectre_analysis("sweep")
        .spectre_analysis("montecarlo")
        // The circuit temperature in an expression (RG, special variable TEMP).
        .temp_name("temp")
        // The VACASK analyses it has (Xyce RG .HB).
        .vacask_analysis("hb")
        // The capability column.
        .refuse(Construct::ForeignText, Code::W0001)
        .refuse(Construct::PrivateText, Code::W0039)
        .native(Construct::XyceSens)
        .refuse(Construct::HspiceAnalysis, Code::W0015)
        .refuse(Construct::HspiceWLine, Code::W0105)
        .omit(Construct::VacaskScript, Code::W0120)
        .refuse(Construct::VacaskElaborate, Code::W0122)
        .refuse(Construct::CaretReread, Code::W0002)
        .native(Construct::StatisticalCall)
        .refuse(Construct::VerilogALoad, Code::W0004)
        .refuse(Construct::OsdiLoad, Code::W0004)
        .refuse(Construct::OsdiInstance, Code::W0004)
        .spell(Construct::LibWholeFile, Alt::Include)
        .omit(Construct::NgBehavior, Code::W0030)
        .native(Construct::LibSection)
        .native(Construct::RclModel)
        .spell(Construct::ChargeCapacitor, Alt::ChargeBraced)
        .spell(Construct::BehaviouralSource, Alt::BracedBehaviour)
        .refuse(Construct::Laplace, Code::W0011)
        .spell(Construct::RfPort, Alt::XycePort)
        .refuse(Construct::PwlRepeatV, Code::W0008)
        .refuse(Construct::PwlRepeatI, Code::W0008)
        .refuse(Construct::NoiseWave, Code::W0009)
        .refuse(Construct::Conditional, Code::W0013)
        .refuse(Construct::BranchParam, Code::W0014)
        .native(Construct::Func)
        .native(Construct::LocalFunc)
        .native(Construct::TableSource)
        .native(Construct::LossyLine)
        .refuse(Construct::AmWave, Code::W0108)
        .native(Construct::Step)
        .native(Construct::StepList)
        .refuse(Construct::TranStartup, Code::W0037)
        .native(Construct::DcLogList)
        .refuse(Construct::Tf, Code::W0015)
        .refuse(Construct::Sens, Code::W0015)
        .refuse(Construct::SensAc, Code::W0015)
        .refuse(Construct::PoleZero, Code::W0015)
        .spell(Construct::SParameter, Alt::XyceLin)
        .native(Construct::Print)
        .omit(Construct::Save, Code::W0023)
        .refuse(Construct::UnmappedOption, Code::W0017)
        .lift(Construct::Statistics, Rule::StatisticsDraws)
        .native(Construct::Measure)
    // A new construct's cell is appended below its stream's marker
    // (`dialects/mod.rs`), with its mirror in `tests/builtin_dialects.py`.
    // --- S1
    // --- S2
    // --- S3
}
