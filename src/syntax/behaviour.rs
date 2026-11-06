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
//!   `pub enum ExprEval`, `pub enum Parasitics`

use crate::syntax::Lang;

/// A language's behaviours.
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
    /// Read and write: whose evaluator reads a `{}` expression (a
    /// then-branch ternary's grouping, `if()`, `sdt()`).
    pub expr_eval: ExprEval,
    /// Write: an LTspice element's parasitics (`Rser=`, `Cpar=`, …).
    pub parasitics: Parasitics,
    /// Write: the Spectre analysis kinds the target has an analysis for.
    pub spectre_analyses: &'static [&'static str],
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
    /// `sdt()` in a B source (manual §5.1).
    Ngspice,
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
        expr_eval: ExprEval::C,
        parasitics: Parasitics::Elements,
        spectre_analyses: &[],
    };
}
