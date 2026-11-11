//! A simulator registered as a command (`backend::Runner::Command`) →
//! `simulation::run_all`: the command is run on the deck with no shell
//! and its result files are read back.
//!
//! The stand-in simulators are coreutils: the deck handed to them is the
//! text of an ngspice ASCII raw file, so `cp {deck} {raw}` "simulates" it
//! and the values read back are the ones written in the deck by hand.
//!
//! Cites: the ngspice raw file format (manual §17.6), as a spec example.
//!
//! Contents: none (tests only).

// ---- private ----

/// An ASCII raw file: one real plot, `v(out)` = `out`, `v(in)` = 1.
#[cfg(feature = "backend")]
fn raw_text(title: &str, out: f64) -> String {
    format!(
        "Title: {title}\nPlotname: Operating Point\nFlags: real\nNo. Variables: 2\n\
         No. Points: 1\nVariables:\n\t0\tv(out)\tvoltage\n\t1\tv(in)\tvoltage\n\
         Values:\n0\t{out:e}\n\t1.000000e+00\n"
    )
}

/// Oracle: the values written into the deck by hand (3.3 and 1.0), read
/// back bit for bit through a real process; and the registry's own
/// refusals for a command that cannot run.
#[cfg(feature = "backend")]
#[test]
fn p8_203_a_command_simulator_runs_the_deck_and_reads_its_results() {
    use spicerack::backend::{
        BackendError, Installed, Kind, Output, Runner, SimDef, SimRegisterError, parse_arg,
        register_simulator, select,
    };
    use spicerack::syntax::{Dialect, Lang};

    let def = |name: &str, ws: &[&str], output: Output| SimDef {
        name: name.into(),
        dialect: Lang::NGSPICE,
        runner: Runner::Command {
            argv: ws.iter().map(|w| parse_arg(w).expect("a word")).collect(),
            output,
        },
        features: 0,
        analyses: 1 << Kind::Op as u32,
        installed: Installed::OnPath(ws.first().copied().unwrap_or("").to_string()),
    };

    // `{raw}`: one file, read whichever nutmeg form the name promises.
    let cp = def("p8-203-cp", &["cp", "{deck}", "{raw}"], Output::Nutascii);
    let cp = register_simulator(cp).expect("registers");
    assert!(cp.installed(), "coreutils `cp` is on PATH");
    let named = select(&[Kind::Op], Some("p8-203-cp"), 0, |_| true);
    assert_eq!(named.unwrap(), cp);
    let plots = spicerack::simulation::run_all(cp, &raw_text("copied", 3.3)).expect("runs");
    assert_eq!(plots.len(), 1);
    assert_eq!(plots[0].title, "copied");
    assert_eq!(plots[0].real_data, [[3.3], [1.0]]);
    assert_eq!(plots[0].backend_hint, Some(Dialect::Ngspice));

    // `raw-dir`: the command runs in `{dir}` and every `*.raw` there is
    // read, in name order; `{dir}` inside a longer word.
    let script = "cp \"$1\" b.raw && sed 's/3.3/4.4/' \"$1\" > \"$2\"";
    let dir = def(
        "p8-203-dir",
        &["sh", "-c", script, "sh", "{deck}", "{dir}/a.raw"],
        Output::RawDir,
    );
    let dir = register_simulator(dir).expect("registers");
    let plots = spicerack::simulation::run_all(dir, &raw_text("two", 3.3)).expect("runs");
    let outs: Vec<f64> = plots.iter().map(|p| p.real_data[0][0]).collect();
    assert_eq!(outs, [4.4, 3.3], "a.raw, then b.raw");

    // A program that is not installed: refused before anything is spawned.
    let ghost = def(
        "p8-203-ghost",
        &["p8-203-no-such-program", "{deck}"],
        Output::NgspiceRaw,
    );
    let ghost = register_simulator(ghost).expect("registers: installed is asked at run time");
    assert!(!ghost.installed());
    assert!(matches!(
        spicerack::simulation::run_all(ghost, "* deck\n"),
        Err(BackendError::NotInstalled { simulator }) if simulator == "p8-203-ghost"
    ));
    // A command that exits 0 and writes no result file is a failed run.
    let quiet = def("p8-203-true", &["true", "{deck}"], Output::NgspiceRaw);
    let quiet = register_simulator(quiet).expect("registers");
    assert!(matches!(
        spicerack::simulation::run_all(quiet, "* deck\n"),
        Err(BackendError::SimulationError(m)) if m.contains("no result file")
    ));

    // The edge: a command the registry refuses, each naming the field.
    let refused = |ws: &[&str]| {
        register_simulator(def("p8-203-bad", ws, Output::NgspiceRaw))
            .expect_err("refused")
            .to_string()
    };
    assert_eq!(refused(&[]), "command: it has no program");
    assert!(refused(&["cp", "{raw}"]).starts_with("command: no word holds {deck}"));
    assert!(matches!(
        parse_arg("{out}"),
        Err(SimRegisterError::Command(m)) if m.contains("{out}")
    ));
    assert_eq!(spicerack::backend::by_name("p8-203-bad"), None);
}

