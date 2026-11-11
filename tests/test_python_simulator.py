"""
Comprehensive simulator configuration and analysis tests.

Tests simulator creation, option setting, netlist building,
temperature, initial conditions, saves, measures, step sweeps.

These tests do NOT require a simulator backend -- they only test
the Python binding layer and netlist generation.

Oracle: the expected netlist lines and Python errors, written by hand.

Run: maturin develop && pytest tests/test_python_simulator.py -v
"""
import math

import pytest


def ps():
    import spicerack  # unsupported fails, never skips: an unbuilt extension is an error
    return spicerack


# ======================================================================
# Simulator creation
# ======================================================================

class TestSimulatorCreation:
    def test_default_simulator(self):
        c = ps().Circuit("t")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        sim = c.simulator()
        assert repr(sim) == "CircuitSimulator"

    def test_simulator_with_backend(self):
        c = ps().Circuit("t")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        sim = c.simulator(simulator="ngspice-subprocess")
        assert repr(sim) == "CircuitSimulator"

    def test_simulator_with_various_backends(self):
        """Test that we can create simulators with various backend names."""
        c = ps().Circuit("t")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        for backend in ["ngspice-subprocess", "ngspice-shared", "ltspice", "vacask", "spectre"]:
            sim = c.simulator(simulator=backend)
            assert repr(sim) == "CircuitSimulator"


# ======================================================================
# Simulator configuration
# ======================================================================

class TestSimulatorConfig:
    def _make_sim(self):
        c = ps().Circuit("cfg")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        c.R(
            name="1",
            positive="vdd",
            negative=c.gnd,
            value=1e3,
        )
        return c.simulator()

    def test_set_temperature(self):
        sim = self._make_sim()
        sim.temperature = 85.0
        # Should not raise

    def test_set_nominal_temperature(self):
        sim = self._make_sim()
        sim.nominal_temperature = 27.0

    def test_save_currents(self):
        sim = self._make_sim()
        sim.save_currents = True

    def test_save_nodes(self):
        sim = self._make_sim()
        sim.save("V(out)", "I(Vdd)")

    def test_options(self):
        sim = self._make_sim()
        sim.options(reltol="0.001", abstol="1e-12")

    def test_initial_condition(self):
        sim = self._make_sim()
        sim.initial_condition(out=0.0, vdd=3.3)

    def test_node_set(self):
        sim = self._make_sim()
        sim.node_set(out=1.65, bias=0.7)

    def test_measure(self):
        sim = self._make_sim()
        sim.measure("tran", "rise_time", "trig", "V(out)", "val=0.1", "rise=1",
                     "targ", "V(out)", "val=0.9", "rise=1")

    def test_step_linear(self):
        sim = self._make_sim()
        sim.step("R1", 500.0, 2000.0, 100.0)

    def test_step_sweep(self):
        sim = self._make_sim()
        sim.step_sweep("C1", 1e-12, 100e-12, 10e-12, "lin")


# ======================================================================
# Analysis method signatures (these will fail without a backend,
# but we test that the methods exist and accept correct args)
# ======================================================================

