//! Run cards → run items: the analyses (`Tag::Analysis`, and `.disto`, `.pss` among the dot
//! cards no other area claims) → `Analysis`; `.options`, `.temp`, `.ic`, `.nodeset` → `Option`,
//! `Env`, `NodeSet` of `holds: WholeRun` (AMEND-A12 A14); the other dot cards ngspice ignores →
//! `Inert`, those it rejects (`.step`, `.hb`, an unknown card) → `InvalidInSource`. A run card in
//! a subcircuit body stays there and is the deck's (R26, `Placed(Subckt, …)`).
//!
//! Each card is one item (or one per key or node) at its own place, in card order (R25): ngspice
//! runs analyses by type rank, reverse card order inside a type (`cktdojob.c:208-216`,
//! `cktnewan.c:34`), which is the order of the results, not of the design; its state cards hold
//! for the whole deck wherever they stand (R24).
//!
//! Every statement handed here becomes IR items in `st.scope`, or one `Opaque` with a coded note
//! (`Lower::opaque`); none is dropped (law L1, asserted by `read.rs`). A card is validated whole
//! before its first push. It never learns a target. Codes: `P1300–P1349` (this file).
//!
//! Cites: ngspice-45 `inp2dot.c` `INP2dot` (the dot cards it reads, ignores or rejects),
//! `analysis.c:36-58`, `inp.c:1159-1186` (`.temp`), `inppas3.c` (`.ic`, `.nodeset`); runs in
//! `rewrite/DIALECT-NGSPICE.md` §3; `rewrite/RULINGS.md` R24–R26.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt)`, `pub(super) use state::KEYS`
//! (ngspice's option spellings, which `write_run` spells back); for the control area, which reads
//! a script command (`tran 1n 1u uic`) as the card without its dot:
//! `pub(super) use words::split as split_words` (`fn(&str) -> Vec<String>`),
//! `pub(super) use analysis::Card`, `pub(super) use analysis::read as read_analysis`
//! (`fn(&mut Lower, Stmt, name: &str,
//! words: &[String]) -> Result<Card, Fail>`), `pub(super) use rows::push as push_analysis`
//! (`fn(&mut Lower, Stmt, &Card)`)

mod analysis;
mod rows;
mod state;
mod words;

#[allow(
    unused_imports,
    reason = "the control area holds a validated Card between its passes"
)]
pub(super) use analysis::Card;
pub(super) use analysis::read as read_analysis;
pub(super) use rows::push as push_analysis;
pub(super) use state::KEYS;
pub(super) use words::split as split_words;

use super::codes::Row;
use super::read::{Lower, Stmt};
use super::tables;
use crate::ir::{self, AnalysisKind, Feature, ItemKind, OptionKey, RunExtra, ScopeKind};

