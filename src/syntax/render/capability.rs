//! What each target can write: one exhaustive `match` from a target and a
//! [`Construct`] (a thing whose spelling varies between dialects) to an
//! [`Action`]. A new construct does not compile until every target answers
//! (no `_` arm), as LLVM's legalize table and VerA's total fact tables.
//! The writers ask [`action`] and do what it says; nothing else decides
//! whether a construct is written.
//!
//! `Spell` and `Lift` keep the meaning: a named alternative form, or a
//! rewrite into extra statements recorded as fixups. Anything that cannot
//! keep its meaning is `Refuse` with a `W` code. Phase 7 decided every
//! cell that answered as the old writers behaved (the `// quirk:` cells).
//!
//! CDL's row is the SPICE writer's (its elements are ngspice cards in the
//! xschem layout). Spectre's and VACASK's are the named writer's
//! (`render::named`). Verilog's answer what its old writer did where the
//! construct reaches it (it stays on the old layout writer:
//! `tests/fixtures/transpiler/PHASE4-STATUS.md`).
//!
//! Cites: LLVM `TargetLowering.h` (`LegalizeAction`, one table read by one
//! generic pass); each row's manual clause in its `W` code (`syntax::diag`).
//!
//! Contents:
//! - `pub enum Construct`, `pub enum Action`, `pub enum Alt`, `pub enum Rule`

use crate::syntax::diag::Code;

/// A construct whose spelling, or whose existence, differs by target.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Construct {
    /// A statement kept as text from another dialect family.
    ForeignText,
    /// Such a statement only its own simulator has (`Rd::kept_construct`).
    PrivateText,
    /// Xyce's `.sens` (objective, parameters, over the deck's analysis),
    /// kept as text.
    XyceSens,
    /// HSPICE's `.sn`, `.snnoise`, `.snxf`, `.trannoise`, `.lstb`, kept as
    /// text.
    HspiceAnalysis,
    /// A VACASK script line with no circuit meaning (`abort`, `print`,
    /// `postprocess`, an `embed` nothing loads: `Rd::vacask_script`).
    VacaskScript,
    /// A `^` the target reads with the other meaning (power, XOR).
    CaretReread,
    /// A call of `agauss`, `gauss`, `aunif`, `unif` or `limit`.
    StatisticalCall,
    /// A Verilog-A source load.
    VerilogALoad,
    /// A compiled OSDI binary load.
    OsdiLoad,
    /// An instance of a loaded module (ngspice's OSDI `N` card).
    OsdiInstance,
    /// `.lib file` with no section (LTspice's whole-file form).
    LibWholeFile,
    /// `.options ngbehavior=<mode>`: ngspice's compatibility mode, read
    /// before the deck (ngspice manual, the compatibility modes).
    NgBehavior,
    /// `.lib name` … `.endl`.
    LibSection,
    /// An R, C or L card naming a model no `render::models` kind maps: an
    /// inductor's, or a model card read in a dialect other than ngspice
    /// (its parameters mean other equations).
    RclModel,
    /// `C1 a b q=expr`.
    ChargeCapacitor,
    /// `B1 a b V=expr` / `I=expr`.
    BehaviouralSource,
    /// A Laplace transfer function in a behavioural source.
    Laplace,
    /// `portnum n z0 Z` on a V source.
    RfPort,
    /// PWL `r=`/`td=` on a V source.
    PwlRepeatV,
    /// PWL `r=`/`td=` on an I source.
    PwlRepeatI,
    /// A TRNOISE or TRRANDOM waveform.
    NoiseWave,
    /// `.if` … `.endif`.
    Conditional,
    /// A `.param` or a load inside a conditional branch.
    BranchParam,
    /// `.func f(x) body`.
    Func,
    /// A `.func` local to a subcircuit (it reads the subcircuit's
    /// parameters, or another `.func` shares its name).
    LocalFunc,
    /// Xyce's `E`/`G` `TABLE`.
    TableSource,
    /// An `O` lossy line on an LTRA model.
    LossyLine,
    /// An AM source waveform.
    AmWave,
    /// `.step`.
    Step,
    /// `.step <name> list <values>`.
    StepList,
    /// LTspice's `.tran … startup` ramp.
    TranStartup,
    /// A `.dc` axis by decade, octave or list.
    DcLogList,
    /// `.tf`.
    Tf,
    /// `.sens` (DC).
    Sens,
    /// `.sens … ac`.
    SensAc,
    /// `.pz`.
    PoleZero,
    /// `.sp`.
    SParameter,
    /// `.print`.
    Print,
    /// `.save` outputs.
    Save,
    /// An option with no portable spelling in the target's option form.
    UnmappedOption,
    /// A Spectre `statistics` block.
    Statistics,
    /// `.measure`.
    Measure,
    /// HSPICE's W element (a multi-conductor lossy line on an RLGC,
    /// tabular, U, FS or S model), kept as text.
    HspiceWLine,
    /// A VACASK `elaborate` the reader does not read as the top (a second
    /// circuit of one run, `Rd::vacask_promoted`).
    VacaskElaborate,
    /// Analyses on more than one circuit state: a parameter changed
    /// between two of them (`read::epochs::Plan`; VACASK `alter`).
    Epochs,
}

