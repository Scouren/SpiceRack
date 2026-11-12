//! The `control` block → run items in listed order (R25): `options` (`options.rs`), `sweep` +
//! `analysis` (`analysis.rs`: a `Dc` over an `op`, a `Loop` of kind sweep around any other),
//! `save` (`read_outputs`), `alter`, `elaborate`, `clear`, `var`, `mc … endmc`,
//! `print`/`abort`/`postprocess` (`plan.rs`). Comments keep their place among the statements.
//!
//! Rules (each with its evidence):
//! - one control block per file, at the top level (VD cmd-overview; run x1: "Only one control
//!   block is allowed."): a second one, or one in a subcircuit, is refused whole;
//! - a `sweep` binds the next `sweep` or `analysis` (VD cmd-sweep; run x3: "expecting sweep or
//!   analysis"): a sweep followed by anything else is refused;
//! - `mc … endmc` is not a CST block: an explicit stack of one (VD cmd-analysis-mc: no nesting);
//!   an `mc` with no `endmc` after it, and an `endmc` with no `mc`, are refused;
//! - every state item holds `FromHere` (A14, R24): VACASK applies options, variables and saves
//!   before the next analysis (VD cmd-overview "Execution model").
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.4; R43, A22; VD cmd-overview, cmd-sweep,
//! cmd-analysis-mc.
//!
//! Contents: `FEATURES`, `CODES`, the codes (`OPTION_PRIVATE`, `ANALYSIS_FIELD`, `ANALYSIS_HB_SMALL`, `PSS_PERIOD`,
//! `OWN_PSS`, `HB_FORM`, `VARIABLE`, `MC_FIELD`, `CLEAR_STATE`, `ELABORATE_TOP`, `SWEEP_FORM`,
//! `ALTER_TYPE`, `SWEEP_WAVE`, `SOURCE_SWEEP_WAVE`, `SOURCE_SWEEP_NAME`, `OUTPUT_FORM`, `MC_NESTED`, `NODE_SET_PLACE`, `REJECTED`, `SAVE_CATEGORY`, `OPTION_ROW_PRIVATE`, `RESET_CLASS`, `NAME_SPELLING`), `bare_name`, `identifier`, `control`,
//! and the area's helpers `assign`, `string`, `field_text`, `push_opaque`, `head` (crate-private)

mod analysis;
mod options;
mod plan;
mod sweep;

use super::codes::UNCLOSED;
use super::read::{self, Lower};
use super::read_outputs;
use crate::ir;
use crate::syntax::ast::{NONE, Node, Tag};

