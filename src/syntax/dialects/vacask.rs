//! VACASK, declared with the definition API: its lexicon rows (each
//! rule's citation as `lexicon/vacask.rs` gave it, where its tests still pin
//! it), its spelling, its behaviours and its capability column.
//!
//! Cites (as the old VACASK grammar did; L = `lib/dfllexer.l` line, P =
//! `lib/dflparser.y` line): L159 title; L176-L187 `//` and `/* */`
//! comments; L913-L921 a newline inside brackets is no statement end, `\`
//! before the newline joins; L594-L693 numbers (`M` `x` `X` `meg` mega, `m`
//! milli, `mil`, letters after the scale, `1e3k` an error); L695 long
//! strings; L705 strings; L869 names (`'…'` parts); L783-L868 keywords at a
//! statement's head; P216-P230 and docs/expr-operators.md operators.
//!
//! Contents:
//! - `pub(super) fn vacask() -> DialectDef`

use crate::syntax::Lang;
use crate::syntax::behaviour::{NamedParams, NamedRules, OptionsForm, Writer};
use crate::syntax::def::DialectDef;
use crate::syntax::diag::Code;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, Join, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::statement::Analysis;
use crate::syntax::token::Tag;

/// L594-L693, in the old VACASK grammar's order.
const SCALES: &[(&str, Scale)] = &[
    ("meg", Scale::Exp(6)),
    ("M", Scale::Exp(6)),
    ("mil", Scale::Mil),
    ("x", Scale::Exp(6)),
    ("X", Scale::Exp(6)),
    ("m", Scale::Exp(-3)),
    ("u", Scale::Exp(-6)),
    ("n", Scale::Exp(-9)),
    ("p", Scale::Exp(-12)),
    ("f", Scale::Exp(-15)),
    ("a", Scale::Exp(-18)),
    ("k", Scale::Exp(3)),
    ("K", Scale::Exp(3)),
    ("G", Scale::Exp(9)),
    ("T", Scale::Exp(12)),
];

/// L783-L868 and P462-P509, P1267-P1296.
const KEYWORDS: &[(&str, Tag)] = &[
    ("include", Tag::KwInclude),
    ("load", Tag::KwLoad),
    ("embed", Tag::KwEmbed),
    ("global", Tag::KwGlobal),
    ("ground", Tag::KwGround),
    ("parameters", Tag::KwParam),
    ("model", Tag::KwModel),
    ("subckt", Tag::KwSubckt),
    ("ends", Tag::KwEnds),
    ("section", Tag::KwSection),
    ("endsection", Tag::KwEndsection),
    ("control", Tag::KwControl),
    ("endc", Tag::KwEndc),
    ("analysis", Tag::KwAnalysis),
    ("options", Tag::KwOptions),
    ("sweep", Tag::KwSweep),
    ("save", Tag::KwSave),
    ("@if", Tag::KwIf),
    ("@elseif", Tag::KwElseif),
    ("@else", Tag::KwElse),
    ("@end", Tag::KwEndif),
];

/// The kinds after `analysis name` (`statement.rs` cites the decks).
const ANALYSES: &[(&str, Analysis)] = &[
    ("op", Analysis::Op),
    ("dcinc", Analysis::Dcinc),
    ("dcxf", Analysis::Dcxf),
    ("ac", Analysis::Ac),
    ("acxf", Analysis::Acxf),
    ("noise", Analysis::Noise),
    ("tran", Analysis::Tran),
    ("hb", Analysis::Hb),
];

