//! The simulator registry: one frozen [`SimDef`] per simulator SpiceRack
//! drives, named by a [`Sim`] handle. The five built-ins (`BACKENDS`, in
//! its order) are registered first, through the path a caller's simulator
//! takes; a built-in's `analyses` bits are read from `Kind::spec`'s routes,
//! so who runs what stays one table.
//!
//! Layout (`syntax::lang`'s registry): a `SimDef` in, a leaked
//! `&'static SimDef` in a slot out; at most [`SIMS_MAX`] slots, so a `Sim`
//! is a `u8`; `features` is a `u8` (`feature` bits) and `analyses` a `u32`
//! (bit `kind as u32` per `Kind`). `select` scans the slots once per deck,
//! every other read is by handle. Rows live for the program. Registration
//! is serialized by one lock; reads take none.
//!
//! Cites: none (the layout is `src/syntax/lang.rs`'s).
//!
//! Contents:
//! - `pub const SIMS_MAX: usize`
//! - `pub struct Sim`, `Sim::{NGSPICE, LTSPICE, VACASK, SPECTRE, ESPICE}: Sim`,
//!   `Sim::of(d: Dialect) -> Option<Sim>`, `Sim::builtin(self) -> Option<Dialect>`,
//!   `Sim::row(self) -> &'static SimDef`, `Sim::name(self) -> &'static str`,
//!   `Sim::family(self) -> Dialect`, `Sim::installed(self) -> bool`,
//!   `Sim::run_all(self, netlist: &str) -> Result<Vec<RawData>, BackendError>`,
//!   `From<Sim> for RunsOn` (a dialect definition's `runs_on`)
//! - `pub struct SimDef`, `SimDef::check(&self) -> Result<(), SimRegisterError>`,
//!   `pub enum Runner` (`BuiltIn`, `Command`), `pub enum Installed` (`Fn`,
//!   `OnPath`), `pub enum SimRegisterError`, `SimRegisterError::code(&self) -> Code`
//! - `pub fn register_simulator(def: SimDef) -> Result<Sim, SimRegisterError>`
//! - `pub fn simulators() -> impl Iterator<Item = (Sim, &'static SimDef)>`
//! - `pub fn by_name(name: &str) -> Option<Sim>`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Once, OnceLock};

use super::command::{self, Arg, Output, Piece};
use super::{BACKENDS, BackendError, Kind, RUNNABLE, feature, is_on_path};
use crate::result::RawData;
use crate::syntax::def::RunsOn;
use crate::syntax::diag::Code;
use crate::syntax::{Dialect, Lang};

/// The most simulators registered at once, the five built-ins among them.
pub const SIMS_MAX: usize = 32;

/// A registered simulator: its slot in the registry. Two handles are equal
/// exactly when they name one registration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Sim(u8);

/// One simulator, as [`register_simulator`] takes it and [`Sim::row`]
/// gives it back. No invariant ties the fields; registration checks each.
#[derive(Clone, Debug, PartialEq)]
pub struct SimDef {
    /// The name `backend=` and `$SPICERACK_BACKEND` select it by (exact
    /// case); no other simulator holds it in any case.
    pub name: String,
    /// The language its decks are written in.
    pub dialect: Lang,
    /// How it runs a deck.
    pub runner: Runner,
    /// What it can do, as `feature` bits.
    pub features: u8,
    /// The analyses it runs: bit `kind as u32` for each [`Kind`].
    pub analyses: u32,
    /// How it is found installed.
    pub installed: Installed,
}

/// How a simulator runs a deck's text.
#[derive(Clone, Debug)]
pub enum Runner {
    /// A function of the calling crate (the five built-ins' `run`):
    /// netlist text in, every plot of the run out.
    BuiltIn(fn(&str) -> Result<Vec<RawData>, BackendError>),
    /// A command (`command.rs`): `argv`'s words with `{deck}`, `{raw}` and
    /// `{dir}` filled in, run with no shell; the results read as `output`
    /// says. Some word holds `{deck}`.
    Command {
        /// The program, then its arguments.
        argv: Vec<Arg>,
        /// How the results are read back.
        output: Output,
    },
}

/// How a simulator is found installed.
#[derive(Clone, Debug)]
pub enum Installed {
    /// A function says (the built-ins' probes).
    Fn(fn() -> bool),
    /// This program is on `$PATH` (a command's first word).
    OnPath(String),
}

