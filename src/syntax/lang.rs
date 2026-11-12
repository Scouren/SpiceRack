//! Languages as handles into one registry of rows: a [`Lang`] → its row,
//! which holds every table its reader and writers read (the lexicon, the
//! spelling, the capability column, the behaviours). The readers and
//! writers reach a language's tables only through its row, and a row is
//! what `syntax::def::register` freezes from a definition
//! (`tests/fixtures/transpiler/DIALECT-PLUGIN.md`).
//!
//! The built-ins are registered on the first read of the registry, in a
//! fixed order, so their handles are constants; a registered language is
//! never removed. A read takes no lock: rows are published once each, by
//! [`append`] under one lock, and never change.
//!
//! Cites: none (a registry; each row's tables cite their manuals).
//!
//! Contents:
//! - `pub struct Lang`, `Lang::{NGSPICE, ESPICE, HSPICE, XYCE, LTSPICE, SPECTRE, VACASK, CDL, VERILOG, NGSPICE_GIVEN,
//!   SPICERACK}: Lang`,
//!   `Lang::BUILTIN_COUNT: usize`
//! - `pub type Cells`, `pub const CONSTRUCTS: [Construct; CONSTRUCT_COUNT]`, `pub const CONSTRUCT_COUNT: usize`,
//!   `pub const LANGS_MAX: usize`
//! - `Lang::name(self) -> &'static str`, `Lang::index(self) -> u32`, `Lang::from_name(name: &str) -> Option<Lang>`,
//!   `Lang::lexicon(self) -> Option<&'static Lexicon>`, `Lang::spelling(self) -> &'static Spelling`,
//!   `Lang::cells(self) -> &'static Cells`, `Lang::action(self, c: Construct) -> Action`,
//!   `Lang::behaviours(self) -> &'static Behaviours`
//! - `Lang::runs_on(self) -> Option<&'static str>`, `Lang::dump(self) -> String`, `Lang::base(self) -> Option<Lang>`,
//!   `Lang::row(self) -> &'static Row`
//! - `pub struct PluginCode`, `pub fn plugin_code(detail: u16) -> PluginCode`
//! - `impl From<Dialect> for Lang`
//! - `pub(crate) struct Row`, `pub(crate) fn append(name: &str, build: impl FnOnce(Lang, u16) -> Row) -> Result<Lang, Option<Lang>>`

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Once, OnceLock};

use crate::syntax::Dialect;
use crate::syntax::behaviour::Behaviours;
use crate::syntax::def::{Hook, RunsOn};
use crate::syntax::diag::Kind;
use crate::syntax::lexicon::Lexicon;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::spelling::Spelling;
use crate::syntax::token::Tag;

/// A language a deck is read or written in: an index into the registry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Lang(u16);

/// A language's capability column: what it does with each construct,
/// indexed by `Construct as usize`.
pub type Cells = [Action; CONSTRUCT_COUNT];

/// How many [`Construct`]s there are.
pub const CONSTRUCT_COUNT: usize = 46;

/// How many languages can be registered, the built-ins among them.
pub const LANGS_MAX: usize = 64;

/// Every [`Construct`], in declaration order, so `CONSTRUCTS[c as usize]`
/// is `c` (asserted at compile time, and [`position`] fails to compile
/// when a construct is added and not listed).
pub const CONSTRUCTS: [Construct; CONSTRUCT_COUNT] = [
    Construct::ForeignText,
    Construct::PrivateText,
    Construct::XyceSens,
    Construct::HspiceAnalysis,
    Construct::VacaskScript,
    Construct::CaretReread,
    Construct::StatisticalCall,
    Construct::VerilogALoad,
    Construct::OsdiLoad,
    Construct::OsdiInstance,
    Construct::LibWholeFile,
    Construct::NgBehavior,
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
    Construct::VacaskElaborate,
    Construct::Epochs,
    Construct::InertText,
];

/// A code a definition declared (`DialectDef::code`): what a `W0201` or
/// `W0202` diagnostic's `detail` names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PluginCode {
    /// The language that declared it.
    pub lang: Lang,
    pub id: &'static str,
    pub kind: Kind,
    pub message: &'static str,
}

/// One language's tables, as registration froze them.
pub(crate) struct Row {
    pub(crate) name: &'static str,
    pub(crate) lexicon: Option<&'static Lexicon>,
    pub(crate) spelling: &'static Spelling,
    pub(crate) cells: Cells,
    /// Per construct: the plugin code a `W0201`/`W0202` cell raises (an
    /// index of the global code table, [`plugin_code`]); 0 elsewhere.
    pub(crate) details: [u16; CONSTRUCT_COUNT],
    pub(crate) behaviours: Behaviours,
    /// Its codes, `(id, kind, message)`: global indices `first_code..`.
    pub(crate) codes: &'static [(&'static str, Kind, &'static str)],
    pub(crate) first_code: u16,
    /// The post-pass's maps (`render::def_pass`): a keyword's written head,
    /// a master's written word, an option's written name; and the hooks.
    pub(crate) heads: &'static [(Tag, &'static str)],
    pub(crate) write_masters: &'static [(&'static str, &'static str)],
    pub(crate) options: &'static [(&'static str, &'static str)],
    pub(crate) hooks: &'static [(&'static str, Hook)],
    /// The simulator that reads its text (`DialectDef::runs_on`).
    pub(crate) runs_on: Option<RunsOn>,
    /// The language it extends.
    pub(crate) base: Option<Lang>,
}