/// What the run area's reader produces (and `write_run` spells): the analyses ngspice-45 has,
/// their fields as its cards give them, and the state items.
pub(super) const FEATURES: ir::FeatureSet = tables::feature_set(&[
    Feature::Item(ItemKind::Analysis),
    Feature::Analysis(AnalysisKind::Op),
    Feature::Analysis(AnalysisKind::Dc),
    Feature::Analysis(AnalysisKind::Ac),
    Feature::Analysis(AnalysisKind::Tran),
    Feature::Analysis(AnalysisKind::Noise),
    Feature::Analysis(AnalysisKind::Tf),
    Feature::Analysis(AnalysisKind::Sens),
    Feature::Analysis(AnalysisKind::SensAc),
    Feature::Analysis(AnalysisKind::Pz),
    Feature::Analysis(AnalysisKind::Disto),
    Feature::Analysis(AnalysisKind::Sp),
    Feature::Analysis(AnalysisKind::Four),
    Feature::Analysis(AnalysisKind::Pss),
    Feature::Run(RunExtra::PssAutonomous),
    Feature::Run(RunExtra::PssUic),
    Feature::Run(RunExtra::PssScIter),
    Feature::Run(RunExtra::PssSteadyCoeff),
    Feature::Run(RunExtra::FourHarmonicsOption),
    Feature::Option(OptionKey::NFreqs),
    Feature::Option(OptionKey::FourGridSize),
    Feature::Run(RunExtra::SeveralAnalyses),
    // an analysis card and a `.dc` axis carry no name (A35)
    Feature::Run(RunExtra::AnalysisUnnamed),
    Feature::Run(RunExtra::DcAxisUnnamed),
    // a `.tran` card always gives its print step (`tstep`, inp2dot.c), R67/A41
    Feature::Run(RunExtra::TranPrintStep),
    // a card argument may read a `.param` (numparam, `{…}`): R68/A42
    Feature::Run(RunExtra::AnalysisParamArgument),
    Feature::Run(RunExtra::TranRecordStart),
    Feature::Run(RunExtra::TranMaxStep),
    Feature::Run(RunExtra::TranCapNgspice),
    Feature::Run(RunExtra::TranUic),
    Feature::Run(RunExtra::DcNested),
    // each outer step starts afresh (R75)
    Feature::Run(RunExtra::DcOuterAfresh),
    Feature::Run(RunExtra::DcAxisTemp),
    Feature::Run(RunExtra::DcAxisElementValue),
    Feature::Run(RunExtra::SweepDcLinStep),
    Feature::Run(RunExtra::SweepFreqLinPoints),
    Feature::Run(RunExtra::SweepFreqDec),
    Feature::Run(RunExtra::SweepFreqOct),
    Feature::Run(RunExtra::NoiseSummary),
    Feature::Run(RunExtra::SpNoise),
    Feature::Run(RunExtra::ProbeVdiff),
    Feature::Run(RunExtra::ProbeI),
    Feature::Run(RunExtra::ProbeHierarchical),
    Feature::Item(ItemKind::Option),
    Feature::Option(OptionKey::RelTol),
    Feature::Option(OptionKey::AbsTol),
    Feature::Option(OptionKey::VnTol),
    Feature::Option(OptionKey::ChgTol),
    Feature::Option(OptionKey::TrTol),
    Feature::Option(OptionKey::PivTol),
    Feature::Option(OptionKey::PivRel),
    Feature::Option(OptionKey::MaxOrd),
    Feature::Option(OptionKey::MethodTrap),
    Feature::Option(OptionKey::MethodGear),
    Feature::Option(OptionKey::Itl1),
    Feature::Option(OptionKey::Itl2),
    Feature::Option(OptionKey::Itl4),
    Feature::Option(OptionKey::GminSteps),
    Feature::Option(OptionKey::SrcSteps),
    Feature::Option(OptionKey::NoOpIter),
    Feature::Option(OptionKey::Gmin),
    Feature::Option(OptionKey::Rshunt),
    Feature::Option(OptionKey::Gshunt),
    Feature::Option(OptionKey::Cshunt),
    Feature::Option(OptionKey::Rseries),
    Feature::Option(OptionKey::Scale),
    Feature::Option(OptionKey::DefL),
    Feature::Option(OptionKey::DefW),
    Feature::Option(OptionKey::DefAd),
    Feature::Option(OptionKey::DefAs),
    Feature::Option(OptionKey::Klu),
    Feature::Option(OptionKey::Sparse),
    Feature::Option(OptionKey::Seed),
    Feature::Option(OptionKey::Autostop),
    Feature::Option(OptionKey::Interp),
    Feature::Option(OptionKey::Acct),
    Feature::Option(OptionKey::NoAcct),
    Feature::Option(OptionKey::List),
    Feature::Option(OptionKey::NoMod),
    Feature::Option(OptionKey::NoPage),
    Feature::Option(OptionKey::Node),
    Feature::Option(OptionKey::Opts),
    Feature::Option(OptionKey::Warn),
    Feature::Option(OptionKey::MaxWarns),
    Feature::Option(OptionKey::NoRefValue),
    Feature::Option(OptionKey::KeepOpInfo),
    Feature::Option(OptionKey::NoInit),
    Feature::Item(ItemKind::NodeSet),
    Feature::Run(RunExtra::NodesetAll),
    Feature::Run(RunExtra::NodeSetHierarchical),
    Feature::Run(RunExtra::IcBeyondNext),
    Feature::Item(ItemKind::Env),
    // every state card holds for the whole run wherever it stands (R24)
    Feature::Run(RunExtra::OptionWholeRun),
    Feature::Run(RunExtra::EnvWholeRun),
    Feature::Run(RunExtra::NodeSetWholeRun),
    Feature::Run(RunExtra::OptionWholeRunMidRun),
    Feature::Run(RunExtra::EnvWholeRunMidRun),
    Feature::Run(RunExtra::NodeSetWholeRunMidRun),
    Feature::Run(RunExtra::SaveWholeRunMidRun),
    // ngspice reads dot cards and element cards in any order (`inp.c` passes over the deck)
    Feature::Run(RunExtra::RunInterleaved),
    // a run card in a `.subckt` body is the deck's (R26, run sub.cir); a `.ic` there holds each
    // instance's node (run icsub.cir); a `.lib` section's cards are its includer's
    Feature::Placed(ScopeKind::Subckt, ItemKind::Analysis),
    Feature::Placed(ScopeKind::Subckt, ItemKind::Option),
    Feature::Placed(ScopeKind::Subckt, ItemKind::Env),
    Feature::Placed(ScopeKind::Subckt, ItemKind::NodeSet),
    Feature::Placed(ScopeKind::Section, ItemKind::Analysis),
    Feature::Placed(ScopeKind::Section, ItemKind::Option),
    Feature::Placed(ScopeKind::Section, ItemKind::Env),
    Feature::Placed(ScopeKind::Section, ItemKind::NodeSet),
]);