/// What this area's rows use: the reader's image (law L5).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::placed(
    ir::ScopeKind::Loop,
    &[
        ir::ItemKind::Analysis,
        ir::ItemKind::Loop,
        ir::ItemKind::NodeSet,
        ir::ItemKind::Option,
        ir::ItemKind::Env,
        ir::ItemKind::Save,
        ir::ItemKind::Alter,
        ir::ItemKind::Elaborate,
        ir::ItemKind::Reset,
        ir::ItemKind::Comment,
        ir::ItemKind::Opaque,
    ],
)
.with(ir::Feature::Item(ir::ItemKind::Analysis))
.with(ir::Feature::Item(ir::ItemKind::Loop))
.with(ir::Feature::Item(ir::ItemKind::Alter))
.with(ir::Feature::Item(ir::ItemKind::Option))
.with(ir::Feature::Item(ir::ItemKind::Env))
.with(ir::Feature::Item(ir::ItemKind::Elaborate))
.with(ir::Feature::Item(ir::ItemKind::Reset))
.with(ir::Feature::Run(ir::RunExtra::ResetSaves))
.with(ir::Feature::Analysis(ir::AnalysisKind::Op))
.with(ir::Feature::Analysis(ir::AnalysisKind::Dc))
.with(ir::Feature::Analysis(ir::AnalysisKind::Ac))
.with(ir::Feature::Analysis(ir::AnalysisKind::Tran))
.with(ir::Feature::Analysis(ir::AnalysisKind::Noise))
.with(ir::Feature::Analysis(ir::AnalysisKind::DcXf))
.with(ir::Feature::Analysis(ir::AnalysisKind::Xf))
.with(ir::Feature::Analysis(ir::AnalysisKind::Sp))
.with(ir::Feature::Analysis(ir::AnalysisKind::Stb))
.with(ir::Feature::Analysis(ir::AnalysisKind::DcInc))
.with(ir::Feature::Option(ir::OptionKey::RelTol))
.with(ir::Feature::Option(ir::OptionKey::AbsTol))
.with(ir::Feature::Option(ir::OptionKey::VnTol))
.with(ir::Feature::Option(ir::OptionKey::ChgTol))
.with(ir::Feature::Option(ir::OptionKey::Gmin))
.with(ir::Feature::Option(ir::OptionKey::Gshunt))
.with(ir::Feature::Option(ir::OptionKey::Scale))
.with(ir::Feature::Option(ir::OptionKey::MaxOrd))
.with(ir::Feature::Option(ir::OptionKey::MethodTrap))
.with(ir::Feature::Option(ir::OptionKey::MethodEuler))
.with(ir::Feature::Option(ir::OptionKey::Itl1))
.with(ir::Feature::Option(ir::OptionKey::Itl4))
.with(ir::Feature::Option(ir::OptionKey::GminSteps))
.with(ir::Feature::Option(ir::OptionKey::SrcSteps))
.with(ir::Feature::Option(ir::OptionKey::NoOpIter))
// Only as a sweep's `option=` target: an option row of a private key is refused (W2304).
.with(ir::Feature::Option(ir::OptionKey::Private))
.with(ir::Feature::Run(ir::RunExtra::SeveralAnalyses))
.with(ir::Feature::Run(ir::RunExtra::AnalysisNamed))
.with(ir::Feature::Run(ir::RunExtra::DcAxisNamed))
.with(ir::Feature::Run(ir::RunExtra::LoopNamed))
// every state item holds from where it stands: `options` (an option row, the temperatures as
// an `Env`) and an analysis line's `ic=`/`nodeset=` lists (next-only node sets)
.with(ir::Feature::Run(ir::RunExtra::OptionFromHere))
.with(ir::Feature::Run(ir::RunExtra::EnvFromHere))
.with(ir::Feature::Run(ir::RunExtra::NodeSetFromHere))
.with(ir::Feature::Run(ir::RunExtra::TranSimulatorStep))
.with(ir::Feature::Run(ir::RunExtra::TranRecordStart))
.with(ir::Feature::Run(ir::RunExtra::TranMaxStep))
.with(ir::Feature::Run(ir::RunExtra::TranInitialStep))
.with(ir::Feature::Run(ir::RunExtra::TranCapSpan50))
.with(ir::Feature::Run(ir::RunExtra::TranUic))
.with(ir::Feature::Run(ir::RunExtra::TranNoise))
.with(ir::Feature::Run(ir::RunExtra::DcNested))
.with(ir::Feature::Run(ir::RunExtra::DcAxisDeviceParam))
.with(ir::Feature::Run(ir::RunExtra::DcAxisModelParam))
.with(ir::Feature::Run(ir::RunExtra::DcAxisTemp))
.with(ir::Feature::Run(ir::RunExtra::DcAxisOption))
.with(ir::Feature::Run(ir::RunExtra::SweepDcLinStep))
.with(ir::Feature::Run(ir::RunExtra::SweepDcLinPoints))
.with(ir::Feature::Run(ir::RunExtra::SweepDcDec))
.with(ir::Feature::Run(ir::RunExtra::SweepDcOct))
.with(ir::Feature::Run(ir::RunExtra::SweepDcList))
.with(ir::Feature::Run(ir::RunExtra::SweepLoopLinStep))
.with(ir::Feature::Run(ir::RunExtra::SweepLoopLinPoints))
.with(ir::Feature::Run(ir::RunExtra::SweepLoopDec))
.with(ir::Feature::Run(ir::RunExtra::SweepLoopOct))
.with(ir::Feature::Run(ir::RunExtra::SweepLoopList))
.with(ir::Feature::Run(ir::RunExtra::SweepFreqLinStep))
.with(ir::Feature::Run(ir::RunExtra::SweepFreqLinPoints))
.with(ir::Feature::Run(ir::RunExtra::SweepFreqDec))
.with(ir::Feature::Run(ir::RunExtra::SweepFreqOct))
.with(ir::Feature::Run(ir::RunExtra::SweepFreqList))
.with(ir::Feature::Run(ir::RunExtra::SpPorts))
.with(ir::Feature::Run(ir::RunExtra::LoopMonteCarlo))
.with(ir::Feature::Run(ir::RunExtra::LoopRestore))
.with(ir::Feature::Run(ir::RunExtra::LoopScoped))
.with(ir::Feature::Run(ir::RunExtra::LoopTemp))
.with(ir::Feature::Run(ir::RunExtra::LoopSourceDc))
.with(ir::Feature::Run(ir::RunExtra::LoopDeviceParam))
.with(ir::Feature::Run(ir::RunExtra::LoopModelParam))
.with(ir::Feature::Run(ir::RunExtra::LoopOption))
.with(ir::Feature::Run(ir::RunExtra::AlterDeviceParam))
.with(ir::Feature::Run(ir::RunExtra::AlterModelParam))
.with(ir::Feature::Run(ir::RunExtra::TargetInInstance))
.with(ir::Feature::Run(ir::RunExtra::McSeed))
.with(ir::Feature::Run(ir::RunExtra::McLatinHypercube))
.with(ir::Feature::Run(ir::RunExtra::OptionMidRun))
.with(ir::Feature::Run(ir::RunExtra::EnvMidRun))
.with(ir::Feature::Run(ir::RunExtra::NodeSetMidRun));