/// The registry: each slot set once, `COUNT` the slots set.
static ROWS: [OnceLock<&'static Row>; LANGS_MAX] = [const { OnceLock::new() }; LANGS_MAX];
static COUNT: AtomicUsize = AtomicUsize::new(0);
/// Taken by [`append`]: one registration at a time.
static APPEND: Mutex<()> = Mutex::new(());
static BUILTINS: Once = Once::new();

thread_local! {
    /// This thread is registering the built-ins (their definitions read the
    /// rows registered before them).
    static REGISTERING: Cell<bool> = const { Cell::new(false) };
}

const _: () = {
    let mut i = 0;
    while i < CONSTRUCT_COUNT {
        assert!(CONSTRUCTS[i] as usize == i);
        assert!(position(CONSTRUCTS[i]) == i);
        i += 1;
    }
};

impl Lang {
    pub const NGSPICE: Lang = Lang(0);
    pub const ESPICE: Lang = Lang(1);
    pub const HSPICE: Lang = Lang(2);
    pub const XYCE: Lang = Lang(3);
    pub const LTSPICE: Lang = Lang(4);
    pub const SPECTRE: Lang = Lang(5);
    pub const VACASK: Lang = Lang(6);
    pub const CDL: Lang = Lang(7);
    pub const VERILOG: Lang = Lang(8);
    /// The Python API's decks: ngspice text with decimal numbers, `.step`
    /// and dialect regions (`syntax::dialects::ngspice_given`).
    pub const NGSPICE_GIVEN: Lang = Lang(9);
    /// SpiceRack's own text (decision D16): ngspice-given's statements
    /// with the neutral expression grammar (`syntax::dialects::spicerack`).
    pub const SPICERACK: Lang = Lang(10);
    /// How many built-ins there are: the first registered handles.
    pub const BUILTIN_COUNT: usize = 11;

    /// The name it was registered under (`ngspice`, ...).
    pub fn name(self) -> &'static str {
        self.row().name
    }

    /// Its position in the registry (a node's word, `ast::Tag::Verbatim`).
    pub fn index(self) -> u32 {
        u32::from(self.0)
    }

    /// The language registered as `name`, compared case-blind.
    pub fn from_name(name: &str) -> Option<Lang> {
        builtins();
        let n = COUNT.load(Ordering::Acquire);
        (0..n)
            .map(|i| Lang(i as u16))
            .find(|l| l.row().name.eq_ignore_ascii_case(name))
    }

    /// The lexicon its text is read with; `None` for a language with no
    /// reader (CDL, Verilog).
    pub fn lexicon(self) -> Option<&'static Lexicon> {
        self.row().lexicon
    }

    /// How its writer spells what every target writes.
    pub fn spelling(self) -> &'static Spelling {
        self.row().spelling
    }

    /// What its writer does with each construct.
    pub fn cells(self) -> &'static Cells {
        &self.row().cells
    }

    /// What its writer does with `c`.
    pub fn action(self, c: Construct) -> Action {
        self.cells()[c as usize]
    }

    /// What its reader and writer do that its tables do not say.
    pub fn behaviours(self) -> &'static Behaviours {
        &self.row().behaviours
    }

    /// The simulator that reads its text, by the name it is registered
    /// under (`backend::by_name`): a built-in's own (its dialect's name,
    /// whether or not SpiceRack drives one), a plugin's declared one
    /// (`DialectDef::runs_on`); `None`: netlist only.
    pub fn runs_on(self) -> Option<&'static str> {
        let own = Dialect::ALL.into_iter().find(|&d| Lang::from(d) == self);
        let declared = self.row().runs_on.map(|on| on.sim);
        own.map(Dialect::name).or(declared)
    }

    /// Its row as canonical text: every table, behaviour, cell, code and
    /// hook head, languages by name; two definitions that register equal
    /// rows dump the same bytes.
    pub fn dump(self) -> String {
        crate::syntax::def::dump(self.row())
    }

    /// The language it extends, `None` for a root.
    pub fn base(self) -> Option<Lang> {
        self.row().base
    }
}

/// The plugin code a `W0201`/`W0202` diagnostic's `detail` names.
///
/// # Panics
/// When no registered language declared a code at `detail`.
pub fn plugin_code(detail: u16) -> PluginCode {
    builtins();
    let n = COUNT.load(Ordering::Acquire);
    let row = (0..n)
        .map(|i| Lang(i as u16))
        .find(|l| {
            let r = l.row();
            (r.first_code..r.first_code + r.codes.len() as u16).contains(&detail)
        })
        .expect("plugin_code: a registered code");
    let r = row.row();
    let (id, kind, message) = r.codes[usize::from(detail - r.first_code)];
    PluginCode {
        lang: row,
        id,
        kind,
        message,
    }
}

