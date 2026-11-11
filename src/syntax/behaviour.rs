//! What a language's reader and writers do that its tables do not say: one
//! [`Behaviours`] row per language, each field a closed enum (or a plain
//! value) that selects one code path in a family's reader or writer. The
//! reader and writers read these fields where they once asked which
//! dialect a deck was in, so a definition that extends a built-in can
//! change any of them (`tests/fixtures/transpiler/DIALECT-PLUGIN.md`
//! §6.1). A variant is named for the dialect whose rules it is
//! (`CardRules::Ltspice`: the old LTspice reader's card rules).
//!
//! The source's fields are read through the reader (`Rd::b`), the
//! target's through the writer (`W::b`).
//!
//! Cites: none (each code path cites its manual where it is read).
//!
//! Contents:
//! - `pub struct Behaviours`, `Behaviours::DEFAULT`
//! - `pub enum Writer`, `pub enum NumberReading`, `pub enum ModelSemantics`, `pub enum SourceRules`,
//!   `pub enum CardRules`, `pub enum NamedRules`, `pub enum OptionsForm`, `pub enum LoadForm`,
//!   `pub enum ControlBlock`, `pub enum AnalysisForm`, `pub enum EndRule`, `pub enum ValueQuoting`,
//!   `pub enum Coupling`, `pub enum ParamForm`, `pub enum IncludeScope`, `pub enum NamedParams`,
//!   `pub enum ExprEval`, `pub enum Parasitics`, `pub enum NetlistRules`, `pub enum OptionTo`,
//!   `pub enum DotRule`
//! - `pub struct ModelMaster`, `Behaviours::model_master(&self, kind: &str, level: u32) -> MasterAnswer`, `pub enum MasterAnswer`

use crate::syntax::Lang;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::Construct;

