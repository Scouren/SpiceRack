//! HSPICE, declared with the definition API: its lexicon rows (each
//! rule's citation as `lexicon/hspice.rs` gave it, where its tests still pin
//! it), its spelling, its behaviours and its capability column.
//!
//! Cites (the rule ids the old HSPICE grammar labels its rules with,
//! `git show 9d1d6bb:grammar/hspice.peg`; UG08 = HSPICE User Guide 2008, CR10 =
//! Command Reference 2010): L1 title; L2 comment lines; L3 the `$` rule; L5
//! `+` continuation; L6 `\` and `\\` breaks; L9 `.END`; T1 separators; T2
//! quotes (`$` is text inside); T3 numbers (`X` is mega, `D` an exponent);
//! X2 operators (UG08 pp.279-283). ESPice `src/frontend/lines_hspice.zig`
//! (`stripComment`, `isNumberToken`: the `$` hook).
//!
//! Contents:
//! - `pub(super) fn hspice() -> DialectDef`

use crate::syntax::behaviour::{
    AnalysisForm, CardRules, EndRule, ExprEval, LoadForm, NetlistRules, ValueQuoting,
};
use crate::syntax::def::{Devices, DialectDef, Keywords};
use crate::syntax::diag::Code;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, End, Join, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Alt, Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::token::Tag;

/// T2, L6: `'…'` and `"…"` protect `$`, have no escapes, and continue across
/// a `\` or `\\` break.
const QUOTED: Quoting = Quoting {
    escapes: false,
    protects: true,
    across_breaks: true,
};

/// X2: no `<>`, `%` or `\` (UG08 Table 19).
const OPERATORS: &[(&str, Tag)] = &[
    ("**", Tag::StarStar),
    ("==", Tag::EqualEqual),
    ("!=", Tag::BangEqual),
    ("<=", Tag::LessEqual),
    (">=", Tag::GreaterEqual),
    ("&&", Tag::AmpAmp),
    ("||", Tag::PipePipe),
];

/// T3: T G MEG X K MIL M U N P F A.
const SCALES: &[(&str, Scale)] = &[
    ("meg", Scale::Exp(6)),
    ("x", Scale::Exp(6)),
    ("mil", Scale::Mil),
    ("t", Scale::Exp(12)),
    ("g", Scale::Exp(9)),
    ("k", Scale::Exp(3)),
    ("m", Scale::Exp(-3)),
    ("u", Scale::Exp(-6)),
    ("n", Scale::Exp(-9)),
    ("p", Scale::Exp(-12)),
    ("f", Scale::Exp(-15)),
    ("a", Scale::Exp(-18)),
];

/// `.probe` selects outputs, as `.save` does elsewhere.
const OWN_KEYWORDS: &[(&str, Tag)] = &[(".probe", Tag::KwProbe)];