/// Oracle: note 12 §4's table (code, kind of refusal, title, cause),
/// copied by hand: each cause of a refused route or registration is one
/// error with its one code.
#[cfg(feature = "backend")]
#[test]
fn p8_206_each_refusal_cause_has_its_code() {
    use spicerack::backend::feature::MEASURES;
    use spicerack::backend::{
        BackendError, Installed, Kind, Output, Runner, Sim, SimDef, SimRegisterError, parse_arg,
        register_simulator, select,
    };
    use spicerack::syntax::Lang;
    use spicerack::syntax::diag::{Code, Kind as Stays, info};

    let table = [
        (
            Code::W0180,
            Stays::Refusal,
            "an analysis the chosen simulator does not declare",
        ),
        (
            Code::W0181,
            Stays::Refusal,
            "a feature the chosen simulator does not declare",
        ),
        (
            Code::W0182,
            Stays::Refusal,
            "no simulator runs every analysis of the deck",
        ),
        (
            Code::R0040,
            Stays::Note,
            "a simulator descriptor that does not validate",
        ),
        (
            Code::R0041,
            Stays::Note,
            "the simulator's command is not installed",
        ),
        (Code::R0042, Stays::Note, "the simulator registry is full"),
    ];
    for (code, kind, title) in table {
        assert_eq!(info(code).title, title);
        assert!(info(code).kind == kind, "{code:?}");
    }

    let def = |name: &str, program: &str| SimDef {
        name: name.into(),
        dialect: Lang::NGSPICE,
        runner: Runner::Command {
            argv: vec![
                parse_arg(program).expect("a word"),
                parse_arg("{deck}").expect("a word"),
            ],
            output: Output::NgspiceRaw,
        },
        features: 0,
        analyses: 1 << Kind::Op as u32,
        installed: Installed::OnPath(program.into()),
    };
    let ghost = register_simulator(def("p8-206-ghost", "p8-206-no-such-program")).expect("ok");
    let refused = |r: Result<Sim, BackendError>| r.expect_err("refused");
    let all = |_: Sim| true;

    // W0180: the named simulator lacks an analysis of the deck.
    let e = refused(select(
        &[Kind::Op, Kind::Tran],
        Some("p8-206-ghost"),
        0,
        all,
    ));
    assert_eq!(e.code(), Some(Code::W0180));
    assert_eq!(e.missing(), Some((vec!["tran"], vec![])));
    // W0181: it lacks a feature the deck needs.
    let e = refused(select(&[Kind::Op], Some("p8-206-ghost"), MEASURES, all));
    assert_eq!(e.code(), Some(Code::W0181));
    assert_eq!(e.missing(), Some((vec![], vec!["measures"])));
    // W0182: no simulator runs both (pz: ngspice; hb: VACASK, Spectre),
    // named or not.
    for name in [None, Some("p8-206-ghost"), Some("ngspice")] {
        let e = refused(select(&[Kind::Pz, Kind::Hb], name, 0, all));
        assert_eq!(e.code(), Some(Code::W0182), "{name:?}");
        assert_eq!(e.missing(), Some((vec!["hb"], vec![])));
    }
    // R0041: named, declared, and not installed: refused at the run.
    assert_eq!(
        select(&[Kind::Op], Some("p8-206-ghost"), 0, all).ok(),
        Some(ghost)
    );
    let e = spicerack::simulation::run_all(ghost, "* deck\n").expect_err("not installed");
    assert_eq!(e.code(), Some(Code::R0041));
    assert_eq!(e.missing(), None);
    // No code: a name that is no simulator's.
    let e = refused(select(&[Kind::Op], Some("p8-206-nobody"), 0, all));
    assert_eq!(e.code(), None);

    // R0040 for every field's refusal, R0042 for the capacity.
    let bad = |d: SimDef| register_simulator(d).expect_err("refused").code();
    let ok = || def("p8-206-bad", "cp");
    assert_eq!(bad(def("", "cp")), Code::R0040);
    assert_eq!(bad(def("p8-206-ghost", "cp")), Code::R0040, "redefined");
    let features = SimDef {
        features: 1 << 7,
        ..ok()
    };
    assert_eq!(bad(features), Code::R0040);
    let analyses = SimDef {
        analyses: 0,
        ..ok()
    };
    assert_eq!(bad(analyses), Code::R0040);
    let command = SimDef {
        runner: Runner::Command {
            argv: Vec::new(),
            output: Output::NgspiceRaw,
        },
        ..ok()
    };
    assert_eq!(bad(command), Code::R0040);
    assert_eq!(SimRegisterError::Full.code(), Code::R0042);
}