/// A language's behaviours.
///
/// Adding a behaviour is these sites, each a line or a row: the field here
/// and its value in [`Behaviours::DEFAULT`]; for a new enum, the enum
/// below and its import in `def/mod.rs`; the definition call (a `set!` in
/// `def/mod.rs`, or a method in `def/tables.rs` for a list) and the
/// header's Contents; the Python binding's method list
/// (`src/python/def_.rs`) and its mirror (`tests/builtin_dialects.py`);
/// the `behaviours` line of the two `tests/fixtures/dialects/*.dump`
/// fixtures (it is the struct's `Debug`); then the call in each
/// `dialects/<name>.rs` that differs from the default, and the one code
/// path that reads it (`Rd::b` in a reader, `W::b` in a writer).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Behaviours {
    /// The writer layout its decks are written in.
    pub writer: Writer,
    /// The language whose text this one's text is: a region kept as that
    /// language's text is written as written here (ESPice: ngspice's).
    pub text_family: Lang,
    /// `^` is power (else XOR, or no operator).
    pub caret_power: bool,
    /// The hierarchy separator in a probe (`v(X1.n)`; Xyce `:`).
    pub hier_sep: char,
    /// Read: how its numbers' digits became doubles.
    pub numbers: NumberReading,
    /// Read: whose equations its models, options and expressions mean.
    pub model_semantics: ModelSemantics,
    /// Read: the grammar of a source's specifications.
    pub source_rules: SourceRules,
    /// Read: the old reader's per-dialect rules for SPICE cards.
    pub card_rules: CardRules,
    /// Read: the named reader's statement rules.
    pub named_rules: NamedRules,
    /// Read and write: whose option names and option cards these are.
    pub options_form: OptionsForm,
    /// Write: how Verilog-A and OSDI loads are written.
    pub load_form: LoadForm,
    /// Write: a `.control` block runs the analyses.
    pub control_block: ControlBlock,
    /// Write: the analysis cards' forms.
    pub analysis_form: AnalysisForm,
    /// Write: when `.end` is written.
    pub end_rule: EndRule,
    /// Write: how a value kept as text is quoted.
    pub value_quoting: ValueQuoting,
    /// Write: how a `K` card couples more than two inductors.
    pub coupling: Coupling,
    /// Write: how parameters and model parameters are written.
    pub param_form: ParamForm,
    /// Write: what a file included inside a subcircuit defines.
    pub include_scope: IncludeScope,
    /// Write: the named writer's parameter rows.
    pub named_params: NamedParams,
    /// Read: the language whose master words make a named instance an
    /// analysis (the old lowering read Spectre's `Word_kind` for VACASK
    /// too); `None`: only its own analysis statements are.
    pub analysis_masters: Option<Lang>,
    /// Read and write: whose evaluator reads a `{}` expression (a
    /// then-branch ternary's grouping, `if()`, `sdt()`).
    pub expr_eval: ExprEval,
    /// Write: an LTspice element's parasitics (`Rser=`, `Cpar=`, …).
    pub parasitics: Parasitics,
    /// Read and write: whose simulator's netlist rules give the deck its
    /// meaning beyond its text (`render::spice`: a model `R`, an `X`
    /// line's `M`, a parameter defined twice, a device `IC=`, SFFM's
    /// order, the default TNOM, a zero TSTEP).
    pub netlist_rules: NetlistRules,
    /// Write: an `X` line's `m` multiplies the instance (ESPice ignores it).
    pub subckt_multiplicity: bool,
    /// Write: the Spectre analysis kinds the target has an analysis for.
    pub spectre_analyses: &'static [&'static str],
    /// Read and write: the expression word for the circuit temperature
    /// (ngspice `temper`, Spectre `temp`, VACASK `$temp`); empty: none.
    pub temp_name: &'static str,
    /// Read: the source's builtin constants (VACASK `M_PI`, `P_K`, …) and
    /// each one's value as a number's text, written as that off the source.
    pub builtin_constants: &'static [(&'static str, &'static str)],
    /// Write: the target's built-in device names, which a model type names
    /// before any OSDI module the deck loads (ngspice `devhelp`).
    pub builtin_devices: &'static [&'static str],
    /// Write: the VACASK analysis kinds no SPICE card reads (`dcinc`,
    /// `hb`, …) that the target has an analysis for, untranslated (W0001);
    /// any other is one it lacks (W0015).
    pub vacask_analyses: &'static [&'static str],
    /// Write: a SPICE `.options` key (lowercase) → its answer on the
    /// target (`DialectDef::option_to`, `option_spell`, `option_method`,
    /// `option_cosmetic`); a key not here is unmapped (Spectre notes it
    /// W0024, VACASK asks `Construct::UnmappedOption`).
    pub option_map: &'static [(&'static str, OptionTo)],
    /// Write: the master a SPICE `.model` type gets at a LEVEL, one row
    /// per (type, LEVELs) (`DialectDef::model_master`).
    pub model_masters: &'static [ModelMaster],
    /// Write: the `.model` types (lowercase) the target reads as SPICE's
    /// (`DialectDef::spice_model_type`): one with no `model_masters` row
    /// at its LEVEL is refused (W0104: "the nearest model" is other
    /// equations); a type not here is a master of the target's own,
    /// written as it is (Spectre `bsim4`, a module a VACASK deck loads).
    pub spice_model_types: &'static [&'static str],
    /// Read: the words whose line, kept as this language's text, is a
    /// construct of the language's own (HSPICE `.stateye`, Xyce `yneuron`),
    /// each lowercased with its construct (`Rd::kept_construct`); any other
    /// kept line is `Construct::ForeignText`. A one-letter key matches every
    /// element line of that letter, and the reader adds the construct's own
    /// shape check there (`Construct::HspiceWLine`: a W model key on the
    /// card; any other construct: none, so `"a"` matches every A line).
    pub kept_words: &'static [(&'static str, Construct)],
    /// Read: the dot cards this language reads its own way ahead of the
    /// shared dot cards, by word (lowercased, without its dot), each with
    /// its rule (`Rd::dialect_card`).
    pub dot_rules: &'static [(&'static str, DotRule)],
}

/// A SPICE `.model` type's master at some LEVELs (`Behaviours::model_masters`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModelMaster {
    /// The SPICE type, lowercase.
    pub kind: &'static str,
    /// The LEVELs it covers (no `level` is 1).
    pub levels: &'static [u32],
    /// The target's master (Spectre `bsim4`; VACASK module `bsim4v8`,
    /// written `sp_bsim4v8`).
    pub master: &'static str,
    /// Its `type=` value (Spectre `n`, VACASK `-1`); `None`: none written.
    pub type_: Option<&'static str>,
}

