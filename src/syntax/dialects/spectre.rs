//! Spectre, declared with the definition API: its lexicon rows (each
//! rule's citation as `lexicon/spectre.rs` gave it, where its tests still pin
//! it), its spelling, its behaviours and its capability column.
//!
//! Cites (as the old Spectre grammar did; § = the labels it gives its rules,
//! `git show 9d1d6bb:grammar/spectre.peg`; UG = Spectre User Guide 5.1.41, R19 = Spectre Reference 19.1): §1.2 title
//! (UG p.29, p.50); §1.3 `*` in column 1 and `//` comments (UG p.59); §1.4
//! `\` and `+` continuation (UG p.59); §2.1 case kept; §2.2 names (`\`
//! escapes, `!`); §2.4 numbers (case-sensitive scales, `M` mega, `m` milli,
//! units after the scale); §2.5 strings; §3.1 the `simulator lang=` switch
//! (UG p.53); §6.1 operators (UG p.87).
//!
//! Contents:
//! - `pub(super) fn spectre() -> DialectDef`

use crate::syntax::Lang;
use crate::syntax::behaviour::{NamedParams, NamedRules, OptionsForm, Writer};
use crate::syntax::def::{Devices, DialectDef};
use crate::syntax::diag::Code;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, Join, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Alt, Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::statement::{Analysis, Command, Statement};
use crate::syntax::token::Tag;

/// §2.4: P T G M K/k `_` %/c m u n p f a.
const SCALES: &[(&str, Scale)] = &[
    ("P", Scale::Exp(15)),
    ("T", Scale::Exp(12)),
    ("G", Scale::Exp(9)),
    ("M", Scale::Exp(6)),
    // `k` first: the writer spells kilo with the first (Cadence's own
    // examples write `1k`).
    ("k", Scale::Exp(3)),
    ("K", Scale::Exp(3)),
    ("_", Scale::Exp(0)),
    ("%", Scale::Exp(-2)),
    ("c", Scale::Exp(-2)),
    ("m", Scale::Exp(-3)),
    ("u", Scale::Exp(-6)),
    ("n", Scale::Exp(-9)),
    ("p", Scale::Exp(-12)),
    ("f", Scale::Exp(-15)),
    ("a", Scale::Exp(-18)),
];

/// The statement words of the old Spectre grammar (R19 pp.175-179, p.491,
/// p.498, p.506; UG p.92, p.94).
const KEYWORDS: &[(&str, Tag)] = &[
    ("simulator", Tag::KwSimulator),
    ("include", Tag::KwInclude),
    ("ahdl_include", Tag::KwAhdlInclude),
    ("library", Tag::KwLibrary),
    ("endlibrary", Tag::KwEndlibrary),
    ("section", Tag::KwSection),
    ("endsection", Tag::KwEndsection),
    ("inline", Tag::KwInline),
    ("subckt", Tag::KwSubckt),
    ("ends", Tag::KwEnds),
    ("model", Tag::KwModel),
    ("parameters", Tag::KwParam),
    ("statistics", Tag::KwStatistics),
    ("process", Tag::KwProcess),
    ("mismatch", Tag::KwMismatch),
    ("vary", Tag::KwVary),
    ("correlate", Tag::KwCorrelate),
    ("truncate", Tag::KwTruncate),
    ("if", Tag::KwIf),
    ("else", Tag::KwElse),
    ("real", Tag::KwReal),
    ("return", Tag::KwReturn),
    ("global", Tag::KwGlobal),
    ("save", Tag::KwSave),
    ("ic", Tag::KwIc),
    ("nodeset", Tag::KwNodeset),
];

/// The analysis and control-statement masters (`statement.rs` cites the
/// `[REF]` pages), in the old reader's order.
const MASTERS: &[(&str, Statement)] = {
    use Analysis as A;
    use Statement::{Analysis as An, Command as C};
    &[
        ("montecarlo", An(A::Montecarlo)),
        ("acmatch", An(A::Acmatch)),
        ("dcmatch", An(A::Dcmatch)),
        ("hbnoise", An(A::Hbnoise)),
        ("qpnoise", An(A::Qpnoise)),
        ("fourier", An(A::Fourier)),
        ("pdisto", An(A::Pdisto)),
        ("pnoise", An(A::Pnoise)),
        ("hbstb", An(A::Hbstb)),
        ("envlp", An(A::Envlp)),
        ("hbsp", An(A::Hbsp)),
        ("hbac", An(A::Hbac)),
        ("hbxf", An(A::Hbxf)),
        ("noise", An(A::Noise)),
        ("qpac", An(A::Qpac)),
        ("qpsp", An(A::Qpsp)),
        ("qpss", An(A::Qpss)),
        ("qpxf", An(A::Qpxf)),
        ("sweep", An(A::Sweep)),
        ("pac", An(A::Pac)),
        ("psp", An(A::Psp)),
        ("pss", An(A::Pss)),
        ("pstb", An(A::Pstb)),
        ("pxf", An(A::Pxf)),
        ("sens", An(A::Sens)),
        ("tran", An(A::Tran)),
        ("tdr", An(A::Tdr)),
        ("stb", An(A::Stb)),
        ("ac", An(A::Ac)),
        ("dc", An(A::Dc)),
        ("hb", An(A::Hb)),
        ("pz", An(A::Pz)),
        ("sp", An(A::Sp)),
        ("xf", An(A::Xf)),
        ("altergroup", C(Command::Altergroup)),
        ("checklimit", C(Command::Checklimit)),
        ("paramtest", C(Command::Paramtest)),
        ("paramset", C(Command::Paramset)),
        ("options", Statement::Options),
        ("assert", C(Command::Assert)),
        ("alter", C(Command::Alter)),
        ("check", C(Command::Check)),
        ("shell", C(Command::Shell)),
        ("info", C(Command::Info)),
        ("set", C(Command::Set)),
    ]
};