class TestAnalysisMethods:
    """Verify all analysis methods exist on the simulator and accept args.

    These will raise RuntimeError (no backend) -- we just check
    the method exists and the binding doesn't crash on arg parsing.
    """

    def _sim(self):
        c = ps().Circuit("t")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        c.R(
            name="1",
            positive="vdd",
            negative="out",
            value=1e3,
        )
        c.R(
            name="2",
            positive="out",
            negative=c.gnd,
            value=1e3,
        )
        return c.simulator()

    def test_operating_point_exists(self):
        sim = self._sim()
        assert hasattr(sim, "operating_point")
        # Will succeed if ngspice is available, RuntimeError otherwise
        try:
            sim.operating_point()
        except RuntimeError:
            pass

    def test_dc_exists(self):
        sim = self._sim()
        assert hasattr(sim, "dc")

    def test_ac_exists(self):
        sim = self._sim()
        assert hasattr(sim, "ac")
        try:
            sim.ac(variation="dec", number_of_points=10,
                   start_frequency=1.0, stop_frequency=1e9)
        except RuntimeError:
            pass

    def test_transient_exists(self):
        sim = self._sim()
        assert hasattr(sim, "transient")
        try:
            sim.transient(step_time=1e-6, end_time=1e-3)
        except RuntimeError:
            pass

    def test_noise_exists(self):
        sim = self._sim()
        assert hasattr(sim, "noise")

    def test_transfer_function_exists(self):
        sim = self._sim()
        assert hasattr(sim, "transfer_function")
        assert hasattr(sim, "tf")

    def test_dc_sensitivity_exists(self):
        sim = self._sim()
        assert hasattr(sim, "dc_sensitivity")

    def test_ac_sensitivity_exists(self):
        sim = self._sim()
        assert hasattr(sim, "ac_sensitivity")

    def test_polezero_exists(self):
        sim = self._sim()
        assert hasattr(sim, "polezero")

    def test_distortion_exists(self):
        sim = self._sim()
        assert hasattr(sim, "distortion")

    def test_pss_exists(self):
        sim = self._sim()
        assert hasattr(sim, "pss")

    def test_s_param_exists(self):
        sim = self._sim()
        assert hasattr(sim, "s_param")

    def test_harmonic_balance_exists(self):
        sim = self._sim()
        assert hasattr(sim, "harmonic_balance")

    def test_stability_exists(self):
        sim = self._sim()
        assert hasattr(sim, "stability")

    def test_transient_noise_exists(self):
        sim = self._sim()
        assert hasattr(sim, "transient_noise")

    def test_network_params_exists(self):
        sim = self._sim()
        assert hasattr(sim, "network_params")

    # Spectre-specific
    def test_spectre_sweep_exists(self):
        sim = self._sim()
        assert hasattr(sim, "spectre_sweep")

    def test_spectre_monte_carlo_is_the_name_and_montecarlo_a_deprecated_alias(self):
        # `Testbench.add_spectre_monte_carlo` spells it with an underscore; the
        # run method now does too. A bad inner type stops both before Spectre.
        import warnings
        sim = self._sim()
        with warnings.catch_warnings():
            warnings.simplefilter("error", DeprecationWarning)
            with pytest.raises(ValueError, match="unknown Spectre analysis 'op'"):
                sim.spectre_monte_carlo(4, "x1", "op")
        with pytest.warns(DeprecationWarning, match="spectre_monte_carlo"):
            with pytest.raises(ValueError, match="unknown Spectre analysis 'op'"):
                sim.spectre_montecarlo(4, "x1", "op")

    def test_spectre_pac_exists(self):
        sim = self._sim()
        assert hasattr(sim, "spectre_pac")

    def test_spectre_pnoise_exists(self):
        sim = self._sim()
        assert hasattr(sim, "spectre_pnoise")

    def test_spectre_pxf_exists(self):
        sim = self._sim()
        assert hasattr(sim, "spectre_pxf")

    def test_spectre_pstb_exists(self):
        sim = self._sim()
        assert hasattr(sim, "spectre_pstb")

    def test_available_backends_static(self):
        sim = self._sim()
        assert hasattr(sim, "available_backends")
        backends = type(sim).available_backends()
        assert isinstance(backends, list)


# ======================================================================
# Live simulation (ngspice; fails without it)
# ======================================================================