/// Why [`register_simulator`] refused a definition. Each message opens
/// with the field at fault.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SimRegisterError {
    /// The name is empty.
    #[error("name: a simulator needs a name")]
    Unnamed,
    /// The name is held (in any case) by a different definition, or is one
    /// of `Dialect::ALIASES`.
    #[error("name: '{0}' is held by a different simulator")]
    Redefined(String),
    /// `features` has a bit outside `feature::ALL`.
    #[error("features: {0:#010b} has a bit that is no feature")]
    Features(u8),
    /// `analyses` is empty, or has a bit past the last `Kind`.
    #[error("analyses: {0:#x} is empty or has a bit past the last analysis kind")]
    Analyses(u32),
    /// A command is empty, has no `{deck}` word, or a word holds a
    /// placeholder that is none of `{deck}`, `{raw}`, `{dir}`.
    #[error("command: {0}")]
    Command(String),
    /// Every slot is taken.
    #[error("the simulator registry is full ({} simulators)", SIMS_MAX)]
    Full,
}

/// The registry: each slot set once, `COUNT` the slots set.
static ROWS: [OnceLock<&'static SimDef>; SIMS_MAX] = [const { OnceLock::new() }; SIMS_MAX];
static COUNT: AtomicUsize = AtomicUsize::new(0);
/// Taken by `append`: one registration at a time.
static APPEND: Mutex<()> = Mutex::new(());
static BUILTINS: Once = Once::new();

// A handle is a `u8` and `analyses` a `u32`: every slot and every kind fits.
const _: () = assert!(SIMS_MAX <= u8::MAX as usize + 1);
const _: () = assert!(Kind::ALL.len() <= u32::BITS as usize);
// The built-in handles are `BACKENDS`' positions (`RUNNABLE`'s order).
const _: () = {
    assert!(BACKENDS.len() <= SIMS_MAX);
    assert!(RUNNABLE[Sim::NGSPICE.0 as usize] as u8 == Dialect::Ngspice as u8);
    assert!(RUNNABLE[Sim::LTSPICE.0 as usize] as u8 == Dialect::Ltspice as u8);
    assert!(RUNNABLE[Sim::VACASK.0 as usize] as u8 == Dialect::Vacask as u8);
    assert!(RUNNABLE[Sim::SPECTRE.0 as usize] as u8 == Dialect::Spectre as u8);
    assert!(RUNNABLE[Sim::ESPICE.0 as usize] as u8 == Dialect::Espice as u8);
};

impl Sim {
    /// The built-in ngspice.
    pub const NGSPICE: Sim = Sim(0);
    /// The built-in LTspice.
    pub const LTSPICE: Sim = Sim(1);
    /// The built-in VACASK.
    pub const VACASK: Sim = Sim(2);
    /// The built-in Spectre.
    pub const SPECTRE: Sim = Sim(3);
    /// The built-in ESPice.
    pub const ESPICE: Sim = Sim(4);

    /// The built-in simulator of dialect `d`; `None` for a dialect no
    /// built-in runs (HSPICE, Xyce, CDL, Verilog).
    pub fn of(d: Dialect) -> Option<Sim> {
        let i = RUNNABLE.iter().position(|&r| r == d)?;
        Some(Sim(i as u8))
    }

    /// The dialect a built-in runs; `None` for a registered simulator.
    pub fn builtin(self) -> Option<Dialect> {
        RUNNABLE.get(usize::from(self.0)).copied()
    }

    /// Its definition, as registered.
    pub fn row(self) -> &'static SimDef {
        builtins();
        let i = usize::from(self.0);
        assert!(i < COUNT.load(Ordering::Acquire), "a registered simulator");
        ROWS[i].get().expect("set before the count")
    }

    /// The name it was registered under (`ngspice`, ...).
    pub fn name(self) -> &'static str {
        &self.row().name
    }

    /// The built-in dialect its decks are a kind of: its own, or the first
    /// built-in in its language's `extends` chain (ngspice for a root
    /// plugin). A run's `.meas` lines and plots are read by this family's
    /// rules (`simulation`).
    pub fn family(self) -> Dialect {
        let mut chain = std::iter::successors(Some(self.row().dialect), |l| l.base());
        let own = |l: Lang| Dialect::ALL.into_iter().find(|&d| Lang::from(d) == l);
        chain.find_map(own).unwrap_or(Dialect::Ngspice)
    }

    /// Whether it is installed on this machine, asked now (not cached).
    pub fn installed(self) -> bool {
        match &self.row().installed {
            Installed::Fn(probe) => probe(),
            Installed::OnPath(program) => is_on_path(program),
        }
    }

    /// Executes `netlist` (already in its dialect) and reads every plot of
    /// the run, in file order.
    ///
    /// # Errors
    /// Whatever the run returns; `NotInstalled` for a command simulator
    /// whose probe says it is absent (asked before anything is spawned).
    pub fn run_all(self, netlist: &str) -> Result<Vec<RawData>, BackendError> {
        let plots = match &self.row().runner {
            Runner::BuiltIn(run) => run(netlist)?,
            Runner::Command { argv, output } => {
                if !self.installed() {
                    return Err(BackendError::NotInstalled {
                        simulator: self.name().to_string(),
                    });
                }
                command::run(argv, *output, self.family(), netlist)?
            }
        };
        assert!(!plots.is_empty(), "a raw file read holds at least one plot");
        Ok(plots)
    }
}