/// What a writer does with a construct.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Write it in the target's own form.
    Native,
    /// Write a named alternative form that means the same.
    Spell(Alt),
    /// Rewrite it into other statements (fixups), meaning the same.
    Lift(Rule),
    /// Refuse the deck: the target cannot say it.
    Refuse(Code),
}

/// A named alternative spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alt {
    /// `Q='expr'` (ngspice §3.3.9, read by HSPICE and ESPice).
    ChargeQuoted,
    /// `Q={expr}` (Xyce).
    ChargeBraced,
    /// `Q=expr` (LTspice).
    ChargeBare,
    /// `E… vol='expr'` / `G… cur='expr'` (HSPICE).
    VolCur,
    /// `B… V={expr}` (Xyce, RG p.238).
    BracedBehaviour,
    /// `.param f(x)='body'` (HSPICE).
    FuncParam,
    /// `.hdl "file"` (ESPice and HSPICE compile Verilog-A themselves).
    Hdl,
    /// `.control` / `pre_osdi file` / `.endc` (ngspice).
    PreOsdi,
    /// `.probe` per output, `.option post` for all (HSPICE).
    Probe,
    /// `.dc src DEC|OCT np start stop`, `.dc src POI n v…` (HSPICE `.DC`,
    /// the `type np` form).
    DcType,
    /// An E/G `value={pwl(expr, x, y, …)}` (ngspice).
    PwlValue,
    /// An E/G `value={x < x1 ? y1 : x < x2 ? … : yn}`: a table's segments
    /// as ternaries, for one probe and up to five segments (ESPice, whose
    /// B source compiles 64 operations and has no `pwl()`); a larger
    /// table is refused (W0102).
    TernaryValue,
    /// `P n+ n- port=n z0=z` (Xyce).
    XycePort,
    /// `AM(SA OC FM FC TD)` (HSPICE).
    HspiceAm,
    /// ngspice's `AM(…)` with no VO offset (ESPice).
    AmNoOffset,
    /// `.pz v(out) src` (HSPICE).
    HspicePz,
    /// `.temp t1 t2 …` for a temperature list, no other list (HSPICE).
    TempList,
    /// `.LIN` beside an `.AC` of the sweep (Xyce).
    XyceLin,
    /// Not written, the code the note the writer records for it.
    Omit(Code),
    /// Written as is, the code the note the writer records for it.
    Copy(Code),
    /// `.include "file"`: LTspice's whole-file `.lib` (ngspice §2.6;
    /// HSPICE `.INCLUDE`; Xyce RG `.INCLUDE`), where `.lib file` alone
    /// would open a section definition.
    Include,
    /// `c1 (a b) bsource q=expr` (Spectre Reference 19.1 p.459).
    BsourceCharge,
    /// `b1 (a b) bsource v=expr` / `i=expr` (Spectre Reference 5.0
    /// pp.856-860).
    Bsource,
    /// PWL `r=0` as `pwlperiod=` where it repeats exactly, `td=` as
    /// `delay=` (Spectre Reference 5.0 `vsource` #4, #17, #18, p.686).
    PwlPeriod,
    /// The `render::NGBEHAVIOR` line, which the ngspice backend lifts onto
    /// its command line (`-D ngbehavior=<mode>`).
    NgBehaviorLine,
    /// SPICE text fenced by `simulator lang=spice` / `lang=spectre`
    /// (`[KUN]` B.2; Spectre Reference 19.1 p.17: Spectre reads `.measure`).
    SpiceRegion,
    /// One `.control` script running every analysis, with `alterparam` and
    /// `reset` before the first run of each later circuit state (ngspice
    /// manual §13.5.5, §13.5.65).
    ControlScript,
}