@pytest.mark.needs("ngspice")
class TestLiveSimulation:
    """Tests that actually run simulations. Require ngspice."""

    def _divider_sim(self):
        c = ps().Circuit("live_test")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        c.R(
            name="1",
            positive="vdd",
            negative="out",
            value=10e3,
        )
        c.R(
            name="2",
            positive="out",
            negative=c.gnd,
            value=10e3,
        )
        return c.simulator(simulator="ngspice-subprocess")

    def test_operating_point(self):
        sim = self._divider_sim()
        op = sim.operating_point()
        vout = op["out"]
        assert abs(vout - 1.65) < 0.01

    def test_dc_sweep(self):
        c = ps().Circuit("dc_live")
        c.V(
            name="in",
            positive="vin",
            negative=c.gnd,
            value=0.0,
        )
        c.R(
            name="1",
            positive="vin",
            negative="out",
            value=1e3,
        )
        c.R(
            name="2",
            positive="out",
            negative=c.gnd,
            value=1e3,
        )
        sim = c.simulator(simulator="ngspice-subprocess")
        dc = sim.dc(Vin=slice(0, 10, 1))
        sweep = dc.sweep
        assert len(sweep) > 5
        # At Vin=10V, Vout should be 5V
        vout = dc["out"]
        assert abs(vout[-1] - 5.0) < 0.1

    def test_transient(self):
        c = ps().Circuit("tran_live")
        c.PulseVoltageSource(
            name="in",
            positive="in",
            negative=c.gnd,
            initial_value=0.0,
            pulsed_value=1.0,
            pulse_width=0.5e-3,
            period=1e-3,
        )
        c.R(
            name="1",
            positive="in",
            negative="out",
            value=1e3,
        )
        c.C(
            name="1",
            positive="out",
            negative=c.gnd,
            value=1e-6,
        )
        sim = c.simulator(simulator="ngspice-subprocess")
        tran = sim.transient(step_time=1e-5, end_time=2e-3)
        time = tran.time
        assert len(time) > 10
        assert time[-1] >= 1.9e-3
        # A result column is a plain list of floats, by item and by attribute.
        for col in (time, tran["out"], tran.out, tran["OUT"]):
            assert type(col) is list and all(type(x) is float for x in col)
        assert tran["out"] == tran.out == tran["OUT"] and len(tran["out"]) == len(time)

    def test_ac_analysis(self):
        c = ps().Circuit("ac_live")
        c.V(
            name="in",
            positive="in",
            negative=c.gnd,
            value=1.0,
        )
        c.R(
            name="1",
            positive="in",
            negative="out",
            value=1e3,
        )
        c.C(
            name="1",
            positive="out",
            negative=c.gnd,
            value=1e-6,
        )
        sim = c.simulator(simulator="ngspice-subprocess")
        ac = sim.ac(variation="dec", number_of_points=10,
                    start_frequency=1.0, stop_frequency=1e6)
        freq = ac.frequency
        assert len(freq) > 10
        assert freq[0] >= 1.0


def test_ac_magnitude_and_phase_match_rc_theory():
    """AC must expose |H| and phase, not just the real part (they differ)."""
    import math
    import spicerack as ps
    from spicerack.unit import u_kOhm, u_uF

    circuit = ps.Circuit("rc_bode")
    circuit.V(name="in", positive="vin", negative=circuit.gnd, value=0.0, ac=1.0)
    circuit.R(name="r1", positive="vin", negative="vout", value=1 @ u_kOhm)
    circuit.C(name="c1", positive="vout", negative=circuit.gnd, value=1 @ u_uF)

    sim = circuit.simulator(simulator="ngspice")
    ac = sim.ac(variation="dec", number_of_points=10, start_frequency=1, stop_frequency=1e5)

    fc = 1.0 / (2 * math.pi * 1e3 * 1e-6)
    for f, mag, phase, real in zip(ac.frequency, ac.magnitude("vout"),
                                   ac.phase("vout"), ac["vout"]):
        expected_mag = 1.0 / math.sqrt(1 + (f / fc) ** 2)
        expected_phase = -math.degrees(math.atan(f / fc))
        assert abs(mag - expected_mag) < 1e-6 * max(1.0, expected_mag)
        assert abs(phase - expected_phase) < 1e-3
        # Real part is not the magnitude anywhere past DC.
        if f > fc:
            assert real < mag
    # The real part of a complex column is still a plain list of floats.
    assert type(ac["vout"]) is list and all(type(x) is float for x in ac["vout"])
    assert type(ac.frequency) is list and ac.frequency[0] == 1.0