impl From<Sim> for RunsOn {
    /// A registered simulator as a dialect definition's `runs_on` takes
    /// it: the name it is held under and the language it reads.
    fn from(sim: Sim) -> RunsOn {
        let row = sim.row();
        RunsOn {
            sim: row.name.as_str(),
            dialect: row.dialect,
        }
    }
}

impl SimDef {
    /// The edge checks of the definition's own fields, as
    /// [`register_simulator`] runs them first: what can be refused before
    /// the registry is asked.
    ///
    /// # Errors
    /// `Unnamed`; `Redefined` for a name that is one of `Dialect::ALIASES`;
    /// `Features`; `Analyses`; `Command` for a command with no word or no
    /// `{deck}`.
    pub fn check(&self) -> Result<(), SimRegisterError> {
        if self.name.is_empty() {
            return Err(SimRegisterError::Unnamed);
        }
        if Dialect::ALIASES.iter().any(|a| a.0 == self.name) {
            return Err(SimRegisterError::Redefined(self.name.clone()));
        }
        if self.features & !feature::ALL != 0 {
            return Err(SimRegisterError::Features(self.features));
        }
        let past = self
            .analyses
            .checked_shr(Kind::ALL.len() as u32)
            .unwrap_or(0);
        if self.analyses == 0 || past != 0 {
            return Err(SimRegisterError::Analyses(self.analyses));
        }
        if let Runner::Command { argv, .. } = &self.runner {
            if argv.is_empty() {
                return Err(SimRegisterError::Command("it has no program".into()));
            }
            if !argv.iter().flatten().any(|p| *p == Piece::Deck) {
                return Err(SimRegisterError::Command(
                    "no word holds {deck}: the simulator is never given the deck".into(),
                ));
            }
        }
        Ok(())
    }
}

impl SimRegisterError {
    /// Its catalogue code: `R0042` for a full registry, `R0040` (a
    /// descriptor that does not validate) for every field's refusal.
    pub fn code(&self) -> Code {
        match self {
            SimRegisterError::Full => Code::R0042,
            SimRegisterError::Unnamed
            | SimRegisterError::Redefined(_)
            | SimRegisterError::Features(_)
            | SimRegisterError::Analyses(_)
            | SimRegisterError::Command(_) => Code::R0040,
        }
    }
}

impl PartialEq for Runner {
    /// Equal when they run a deck the same way: the same function.
    fn eq(&self, other: &Runner) -> bool {
        match (self, other) {
            (Runner::BuiltIn(a), Runner::BuiltIn(b)) => std::ptr::fn_addr_eq(*a, *b),
            (Runner::Command { argv: a, output: x }, Runner::Command { argv: b, output: y }) => {
                a == b && x == y
            }
            _ => false,
        }
    }
}

impl PartialEq for Installed {
    /// Equal when they probe the same way: the same function.
    fn eq(&self, other: &Installed) -> bool {
        match (self, other) {
            (Installed::Fn(a), Installed::Fn(b)) => std::ptr::fn_addr_eq(*a, *b),
            (Installed::OnPath(a), Installed::OnPath(b)) => a == b,
            _ => false,
        }
    }
}

/// Registers `def` and returns its handle. Registering a definition equal
/// to the one held under its name returns that handle and registers
/// nothing.
///
/// # Errors
/// [`SimRegisterError`], each naming the field: the name is empty, an
/// alias, or held by a different definition; a `features` or `analyses`
/// bit names nothing, or `analyses` is empty; a command is empty or has
/// no `{deck}` word; the registry is full. Nothing is registered then.
pub fn register_simulator(def: SimDef) -> Result<Sim, SimRegisterError> {
    builtins();
    def.check()?;
    append(def)
}