/// A key only VACASK has that names an accuracy target or changes the equations.
pub(super) const OPTION_PRIVATE: ir::Code = ir::Code::new(b'P', 2301);
/// An analysis parameter with no IR field (a solution slot, `write`, a solver, …).
pub(super) const ANALYSIS_FIELD: ir::Code = ir::Code::new(b'P', 2302);
/// `hbac`, `hbnoise`.
pub(super) const ANALYSIS_HB_SMALL: ir::Code = ir::Code::new(b'P', 2303);
/// `pss` given by its period.
pub(super) const PSS_PERIOD: ir::Code = ir::Code::new(b'P', 2304);
/// `pac`, `pnoise` with their own steady-state solve.
pub(super) const OWN_PSS: ir::Code = ir::Code::new(b'P', 2305);
/// `hb` with the default truncation or a scalar harmonic count.
pub(super) const HB_FORM: ir::Code = ir::Code::new(b'P', 2306);
/// `var`, and a sweep of a circuit variable.
pub(super) const VARIABLE: ir::Code = ir::Code::new(b'P', 2307);
/// An `mc` parameter with no field (`strict`, `debug`).
pub(super) const MC_FIELD: ir::Code = ir::Code::new(b'P', 2309);
/// `clear options`, `clear variables`, `clear`.
pub(super) const CLEAR_STATE: ir::Code = ir::Code::new(b'P', 2310);
/// `elaborate` with `topdef` or `topinst`.
pub(super) const ELABORATE_TOP: ir::Code = ir::Code::new(b'P', 2311);
/// A sweep form with no field (the principal parameter, `continuation=0`, values by name).
pub(super) const SWEEP_FORM: ir::Code = ir::Code::new(b'P', 2312);
/// An `alter` of a source's `type` (the fold of R42 is the scaffold's declare step, not yet).
pub(super) const ALTER_TYPE: ir::Code = ir::Code::new(b'P', 2313);
/// A sweep of a waveform source's `dc`, which VACASK ignores (settle run/sweep_source).
pub(super) const SWEEP_WAVE: ir::Code = ir::Code::new(b'P', 2314);
/// A sweep of a source's DC value where the source has a wave and its DC is the wave's: a
/// feature candidate (class F: ngspice has the form; a pass giving such a source its wave's
/// value at zero as a given DC, spelled as R42's fold, would convert it when a pair deck needs it).
pub(super) const SOURCE_SWEEP_WAVE: ir::Code = ir::Code::new(b'W', 2307);
/// A sweep of a source's DC value whose element is no independent source found from the top.
pub(super) const SOURCE_SWEEP_NAME: ir::Code = ir::Code::new(b'W', 2309);
/// A transfer or noise analysis whose output is no node or node pair.
pub(super) const OUTPUT_FORM: ir::Code = ir::Code::new(b'W', 2311);
/// An `mc` loop inside another.
pub(super) const MC_NESTED: ir::Code = ir::Code::new(b'W', 2310);
/// A node set no analysis line that takes its list follows.
pub(super) const NODE_SET_PLACE: ir::Code = ir::Code::new(b'W', 2308);
/// A control statement VACASK rejects.
pub(super) const REJECTED: ir::Code = ir::Code::new(b'P', 2320);