def test_raw_meas_line_reports_its_measure_on_ngspice():
    """A `.meas` written as raw lines (a `+` continuation included) gets the
    `.control` block ngspice needs to report it; under `-r` it would be lost."""
    import math
    import spicerack as ps

    c = ps.Circuit("rc_meas")
    c.PulseVoltageSource(name="in", positive="in", negative=c.gnd, initial_value=0.0,
                         pulsed_value=1.0, rise_time=1e-9, fall_time=1e-9, pulse_width=1.0, period=2.0)
    c.R(name="1", positive="in", negative="out", value=1e3)
    c.C(name="1", positive="out", negative=c.gnd, value=1e-6)
    c.raw_spice(".MEASURE tran vend find v(out)")
    c.raw_spice("+ at=1m")
    tran = c.simulator(simulator="ngspice").transient(step_time=1e-6, end_time=1e-3, max_time=1e-6)
    # RC step response at t = RC: 1 - 1/e (closed form).
    assert tran.measures["vend"] == pytest.approx(1 - math.exp(-1), rel=2e-3)


def test_raw_four_line_runs_as_fourier_on_ngspice():
    """A raw `.four` testbench line becomes `fourier` in the `.control` block
    (the table goes to stdout, which Python does not expose; the Rust suite
    checks its numbers), and the transient still comes back."""
    import spicerack as ps

    dut = ps.Subcircuit("rc4", ["in", "out"])
    dut.R(name="1", positive="in", negative="out", value=1e3)
    dut.C(name="1", positive="out", negative="0", value=1e-9)
    tb = ps.Testbench(dut)
    tb.SinusoidalVoltageSource(name="in", positive="in", negative="0", amplitude=1.0, frequency=1e3)
    tb.extra_line(".four 1k v(out)")
    tb.add_transient(step_time=1e-6, end_time=2e-3)
    deck = tb.netlist("ngspice")
    assert ".control\ntran 1u 2m\nfourier 1k v(out)\n.endc" in deck and ".four" not in deck
    tran = tb.transient(step_time=1e-6, end_time=2e-3)
    assert tran.time[-1] == pytest.approx(2e-3)


def test_noise_runs_when_saves_are_set():
    """A node-name .save cannot name onoise_spectrum; emitting it fails the run."""
    import math

    import spicerack as ps

    dut = ps.Subcircuit("nz", ["vin", "vout"])
    dut.R(name="r1", positive="vin", negative="vout", value=1000.0)
    dut.C(name="c1", positive="vout", negative="0", value=1e-6)

    tb = ps.Testbench(dut)
    tb.V(name="in", positive="vin", negative="0", value=1.0, ac=1.0)
    tb.save("V(vout)")
    noise = tb.noise(output_node="vout", ref_node="0", src="Vin",
                     variation="dec", points=10, start_frequency=1.0,
                     stop_frequency=1e5)

    spectrum = noise["onoise_spectrum"]
    assert len(spectrum) == len(noise.frequency)
    # Flat band is the 1k resistor's thermal noise, sqrt(4kTR).
    expected = math.sqrt(4 * 1.380649e-23 * 300.15 * 1000.0)
    assert spectrum[0] == pytest.approx(expected, rel=0.02)