/// A target's answer for a `.model` type at a LEVEL (`Behaviours::model_master`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MasterAnswer {
    /// The target's master for it.
    Master(&'static ModelMaster),
    /// A SPICE type the target has no master of at that LEVEL.
    Refused,
    /// No SPICE type the target lists: a master of the target's own.
    AsWritten,
}

/// What a SPICE `.options` key is on a target (`Behaviours::option_map`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionTo {
    /// The target's option of the same quantity, its value respelled.
    Name(&'static str),
    /// The target's own text for the key, `{}` in it standing for the
    /// value respelled (VACASK `gshunt=1/({})` for `rshunt`); a text
    /// with no `{}` is written whatever the value (`op_skipinitial=1` for
    /// the flag `noopiter`).
    Spell(&'static str),
    /// The integration method (`method=trap|gear`): the writer spells the
    /// value (VACASK `tran_method="gear2"`).
    Method,
    /// Changes no number in the result (bookkeeping, printing): left out,
    /// noted W0024.
    Cosmetic,
}

/// How the reader takes one of a language's own dot cards
/// ([`Behaviours::dot_rules`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DotRule {
    /// A card with no fields is left out, with this note (a `Kind::Note`
    /// code; `register` refuses another: LTspice `.backanno`, W0036).
    Note(Code),
    /// The S-parameter analysis over the deck's `.ac`, when the card's
    /// testbench rule reads it (Xyce `.LIN`: ngspice's `.sp` of that sweep).
    SParams,
}

/// The writer layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Writer {
    /// SPICE cards.
    Cards,
    /// CDL in the xschem layout.
    Cdl,
    /// Structural Verilog.
    Verilog,
    /// Spectre statements.
    Spectre,
    /// VACASK statements.
    Vacask,
}

/// How a number's digits became a double when the text was read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NumberReading {
    /// ngspice's `INPevaluate`, digit by digit.
    Ngspice,
    /// Correctly rounded decimal.
    Decimal,
}

/// Whose equations a read model, option or expression means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModelSemantics {
    Ngspice,
    Other,
}

/// The grammar of a source's specifications.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceRules {
    Ngspice,
    Spice,
}

/// The old reader's per-dialect rules for SPICE cards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardRules {
    Spice,
    Ngspice,
    Hspice,
    Ltspice,
    Xyce,
}

/// The named reader's statement rules.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NamedRules {
    /// Not a named language.
    None,
    Spectre,
    Vacask,
}

/// Whose option names and option cards these are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionsForm {
    Spice,
    /// Xyce's `.options <package>` cards.
    XycePackages,
    Spectre,
    Vacask,
}

/// How Verilog-A and OSDI loads are written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoadForm {
    Ngspice,
    Espice,
    Hspice,
    Xyce,
    Ltspice,
    /// No load form of its own.
    None,
}

/// Whether a `.control` block runs the analyses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlBlock {
    Ngspice,
    None,
}

/// The analysis cards' forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnalysisForm {
    Ngspice,
    Espice,
    Hspice,
    Xyce,
    Ltspice,
    Plain,
}

/// When `.end` is written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EndRule {
    /// As the deck's role says.
    Always,
    /// Only when the deck runs (the old HSPICE writer's guess).
    WhenRuns,
}

/// How a value kept as text is quoted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueQuoting {
    AsWritten,
    /// HSPICE's `'…'` around any expression it would read as words.
    Hspice,
}

/// How a `K` card couples more than two inductors.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Coupling {
    /// One card per pair.
    Pairs,
    /// Every pair on one card (Xyce, LTspice).
    OneCard,
}

/// What a file included inside a subcircuit defines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IncludeScope {
    /// The subcircuit's own.
    Local,
    /// Global definitions (Xyce, run by hand): a deck read elsewhere
    /// loses its scope there.
    Global,
}

/// How parameters and model parameters are written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParamForm {
    Plain,
    Xyce,
}