/// A save of a probe category VACASK has no directive for (refused by `write_outputs::admits`).
pub(super) const SAVE_CATEGORY: ir::Code = ir::Code::new(b'W', 2303);

/// An option row of a private key: VACASK's own options are read as Hint or Private opaques
/// (refused by `write_run::admits`).
pub(super) const OPTION_ROW_PRIVATE: ir::Code = ir::Code::new(b'W', 2304);

/// A reset of a class other than the saves alone (VACASK's `clear saves`; refused by
/// `write_run::admits`).
pub(super) const RESET_CLASS: ir::Code = ir::Code::new(b'W', 2305);

/// An analysis, sweep or mc name VACASK cannot spell: no identifier, or a sweep name repeated
/// in one analysis's chain (refused by `write_run::admits`; settle names_quoted, names_chain).
pub(super) const NAME_SPELLING: ir::Code = ir::Code::new(b'W', 2306);

/// This area's rows (the writer's two notes too: `write_run` has no table of its own).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    row(
        OPTION_PRIVATE,
        "an option only VACASK has, kept as its text",
        "VD cmd-options-tol, -relref, -temp, -tran, -pss; R19 (an accuracy target is Private)",
        "The key scales an accuracy target or changes the equations (tolscale, tolmode, fluxtol, relref, minr, a tran_method other than trap or euler, …); VACASK's writer copies the group, another dialect refuses it.",
        ir::Kind::Note,
    ),
    row(
        ANALYSIS_FIELD,
        "an analysis parameter the IR has no field for",
        "VD cmd-analysis-*; R43",
        "store, write, writeop, opsolve, solver, opsolver, a stored-solution name as ic or nodeset, localgnd, the transient-noise generator's knobs: the analysis (and its sweeps) are kept as text.",
        ir::Kind::Note,
    ),
    row(
        ANALYSIS_HB_SMALL,
        "an hbac or hbnoise analysis, kept as its text",
        "VD cmd-analysis-hbac, -hbnoise; COVERAGE-run",
        "The harmonic-balance small-signal analyses carry their own large-signal solve; no IR row states them yet.",
        ir::Kind::Note,
    ),
    row(
        PSS_PERIOD,
        "a pss given by its period, kept as its text",
        "VD cmd-analysis-pss (tper); R43",
        "The IR's Pss states a fundamental frequency; 1/tper is a division the reader may not do.",
        ir::Kind::Note,
    ),
    row(
        OWN_PSS,
        "a pac or pnoise with its own steady-state solve, kept as its text",
        "VD cmd-analysis-pac, -pnoise (psssolve=1); R43",
        "VACASK's periodic small-signal analyses solve their own PSS; the IR's Pac and Pnoise follow a separate Pss.",
        ir::Kind::Note,
    ),
    row(
        HB_FORM,
        "an hb analysis, kept as its text",
        "VD cmd-analysis-hb; R43",
        "VACASK's default truncation (hybrid) is not shown to be the IR's, and a scalar nharm over several tones would be copied per tone (R13 b).",
        ir::Kind::Note,
    ),
    row(
        VARIABLE,
        "a circuit variable, kept as its text",
        "VD cmd-var, expr-cirvars; R43",
        "A circuit variable is looked up below parameters; the IR has no home for one yet, so a var statement and a sweep of a variable are kept as text.",
        ir::Kind::Note,
    ),
    row(
        MC_FIELD,
        "an mc loop with strict or debug, kept as its text",
        "VD cmd-analysis-mc",
        "The loop, its body and its endmc are kept as text.",
        ir::Kind::Note,
    ),
    row(
        CLEAR_STATE,
        "a clear of options or variables, kept as its text",
        "VD cmd-options-set, cmd-save, cmd-var",
        "Resetting every option or variable to its default has no IR row.",
        ir::Kind::Note,
    ),
    row(
        ELABORATE_TOP,
        "an elaborate with topdef or topinst, kept as its text",
        "VD cmd-elaboration",
        "The names of the top definition and instance have no IR field.",
        ir::Kind::Note,
    ),
    row(
        SWEEP_FORM,
        "a sweep form the IR has no field for, kept as its text",
        "VD cmd-sweep",
        "A sweep of an instance's principal parameter (only its module knows which), continuation=0, values given by a variable's name: the sweep and its analysis are kept as text.",
        ir::Kind::Note,
    ),
    row(
        ALTER_TYPE,
        "an alter of a source's type, kept as its text",
        "R42 (the dc/wave alternation folds into the source; any other type alter is Private)",
        "The alternation's recognition is the scaffold's declare step; until it lands every type alter is kept as text.",
        ir::Kind::Note,
    ),
    row(
        SWEEP_WAVE,
        "a sweep of a waveform source's dc, kept as its text",
        "R64; settle run/sweep_source (VACASK 36be560)",
        "VACASK reads `dc` only on a source of type dc: a sweep of `dc` on a pulse, sine or other waveform source leaves its value as the wave gives it (0, 0 where ngspice's .dc gives 2, 4), a meaning only VACASK has.",
        ir::Kind::Note,
    ),
    row(
        SOURCE_SWEEP_WAVE,
        "a sweep of the DC value of a source whose DC is its wave's",
        "R64; settle run/sweep_source (VACASK 36be560)",
        "VACASK ignores `dc` on a waveform source, so the sweep would move nothing (0, 0 where ngspice's .dc gives 2, 4); the source needs a given DC value (R42's type alternation) to be swept.",
        ir::Kind::Refusal,
    ),
    row(
        SOURCE_SWEEP_NAME,
        "a sweep of the DC value of a source VACASK would not find",
        "R64; VD cmd-sweep (instance= is a hierarchical instance name)",
        "VACASK finds a swept instance by its path from the top; a source-DC target whose path or name names no independent source there reads back as a device parameter of an unknown instance.",
        ir::Kind::Refusal,
    ),
    row(
        OUTPUT_FORM,
        "a transfer or noise analysis whose output is no node voltage",
        "VD cmd-analysis-dcxf, -acxf, -noise (`out`: one node or a node pair)",
        "VACASK's dcxf, acxf and noise name their output as a node or a node pair; a current or another probe has no VACASK spelling there (it would read back as a node of that name).",
        ir::Kind::Refusal,
    ),
    row(
        MC_NESTED,
        "an mc loop inside another",
        "VD cmd-analysis-mc (an mc block cannot be nested); the reader refuses one (P2320)",
        "VACASK runs one Monte Carlo loop at a time; a loop of samples inside another has no VACASK spelling.",
        ir::Kind::Refusal,
    ),
    row(
        NODE_SET_PLACE,
        "a node set no analysis that takes it follows",
        "VD cmd-analysis-op, -tran, -dcinc, -dcxf (`ic=` on tran, `nodeset=` on op, dcinc, dcxf and tran)",
        "VACASK states an initial condition or a nodeset only as a list on the analysis line it applies to; a node set before any other item, or before an analysis without that list, has no VACASK spelling.",
        ir::Kind::Refusal,
    ),
    row(
        REJECTED,
        "a control statement VACASK rejects",
        "VD cmd-overview, cmd-sweep, cmd-analysis-*, cmd-options; runs e3, e6, x1, x3",
        "An unknown option or analysis parameter, an unknown analysis kind, a sweep not followed by a sweep or an analysis, an unmatched mc or endmc, a second control block: VACASK stops with an error.",
        ir::Kind::Refusal,
    ),
    row(
        OPTION_ROW_PRIVATE,
        "an option row of a simulator's own key",
        "R19; DIALECT-VACASK §3.4 options",
        "VACASK's reader keeps its own options as Hint or Private text, never as a private option row; another simulator's private option has no VACASK spelling.",
        ir::Kind::Refusal,
    ),
    row(
        RESET_CLASS,
        "a reset VACASK has no statement for",
        "VD cmd-save, cmd-options-set; run sv2",
        "VACASK's `clear saves` restores the saves alone; `clear options` also resets what the IR holds as opaque options and the temperatures, so it is kept as text, and no other reset has a spelling.",
        ir::Kind::Refusal,
    ),
    row(
        NAME_SPELLING,
        "an analysis or sweep name VACASK cannot spell",
        "settle names_quoted, names_chain (VACASK 36be560)",
        "VACASK reads an analysis, sweep or mc name as a bare identifier only (a quoted name is \"Syntax error, unexpected string.\"), and the sweeps of one analysis must have distinct names (\"Sweep does not have a unique name.\").",
        ir::Kind::Refusal,
    ),
    row(
        SAVE_CATEGORY,
        "a save of a probe category VACASK has no directive for",
        "VD cmd-save; run e5",
        "VACASK's one category is `default`, every node voltage and every branch current together; every node voltage alone, every branch current alone or another category has no directive.",
        ir::Kind::Refusal,
    ),
];