@pytest.mark.needs("ngspice")
def test_p3_034_api_distortion_always_only_hd2():
    """`distof1=(mag, phase)` on `V`/`I` is ngspice's `DISTOF1`: the source's
    `.disto` input, without which every distortion vector is 0. Oracle
    (closed form): a diode under 0.6 V driven by a*cos(wt) carries
    I0*exp(a*cos(wt)/Vt), I0 = Is*exp(0.6/Vt); its 2F1 component is
    I0*a^2/(4*Vt^2) = 0.0443646 A for a = 1, the source's the negative.
    ngspice-45 and ESPice 0.3.0 print -4.43650e-02 on the hand deck (their
    kT/q is 3.4e-7 off SI 2019's, 8e-6 on I0: hence rel=1e-4;
    research/impl/evidence/P3-034). Half the input at 90 degrees is a
    quarter of it, turned by 2*90 degrees."""
    import spicerack as ps

    vt = 1.380649e-23 * 300.15 / 1.602176634e-19
    second = 1e-14 * math.exp(0.6 / vt) / (4 * vt**2)

    def at_2f1(**disto):
        dut = ps.Subcircuit("dd", ["a"])
        dut.model("dn", "D", IS=1e-14)
        dut.D("1", "a", dut.gnd, model="dn")
        tb = ps.Testbench(dut)
        tb.with_backend("ngspice")
        tb.V("1", "a", "0", 0.6, **disto)
        d = tb.distortion(variation="dec", points=1, start_frequency=1e3, stop_frequency=1e3)
        return tb, d["i(v1)"][0]

    assert at_2f1()[1] == 0.0
    assert at_2f1(distof1=(1.0, 0.0))[1] == pytest.approx(-second, rel=1e-4)
    tb, quarter = at_2f1(distof1=(0.5, 90.0), distof2=(0.25, 0.0))
    assert quarter == pytest.approx(second / 4, rel=1e-4)
    assert "V1 a 0 600m distof1 0.5 90 distof2 0.25 0" in tb.netlist("ngspice")
    sub = ps.Subcircuit("s", ["a"])
    sub.V("1", "a", sub.gnd, 0.0, ac=1.0, distof1=(1.0, 0.0))
    sub.I("2", "a", sub.gnd, 0.0, distof2=(2.0, -90.0))
    assert "V1 a 0 0 AC 1 distof1 1 0\nI2 a 0 0 distof2 2 -90\n" in sub.to_source()


@pytest.mark.needs("ngspice")
def test_p3_033_api_add_noise_written_noise():
    """`output_node` is a node: "out" and "v(out)" name the same one, and no
    other form reaches the deck. Oracle (closed form): at 1 Hz the 1 uF of a
    1k/1k divider is open (corner 318 Hz), so its output noise is the thermal
    noise of 1k || 1k, sqrt(4kT * 500 Ohm) = 2.87889e-9 V/sqrt(Hz) at
    ngspice's 27 C. ngspice-45 prints 2.878880e-09 for `.noise V(out)` and 0
    for `.noise V(v(out))` (research/impl/evidence/P3-033)."""
    import spicerack as ps

    dut = ps.Subcircuit("nz", ["in", "out"])
    dut.R(name="1", positive="in", negative="out", value=1e3)
    dut.R(name="2", positive="out", negative="0", value=1e3)
    dut.C(name="1", positive="out", negative="0", value=1e-6)
    tb = ps.Testbench(dut)
    tb.with_backend("ngspice")
    tb.V(name="in", positive="in", negative="0", value=0.0, ac=1.0)
    # 1e-3: the capacitor moves the 1 Hz point by 5e-6, ngspice's k by 1e-8.
    expected = math.sqrt(4 * 1.380649e-23 * 300.15 * 500.0)
    for form in ("out", "v(out)", "V(out)", " v( out ) "):
        noise = tb.noise(output_node=form, ref_node="0", src="Vin", variation="dec", points=10,
                         start_frequency=1.0, stop_frequency=1e5)
        assert noise["onoise_spectrum"][0] == pytest.approx(expected, rel=1e-3), form
    for bad in ("v(out", "i(Vin)", "v(in,out)", "out 0", ""):
        with pytest.raises(ValueError, match="is not a node"):
            tb.add_noise(bad, "0", "Vin")
    with pytest.raises(ValueError, match="is not a node"):
        tb.add_noise("out", "v(0", "Vin")
    # `.tf outvar insrc` (ngspice manual 11.3.9: `v(5, 3)`, `i(VLOAD)`): the
    # signal is kept, a node is written v(node) (ngspice-45 refuses the bare
    # `.tf out Vin`: "voltage or current expected").
    for form, card in (("out", ".tf v(out) Vin"), ("v(out)", ".tf v(out) Vin"), ("i(Vin)", ".tf i(Vin) Vin")):
        bench = ps.Testbench(dut)
        bench.V(name="in", positive="in", negative="0", value=0.0, ac=1.0)
        bench.add_tf(form, "Vin")
        assert card in bench.netlist("ngspice").splitlines(), form
    for bad in ("v(out", "v(v(out))", "out 0", "db(out)", ""):
        with pytest.raises(ValueError, match="is not a node"):
            tb.add_tf(bad, "Vin")