/// Whose evaluator reads a `{}` expression (`render::spice::ternary`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExprEval {
    /// C's grammar: `a?b?c:d:e` is `a?(b?c:d):e`; `if()` and `sdt()` are
    /// whatever the dialect's function table holds.
    C,
    /// ngspice-45's numparam: `a?b?c:d:e` is `a?c:e`; no `if()`, no
    /// `sdt()` in a B source (manual §5.1); `log` natural, `nint` to even,
    /// a statistical call drawn, `limit(nom, var)` a distribution.
    Ngspice,
    /// Xyce's: C's grouping; `log` base 10 (RG §2.3), `nint` away from
    /// zero, a statistical call at its nominal outside sampling, `rand()`
    /// 0.5, `limit(x, lo, hi)` a clamp.
    Xyce,
    /// HSPICE's: C's grouping; `log` natural, a statistical call at its
    /// nominal outside a Monte Carlo run, a double-quoted value an
    /// expression.
    Hspice,
    /// LTspice's: C's grouping; `log` natural, `limit(x, lo, hi)` a clamp.
    Ltspice,
}

/// Whose simulator's netlist rules give a deck its meaning
/// (`Behaviours::netlist_rules`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetlistRules {
    /// None of the rules below of its own.
    Spice,
    /// ngspice: a model `r=` is the resistance; an `X` line's `m` the
    /// multiplicity unless declared; a parameter's last definition; a
    /// device IC only under UIC; SFFM (VO VA FM MDI FC); TNOM 27 °C; a
    /// TSTEP of 0 refused.
    Ngspice,
    /// Xyce: a model `R` multiplies the instance value; `M` the
    /// multiplicity; a parameter's first definition; a device IC in the
    /// DCOP; `$G_` global nodes; TSTEP 0 the simulator's choice.
    Xyce,
    /// HSPICE: TNOM and TEMP 25 °C; GSHUNT, GMINDC, SEARCH options.
    Hspice,
    /// LTspice: TSTEP 0 the simulator's choice.
    Ltspice,
}

/// How an LTspice element's parasitics are written (`spice::parasitics`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Parasitics {
    /// As explicit R, L and C elements.
    Elements,
    /// LTspice's own parameters, as read.
    Ltspice,
    /// A C's or L's as read, a V source's as elements (CDL, Verilog).
    Netlist,
}

/// The named writer's parameter rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NamedParams {
    /// Not a named target.
    None,
    Spectre,
    Vacask,
}

impl Behaviours {
    /// A SPICE-card language's behaviours where none is declared: SPICE
    /// cards, the SPICE rules, no dialect's own forms. `text_family` is
    /// the language itself, set when it registers.
    pub const DEFAULT: Behaviours = Behaviours {
        writer: Writer::Cards,
        text_family: Lang::NGSPICE,
        caret_power: true,
        hier_sep: '.',
        numbers: NumberReading::Decimal,
        model_semantics: ModelSemantics::Other,
        source_rules: SourceRules::Spice,
        card_rules: CardRules::Spice,
        named_rules: NamedRules::None,
        options_form: OptionsForm::Spice,
        load_form: LoadForm::None,
        control_block: ControlBlock::None,
        analysis_form: AnalysisForm::Plain,
        end_rule: EndRule::Always,
        value_quoting: ValueQuoting::AsWritten,
        coupling: Coupling::Pairs,
        param_form: ParamForm::Plain,
        include_scope: IncludeScope::Local,
        named_params: NamedParams::None,
        analysis_masters: None,
        expr_eval: ExprEval::C,
        parasitics: Parasitics::Elements,
        netlist_rules: NetlistRules::Spice,
        subckt_multiplicity: true,
        spectre_analyses: &[],
        temp_name: "",
        builtin_constants: &[],
        builtin_devices: &[],
        vacask_analyses: &[],
        option_map: &[],
        model_masters: &[],
        spice_model_types: &[],
        kept_words: &[],
        dot_rules: &[],
    };

    /// The master SPICE `.model` type `kind` (lowercase) gets at `level`.
    pub fn model_master(&self, kind: &str, level: u32) -> MasterAnswer {
        debug_assert!(!kind.bytes().any(|b| b.is_ascii_uppercase()));
        let row = self
            .model_masters
            .iter()
            .find(|m| m.kind == kind && m.levels.contains(&level));
        match row {
            Some(m) => MasterAnswer::Master(m),
            None if self.spice_model_types.contains(&kind) => MasterAnswer::Refused,
            None => MasterAnswer::AsWritten,
        }
    }
}