impl From<Dialect> for Lang {
    fn from(d: Dialect) -> Lang {
        match d {
            Dialect::Ngspice => Lang::NGSPICE,
            Dialect::Espice => Lang::ESPICE,
            Dialect::Hspice => Lang::HSPICE,
            Dialect::Xyce => Lang::XYCE,
            Dialect::Ltspice => Lang::LTSPICE,
            Dialect::Spectre => Lang::SPECTRE,
            Dialect::Vacask => Lang::VACASK,
            Dialect::Cdl => Lang::CDL,
            Dialect::Verilog => Lang::VERILOG,
        }
    }
}

/// Publishes the row `build` makes for the next handle (and its first
/// global code index), under the registry's lock: `Err(Some(held))` when
/// a language named `name` is registered (checked under the lock, so two
/// threads registering one name get one row), `Err(None)` when every
/// slot, or the code table, is full.
///
/// # Panics
/// When `build` registers (the lock is held).
pub(crate) fn append(
    name: &str,
    build: impl FnOnce(Lang, u16) -> Row,
) -> Result<Lang, Option<Lang>> {
    builtins();
    let _held = APPEND.lock().unwrap_or_else(|p| p.into_inner());
    let i = COUNT.load(Ordering::Acquire);
    let taken = (0..i).find(|&k| {
        let r = ROWS[k].get().expect("set before the count");
        r.name.eq_ignore_ascii_case(name)
    });
    if let Some(k) = taken {
        return Err(Some(Lang(k as u16)));
    }
    if i == LANGS_MAX {
        return Err(None);
    }
    let me = Lang(i as u16);
    let first = match i {
        0 => 0,
        _ => {
            let last = ROWS[i - 1].get().expect("set before the count");
            usize::from(last.first_code) + last.codes.len()
        }
    };
    let built = build(me, u16::try_from(first).map_err(|_| None)?);
    if first + built.codes.len() > usize::from(u16::MAX) {
        return Err(None);
    }
    assert!(
        built.first_code as usize == first,
        "append: the code index given"
    );
    let row: &'static Row = Box::leak(Box::new(built));
    assert!(ROWS[i].set(row).is_ok(), "append: a slot set once");
    COUNT.store(i + 1, Ordering::Release);
    Ok(me)
}

impl Lang {
    /// Its row.
    pub(crate) fn row(self) -> &'static Row {
        builtins();
        let i = usize::from(self.0);
        assert!(i < LANGS_MAX);
        ROWS[i].get().expect("a registered language")
    }
}

// ---- private ----

/// Registers the built-ins once, before the first read; a read from their
/// own definitions (while registering) sees the ones before it.
fn builtins() {
    if REGISTERING.get() {
        return;
    }
    BUILTINS.call_once(|| {
        REGISTERING.set(true);
        crate::syntax::dialects::register_builtins();
        REGISTERING.set(false);
        assert!(COUNT.load(Ordering::Acquire) == Lang::BUILTIN_COUNT);
    });
}

/// `c`'s index in [`CONSTRUCTS`]: an exhaustive match, so a construct
/// added to [`Construct`] does not compile until it is listed.
const fn position(c: Construct) -> usize {
    match c {
        Construct::ForeignText => 0,
        Construct::PrivateText => 1,
        Construct::XyceSens => 2,
        Construct::HspiceAnalysis => 3,
        Construct::VacaskScript => 4,
        Construct::CaretReread => 5,
        Construct::StatisticalCall => 6,
        Construct::VerilogALoad => 7,
        Construct::OsdiLoad => 8,
        Construct::OsdiInstance => 9,
        Construct::LibWholeFile => 10,
        Construct::NgBehavior => 11,
        Construct::LibSection => 12,
        Construct::RclModel => 13,
        Construct::ChargeCapacitor => 14,
        Construct::BehaviouralSource => 15,
        Construct::Laplace => 16,
        Construct::RfPort => 17,
        Construct::PwlRepeatV => 18,
        Construct::PwlRepeatI => 19,
        Construct::NoiseWave => 20,
        Construct::Conditional => 21,
        Construct::BranchParam => 22,
        Construct::Func => 23,
        Construct::LocalFunc => 24,
        Construct::TableSource => 25,
        Construct::LossyLine => 26,
        Construct::AmWave => 27,
        Construct::Step => 28,
        Construct::StepList => 29,
        Construct::TranStartup => 30,
        Construct::DcLogList => 31,
        Construct::Tf => 32,
        Construct::Sens => 33,
        Construct::SensAc => 34,
        Construct::PoleZero => 35,
        Construct::SParameter => 36,
        Construct::Print => 37,
        Construct::Save => 38,
        Construct::UnmappedOption => 39,
        Construct::Statistics => 40,
        Construct::Measure => 41,
        Construct::HspiceWLine => 42,
        Construct::VacaskElaborate => 43,
        Construct::Epochs => 44,
        Construct::InertText => 45,
    }
}