/// Spectre: case kept, `*` (column 1) and `//` comments, `\` and `+`
/// continuation, keywords at a statement's head, `M` mega; a
/// `simulator lang=spice` region reads as HSPICE.
pub(super) fn spectre() -> DialectDef {
    let mut d = DialectDef::new("spectre");
    d = d.title(true);
    d = d.comment_line("//");
    d = d.comment_col1(b'*');
    d = d.cont_skip("//");
    d = d.comment_tail("//");
    d = d.dollar(Dollar::Text);
    d = d.plus(true);
    d = d.backslash(Backslash::Join);
    d = d.end(None);
    d = d.stop(None);
    d = d.join(Join::Parens);
    d = d.quote(Quote::Plain);
    d = d.string(Quoting {
        escapes: true,
        protects: true,
        across_breaks: false,
    });
    d = d.name_escape(true);
    d = d.ident_start("$");
    d = d.ident_continue("!$");
    d = d.directive(None);
    d = d.head_words(true);
    d = d.head_after(Tag::LBrace);
    d = d.head_after(Tag::RBrace);
    d = d.head_after(Tag::KwInline);
    d = d.head_after(Tag::KwElse);
    d = d.operator("**", Tag::StarStar);
    d = d.operator("==", Tag::EqualEqual);
    d = d.operator("!=", Tag::BangEqual);
    d = d.operator("<=", Tag::LessEqual);
    d = d.operator(">=", Tag::GreaterEqual);
    d = d.operator("&&", Tag::AmpAmp);
    d = d.operator("||", Tag::PipePipe);
    d = d.caret(Caret::None);
    // §6.1 (UG p.87), decreasing: unary, `**` (right), `* / %`, `+ -`,
    // relational, equality, `&&`, `||`, `?:`.
    d = d.infix(Tag::PipePipe, 2);
    d = d.infix(Tag::AmpAmp, 3);
    d = d.infix(Tag::EqualEqual, 4);
    d = d.infix(Tag::BangEqual, 4);
    d = d.infix(Tag::LessEqual, 5);
    d = d.infix(Tag::GreaterEqual, 5);
    d = d.infix(Tag::Less, 5);
    d = d.infix(Tag::Greater, 5);
    d = d.infix(Tag::Plus, 6);
    d = d.infix(Tag::Minus, 6);
    d = d.infix(Tag::Star, 7);
    d = d.infix(Tag::Slash, 7);
    d = d.infix(Tag::Percent, 7);
    d = d.infix(Tag::StarStar, 8);
    d = d.right(Tag::StarStar);
    d = d.prefix(Tag::Plus);
    d = d.prefix(Tag::Minus);
    d = d.prefix(Tag::Bang);
    d = d.prefix_bp(9);
    d = d.lib_sections(true);
    d = d.exponent("eE");
    for &(s, k) in SCALES {
        d = d.scale(s, k);
    }
    d = d.tail(Tail::Letters {
        after_exponent: true,
        hex: false,
    });
    d = d.arithmetic(Arithmetic::Decimal);
    for &(s, t) in KEYWORDS {
        d = d.keyword(s, t);
    }
    d = d.named(true);
    d = d.devices(Devices::Spectre);
    for &(s, m) in MASTERS {
        d = d.master(s, m);
    }
    d = d.region("spice", Lang::HSPICE);
    d
        // The spelling.
        .comment("//")
        .path_quote("\"")
        .expr("", "")
        .func(FuncForm::Real)
        .subckt_params(None)
        // The behaviours.
        .writer(Writer::Spectre)
        .named_rules(NamedRules::Spectre)
        .options_form(OptionsForm::Spectre)
        .named_params(NamedParams::Spectre)
        // The capability column.
        .refuse(Construct::ForeignText, Code::W0001)
        .refuse(Construct::PrivateText, Code::W0039)
        .refuse(Construct::XyceSens, Code::W0015)
        .refuse(Construct::HspiceAnalysis, Code::W0015)
        .omit(Construct::VacaskScript, Code::W0120)
        .refuse(Construct::CaretReread, Code::W0002)
        .lift(Construct::StatisticalCall, Rule::RackVars)
        .native(Construct::VerilogALoad)
        .native(Construct::OsdiLoad)
        .native(Construct::OsdiInstance)
        .native(Construct::LibWholeFile)
        .omit(Construct::NgBehavior, Code::W0030)
        .native(Construct::LibSection)
        .refuse(Construct::RclModel, Code::W0006)
        .spell(Construct::ChargeCapacitor, Alt::BsourceCharge)
        .spell(Construct::BehaviouralSource, Alt::Bsource)
        .refuse(Construct::Laplace, Code::W0011)
        .native(Construct::RfPort)
        .spell(Construct::PwlRepeatV, Alt::PwlPeriod)
        .spell(Construct::PwlRepeatI, Alt::PwlPeriod)
        .refuse(Construct::NoiseWave, Code::W0009)
        .native(Construct::Conditional)
        .native(Construct::BranchParam)
        .native(Construct::Func)
        .refuse(Construct::LocalFunc, Code::W0106)
        .refuse(Construct::TableSource, Code::W0102)
        .refuse(Construct::LossyLine, Code::W0105)
        .native(Construct::Step)
        .native(Construct::StepList)
        .refuse(Construct::TranStartup, Code::W0037)
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
        .spell(Construct::Measure, Alt::SpiceRegion)
}