# ======================================================================
# Testbench.run: every queued analysis in one deck
# ======================================================================

def _queued_bench(backend):
    """1k/1k divider with 1 uF on its output, driven by a 0 -> 2 V step with
    AC 1; a second 1k/1k divider off a 3 V rail. Closed forms: op b = 1.5 V
    and out = 0 (the step is 0 at t = 0); tran out = 1 - exp(-t/0.5 ms);
    ac out = 1 / |2 + j*2*pi*f*RC|. Queued tran, op, ac: ngspice runs its
    own order (ac, dc, op, tran), so a positional match would misfile them."""
    import spicerack as ps

    dut = ps.Subcircuit("q", ["in", "out", "b"])
    dut.R(name="1", positive="in", negative="out", value=1e3)
    dut.R(name="2", positive="out", negative="0", value=1e3)
    dut.C(name="1", positive="out", negative="0", value=1e-6)
    dut.R(name="3", positive="rail", negative="b", value=1e3)
    dut.R(name="4", positive="b", negative="0", value=1e3)
    tb = ps.Testbench(dut)
    tb.with_backend(backend)
    tb.PulseVoltageSource(name="in", positive="in", negative="0", initial_value=0.0, pulsed_value=2.0,
                          delay=0.0, rise_time=1e-9, fall_time=1e-9, pulse_width=1.0, period=2.0, ac=1.0)
    tb.V(name="rail", positive="rail", negative="0", value=3.0)
    tb.add_transient(step_time=1e-6, end_time=1e-3, max_time=1e-6)
    tb.add_operating_point()
    tb.add_ac(variation="dec", number_of_points=5, start_frequency=10.0, stop_frequency=1e4)
    return tb


def _check_queued(tran, op, ac):
    import math

    assert op["b"] == pytest.approx(1.5, abs=1e-9)
    assert op["out"] == pytest.approx(0.0, abs=1e-9)
    for t, v in zip(tran.time, tran["out"]):
        assert v == pytest.approx(1.0 - math.exp(-t / 0.5e-3), abs=2e-3)
    assert tran.time[-1] == pytest.approx(1e-3)
    for f, mag in zip(ac.frequency, ac.magnitude("out")):
        assert mag == pytest.approx(1.0 / abs(complex(2.0, 2 * math.pi * f * 1e-3)), rel=1e-6)


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_run_returns_one_result_per_queued_analysis_in_queue_order(backend):
    import spicerack as ps

    tran, op, ac = _queued_bench(backend).run()
    assert (type(tran), type(op), type(ac)) == (ps.TransientAnalysis, ps.OperatingPoint, ps.AcAnalysis)
    _check_queued(tran, op, ac)


def test_run_with_a_measure_runs_every_analysis_from_the_control_block():
    """A `.meas` moves ngspice's analyses into a `.control` block, which runs
    them in card order; the results still land on their own analyses."""
    tb = _queued_bench("ngspice")
    tb.measure("tran", "vend", "find", "v(out)", "at=1m")
    tran, op, ac = tb.run()
    _check_queued(tran, op, ac)
    assert tran.measures["vend"] == pytest.approx(1.0 - math.exp(-2.0), abs=2e-3)