/// Oracle: the values the deck holds by hand (2.5 and 1.0), read back
/// through a real process by a reader of the test's own, and the row that
/// reader made up. The deck's `.meas` card is the trap: a run that still
/// read the measures from stdout would give ngspice's failed `never` row
/// in the reader's place.
#[cfg(feature = "backend")]
#[test]
fn p8_209_a_hook_simulator_runs_its_command_and_keeps_what_the_hook_read() {
    use std::path::Path;

    use spicerack::backend::{
        BackendError, Installed, Kind, Runner, SimDef, SimRegisterError, parse_arg,
        register_simulator,
    };
    use spicerack::result::{MeasureResult, RawData};
    use spicerack::syntax::{Dialect, Lang};

    fn read(id: u32, dir: &Path) -> Result<Vec<RawData>, BackendError> {
        if id == 9 {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(dir.join("copy.raw"))?;
        let mut plots = spicerack::rawfile::parse_raw_all(&bytes)?;
        for p in &mut plots {
            p.measures = vec![MeasureResult::ok("id", f64::from(id))];
        }
        Ok(plots)
    }
    // The raw file is the deck without its `.meas` card.
    let script = "grep -v '^\\.meas' \"$1\" > \"$2\" && echo ran";
    let def = |name: &str, id: u32, ws: &[&str]| SimDef {
        name: name.into(),
        dialect: Lang::NGSPICE,
        runner: Runner::Hook {
            argv: ws.iter().map(|w| parse_arg(w).expect("a word")).collect(),
            read,
            id,
        },
        features: 0,
        analyses: 1 << Kind::Op as u32,
        installed: Installed::OnPath(ws.first().copied().unwrap_or("").to_string()),
    };
    let ws = ["sh", "-c", script, "sh", "{deck}", "{dir}/copy.raw"];
    let sim = register_simulator(def("p8-209-hook", 7, &ws)).expect("registers");
    assert_eq!(
        register_simulator(def("p8-209-hook", 7, &ws)),
        Ok(sim),
        "idempotent"
    );
    assert_eq!(
        register_simulator(def("p8-209-hook", 8, &ws)),
        Err(SimRegisterError::Redefined("p8-209-hook".into())),
        "another hook id is another definition"
    );

    let deck = raw_text("hooked", 2.5) + ".meas op never find v(out) at=1\n";
    let plots = spicerack::simulation::run_all(sim, &deck).expect("runs");
    assert_eq!(plots.len(), 1);
    assert_eq!(plots[0].title, "hooked");
    assert_eq!(plots[0].real_data, [[2.5], [1.0]]);
    assert_eq!(plots[0].stdout, "ran\n");
    assert_eq!(plots[0].backend_hint, Some(Dialect::Ngspice));
    let rows: Vec<(&str, f64)> = plots[0]
        .measures
        .iter()
        .map(|m| (m.name.as_str(), m.value))
        .collect();
    assert_eq!(rows, [("id", 7.0)], "the hook's row, not the deck's card");

    // A reader with no plot is a failed run; its command is checked as a
    // `Command`'s and asked for before anything is spawned.
    let none = register_simulator(def("p8-209-none", 9, &ws)).expect("registers");
    assert!(matches!(
        spicerack::simulation::run_all(none, &deck),
        Err(BackendError::SimulationError(m)) if m == "read hook: it returned no plot"
    ));
    let undecked = register_simulator(def("p8-209-bad", 7, &["sh", "{raw}"]));
    assert!(matches!(undecked, Err(SimRegisterError::Command(_))));
    let ghost = def("p8-209-ghost", 7, &["p8-209-no-such-program", "{deck}"]);
    let ghost = register_simulator(ghost).expect("registers: installed is asked at run time");
    assert!(matches!(
        spicerack::simulation::run_all(ghost, &deck),
        Err(BackendError::NotInstalled { simulator }) if simulator == "p8-209-ghost"
    ));
}
