//! Languages as handles into one table of rows: a [`Lang`] → its row,
//! which holds every table its reader and writers read (the lexicon, the
//! spelling, the capability column) and the code path that reads them
//! (its [`Dialect`]). The readers and writers reach a language's tables
//! only through its row, so a row is what a dialect definition fills
//! (`tests/fixtures/transpiler/DIALECT-PLUGIN.md`).
//!
//! Today the rows are the built-ins, filled from the per-dialect tables
//! (`lexicon::*`, `render::spelling`, `render::capability::action`) at
//! compile time; registration of a definition replaces this (P3, P8 there).
//!
//! Cites: none (a registry; each row's tables cite their manuals).
//!
//! Contents:
//! - `pub struct Lang`, `Lang::{NGSPICE, LTSPICE, SPECTRE, VACASK, ESPICE, HSPICE, XYCE, CDL, VERILOG}: Lang`
//! - `pub type Cells`, `pub const CONSTRUCTS: [Construct; CONSTRUCT_COUNT]`, `pub const CONSTRUCT_COUNT: usize`
//! - `Lang::dialect(self) -> Dialect`, `Lang::lexicon(self) -> Option<&'static Lexicon>`,
//!   `Lang::spelling(self) -> &'static Spelling`, `Lang::cells(self) -> &'static Cells`,
//!   `Lang::action(self, c: Construct) -> Action`
//! - `impl From<Dialect> for Lang`

use crate::syntax::Dialect;
use crate::syntax::lexicon::{self, Lexicon};
use crate::syntax::render::capability::{Action, Construct, action};
use crate::syntax::render::spelling::{Spelling, spelling};

/// A language a deck is read or written in: an index into the rows. The
/// built-ins are their [`Dialect`]'s position in [`Dialect::ALL`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Lang(u16);

/// A language's capability column: what it does with each construct,
/// indexed by `Construct as usize`.
pub type Cells = [Action; CONSTRUCT_COUNT];

/// How many [`Construct`]s there are.
pub const CONSTRUCT_COUNT: usize = 35;

/// Every [`Construct`], in declaration order, so `CONSTRUCTS[c as usize]`
/// is `c` (asserted at compile time, and [`position`] fails to compile
/// when a construct is added and not listed).
pub const CONSTRUCTS: [Construct; CONSTRUCT_COUNT] = [
    Construct::ForeignText,
    Construct::PrivateText,
    Construct::XyceSens,
    Construct::CaretReread,
    Construct::StatisticalCall,
    Construct::VerilogALoad,
    Construct::OsdiLoad,
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
];

/// One language's tables.
struct Row {
    dialect: Dialect,
    lexicon: Option<&'static Lexicon>,
    spelling: &'static Spelling,
    cells: Cells,
}

/// The rows, the built-ins at their [`Dialect::ALL`] positions.
// ponytail: built-ins only, filled at compile time; the registry that
// appends a registered definition's row lands with the definitions (P3).
static ROWS: [Row; 9] = [
    builtin(Dialect::Ngspice),
    builtin(Dialect::Ltspice),
    builtin(Dialect::Spectre),
    builtin(Dialect::Vacask),
    builtin(Dialect::Espice),
    builtin(Dialect::Hspice),
    builtin(Dialect::Xyce),
    builtin(Dialect::Cdl),
    builtin(Dialect::Verilog),
];

const _: () = {
    let mut i = 0;
    while i < CONSTRUCT_COUNT {
        assert!(CONSTRUCTS[i] as usize == i);
        assert!(position(CONSTRUCTS[i]) == i);
        i += 1;
    }
    let mut d = 0;
    while d < ROWS.len() {
        assert!(Dialect::ALL[d] as usize == d);
        d += 1;
    }
    assert!(ROWS.len() == Dialect::ALL.len());
};

impl Lang {
    pub const NGSPICE: Lang = Lang(0);
    pub const LTSPICE: Lang = Lang(1);
    pub const SPECTRE: Lang = Lang(2);
    pub const VACASK: Lang = Lang(3);
    pub const ESPICE: Lang = Lang(4);
    pub const HSPICE: Lang = Lang(5);
    pub const XYCE: Lang = Lang(6);
    pub const CDL: Lang = Lang(7);
    pub const VERILOG: Lang = Lang(8);