def test_run_gives_none_for_a_fourier_and_refuses_an_empty_queue():
    """`.four` prints its table to stdout and writes no plot."""
    import spicerack as ps

    tb = _queued_bench("ngspice")
    tb.add_fourier(fundamental_frequency=1e3, outputs=["v(out)"])
    *_, four = tb.run()
    assert four is None
    empty = ps.Testbench(ps.Subcircuit("e", ["a"]))
    with pytest.raises(ValueError, match="add_"):
        empty.run()


@pytest.mark.parametrize("backend, measured", [("ngspice", False), ("ngspice", True), ("espice", False)])
def test_p3_031_api_two_analyses_same_kind(backend, measured):
    """ZERO-LEDGER P3-031: two analyses of one kind come back in queue order.
    Oracle: each result's own axis is the sweep its card asked for
    (`.ac lin 3 100 300`: 100, 200, 300 Hz; `.ac dec 1 1 10`: 1, 10 Hz; the
    transients stop at 10 us and 20 us). ngspice-45 runs dot cards kind by
    kind, the last card of a kind first (`cktnewan.c:34-35`; this deck's raw
    file holds the decade plot before the linear one), and the commands of
    the `.control` block a `.meas` gives the deck in order; ESPice runs its
    dot cards in deck order."""
    import spicerack as ps

    dut = ps.Subcircuit("q", ["in", "out"])
    dut.R(name="1", positive="in", negative="out", value=1e3)
    dut.R(name="2", positive="out", negative="0", value=1e3)
    dut.C(name="1", positive="out", negative="0", value=1e-6)
    tb = ps.Testbench(dut)
    tb.with_backend(backend)
    tb.V(name="in", positive="in", negative="0", value=1.0, ac=1.0)
    tb.add_ac(variation="lin", number_of_points=3, start_frequency=100.0, stop_frequency=300.0)
    tb.add_transient(step_time=1e-6, end_time=10e-6)
    tb.add_ac(variation="dec", number_of_points=1, start_frequency=1.0, stop_frequency=10.0)
    tb.add_transient(step_time=1e-6, end_time=20e-6)
    if measured:
        tb.measure("ac", "vmax", "max", "vm(out)")
    lin, short, dec, long = tb.run()
    assert list(lin.frequency) == pytest.approx([100.0, 200.0, 300.0])
    assert list(dec.frequency) == pytest.approx([1.0, 10.0])
    assert short.time[-1] == pytest.approx(10e-6)
    assert long.time[-1] == pytest.approx(20e-6)


# ======================================================================
# ngspice compatibility mode, and no init file reaching a run
# ======================================================================

# A 1 V source across 1 kOhm: V(a) = 1 exactly.
ONE_VOLT = """
import spicerack as ps
c = ps.Circuit("init")
c.V(name="1", positive="a", negative=c.gnd, value=1.0)
c.R(name="1", positive="a", negative=c.gnd, value=1e3)
print(c.simulator(simulator="ngspice").operating_point()["a"])
"""