/// This area's reading codes, `P1300–P1349`.
pub(super) const CODES: &[Row] = &[
    note(
        RUN_INVALID,
        "a run card ngspice rejects",
        "ngspice-45 inp2dot.c dot_* (LITERR); runs",
        "a missing or unknown word, or a zero DC step, stops ngspice before the run; kept as text (InvalidInSource).",
    ),
    note(
        NO_SUCH_CARD,
        "a dot card ngspice-45 does not have",
        "ngspice-45 inp2dot.c:962 (unimplemented dot command)",
        "ngspice stops before the run (.step, .hb, .sens2, .dcvolt are not its cards); kept as text (InvalidInSource).",
    ),
    note(
        RUN_DEFAULTS,
        "an .ac sweep whose numbers ngspice replaces",
        "ngspice-45 inp2dot.c:205-231",
        "a point count below 1, a negative start or a stop below the start runs ngspice's defaults instead; kept as text (Unread).",
    ),
    note(
        NO_FIELD,
        "a run card form the IR has no field for",
        "rewrite/IR-RUN.md §2",
        "kept as text (Unread): a .sens filter, a .four whose count a script sets, a .pss with fewer arguments, a .dc of more than two axes.",
    ),
    note(
        TEMP_27,
        "a .temp ngspice runs at 27 °C",
        "ngspice-45 inp.c:1173-1182; runs .temp 27 85, .temp 0.1k",
        "ngspice reads the value with strtod and warns and runs 27 °C when anything is left (a list, a scale suffix); the Env holds 27.",
    ),
    note(
        TEMP_BEATEN,
        "an .options temp= a .temp card beats",
        "ngspice-45 inp.c:1159-1186; runs temp1.cir, temp2.cir",
        "a .temp card sets the temperature whatever the order; this key changes nothing and is kept as Inert text.",
    ),
    note(
        INERT_CARD,
        "a dot card ngspice reads and ignores",
        "ngspice-45 inp2dot.c:955; run: .csparam x=1",
        "it moves no number; kept as Inert text.",
    ),
    note(
        OPTION_FORM,
        "a state card form whose reading is not established",
        "ngspice-45 cktsopt.c, inppas3.c:79-86; run: .options reltol 1e-4",
        "a value option with no = (ngspice's bad type error, the key left out) or words after .nodeset all=; kept as text (Unread).",
    ),
    note(
        PRIVATE_OPTION,
        "an ngspice option the IR has no key for",
        "rewrite/RULINGS.md R19; ngspice-45 cktsopt.c",
        "kept as ngspice text (Private, or Hint for a solver knob that names no accuracy target: R19): only ngspice reads it.",
    ),
];

