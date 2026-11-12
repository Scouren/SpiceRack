//! LTspice, declared with the definition API: its lexicon rows (each
//! rule's citation as `lexicon/ltspice.rs` gave it, where its tests still pin
//! it), its spelling, its behaviours and its capability column.
//!
//! Cites (as the old LTspice grammar did): ADI ltspice-reference `ai_ref/`
//! SPICE-SYNTAX-REFERENCE.md (SSR) "Netlist Structure" (title), "Comments"
//! (`*` lines, `;` anywhere), "Engineering Multipliers" (T G Meg K mil m u
//! n p f, `µ`, `6K34`); SIMULATION-COMMANDS-REFERENCE.md `.END`;
//! CIRCUIT-ELEMENTS-REFERENCE.md "Supported Operators" (`& | ^` Boolean,
//! `**`); LTspice XVII help DotParam.htm (`{expr}`).
//!
//! Contents:
//! - `pub(super) fn ltspice() -> DialectDef`

use crate::syntax::behaviour::{
    AnalysisForm, CardRules, Coupling, DotRule, ExprEval, LoadForm, NetlistRules, Parasitics,
};
use crate::syntax::def::{Devices, DialectDef, Keywords};
use crate::syntax::diag::Code;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, End, Join, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Alt, Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::token::Tag;

const OPERATORS: &[(&str, Tag)] = &[
    ("**", Tag::StarStar),
    ("==", Tag::EqualEqual),
    ("!=", Tag::BangEqual),
    ("<=", Tag::LessEqual),
    (">=", Tag::GreaterEqual),
];

/// SSR "Engineering Multipliers": no atto.
const SCALES: &[(&str, Scale)] = &[
    ("meg", Scale::Exp(6)),
    ("mil", Scale::Mil),
    ("µ", Scale::Exp(-6)),
    ("t", Scale::Exp(12)),
    ("g", Scale::Exp(9)),
    ("k", Scale::Exp(3)),
    ("m", Scale::Exp(-3)),
    ("u", Scale::Exp(-6)),
    ("n", Scale::Exp(-9)),
    ("p", Scale::Exp(-12)),
    ("f", Scale::Exp(-15)),
];

const OWN_KEYWORDS: &[(&str, Tag)] = &[
    (".params", Tag::KwParam),
    (".step", Tag::KwStep),
    (".fourier", Tag::Directive),
];

