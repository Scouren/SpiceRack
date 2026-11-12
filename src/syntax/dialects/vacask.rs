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
        // the same quantity (`itl1` is `op_itl`, a row below). `scale`
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
        // `maxord=n` is `tran_maxord` (`lib/options.cpp`); the writer
        // pairs it with the `am`/`gear` method names that read it.
        .option_to("maxord", "tran_maxord")
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
        // HSPICE's SEARCH directories: the kit read each file under the
        // path its card names (`resolve::Kit::load`).
        .option_cosmetic("search")
        // HSPICE's and LTspice's listing and output keys (ngspice manual
        // §11.1.1 NOINIT; HSPICE WARN, MAXWARNS, MEASDGT; LTspice
        // `plotwinsize`): no number in the raw file.
        .option_cosmetic("warn")
        .option_cosmetic("maxwarns")
        .option_cosmetic("measdgt")
        .option_cosmetic("plotwinsize")
        .option_cosmetic("noinit")
        // The same quantity under VACASK's own word (36be560
        // `docs/cmd-options-op.md` `op_itl`, `-tran.md` `tran_itl`,
        // `-homotopy.md` `homotopy_gminsteps` and `homotopy_srcsteps`, a
        // step count with 0 = off as SPICE's; ZERO-LEDGER P4-009).
        // `maxord` is the `tran_maxord` row above: `tran_maxord` bounds
        // the variable-order methods only, so the writer pairs it with
        // `am`/`gear`, not the fixed-order `trap`/`gear2` (P3-004).
        .option_to("itl1", "op_itl")
        .option_to("itl4", "tran_itl")
        .option_to("gminsteps", "homotopy_gminsteps")
        .option_to("srcsteps", "homotopy_srcsteps")
        // RSHUNT, a resistor from every node to ground, is the inverse of
        // `gshunt` (`docs/cmd-options-temp.md`; ngspice-45 and VACASK both
        // give 3/7 V on a 1k/3k divider with 1k). NOOPITER, no direct
        // solve before the stepping, is `op_skipinitial`
        // (`docs/cmd-options-op.md`).
        .option_spell("rshunt", "gshunt=1/({})")
        .option_spell("noopiter", "op_skipinitial=1")
        // The module a SPICE `.model` type gets at a LEVEL (VACASK 36be560
        // `devices/spice/*.va`: `sp_<module>`, `type=±1`; ngspice-45
        // `inpdomod.c` LEVEL per type): `.model x NPN (LEVEL=4)` is VBIC, other
        // equations, so no row. R, C, L and switch types have none (`sp_resistor`
        // renames the card's `r`/`tc1` to `model_r`…); the line types are no
        // SPICE type here: passed through ("Model not found" beats a substitute).
        .model_master("nmos", &[1], "mos1", Some("1"))
        .model_master("nmos", &[2], "mos2", Some("1"))
        .model_master("nmos", &[3], "mos3", Some("1"))
        .model_master("nmos", &[6], "mos6", Some("1"))
        .model_master("nmos", &[9], "mos9", Some("1"))
        .model_master("nmos", &[8, 49], "bsim3v3", Some("1"))
        .model_master("nmos", &[14, 54], "bsim4v8", Some("1"))
        .model_master("pmos", &[1], "mos1", Some("-1"))
        .model_master("pmos", &[2], "mos2", Some("-1"))
        .model_master("pmos", &[3], "mos3", Some("-1"))
        .model_master("pmos", &[6], "mos6", Some("-1"))
        .model_master("pmos", &[9], "mos9", Some("-1"))
        .model_master("pmos", &[8, 49], "bsim3v3", Some("-1"))
        .model_master("pmos", &[14, 54], "bsim4v8", Some("-1"))
        .model_master("d", &[1], "diode", None)
        .model_master("npn", &[1], "bjt", Some("1"))
        .model_master("pnp", &[1], "bjt", Some("-1"))
        .model_master("njf", &[1], "jfet1", Some("1"))
        .model_master("pjf", &[1], "jfet1", Some("-1"))
        // LEVEL 2 (Parker-Skellern): `spice/jfet2.osdi` ships at 36be560
        // too (ZERO-LEDGER P4-001; NG dossier §4 M4). Was W0104 "no
        // master" though the module is built; `vacask_params.txt`'s
        // `jfet2` row was already generated and unused.
        .model_master("njf", &[2], "jfet2", Some("1"))
        .model_master("pjf", &[2], "jfet2", Some("-1"))
        .model_master("nmf", &[1], "mes1", Some("1"))
        .model_master("pmf", &[1], "mes1", Some("-1"))
        .model_master("vdmos", &[1], "vdmos", Some("1"))
        // Xyce's VBIC 1.3 (RG 7.10 §2.3.17: `Q` level 11 on three
        // terminals, 12 with the substrate, the thermal node `dt` optional
        // after them) is the CMC source VACASK builds too (36be560
        // `devices/vbic/vbic_1p3.va` L194-L229, `devices/CMakeLists.txt`
        // L10-L12): `vbic13` (c b e) and `vbic13_5t` (c b e s dt), each a
        // file of its own, named as it is. With `dt` left unconnected
        // `vbic13_5t` is `vbic13_4t` (both run: the same twelve digits), so
        // level 12 is one row. ngspice-45 has no BJT at 11 or 12 ("Unable
        // to find definition of given model").
        .model_master("npn", &[11], "vbic13", Some("1"))
        .model_master("pnp", &[11], "vbic13", Some("-1"))
        .model_master("npn", &[12], "vbic13_5t", Some("1"))
        .model_master("pnp", &[12], "vbic13_5t", Some("-1"))
        .model_load("vbic13", "vbic_1p3.osdi")
        .model_load("vbic13_5t", "vbic_1p3_5t.osdi")
        // PSP 103 (the CMC source, version 103.4): Xyce's `M` level 103
        // (RG 7.10 §2.3.20.12) and ESPice's level 1040 (its CMC QA decks,
        // read as ngspice text: ESPice 0.3.0 gives their PSP 103
        // references) are `psp103va` of `psp103v4.osdi` (36be560
        // `devices/psp103v4/psp103.va` L46, `devices/CMakeLists.txt` L13).
        // Each of the two simulators stops on the other's number. Xyce's
        // level 1031 is the self-heating build (`psp103t.va`, five
        // terminals), which VACASK does not ship: no row.
        .model_master("nmos", &[103, 1040], "psp103va", Some("1"))
        .model_master("pmos", &[103, 1040], "psp103va", Some("-1"))
        .model_load("psp103va", "psp103v4.osdi")
        .spice_model_type("nmos")
        .spice_model_type("pmos")
        .spice_model_type("npn")
        .spice_model_type("pnp")
        .spice_model_type("d")
        .spice_model_type("njf")
        .spice_model_type("pjf")
        .spice_model_type("nmf")
        .spice_model_type("pmf")
        .spice_model_type("vdmos")
        .spice_model_type("r")
        .spice_model_type("c")
        .spice_model_type("l")
        .spice_model_type("sw")
        .spice_model_type("csw")
        .spice_model_type("res")
        .spice_model_type("cap")
        .spice_model_type("ind")
        .spectre_analysis("hb")
        .spectre_analysis("sweep")
        .spectre_analysis("xf")
        .spectre_analysis("montecarlo")
        // Misclassified as "target lacks it" (W0015) though VACASK
        // 36be560 runs each one (ZERO-LEDGER P4-001; VS dossier §3 M4):
        // `pss`, `pac`, `pnoise`, `hbac`, `hbnoise` are its own spelling
        // of Spectre's same-named analyses ([SR] pp.771,748,759; no
        // VACASK page for hbac/hbnoise, run-confirmed). Kept untranslated
        // (W0001), not refused as absent (W0015); the writer still has no
        // `Ana` arm for them (`render/named/vacask_bench.rs`), only the
        // generic `Ana::Spectre` fallback.
        .spectre_analysis("pss")
        .spectre_analysis("pac")
        .spectre_analysis("pnoise")
        .spectre_analysis("hbac")
        .spectre_analysis("hbnoise")
        // Spectre's `stb` is VACASK's `acstb` (`docs/cmd-analysis-acstb.md`;
        // `demo/stb/osc.sim` runs it at 36be560; ZERO-LEDGER P4-106): the
        // target has it, so W0001 until P4-008 writes the `acstb` line.
        .spectre_analysis("stb")
        // SPICE's `.sp` (Spectre `sp`) is VACASK's `acsp`
        // (`docs/cmd-analysis-acsp.md`; `demo/sp/amp.sim` runs it at 36be560;
        // ZERO-LEDGER P4-107, VS dossier §3 M4).
        .spectre_analysis("sp")
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
        // `c1 (a b) i=ddt(<q>)`: the source line's `ddt`
        // (`docs/dev-builtin-behavioral.md` at 36be560; ZERO-LEDGER P4-017).
        .native(Construct::ChargeCapacitor)
        // `<name> (p n) v=<expr>` / `i=<expr>`, no master
        // (`docs/dev-builtin-behavioral.md` at 36be560; ZERO-LEDGER P4-002).
        .native(Construct::BehaviouralSource)
        .refuse(Construct::Laplace, Code::W0011)
        .native(Construct::RfPort)
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
        .lift(Construct::TranStartup, Rule::StartupRamp)
        .native(Construct::DcLogList)
        .native(Construct::Tf)
        .refuse(Construct::Sens, Code::W0015)
        .refuse(Construct::SensAc, Code::W0015)
        .refuse(Construct::PoleZero, Code::W0015)
        .native(Construct::SParameter)
        .native(Construct::Print)
        .omit(Construct::Save, Code::W0023)
        .refuse(Construct::UnmappedOption, Code::W0017)
        .lift(Construct::Statistics, Rule::StatisticsDraws)
        .refuse(Construct::Measure, Code::W0109)
        // A new construct's cell is appended below its stream's marker
        // (`dialects/mod.rs`), with its mirror in `tests/builtin_dialects.py`.
        // --- S1
        // --- S2
        // `alter` in the control block (`docs/cmd-alter.md`): the
        // statement is copied where it stood.
        .native(Construct::Epochs)
        // --- S3
        // --- S4
        // A statement another dialect's simulator runs to the same numbers
        // without (its `kept_word` rows) is left out, noted.
        .omit(Construct::InertText, Code::W0336)
}