/// A run card ngspice rejects (a missing or unknown word, a zero DC step): `InvalidInSource`.
const RUN_INVALID: ir::Code = ir::Code::new(b'P', 1300);
/// A dot card ngspice-45 does not have ("unimplemented dot command": `.step`, `.hb`, `.sens2`):
/// `InvalidInSource`.
const NO_SUCH_CARD: ir::Code = ir::Code::new(b'P', 1301);
/// An `.ac` whose numbers ngspice replaces with defaults (`np < 1`, `fstop < fstart`): `Unread`.
const RUN_DEFAULTS: ir::Code = ir::Code::new(b'P', 1302);
/// A run card form the IR has no field for (a `.sens` filter, a `.four`
/// whose count a script's `nfreqs` gives): `Unread`.
const NO_FIELD: ir::Code = ir::Code::new(b'P', 1303);
/// A `.temp` ngspice cannot read (a list, a scale suffix): it runs 27 °C, the `Env`'s literal.
const TEMP_27: ir::Code = ir::Code::new(b'P', 1304);
/// An `.options temp=` a `.temp` card beats (in either order): `Inert`.
const TEMP_BEATEN: ir::Code = ir::Code::new(b'P', 1305);
/// A dot card ngspice reads and ignores (`.csparam`, `.prot`): `Inert`.
const INERT_CARD: ir::Code = ir::Code::new(b'P', 1306);
/// An ngspice option the IR has no key for: `Private` (a solver knob becomes `Hint`, R19).
const PRIVATE_OPTION: ir::Code = ir::Code::new(b'P', 1307);
/// A state card form whose reading is not established (a value key with no `=`, words after
/// `.nodeset all=`): `Unread`.
const OPTION_FORM: ir::Code = ir::Code::new(b'P', 1308);

/// A run card or an unclaimed dot card.
pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt) {
    let keyword = lw.keyword(st.node);
    let name = keyword.strip_prefix('.').unwrap_or(&keyword);
    match name {
        "op" | "dc" | "ac" | "tran" | "noise" | "tf" | "sens" | "pz" | "disto" | "sp" | "pss"
        | "four" | "fourier" => {
            // the control area's path: the words, one analysis, its rows
            let w = split_words(&lw.rest(st.node));
            match read_analysis(lw, st, name, &w) {
                Ok(c) => push_analysis(lw, st, &c),
                Err(f) => lw.opaque(st, f.class, f.code),
            }
        }
        "csparam" | "prot" | "unprot" => {
            lw.opaque(st, ir::OpaqueClass::Inert, INERT_CARD);
        }
        "options" | "option" | "opt" => state::options(lw, st),
        "ic" => state::node_sets(lw, st, ir::NodeSetKind::Ic),
        "nodeset" => state::node_sets(lw, st, ir::NodeSetKind::Nodeset),
        "temp" => state::temp(lw, st),
        // frontend cards of compatibility modes, and `.opts` (accepted, its reading unknown)
        "opts" | "backanno" | "distribution" | "incpslt" | "cmodel" | "libsave" => lw.unread(st),
        _ => lw.opaque(st, ir::OpaqueClass::InvalidInSource, NO_SUCH_CARD),
    }
}

// ---- private ----

/// A note row of this area.
const fn note(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
    (
        code,
        ir::Info {
            title,
            cite,
            explain,
            kind: ir::Kind::Note,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::split_words;
    use crate::dialect::{self, Policy, Sources};
    use crate::ir;

    #[test]
    fn a_script_commands_words_read_to_its_analysis() {
        // Oracle: inp2dot.c dot_tran (tstep tstop [tstart [tmax]] [uic]) and INPgetTok's
        // separators. The contract the control area calls: `split_words` on a command's
        // arguments, `read_analysis` on the name and the words, `push_analysis` on the card;
        // a dot card goes the same three steps (`card`), so a deck pins all three.
        assert_eq!(split_words("1n 1u uic"), ["1n", "1u", "uic"]);
        assert_eq!(
            split_words("v(a, b) vin dec 10 1 1k"),
            ["v(a, b)", "vin", "dec", "10", "1", "1k"]
        );
        let row = dialect::by_name("ngspice").expect("the ngspice row");
        let (mut design, mut diags) = (ir::Design::new(), ir::Diags::new());
        let src = Sources::one("t.sp", "t\n.tran 1n 1u uic\n");
        dialect::read(&src, row, Policy::Lenient, &mut design, &mut diags);
        assert!(diags.rows().is_empty(), "{diags:?}");
        let top = design.file(design.entry()).top;
        let item = design.items(top)[1];
        assert_eq!(item.kind, ir::ItemKind::Analysis);
        let a: ir::Analysis = design.row(item.index);
        assert_eq!(a.kind, ir::AnalysisKind::Tran);
        let t: ir::tran::Tran = design.detail(a.detail);
        assert!(t.uic && t.record_start.is_none());
        let rule = ir::tran::StepCeiling::Rule(ir::tran::StepCap::NgspiceRule);
        assert_eq!(t.ceiling, rule);
    }
}