/// Every registered simulator in registration order, the built-ins first.
pub fn simulators() -> impl Iterator<Item = (Sim, &'static SimDef)> {
    builtins();
    let n = COUNT.load(Ordering::Acquire);
    assert!(n <= SIMS_MAX);
    ROWS[..n]
        .iter()
        .enumerate()
        .map(|(i, r)| (Sim(i as u8), *r.get().expect("set before the count")))
}

/// The simulator a backend name selects: one registered under exactly
/// `name`, else the built-in a `Dialect::ALIASES` name stands for.
pub fn by_name(name: &str) -> Option<Sim> {
    let exact = simulators().find(|(_, r)| r.name == name).map(|(s, _)| s);
    exact.or_else(|| Dialect::from_backend_name(name).and_then(Sim::of))
}

// ---- private ----

/// Registers the built-ins once, before the first read: `BACKENDS`' rows,
/// in its order, so `Sim(i)` is `RUNNABLE[i]`'s simulator.
fn builtins() {
    BUILTINS.call_once(|| {
        for b in &BACKENDS {
            let def = SimDef {
                name: b.dialect.name().to_string(),
                dialect: b.dialect.into(),
                runner: Runner::BuiltIn(b.run),
                features: b.caps,
                analyses: routed(b.dialect),
                installed: Installed::Fn(b.installed),
            };
            let sim = append(def).expect("a built-in registers");
            assert!(
                Sim::of(b.dialect) == Some(sim),
                "built-ins in RUNNABLE's order"
            );
        }
        assert!(COUNT.load(Ordering::Acquire) == BACKENDS.len());
    });
}

/// The kinds `Kind::spec` routes to `d`, one bit each.
fn routed(d: Dialect) -> u32 {
    let runs = |k: &&Kind| k.spec().route.contains(&d);
    let bits = Kind::ALL
        .iter()
        .filter(runs)
        .fold(0u32, |m, &k| m | (1 << k as u32));
    assert!(bits != 0, "every built-in runs some analysis");
    bits
}

/// Publishes `def` in the next slot, under the registry's lock; the held
/// handle when an equal definition is registered under its name.
fn append(def: SimDef) -> Result<Sim, SimRegisterError> {
    let _held = APPEND.lock().unwrap_or_else(|p| p.into_inner());
    let n = COUNT.load(Ordering::Acquire);
    assert!(n <= SIMS_MAX);
    let named = |r: &OnceLock<&'static SimDef>| {
        let held = r.get().expect("set before the count");
        held.name.eq_ignore_ascii_case(&def.name)
    };
    if let Some(k) = ROWS[..n].iter().position(named) {
        let held = *ROWS[k].get().expect("set before the count");
        return match *held == def {
            true => Ok(Sim(k as u8)),
            false => Err(SimRegisterError::Redefined(def.name)),
        };
    }
    if n == SIMS_MAX {
        return Err(SimRegisterError::Full);
    }
    let row: &'static SimDef = Box::leak(Box::new(def));
    assert!(ROWS[n].set(row).is_ok(), "append: a slot set once");
    COUNT.store(n + 1, Ordering::Release);
    Ok(Sim(n as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Result<Vec<RawData>, BackendError> {
        Err(BackendError::NoBackend)
    }

    fn absent() -> bool {
        false
    }

    /// Oracle: the capacity's own arithmetic: `SIMS_MAX` slots, five
    /// built-ins, so 27 more register and the next is `Full`. The filler
    /// is never installed, so no other test's `select` can route to it.
    /// (The one lib unit test that registers: the registry is the
    /// process's.)
    #[test]
    fn p8_201_the_registry_holds_sims_max_and_refuses_the_next() {
        let filler = |i: usize| SimDef {
            name: format!("filler-{i}"),
            dialect: Lang::NGSPICE,
            runner: Runner::BuiltIn(none),
            features: 0,
            analyses: 1 << Kind::Op as u32,
            installed: Installed::Fn(absent),
        };
        for i in BACKENDS.len()..SIMS_MAX {
            let sim = register_simulator(filler(i)).expect("a free slot");
            assert_eq!(sim, Sim(i as u8));
            assert_eq!(register_simulator(filler(i)), Ok(sim), "equal: idempotent");
        }
        assert_eq!(
            register_simulator(filler(SIMS_MAX)),
            Err(SimRegisterError::Full)
        );
        assert_eq!(simulators().count(), SIMS_MAX);
        assert_eq!(by_name("filler-7"), Some(Sim(7)));
    }
}