/// `control` … `endc`: every statement of the block, in order, into the file's top scope.
pub(super) fn control(l: &mut Lower<'_, '_, '_>, n: Node) {
    let ast = &l.cst.ast;
    let first = ast
        .fields(ast.root())
        .iter()
        .copied()
        .find(|&s| ast.tag(Node(s)) == Tag::Control);
    let top = l.b.design().file(l.cst.file).top;
    if first != Some(n.0) || l.scope != top {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
    }
    let body = ast.body(n).to_vec();
    let before = l.b.design().items(l.scope).len();
    let saved = (l.scope, l.stmt);
    let mut sweeps: Vec<Node> = Vec::new();
    // The open `mc`: the scope it was opened from, and whether it is kept as text.
    let mut mc: Option<(ir::ScopeId, bool)> = None;
    for (k, &s) in body.iter().enumerate() {
        let s = Node(s);
        let start = l.start(s);
        l.comments_until(start, true);
        l.stmt = l.span(s);
        if read::unclosed(l.cst, s) {
            for sw in sweeps.drain(..) {
                read::opaque(l, sw, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
            }
            read::opaque(l, s, ir::OpaqueClass::InvalidInSource, Some(UNCLOSED));
            continue;
        }
        let word = head(l, s);
        let private = mc.is_some_and(|(_, p)| p);
        if word == "sweep" && !private {
            sweeps.push(s);
            continue;
        }
        if !sweeps.is_empty() && word != "analysis" {
            for sw in sweeps.drain(..) {
                read::opaque(l, sw, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
            }
        }
        match (word.as_str(), mc) {
            ("endmc", Some((outer, p))) => {
                if p {
                    read::opaque(l, s, ir::OpaqueClass::Private, None);
                }
                l.scope = outer;
                mc = None;
            }
            ("endmc", None) => read::opaque(l, s, ir::OpaqueClass::InvalidInSource, Some(REJECTED)),
            ("mc", None) => {
                let closed = body[k + 1..].iter().any(|&t| head(l, Node(t)) == "endmc");
                mc = match closed {
                    true => plan::mc(l, s).map(|p| (saved.0, p)),
                    false => {
                        read::opaque(l, s, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
                        None
                    }
                };
            }
            (_, Some((_, true))) => read::opaque(l, s, ir::OpaqueClass::Private, None),
            ("analysis", _) => {
                let bound = std::mem::take(&mut sweeps);
                analysis::analysis(l, &bound, s);
            }
            ("options", _) => options::options(l, s),
            ("save", _) => read_outputs::save(l, s),
            _ => plan::statement(l, s, &word),
        }
        let end = start + l.stmt.len;
        l.comments_until(end, true);
    }
    for sw in sweeps {
        read::opaque(l, sw, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
    }
    (l.scope, l.stmt) = saved;
    // An empty block is still a statement (law L1).
    if l.b.design().items(l.scope).len() == before {
        read::opaque(l, n, ir::OpaqueClass::Inert, None);
    }
}

/// `(key, value node)` of an `Assign` field, or `None`.
pub(super) fn assign(l: &Lower<'_, '_, '_>, f: u32) -> Option<(String, u32)> {
    let ast = &l.cst.ast;
    let n = Node(f);
    (ast.tag(n) == Tag::Assign).then(|| (read::field_name(l.cst, ast.lhs(n)), ast.rhs(n)))
}

/// The text of a string literal node, its quotes off; `None` for any other node.
pub(super) fn string(l: &Lower<'_, '_, '_>, node: u32) -> Option<String> {
    (node != NONE && l.cst.ast.tag(Node(node)) == Tag::Str).then(|| read::field_name(l.cst, node))
}

/// The source text of field `k` of statement `n`: from its first token to the next field's
/// (or the statement's end), trailing blanks off.
pub(super) fn field_text<'s>(l: &Lower<'_, '_, 's>, n: Node, k: usize) -> &'s str {
    let ast = &l.cst.ast;
    let tokens = &l.cst.tokens;
    let fields = ast.fields(n);
    let at = |f: u32| tokens.start(first_token(l, f));
    let from = at(fields[k]);
    let to = match fields.get(k + 1) {
        Some(&next) => at(next),
        None => l.start(n) + l.span(n).len + l.cst.shift,
    };
    tokens.source()[from as usize..to as usize].trim_end()
}

/// Pushes an opaque of `class` with `text` (a statement rebuilt from some of its fields) at
/// the current statement, with its class's note and `code`.
pub(super) fn push_opaque(
    l: &mut Lower<'_, '_, '_>,
    class: ir::OpaqueClass,
    text: &str,
    code: Option<ir::Code>,
) {
    let span = l.stmt;
    l.b.opaque(l.scope, "vacask", class, text, span);
    l.diags.push(ir::diag::opaque_code(class), span, 0);
    if let Some(code) = code {
        l.diags.push(code, span, 0);
    }
}

/// Statement `n`'s head word (`analysis`, `options`, `save`, or a card's first token).
pub(super) fn head(l: &Lower<'_, '_, '_>, n: Node) -> String {
    l.cst.tokens.text(l.cst.ast.main_token(n)).to_string()
}

/// Whether the name token of field `f` is a bare identifier: VACASK reads an analysis, sweep
/// or mc name as an identifier only (settle names_quoted: `analysis 'a b' op` is "Syntax error,
/// unexpected string."; a leading digit is no identifier either).
pub(super) fn bare_name(l: &Lower<'_, '_, '_>, f: u32) -> bool {
    let text = l.cst.tokens.text(l.cst.ast.main_token(Node(f)));
    identifier(text)
}

/// Whether `name` is an identifier VACASK reads bare: a letter, `_` or `$`, then letters,
/// digits, `_`, `$` (settle names).
pub(super) fn identifier(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_' || b == b'$')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
}

// ---- private ----

/// A code row.
const fn row(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
    kind: ir::Kind,
) -> (ir::Code, ir::Info) {
    (
        code,
        ir::Info {
            title,
            cite,
            explain,
            kind,
        },
    )
}

/// The first token of field `f`: an `Assign`'s key, a `Vector`'s or `List`'s bracket, an
/// operand's own first token (a binary or ternary starts at its left operand).
fn first_token(l: &Lower<'_, '_, '_>, f: u32) -> u32 {
    let ast = &l.cst.ast;
    let mut n = Node(f);
    // Bounded by the tree's depth: each step moves to a child.
    for _ in 0..ast.len() {
        match ast.tag(n) {
            Tag::Assign | Tag::Binary | Tag::Ternary | Tag::Index => n = Node(ast.lhs(n)),
            _ => break,
        }
    }
    ast.main_token(n)
}