/// A rewrite rule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    /// Laplace → an XSPICE `s_xfer` instance and its `.model` (ngspice).
    LaplaceXfer,
    /// Laplace → `E/G … LAPLACE c+ c- num… / den…` (HSPICE, ESPice).
    LaplaceTable,
    /// Laplace → `E/G … Laplace=num/den` (LTspice).
    LaplaceAttr,
    /// A `statistics` block → two-layer `.param` draws (SPICE).
    StatisticsDraws,
    /// A statistical call → its `rack_*` function of a normalized variable
    /// declared `parameters v=0` and varied in a `statistics` block
    /// (Spectre; Spectre Reference 19.1 pp.175-179, p.481).
    RackVars,
    /// The statistical calls VACASK lacks spelled out: `limit` → an
    /// `aunif` ternary, Xyce's clamp → `min(max())`, a two-argument
    /// `agauss` its sigma 1 (VACASK `docs/cmd-analysis-mc.md`).
    VacaskCalls,
    /// LTspice's `.tran … startup` → `PWL(0 0 20u X)` beside the DC value
    /// `X` of each DC-only V and I at the top (LTspice help, `.TRAN`;
    /// LTspice 26.1.1 ran the ramp linear, waveform and subcircuit
    /// sources unramped).
    StartupRamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Dialect, Lang};

    #[test]
    fn refusals_are_w_refusal_codes_and_omissions_w_notes() {
        // Every refusal is a render-stage refusal code, every drop or copy a
        // render-stage note; the table is total over the targets the writers
        // serve.
        use crate::syntax::diag::{Kind, info};
        for d in Dialect::ALL {
            for c in CONSTRUCTS {
                let (code, kind) = match Lang::from(d).action(c) {
                    Action::Refuse(code) => (code, Kind::Refusal),
                    Action::Spell(Alt::Omit(code) | Alt::Copy(code)) => (code, Kind::Note),
                    Action::Native | Action::Spell(_) | Action::Lift(_) => continue,
                };
                assert!(format!("{code:?}").starts_with('W'), "{d:?} {c:?}");
                assert_eq!(info(code).kind, kind, "{d:?} {c:?}");
            }
        }
    }

    #[test]
    fn construct_order_is_pinned() {
        // A cell's index is its construct's discriminant (`Lang::cells`,
        // the dumps): a new construct is appended, never inserted.
        let names: Vec<String> = crate::syntax::lang::CONSTRUCTS
            .iter()
            .map(|c| format!("{c:?}"))
            .collect();
        assert_eq!(
            names,
            [
                "ForeignText",
                "PrivateText",
                "XyceSens",
                "HspiceAnalysis",
                "VacaskScript",
                "CaretReread",
                "StatisticalCall",
                "VerilogALoad",
                "OsdiLoad",
                "OsdiInstance",
                "LibWholeFile",
                "NgBehavior",
                "LibSection",
                "RclModel",
                "ChargeCapacitor",
                "BehaviouralSource",
                "Laplace",
                "RfPort",
                "PwlRepeatV",
                "PwlRepeatI",
                "NoiseWave",
                "Conditional",
                "BranchParam",
                "Func",
                "LocalFunc",
                "TableSource",
                "LossyLine",
                "AmWave",
                "Step",
                "StepList",
                "TranStartup",
                "DcLogList",
                "Tf",
                "Sens",
                "SensAc",
                "PoleZero",
                "SParameter",
                "Print",
                "Save",
                "UnmappedOption",
                "Statistics",
                "Measure",
                "HspiceWLine",
                "VacaskElaborate",
                "Epochs",
            ]
        );
        for (i, c) in crate::syntax::lang::CONSTRUCTS.into_iter().enumerate() {
            assert_eq!(c as usize, i, "{c:?}");
        }
    }

    #[test]
    fn the_spice_family_differs_only_where_its_manuals_do() {
        use Dialect as D;
        assert_eq!(
            Lang::from(D::Ngspice).action(Construct::Conditional),
            Action::Native
        );
        assert_eq!(
            Lang::from(D::Xyce).action(Construct::Conditional),
            Action::Refuse(Code::W0013)
        );
        assert_eq!(
            Lang::from(D::Xyce).action(Construct::ChargeCapacitor),
            Action::Spell(Alt::ChargeBraced)
        );
        assert_eq!(
            Lang::from(D::Ltspice).action(Construct::LibWholeFile),
            Action::Native
        );
    }

    #[test]
    fn vacask_cells_match_36be560() {
        // VACASK upstream 36be560's own lists, pinned so a cell written
        // against an older build (ZERO-LEDGER P4-001) cannot drift
        // unnoticed: the modules `.#testing`'s `vacask` ships
        // (`lib/vacask/mod/spice/*.osdi`; GA dossier census) and the
        // Spectre-spelled analyses it runs though [REF] calls them
        // Spectre's (VS dossier §3 M4; SR pp.771,748,759 and the
        // dossier's own 36be560 runs for hbac/hbnoise, which [REF] 5.0
        // does not have at all). `stb` is not pinned here: an existing
        // test (`syntax::tests::render`) still claims VACASK lacks it;
        // left for team-lead, not this row's owner files.
        const MODULES_36BE560: &[&str] = &[
            "bjt",
            "bsim3v3",
            "bsim4v8",
            "capacitor",
            "diode",
            "inductor",
            "jfet1",
            "jfet2",
            "mes1",
            "mos1",
            "mos2",
            "mos3",
            "mos6",
            "mos9",
            "resistor",
            "vdmos",
        ];
        // The modules in a file of their own (`lib/vacask/mod/*.osdi`,
        // `devices/CMakeLists.txt` L10-L12; ZERO-LEDGER P4-012): a master
        // with a `model_load` row ships as that file.
        const FILES_36BE560: &[&str] = &["vbic_1p3.osdi", "vbic_1p3_4t.osdi", "vbic_1p3_5t.osdi"];
        let b = Lang::from(Dialect::Vacask).behaviours();
        for m in b.model_masters {
            let ships = match b.model_load(m.master) {
                Some(file) => FILES_36BE560.contains(&file),
                None => MODULES_36BE560.contains(&m.master),
            };
            assert!(ships, "{m:?}: no file of that module at 36be560");
        }
        // NG dossier §4 M4: `spice/jfet2.osdi` (JFET LEVEL 2) ships but
        // had no master, so a LEVEL 2 `.model` gave W0104 "no master".
        assert!(
            b.model_masters.iter().any(|m| m.master == "jfet2"),
            "jfet2.osdi ships at 36be560; LEVEL 2 should have a master"
        );
        // VS dossier §3 M4: VACASK runs pss/pac/pnoise/hbac/hbnoise, so
        // the target does not lack them (W0015 was false; `sp` keeps its
        // own hardcoded refusal, and `stb` its W0015, both untouched here).
        for kind in ["pss", "pac", "pnoise", "hbac", "hbnoise"] {
            assert!(
                b.spectre_analyses.contains(&kind),
                "VACASK 36be560 runs {kind}; spectre_analyses should say so"
            );
        }
    }

    const CONSTRUCTS: [Construct; 45] = [
        Construct::ForeignText,
        Construct::PrivateText,
        Construct::XyceSens,
        Construct::HspiceAnalysis,
        Construct::VacaskScript,
        Construct::VacaskElaborate,
        Construct::NgBehavior,
        Construct::CaretReread,
        Construct::StatisticalCall,
        Construct::VerilogALoad,
        Construct::OsdiLoad,
        Construct::OsdiInstance,
        Construct::LibWholeFile,
        Construct::LibSection,
        Construct::RclModel,
        Construct::ChargeCapacitor,
        Construct::BehaviouralSource,
        Construct::Laplace,
        Construct::RfPort,
        Construct::PwlRepeatV,
        Construct::PwlRepeatI,
        Construct::NoiseWave,
        Construct::Conditional,
        Construct::BranchParam,
        Construct::Func,
        Construct::LocalFunc,
        Construct::TableSource,
        Construct::LossyLine,
        Construct::AmWave,
        Construct::Step,
        Construct::StepList,
        Construct::TranStartup,
        Construct::DcLogList,
        Construct::Tf,
        Construct::Sens,
        Construct::SensAc,
        Construct::PoleZero,
        Construct::SParameter,
        Construct::Print,
        Construct::Save,
        Construct::UnmappedOption,
        Construct::Statistics,
        Construct::Measure,
        Construct::HspiceWLine,
        Construct::Epochs,
    ];
}