/// HSPICE: case-blind, `*`/`$` comment lines, the `$` hook, `+`, `\` and
/// `\\` continuation, `'…'` expressions, braces as name bytes (#203).
pub(super) fn hspice() -> DialectDef {
    let mut d = DialectDef::new("hspice");
    d = d.title(true);
    d = d.fold_case(true);
    d = d.comment_line("*");
    d = d.comment_line("$");
    d = d.cont_skip("*");
    d = d.cont_skip("$");
    d = d.dollar(Dollar::Hspice);
    d = d.plus(true);
    d = d.backslash(Backslash::Hspice);
    // L9: `.END` ends the run; what follows is a further run, not read.
    d = d.end(Some(End {
        col1: false,
        then_blank: false,
        stops: true,
    }));
    d = d.stop(None);
    d = d.join(Join::None);
    d = d.quote(Quote::Expr(QUOTED));
    d = d.string(QUOTED);
    d = d.string_expr(true);
    d = d.ident_start("$");
    d = d.ident_continue("$.");
    d = d.directive(Some(b'.'));
    for &(s, t) in OPERATORS {
        d = d.operator(s, t);
    }
    d = d.caret(Caret::Pow);
    // X2 (UG08 pp.279-283, Table 19): no `%`, `\\` or unary `!`.
    d = d.infix(Tag::PipePipe, 2);
    d = d.infix(Tag::AmpAmp, 3);
    d = d.infix(Tag::EqualEqual, 4);
    d = d.infix(Tag::BangEqual, 4);
    d = d.infix(Tag::LessEqual, 4);
    d = d.infix(Tag::GreaterEqual, 4);
    d = d.infix(Tag::Less, 4);
    d = d.infix(Tag::Greater, 4);
    d = d.infix(Tag::Plus, 5);
    d = d.infix(Tag::Minus, 5);
    d = d.infix(Tag::Star, 6);
    d = d.infix(Tag::Slash, 6);
    d = d.infix(Tag::Caret, 7);
    d = d.infix(Tag::StarStar, 7);
    d = d.prefix(Tag::Plus);
    d = d.prefix(Tag::Minus);
    d = d.prefix_bp(6);
    d = d.probe("v");
    d = d.probe("i");
    d = d.lib_sections(true);
    d = d.exponent("eEdD");
    for &(s, k) in SCALES {
        d = d.scale(s, k);
    }
    d = d.tail(Tail::Spice { dot: true });
    d = d.arithmetic(Arithmetic::Decimal);
    d = d.keywords(Keywords::Spice);
    for &(s, t) in OWN_KEYWORDS {
        d = d.keyword(s, t);
    }
    d = d.devices(Devices::Spice);
    d
        // The spelling.
        .comment("*")
        .path_quote("'")
        .expr("'", "'")
        .func(FuncForm::Param)
        .subckt_params(None)
        // The behaviours.
        .card_rules(CardRules::Hspice)
        .load_form(LoadForm::Hspice)
        .analysis_form(AnalysisForm::Hspice)
        .end_rule(EndRule::WhenRuns)
        .value_quoting(ValueQuoting::Hspice)
        .expr_eval(ExprEval::Hspice)
        .netlist_rules(NetlistRules::Hspice)
        .spectre_analysis("pz")
        .spectre_analysis("sens")
        .spectre_analysis("sp")
        .spectre_analysis("hb")
        .spectre_analysis("sweep")
        .spectre_analysis("montecarlo")
        // The circuit temperature in an expression (HSPICE UG, temper).
        .temp_name("temper")
        // The VACASK analyses it has (HSPICE RF .HB, .HBAC, .HBNOISE).
        .vacask_analysis("hb")
        .vacask_analysis("hbac")
        .vacask_analysis("hbnoise")
        // The capability column.
        .refuse(Construct::ForeignText, Code::W0001)
        .refuse(Construct::PrivateText, Code::W0039)
        .refuse(Construct::XyceSens, Code::W0015)
        .native(Construct::HspiceAnalysis)
        .native(Construct::HspiceWLine)
        .omit(Construct::VacaskScript, Code::W0120)
        .refuse(Construct::VacaskElaborate, Code::W0122)
        .refuse(Construct::CaretReread, Code::W0002)
        .native(Construct::StatisticalCall)
        .spell(Construct::VerilogALoad, Alt::Hdl)
        .refuse(Construct::OsdiLoad, Code::W0004)
        .refuse(Construct::OsdiInstance, Code::W0004)
        .spell(Construct::LibWholeFile, Alt::Include)
        .omit(Construct::NgBehavior, Code::W0030)
        .native(Construct::LibSection)
        .native(Construct::RclModel)
        .spell(Construct::ChargeCapacitor, Alt::ChargeQuoted)
        .spell(Construct::BehaviouralSource, Alt::VolCur)
        .lift(Construct::Laplace, Rule::LaplaceTable)
        .refuse(Construct::RfPort, Code::W0007)
        .refuse(Construct::PwlRepeatV, Code::W0008)
        .refuse(Construct::PwlRepeatI, Code::W0008)
        .refuse(Construct::NoiseWave, Code::W0009)
        .native(Construct::Conditional)
        .refuse(Construct::BranchParam, Code::W0014)
        .spell(Construct::Func, Alt::FuncParam)
        .native(Construct::LocalFunc)
        .refuse(Construct::TableSource, Code::W0102)
        .refuse(Construct::LossyLine, Code::W0105)
        .spell(Construct::AmWave, Alt::HspiceAm)
        .refuse(Construct::Step, Code::W0016)
        .spell(Construct::StepList, Alt::TempList)
        .refuse(Construct::TranStartup, Code::W0037)
        .spell(Construct::DcLogList, Alt::DcType)
        .native(Construct::Tf)
        .native(Construct::Sens)
        .refuse(Construct::SensAc, Code::W0015)
        // HSPICE UG `.PZ out src`: where a source stands across the input.
        .spell(Construct::PoleZero, Alt::HspicePz)
        .refuse(Construct::SParameter, Code::W0015)
        .native(Construct::Print)
        .spell(Construct::Save, Alt::Probe)
        .native(Construct::UnmappedOption)
        .lift(Construct::Statistics, Rule::StatisticsDraws)
        .native(Construct::Measure)
}