/// LTspice: case-blind, `*`/`;` comments, `+` continuation, `{…}`
/// expressions, `"…"` strings, no `$` or `//` comments.
pub(super) fn ltspice() -> DialectDef {
    let mut d = DialectDef::new("ltspice");
    d = d.title(true);
    d = d.fold_case(true);
    d = d.comment_line("*");
    d = d.comment_line(";");
    d = d.cont_skip("*");
    d = d.cont_skip(";");
    d = d.comment_tail(";");
    d = d.dollar(Dollar::Text);
    d = d.plus(true);
    d = d.backslash(Backslash::None);
    d = d.end(Some(End {
        col1: false,
        then_blank: false,
        stops: true,
    }));
    d = d.stop(None);
    d = d.join(Join::None);
    d = d.quote(Quote::Plain);
    // A `;` inside double quotes (a path) is no comment (unverified).
    d = d.string(Quoting {
        escapes: false,
        protects: true,
        across_breaks: false,
    });
    d = d.ident_start("$");
    d = d.ident_continue("$.");
    d = d.directive(Some(b'.'));
    for &(s, t) in OPERATORS {
        d = d.operator(s, t);
    }
    d = d.expr_braces(true);
    d = d.caret(Caret::Xor);
    // CER "Supported Operators (ascending precedence)": `& | ^`,
    // comparisons, `+ -`, `* / %`, `**`, `! ~`.
    d = d.infix(Tag::Amp, 2);
    d = d.infix(Tag::Pipe, 2);
    d = d.infix(Tag::Caret, 2);
    d = d.infix(Tag::EqualEqual, 3);
    d = d.infix(Tag::BangEqual, 3);
    d = d.infix(Tag::LessEqual, 3);
    d = d.infix(Tag::GreaterEqual, 3);
    d = d.infix(Tag::Less, 3);
    d = d.infix(Tag::Greater, 3);
    d = d.infix(Tag::Plus, 4);
    d = d.infix(Tag::Minus, 4);
    d = d.infix(Tag::Star, 5);
    d = d.infix(Tag::Slash, 5);
    d = d.infix(Tag::Percent, 5);
    d = d.infix(Tag::StarStar, 6);
    // LTspice 26.1.1, run by hand: `2**3**2` is 512 in a `.param` and in
    // a B source (ZERO-LEDGER P3-077).
    d = d.right(Tag::StarStar);
    d = d.prefix(Tag::Plus);
    d = d.prefix(Tag::Minus);
    d = d.prefix(Tag::Bang);
    d = d.prefix(Tag::Tilde);
    d = d.prefix_bp(7);
    d = d.probe("v");
    d = d.probe("i");
    d = d.exponent("eE");
    for &(s, k) in SCALES {
        d = d.scale(s, k);
    }
    d = d.tail(Tail::Ltspice);
    d = d.arithmetic(Arithmetic::Decimal);
    // `.fourier` is LTspice's own card, not `.four`.
    for &(s, t) in OWN_KEYWORDS {
        d = d.keyword(s, t);
    }
    d = d.keywords(Keywords::Spice);
    d = d.devices(Devices::Spice);
    d = d.devices(Devices::LossyLine);
    d
        // The spelling.
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm::Func)
        .subckt_params(Some("PARAMS:"))
        // The behaviours.
        .caret_power(false)
        .card_rules(CardRules::Ltspice)
        .load_form(LoadForm::Ltspice)
        .analysis_form(AnalysisForm::Ltspice)
        .expr_eval(ExprEval::Ltspice)
        .netlist_rules(NetlistRules::Ltspice)
        .parasitics(Parasitics::Ltspice)
        .coupling(Coupling::OneCard)
        .spectre_analysis("sweep")
        // The circuit temperature in an expression (help, temp).
        .temp_name("temp")
        // The kept words: statements only LTspice has (ZERO-LEDGER P6-003;
        // help, `.FRA`: a time-domain frequency response analysis, its `@`
        // analyzer and `&` probe devices), W0039 on any other target.
        .kept_word(".fra", Construct::PrivateText)
        .kept_word("@", Construct::PrivateText)
        .kept_word("&", Construct::PrivateText)
        // P4-067: every `A` line. Help, "A. Special Functions": "Analog
        // Devices proprietary special function/mixed mode simulation
        // devices" (gates, flip-flops, Schmitt triggers, the modulator,
        // sample-and-hold, phase detector, varistor); ngspice reads `A` as
        // an XSPICE code model, Xyce and HSPICE have no `A` element.
        .kept_word("a", Construct::PrivateText)
        // Its state files (ZERO-LEDGER P6-006; help, `.SAVESTATE`,
        // `.LOADSTATE`: "a proprietary format"; `.SAVEBIAS`: a file for
        // `.loadbias`, P5-026's).
        .kept_word(".savestate", Construct::PrivateText)
        .kept_word(".loadstate", Construct::PrivateText)
        .kept_word(".savebias", Construct::PrivateText)
        // The dot rules: `.backanno` names port currents for LTspice's
        // viewer and changes no number (LTspice help): left out, noted.
        .dot_rule("backanno", DotRule::Note(Code::W0036))
        // The capability column.
        .refuse(Construct::ForeignText, Code::W0001)
        .refuse(Construct::PrivateText, Code::W0039)
        .refuse(Construct::XyceSens, Code::W0015)
        .refuse(Construct::HspiceAnalysis, Code::W0015)
        .refuse(Construct::HspiceWLine, Code::W0105)
        .omit(Construct::VacaskScript, Code::W0120)
        .refuse(Construct::VacaskElaborate, Code::W0122)
        .refuse(Construct::CaretReread, Code::W0002)
        .refuse(Construct::StatisticalCall, Code::W0003)
        .refuse(Construct::VerilogALoad, Code::W0004)
        .refuse(Construct::OsdiLoad, Code::W0004)
        .refuse(Construct::OsdiInstance, Code::W0004)
        .native(Construct::LibWholeFile)
        .omit(Construct::NgBehavior, Code::W0030)
        .refuse(Construct::LibSection, Code::W0012)
        .refuse(Construct::RclModel, Code::W0006)
        .spell(Construct::ChargeCapacitor, Alt::ChargeBare)
        .native(Construct::BehaviouralSource)
        .lift(Construct::Laplace, Rule::LaplaceAttr)
        .refuse(Construct::RfPort, Code::W0007)
        .refuse(Construct::PwlRepeatV, Code::W0008)
        .refuse(Construct::PwlRepeatI, Code::W0008)
        .refuse(Construct::NoiseWave, Code::W0009)
        .refuse(Construct::Conditional, Code::W0013)
        .refuse(Construct::BranchParam, Code::W0014)
        .native(Construct::Func)
        .refuse(Construct::LocalFunc, Code::W0106)
        .refuse(Construct::TableSource, Code::W0102)
        .native(Construct::LossyLine)
        .refuse(Construct::AmWave, Code::W0108)
        .native(Construct::Step)
        .native(Construct::StepList)
        .native(Construct::TranStartup)
        .native(Construct::DcLogList)
        .native(Construct::Tf)
        .refuse(Construct::Sens, Code::W0015)
        .refuse(Construct::SensAc, Code::W0015)
        .refuse(Construct::PoleZero, Code::W0015)
        .refuse(Construct::SParameter, Code::W0015)
        .omit(Construct::Print, Code::W0023)
        .native(Construct::Save)
        .native(Construct::UnmappedOption)
        .lift(Construct::Statistics, Rule::StatisticsDraws)
        .native(Construct::Measure)
        // A new construct's cell is appended below its stream's marker
        // (`dialects/mod.rs`), with its mirror in `tests/builtin_dialects.py`.
        // --- S1
        // --- S2
        // The circuit states are the points of an index step, each state
        // parameter a `table()` of the index: a list step is sorted and
        // loses its repeats (help `.STEP`; run, ZERO-LEDGER P5-001 step
        // 5); a plan no table of states holds is W0130
        // (`render/spice/states.rs`).
        .spell(Construct::Epochs, Alt::StatesTable)
    // --- S3
}
