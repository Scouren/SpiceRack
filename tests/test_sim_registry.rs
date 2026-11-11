//! The simulator registry (ZERO D38) -> `spicerack::backend::{Sim, SimDef,
//! register_simulator, simulators, by_name, select}`: the five built-ins
//! hold the rows the tree held before the registry, and a caller's
//! simulator registers, reads back, runs, and is routed for what it
//! declares. (`test_backends.rs` is at the file-size cap: the registry's
//! tests live here.)
//!
//! Cites: ZERO D38 (`research/DESIGN-custom-simulators.md`).
//!
//! Contents: none (tests only).

// ---- private ----

/// Oracle: the built-in table hand-copied from the tree before the
/// registry (`BACKENDS`' order, names and caps; every `Kind::spec` route,
/// `research/impl/evidence/P8-201/kind-spec-routes.txt`), and a
/// definition handed in, read back. Nothing here is computed from
/// `Kind::spec`.
#[cfg(feature = "backend")]
#[test]
fn p8_201_the_registry_holds_the_built_ins_and_what_is_registered() {
    use Kind::*;
    use spicerack::backend::feature::*;
    use spicerack::backend::{
        BackendError, Installed, Kind, Runner, Sim, SimDef, SimRegisterError, by_name,
        register_simulator, simulators,
    };
    use spicerack::result::RawData;
    use spicerack::syntax::{Dialect, Lang};

    let universal = [
        Op,
        Dc,
        Ac,
        Tran,
        Noise,
        Tf,
        Fourier,
        SpectreSweep,
        SpectreMonteCarlo,
    ];
    let ngspice = XSPICE | OSDI | MEASURES | CONTROL_BLOCKS | LAPLACE | VERILOG_COSIM;
    let spectre: &[Kind] = &[Sens, SensAc, Pss, Hb, Sp, Stb, Pac, Pnoise, Pxf, Pstb];
    let want: [(&str, Dialect, u8, &[Kind]); 5] = [
        (
            "ngspice",
            Dialect::Ngspice,
            ngspice,
            &[Sens, SensAc, Pz, Disto, Pss, Sp, TranNoise],
        ),
        (
            "ltspice",
            Dialect::Ltspice,
            MEASURES | STEP_PARAMS | LAPLACE,
            &[],
        ),
        ("vacask", Dialect::Vacask, OSDI | STEP_PARAMS, &[Hb]),
        (
            "spectre",
            Dialect::Spectre,
            OSDI | MEASURES | STEP_PARAMS,
            spectre,
        ),
        (
            "espice",
            Dialect::Espice,
            OSDI | MEASURES | LAPLACE,
            &[Sens],
        ),
    ];
    let bits = |kinds: &[Kind]| kinds.iter().fold(0u32, |m, &k| m | (1 << k as u32));
    let got: Vec<(Sim, &SimDef)> = simulators().take(5).collect();
    assert_eq!(got.len(), 5);
    for (i, ((sim, row), (name, d, caps, own))) in got.iter().zip(want).enumerate() {
        assert_eq!(row.name, name);
        assert_eq!(row.dialect, Lang::from(d), "{name}");
        assert_eq!(row.features, caps, "{name}");
        assert_eq!(row.analyses, bits(&universal) | bits(own), "{name}");
        assert_eq!(Sim::of(d), Some(*sim), "{name}");
        assert_eq!(sim.builtin(), Some(d), "{name}");
        assert_eq!(sim.family(), d, "{name}");
        assert_eq!(by_name(name), Some(*sim));
        assert_eq!(d.capabilities(), caps, "{name}");
        assert_eq!(spicerack::backend::RUNNABLE[i], d);
    }
    let named = [
        Sim::NGSPICE,
        Sim::LTSPICE,
        Sim::VACASK,
        Sim::SPECTRE,
        Sim::ESPICE,
    ];
    assert_eq!(
        named.map(Sim::name),
        ["ngspice", "ltspice", "vacask", "spectre", "espice"]
    );
    assert_eq!(by_name("vacask-shared"), Some(Sim::VACASK), "an alias");
    assert_eq!(by_name("hspice"), None, "written, never run");
    assert_eq!(Sim::of(Dialect::Xyce), None);

    // A simulator of the caller's: registered, read back, run.
    fn fixed(netlist: &str) -> Result<Vec<RawData>, BackendError> {
        Ok(vec![RawData {
            title: netlist.to_string(),
            ..RawData::default()
        }])
    }
    fn present() -> bool {
        true
    }
    let def = SimDef {
        name: "p8-201-fixed".into(),
        dialect: Lang::NGSPICE,
        runner: Runner::BuiltIn(fixed),
        features: MEASURES,
        analyses: bits(&[Op, Dc]),
        installed: Installed::Fn(present),
    };
    let sim = register_simulator(def.clone()).expect("registers");
    assert_eq!(sim.builtin(), None);
    assert_eq!(sim.row(), &def);
    assert_eq!(by_name("p8-201-fixed"), Some(sim));
    assert_eq!(by_name("P8-201-FIXED"), None, "names are exact");
    assert!(sim.installed());
    assert_eq!(sim.family(), Dialect::Ngspice);
    let plots = sim.run_all("* deck\n").expect("runs");
    assert_eq!(plots[0].title, "* deck\n");
    assert_eq!(register_simulator(def.clone()), Ok(sim), "idempotent");

    let held = |name: &str| SimRegisterError::Redefined(name.into());
    let edge = |d: SimDef| register_simulator(d).expect_err("refused");
    let named = |name: &str| SimDef {
        name: name.into(),
        ..def.clone()
    };
    let other = SimDef {
        features: 0,
        ..def.clone()
    };
    assert_eq!(edge(other), held("p8-201-fixed"));
    assert_eq!(edge(named("P8-201-Fixed")), held("P8-201-Fixed"));
    assert_eq!(edge(named("")), SimRegisterError::Unnamed);
    assert_eq!(edge(named("ngspice-shared")), held("ngspice-shared"));
    let a = SimDef {
        features: 1 << 7,
        ..named("p8-201-a")
    };
    assert_eq!(edge(a), SimRegisterError::Features(1 << 7));
    let b = SimDef {
        analyses: 0,
        ..named("p8-201-b")
    };
    assert_eq!(edge(b), SimRegisterError::Analyses(0));
    let c = SimDef {
        analyses: 1 << 22,
        ..named("p8-201-c")
    };
    assert_eq!(edge(c), SimRegisterError::Analyses(1 << 22));
    for refused in ["p8-201-a", "p8-201-b", "p8-201-c", "P8-201-Fixed"] {
        assert_eq!(by_name(refused), None, "nothing refused is registered");
    }
}