class TestNgbehavior:
    def _tb(self):
        dut = ps().Subcircuit("nb", ["a"])
        dut.R(name="1", positive="a", negative=dut.gnd, value=1e3)
        tb = ps().Testbench(dut)
        tb.V(name="1", positive="a", negative=dut.gnd, value=1.0)
        return tb

    def test_the_mode_lands_on_the_ngspice_deck_only(self):
        tb = self._tb()
        assert tb.ngbehavior is None
        tb.ngbehavior = "hsa"
        assert tb.ngbehavior == "hsa"
        assert "*spicerack ngbehavior=hsa" in tb.netlist("ngspice").splitlines()
        for other in ("espice", "hspice", "ltspice", "vacask", "spectre"):
            assert "ngbehavior" not in tb.netlist(other), other
        tb.ngbehavior = None
        assert "ngbehavior" not in tb.netlist("ngspice")

    @pytest.mark.parametrize("bad", ["", "HSA", "hs a", "hsa\nshell ls"])
    def test_anything_but_lowercase_letters_is_refused(self, bad):
        tb = self._tb()
        with pytest.raises(ValueError, match="lowercase letters"):
            tb.ngbehavior = bad

    @pytest.mark.needs("ngspice")
    def test_a_mode_ngspice_rejects_fails_the_run(self):
        tb = self._tb()
        tb.with_backend("ngspice")
        tb.ngbehavior = "hsa"
        assert tb.operating_point()["a"] == 1.0
        tb.ngbehavior = "xyz"
        with pytest.raises(RuntimeError, match="ngbehavior=xyz"):
            tb.operating_point()

    # Oracle: a `.spiceinit` whose `shell touch` leaves a file behind when
    # ngspice reads it. ngspice reads ./.spiceinit, else ~/.spiceinit.
    @pytest.mark.needs("ngspice")
    @pytest.mark.parametrize("where", ["cwd", "home"])
    def test_no_spiceinit_reaches_a_run(self, tmp_path, where):
        import os
        import subprocess
        import sys

        home, work = tmp_path / "home", tmp_path / "work"
        home.mkdir()
        work.mkdir()
        marker = tmp_path / "read"
        (home if where == "home" else work).joinpath(".spiceinit").write_text(f"shell touch {marker}\n")
        r = subprocess.run(
            [sys.executable, "-c", ONE_VOLT], cwd=work, capture_output=True, text=True, timeout=60,
            env={**os.environ, "HOME": str(home)},
        )
        assert r.returncode == 0, r.stderr
        assert float(r.stdout) == 1.0
        assert not marker.exists(), f"ngspice read the {where} .spiceinit"


def test_transient_noise_is_a_tran_over_trnoise_sources():
    """ngspice manual §4.1.7: TRNOISE(NA NT 0 0) draws white samples of rms
    NA every NT and interpolates linearly between them, so the node's rms
    over the run lies between sqrt(2/3)*NA (every point mid-interval) and
    NA (every point a sample); 2000 samples put the estimate within ~3%."""
    import spicerack as ps

    dut = ps.Subcircuit("n", ["out"])
    dut.R(name="1", positive="out", negative=dut.gnd, value=1e3)
    tb = ps.Testbench(dut)
    tb.TransientNoiseVoltageSource("n", "out", "0", noise_rms=1e-3, time_step=1e-9)
    tb.add_transient_noise(1e-9, 2e-6)
    assert ".tran 1n 2u" in tb.netlist("ngspice").splitlines()
    (res,) = tb.run()
    v = res["out"]
    rms = math.sqrt(sum(x * x for x in v) / len(v))
    assert 0.75e-3 < rms < 1.06e-3, rms


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_p8_013_nested_definitions_run_to_the_closed_form(backend):
    """ngspice-45 manual §2.6.1: a definition inside a definition is local to
    it. Two parents each hold their own `leaf` (1 kOhm in `one`, 3 kOhm in
    `two`); in series under 1 V the midpoint is 3/(1+3) = 0.75 V (closed
    form). One shared `leaf` would give 0.5 V."""
    import spicerack as ps

    def wrapped(name, ohms):
        leaf = ps.Subcircuit("leaf", ["p", "q"])
        leaf.R("1", "p", "q", ohms)
        wrap = ps.Subcircuit(name, ["a", "b"])
        wrap.subcircuit(leaf)
        wrap.X("1", "leaf", "a", "b")
        return wrap

    one, two = wrapped("one", 1e3), wrapped("two", 3e3)
    d = ps.Subcircuit("div", ["in", "out"])
    d.instance(one, "1", "in", "out")
    d.instance(two, "2", "out", "0")
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.add_subcircuit(one)
    tb.add_subcircuit(two)
    tb.V("in", "in", "0", 1.0)
    # A linear DC solve: exact to rounding.
    assert tb.operating_point()["out"] == pytest.approx(0.75, rel=1e-9)