    /// The code path that reads this language's rows: the reader and
    /// writer family, and (until the dialect checks become rows) the
    /// dialect they ask about.
    pub fn dialect(self) -> Dialect {
        self.row().dialect
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
}

impl From<Dialect> for Lang {
    fn from(d: Dialect) -> Lang {
        let lang = Lang(d as u16);
        assert!(lang.row().dialect == d);
        lang
    }
}

// ---- private ----

impl Lang {
    fn row(self) -> &'static Row {
        let row = &ROWS[usize::from(self.0)];
        assert!(row.dialect as u16 == self.0, "a built-in row at its index");
        row
    }
}

/// Built-in `d`'s row, from its per-dialect tables.
const fn builtin(d: Dialect) -> Row {
    let lexicon = match d {
        Dialect::Ngspice | Dialect::Espice => Some(&lexicon::NGSPICE),
        Dialect::Hspice => Some(&lexicon::HSPICE),
        Dialect::Ltspice => Some(&lexicon::LTSPICE),
        Dialect::Xyce => Some(&lexicon::XYCE),
        Dialect::Spectre => Some(&lexicon::SPECTRE),
        Dialect::Vacask => Some(&lexicon::VACASK),
        Dialect::Cdl | Dialect::Verilog => None,
    };
    let mut cells = [Action::Native; CONSTRUCT_COUNT];
    let mut i = 0;
    while i < CONSTRUCT_COUNT {
        cells[i] = action(d, CONSTRUCTS[i]);
        i += 1;
    }
    Row {
        dialect: d,
        lexicon,
        spelling: spelling(d),
        cells,
    }
}

/// `c`'s index in [`CONSTRUCTS`]: an exhaustive match, so a construct
/// added to [`Construct`] does not compile until it is listed.
const fn position(c: Construct) -> usize {
    match c {
        Construct::ForeignText => 0,
        Construct::PrivateText => 1,
        Construct::XyceSens => 2,
        Construct::CaretReread => 3,
        Construct::StatisticalCall => 4,
        Construct::VerilogALoad => 5,
        Construct::OsdiLoad => 6,
        Construct::LibWholeFile => 7,
        Construct::NgBehavior => 8,
        Construct::LibSection => 9,
        Construct::RclModel => 10,
        Construct::ChargeCapacitor => 11,
        Construct::BehaviouralSource => 12,
        Construct::Laplace => 13,
        Construct::RfPort => 14,
        Construct::PwlRepeatV => 15,
        Construct::PwlRepeatI => 16,
        Construct::NoiseWave => 17,
        Construct::Conditional => 18,
        Construct::BranchParam => 19,
        Construct::Func => 20,
        Construct::Step => 21,
        Construct::StepList => 22,
        Construct::TranStartup => 23,
        Construct::DcLogList => 24,
        Construct::Tf => 25,
        Construct::Sens => 26,
        Construct::SensAc => 27,
        Construct::PoleZero => 28,
        Construct::SParameter => 29,
        Construct::Print => 30,
        Construct::Save => 31,
        Construct::UnmappedOption => 32,
        Construct::Statistics => 33,
        Construct::Measure => 34,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_built_in_row_holds_its_dialects_tables() {
        // The oracle is the per-dialect tables the writers read before the
        // rows: the spelling row, the capability match, and the lexicon
        // each dialect's text was read with (ESPice reads ngspice's).
        for d in Dialect::ALL {
            let l = Lang::from(d);
            assert_eq!(l.dialect(), d);
            // A `const` row's address is its use site's, so the rows
            // compare by their fields.
            let (got, want) = (l.spelling(), spelling(d));
            assert_eq!(got.target, want.target);
            assert_eq!(got.lexicon.dialect, want.lexicon.dialect, "{d:?}");
            assert_eq!(got.comment, want.comment, "{d:?}");
            assert_eq!(got.path_quote, want.path_quote, "{d:?}");
            assert_eq!(got.expr, want.expr, "{d:?}");
            assert_eq!(got.func, want.func, "{d:?}");
            assert_eq!(got.subckt_params, want.subckt_params, "{d:?}");
            assert_eq!(got.ngspice_numbers, want.ngspice_numbers, "{d:?}");
            for c in CONSTRUCTS {
                assert_eq!(l.action(c), action(d, c), "{d:?} {c:?}");
            }
            let read = l.lexicon().map(|x| x.dialect);
            let want = match d {
                Dialect::Cdl | Dialect::Verilog => None,
                _ => Some(d.family()),
            };
            assert_eq!(read, want, "{d:?}");
        }
        assert_eq!(Lang::from(Dialect::Hspice), Lang::HSPICE);
        assert_eq!(Lang::VERILOG.dialect(), Dialect::Verilog);
    }
}