/// Oracle: the route written out by hand. A registered simulator that
/// declares op and dc with `measures` comes after the built-ins in those
/// two routes and in no other, is picked by name for what it declares and
/// refused for the rest, and is the route's pick when the writer's verdict
/// (`writes`) leaves it alone (the stand-in for "no built-in installed").
#[cfg(feature = "backend")]
#[test]
fn p8_202_a_registered_simulator_is_routed_for_what_it_declares() {
    use spicerack::backend::feature::{MEASURES, STEP_PARAMS};
    use spicerack::backend::{
        BackendError, Installed, Kind, Runner, Sim, SimDef, register_simulator, select,
    };
    use spicerack::result::RawData;
    use spicerack::syntax::{Dialect, Lang};

    fn fixed(netlist: &str) -> Result<Vec<RawData>, BackendError> {
        Ok(vec![RawData {
            title: netlist.to_string(),
            ..RawData::default()
        }])
    }
    fn present() -> bool {
        true
    }
    let name = "p8-202-fixed";
    let sim = register_simulator(SimDef {
        name: name.into(),
        dialect: Lang::NGSPICE,
        runner: Runner::BuiltIn(fixed),
        features: MEASURES,
        analyses: (1 << Kind::Op as u32) | (1 << Kind::Dc as u32),
        installed: Installed::Fn(present),
    })
    .expect("registers");
    let only = |s: Sim| s == sim;
    let all = |_: Sim| true;

    // By name: what it declares, and nothing else.
    let both = [Kind::Dc, Kind::Op];
    assert_eq!(select(&both, Some(name), MEASURES, all).unwrap(), sim);
    assert!(matches!(
        select(&[Kind::Op, Kind::Tran], Some(name), 0, all),
        Err(BackendError::UnsupportedAnalysis { analysis, backend, required, .. })
            if analysis == "tran" && backend == name && required == "ngspice"
    ));
    assert!(matches!(
        select(&[Kind::Op], Some(name), MEASURES | STEP_PARAMS, all),
        Err(BackendError::UnsupportedFeature { feature, backend, .. })
            if feature == "step" && backend == name
    ));
    assert!(matches!(
        select(&[Kind::Pz, Kind::Hb], Some(name), 0, all),
        Err(BackendError::NoSimulator { analysis, .. }) if analysis == "hb"
    ));
    assert!(matches!(
        select(&[Kind::Op], Some("p8-202-nobody"), 0, all),
        Err(BackendError::SimulationError(m)) if m == "Unknown backend: p8-202-nobody"
    ));

    // Routed: after the built-ins (ngspice is installed in `.#testing` and
    // leads every universal route), first when only it writes the deck.
    assert_eq!(select(&[Kind::Op], None, 0, all).unwrap(), Sim::NGSPICE);
    assert_eq!(select(&[], None, 0, only).unwrap(), sim);
    assert_eq!(select(&both, None, MEASURES, only).unwrap(), sim);
    // No last resort onto a registered simulator: its declaration holds.
    assert!(matches!(
        select(&[Kind::Op], None, STEP_PARAMS, only),
        Err(BackendError::UnsupportedFeature { feature, .. }) if feature == "step"
    ));
    assert!(matches!(
        select(&[Kind::Tran], None, 0, only),
        Err(BackendError::UnsupportedAnalysis { analysis, .. }) if analysis == "tran"
    ));

    // A run through `simulation`: read by its family's rules (ngspice's).
    let raw = spicerack::simulation::run(sim, "* p8-202\n").expect("runs");
    assert_eq!(raw.title, "* p8-202\n");
    assert_eq!(raw.backend_hint, Some(Dialect::Ngspice));
}
