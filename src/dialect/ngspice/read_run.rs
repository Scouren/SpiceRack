//! Run cards → run items: the analyses (`Tag::Analysis`, and `.disto`, `.pss` among the dot
//! cards no other area claims) → `Analysis`; `.options`, `.temp`, `.ic`, `.nodeset` → `Option`,
//! `Env`, `NodeSet` (AMEND-A12 A14: state that holds for the whole run); the other dot cards
//! ngspice ignores → `Inert`, those it rejects (`.step`, `.hb`, an unknown card) →
//! `InvalidInSource`.
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
//! (ngspice's option spellings, which `write_run` spells back)

mod analysis;
mod state;
mod words;

pub(super) use state::KEYS;

use super::codes::Row;
use super::read::{Lower, Stmt};
use super::tables;
use crate::ir::{self, AnalysisKind, Feature, ItemKind, OptionKey, RunExtra};

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
    Feature::Run(RunExtra::SeveralAnalyses),
    Feature::Run(RunExtra::TranRecordStart),
    Feature::Run(RunExtra::TranMaxStep),
    Feature::Run(RunExtra::TranCapNgspice),
    Feature::Run(RunExtra::TranUic),
    Feature::Run(RunExtra::DcNested),
    Feature::Run(RunExtra::DcAxisTemp),
    Feature::Run(RunExtra::DcAxisElementValue),
    Feature::Run(RunExtra::SweepLinPoints),
    Feature::Run(RunExtra::SweepDec),
    Feature::Run(RunExtra::SweepOct),
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
    Feature::Run(RunExtra::NodeSetInSubckt),
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
        "kept as text (Unread) until the IR has the field (.pss sc_iter and steady_coeff, a .sens filter, a .four under nfreqs).",
    ),
    note(
        INERT_CARD,
        "a dot card ngspice reads and ignores",
        "ngspice-45 inp2dot.c:845, 955",
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
        "kept as ngspice text (Private): only ngspice reads it.",
    ),
];

/// A run card ngspice rejects (a missing or unknown word, a zero DC step): `InvalidInSource`.
const RUN_INVALID: ir::Code = ir::Code::new(b'P', 1300);
/// A dot card ngspice-45 does not have ("unimplemented dot command": `.step`, `.hb`, `.sens2`):
/// `InvalidInSource`.
const NO_SUCH_CARD: ir::Code = ir::Code::new(b'P', 1301);
/// An `.ac` whose numbers ngspice replaces with defaults (`np < 1`, `fstop < fstart`): `Unread`.
const RUN_DEFAULTS: ir::Code = ir::Code::new(b'P', 1302);
/// A run card form the IR has no field for (`.pss`'s `sc_iter`, a `.sens` filter, a `.four`
/// whose harmonic count is the deck's `nfreqs`): `Unread`.
const NO_FIELD: ir::Code = ir::Code::new(b'P', 1303);
/// A dot card ngspice reads and ignores (`.width`, `.csparam`, `.prot`): `Inert`.
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
            let w = words::split(&lw.rest(st.node));
            match analysis::read(lw, st, name, &w) {
                Ok(c) => analysis::push(lw, st, &c),
                Err(f) => lw.opaque(st, f.class, f.code),
            }
        }
        "width" | "csparam" | "prot" | "unprot" => {
            lw.opaque(st, ir::OpaqueClass::Inert, INERT_CARD);
        }
        "options" | "option" | "opt" => state::options(lw, st),
        "ic" => state::node_sets(lw, st, ir::NodeSetKind::Ic),
        "nodeset" => state::node_sets(lw, st, ir::NodeSetKind::Nodeset),
        // ponytail: `.temp` is an `Env` of one field with A14; until it lands
        "temp" => lw.unread(st),
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