/// VACASK: case kept, `//` and `/* */` comments, statements across lines
/// inside brackets, `@if` directives, keywords at a statement's head.
pub(super) fn vacask() -> DialectDef {
    let mut d = DialectDef::new("vacask");
    d = d.title(true);
    d = d.comment_tail("//");
    d = d.block_comment(true);
    d = d.long_strings(true);
    d = d.dollar(Dollar::Text);
    d = d.backslash(Backslash::Tight);
    d = d.end(None);
    d = d.stop(None);
    d = d.join(Join::Brackets);
    d = d.quote(Quote::Name);
    d = d.string(Quoting {
        escapes: true,
        protects: true,
        across_breaks: false,
    });
    d = d.ident_start("$");
    d = d.ident_continue("$");
    d = d.directive(Some(b'@'));
    d = d.head_words(true);
    d = d.operator("**", Tag::StarStar);
    d = d.operator("==", Tag::EqualEqual);
    d = d.operator("!=", Tag::BangEqual);
    d = d.operator("<=", Tag::LessEqual);
    d = d.operator(">=", Tag::GreaterEqual);
    d = d.operator("<<", Tag::LessLess);
    d = d.operator(">>", Tag::GreaterGreater);
    d = d.operator("&&", Tag::AmpAmp);
    d = d.operator("||", Tag::PipePipe);
    d = d.caret(Caret::Xor);
    // P216-P230, docs/expr-operators.md.
    d = d.infix(Tag::PipePipe, 2);
    d = d.infix(Tag::AmpAmp, 3);
    d = d.infix(Tag::Pipe, 4);
    d = d.infix(Tag::Caret, 5);
    d = d.infix(Tag::Amp, 6);
    d = d.infix(Tag::EqualEqual, 7);
    d = d.infix(Tag::BangEqual, 7);
    d = d.infix(Tag::LessEqual, 8);
    d = d.infix(Tag::GreaterEqual, 8);
    d = d.infix(Tag::Less, 8);
    d = d.infix(Tag::Greater, 8);
    d = d.infix(Tag::LessLess, 9);
    d = d.infix(Tag::GreaterGreater, 9);
    d = d.infix(Tag::Plus, 10);
    d = d.infix(Tag::Minus, 10);
    d = d.infix(Tag::Star, 11);
    d = d.infix(Tag::Slash, 11);
    d = d.infix(Tag::StarStar, 12);
    d = d.right(Tag::StarStar);
    d = d.prefix(Tag::Plus);
    d = d.prefix(Tag::Minus);
    d = d.prefix(Tag::Bang);
    d = d.prefix(Tag::Tilde);
    d = d.prefix_bp(13);
    d = d.index(true);
    d = d.lib_sections(true);
    d = d.exponent("eE");
    for &(s, k) in SCALES {
        d = d.scale(s, k);
    }
    d = d.tail(Tail::Letters {
        after_exponent: false,
        hex: true,
    });
    d = d.arithmetic(Arithmetic::Decimal);
    for &(s, t) in KEYWORDS {
        d = d.keyword(s, t);
    }
    d = d.named(true);
    // An instance's master is a model or a subcircuit, never a device word;
    // the modules a `model` names are `device::VACASK`, which resolve reads.
    for &(s, a) in ANALYSES {
        d = d.analysis(s, a);
    }
    d
        // The spelling.
        .comment("")
        .path_quote("\"")
        .expr("", "")
        .func(FuncForm::None)
        .subckt_params(None)
        // The behaviours.
        .writer(Writer::Vacask)
        .caret_power(false)
        .named_rules(NamedRules::Vacask)
        .options_form(OptionsForm::Vacask)
        .named_params(NamedParams::Vacask)
        .analysis_masters(Lang::SPECTRE)
        // `.options KEY=VALUE` whose VACASK spelling is the same word and
        // the same quantity (`itl1`, `maxord` are rejected there). `scale`
        // is the `$simparam("scale")` VACASK's `sp_*` modules read where
        // ngspice's code reads `CKTscale` (`devices/spice/*.va` headers;
        // `lib/osdidevice.cpp` `simParamNames`).
        .option_to("reltol", "reltol")
        .option_to("abstol", "abstol")
        .option_to("vntol", "vntol")
        .option_to("chgtol", "chgtol")
        .option_to("gmin", "gmin")
        .option_to("temp", "temp")
        .option_to("tnom", "tnom")
        .option_to("scale", "scale")
        // `method=trap|gear` is `tran_method` (`docs/cmd-options-tran.md`).
        .option_method("method")
        // Keys that only steer a simulator's bookkeeping or printing:
        // dropping one changes no number in the raw file.
        .option_cosmetic("noacct")
        .option_cosmetic("acct")
        .option_cosmetic("list")
        .option_cosmetic("node")
        .option_cosmetic("post")
        .option_cosmetic("trans")
        .option_cosmetic("nopage")
        .option_cosmetic("nomod")
        .option_cosmetic("lvlcod")
        .option_cosmetic("klu")
        .option_cosmetic("sparse")
        .option_cosmetic("savecurrents")
        .option_cosmetic("numdgt")
        .spectre_analysis("hb")
        .spectre_analysis("sweep")
        .spectre_analysis("xf")
        .spectre_analysis("montecarlo")
        // The circuit temperature in an expression (docs/expr-special.md).
        .temp_name("$temp")
        // docs/expr-constants.md: the builtin constants and their values.
        .builtin_constant("M_E", "2.718281828459045")
        .builtin_constant("M_LOG2E", "1.4426950408889634")
        .builtin_constant("M_LOG10E", "0.4342944819032518")
        .builtin_constant("M_LN2", "0.6931471805599453")
        .builtin_constant("M_LN10", "2.302585092994046")
        .builtin_constant("M_PI", "3.141592653589793")
        .builtin_constant("M_TWO_PI", "6.283185307179586")
        .builtin_constant("M_PI_2", "1.5707963267948966")
        .builtin_constant("M_PI_4", "0.7853981633974483")
        .builtin_constant("M_1_PI", "0.3183098861837907")
        .builtin_constant("M_2_PI", "0.6366197723675814")
        .builtin_constant("M_2_SQRTPI", "1.1283791670955126")
        .builtin_constant("M_SQRT2", "1.4142135623730951")
        .builtin_constant("M_SQRT1_2", "0.7071067811865475")
        .builtin_constant("M_DEGPERRAD", "57.29577951308232")
        .builtin_constant("P_Q", "1.6021918e-19")
        .builtin_constant("P_C", "299792456.2")
        .builtin_constant("P_K", "1.3806226e-23")
        .builtin_constant("P_H", "6.6260755e-34")
        .builtin_constant("P_EPS0", "8.854187924e-12")
        .builtin_constant("P_U0", "1.2566370614359173e-06")
        .builtin_constant("P_CELSIUS0", "273.15")
        // The VACASK analyses it has (docs/cmd-analysis-*.md).
        .vacask_analysis("dcinc")
        .vacask_analysis("acxf")
        .vacask_analysis("acstb")
        .vacask_analysis("hb")
        .vacask_analysis("hbac")
        .vacask_analysis("hbnoise")
        .vacask_analysis("pss")
        .vacask_analysis("pac")
        .vacask_analysis("pnoise")
        .vacask_analysis("trannoise")
        // The capability column.
        .refuse(Construct::ForeignText, Code::W0001)
        .refuse(Construct::PrivateText, Code::W0039)
        .refuse(Construct::XyceSens, Code::W0015)
        .refuse(Construct::HspiceAnalysis, Code::W0015)
        .refuse(Construct::HspiceWLine, Code::W0105)
        .native(Construct::VacaskScript)
        .native(Construct::VacaskElaborate)
        .refuse(Construct::CaretReread, Code::W0002)
        .lift(Construct::StatisticalCall, Rule::VacaskCalls)
        .native(Construct::VerilogALoad)
        .native(Construct::OsdiLoad)
        .native(Construct::OsdiInstance)
        .native(Construct::LibWholeFile)
        .omit(Construct::NgBehavior, Code::W0030)
        .native(Construct::LibSection)
        .refuse(Construct::RclModel, Code::W0006)
        .refuse(Construct::ChargeCapacitor, Code::W0105)
        .refuse(Construct::BehaviouralSource, Code::W0102)
        .refuse(Construct::Laplace, Code::W0011)
        .refuse(Construct::RfPort, Code::W0007)
        .refuse(Construct::PwlRepeatV, Code::W0008)
        .refuse(Construct::PwlRepeatI, Code::W0008)
        .refuse(Construct::NoiseWave, Code::W0009)
        .native(Construct::Conditional)
        .refuse(Construct::BranchParam, Code::W0014)
        .refuse(Construct::Func, Code::W0106)
        .refuse(Construct::LocalFunc, Code::W0106)
        .refuse(Construct::TableSource, Code::W0102)
        .refuse(Construct::LossyLine, Code::W0105)
        .native(Construct::AmWave)
        .native(Construct::Step)
        .native(Construct::StepList)
        .refuse(Construct::TranStartup, Code::W0037)
        .native(Construct::DcLogList)
        .native(Construct::Tf)
        .refuse(Construct::Sens, Code::W0015)
        .refuse(Construct::SensAc, Code::W0015)
        .refuse(Construct::PoleZero, Code::W0015)
        .refuse(Construct::SParameter, Code::W0015)
        .native(Construct::Print)
        .omit(Construct::Save, Code::W0023)
        .refuse(Construct::UnmappedOption, Code::W0017)
        .lift(Construct::Statistics, Rule::StatisticsDraws)
        .refuse(Construct::Measure, Code::W0109)
    // A new construct's cell is appended below its stream's marker
    // (`dialects/mod.rs`), with its mirror in `tests/builtin_dialects.py`.
    // --- S1
    // --- S2
    // --- S3
}
