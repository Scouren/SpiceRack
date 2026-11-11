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
