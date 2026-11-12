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


@pytest.mark.parametrize("backend, zout", [("ngspice", 750.0), ("espice", 750.0), ("vacask", None)])
def test_p3_095_api_tf_result_names_differ(backend, zout):
    """ZERO-LEDGER P3-095: a `.tf` result has the same names on every backend.
    Oracle: the closed form of a 1k/3k divider (gain 3k/4k, input impedance
    1k + 3k, output impedance 1k || 3k), which each simulator's raw file
    holds under its own names (ngspice `vin#Input_impedance`, ESPice
    `input_resistance`, VACASK `zin(vin)`). VACASK's `dcxf` reports a gain
    and an input impedance for every source of the deck (`Vaux` here: 0 and
    1k) and no output impedance."""
    import spicerack as ps

    dut = ps.Subcircuit("div", ["in", "out"])
    dut.R(name="1", positive="in", negative="out", value=1e3)
    dut.R(name="2", positive="out", negative="0", value=3e3)
    tb = ps.Testbench(dut)
    tb.with_backend(backend)
    tb.V(name="in", positive="in", negative="0", value=1.0)
    tb.V(name="aux", positive="aux", negative="0", value=1.0)
    tb.R(name="aux", positive="aux", negative="0", value=1e3)
    tf = tb.transfer_function("v(out)", "Vin")
    assert tf["transfer_function"] == pytest.approx([0.75], rel=1e-9)
    assert tf["input_impedance"] == pytest.approx([4000.0], rel=1e-9)
    if zout is None:
        assert tf["transfer_function(vaux)"] == pytest.approx([0.0], abs=1e-12)
        with pytest.raises(KeyError):
            tf["output_impedance"]
    else:
        assert tf["output_impedance"] == pytest.approx([zout], rel=1e-9)


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


@pytest.mark.needs("vacask")
def test_p3_032_api_vacask_subcircuit_node_xu1():
    """ZERO-LEDGER P3-032: a node inside a VACASK instance is a voltage under
    its hierarchical name, not the current `i(<instance>)`. Oracle: the
    closed form of 1.5 V over 1k + 1k + 2k (m = 1.5*3/4, q = 1.5*2/4, the
    source's branch current -1.5/4k), which VACASK 36be560's own raw file
    holds under `xu1:m`, `xu1:q` and `vdd:flow(br)`."""
    import spicerack as ps

    half = ps.Subcircuit("half", ["a", "b"])
    half.R(name="1", positive="a", negative="m", value=1e3)
    half.R(name="2", positive="m", negative="q", value=1e3)
    half.R(name="3", positive="q", negative="b", value=2e3)
    top = ps.Subcircuit("top", ["vdd"])
    top.X("u1", "half", "vdd", "0")
    tb = ps.Testbench(top)
    tb.with_backend("vacask")
    tb.add_subcircuit(half)
    tb.V(name="dd", positive="vdd", negative="0", value=1.5)
    op = tb.operating_point()
    assert op["xu1.m"] == pytest.approx(1.125, rel=1e-9)
    assert op["xu1.q"] == pytest.approx(0.75, rel=1e-9)
    assert op["i(vdd)"] == pytest.approx(-1.5 / 4e3, rel=1e-9)
    with pytest.raises(KeyError):
        op["i(xu1)"]


def test_raw_four_line_runs_as_fourier_on_ngspice():
    """A raw `.four` testbench line becomes `fourier` in the `.control` block
    (the table goes to stdout, which `tran.fourier` reads: the P3-049 test
    below), and the transient still comes back."""
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
    assert tran.fourier[0]["output"] == "v(out)"


def test_p3_049_api_fourier_results_unreachable():
    """ZERO-LEDGER P3-049: the `.four` tables of a run are read back, on its
    transient result. Oracle: closed form. 1 V at 1 kHz in series with 0.1 V
    at 3 kHz: harmonic 1 is 1 V, harmonic 3 is 0.1 V, the others 0, THD =
    0.1/1 = 10 % (ngspice-45 printed 0.999996, 0.0999964 and 9.99968 %)."""
    import spicerack as ps

    dut = ps.Subcircuit("load", ["in"])
    dut.R(name="1", positive="in", negative="0", value=1e3)
    tb = ps.Testbench(dut)
    tb.with_backend("ngspice")
    tb.SinusoidalVoltageSource(name="1", positive="in", negative="mid", amplitude=1.0, frequency=1e3)
    tb.SinusoidalVoltageSource(name="3", positive="mid", negative="0", amplitude=0.1, frequency=3e3)
    tb.add_transient(step_time=1e-6, end_time=10e-3, max_time=1e-6)
    tb.add_fourier(fundamental_frequency=1e3, outputs=["v(in)"])
    tran, four = tb.run()
    assert four is None
    (table,) = tran.fourier
    assert table["output"] == "v(in)"
    assert table["frequency"] == [n * 1e3 for n in range(10)]
    assert table["magnitude"][1] == pytest.approx(1.0, rel=1e-3)
    assert table["magnitude"][3] == pytest.approx(0.1, rel=1e-3)
    assert abs(table["magnitude"][2]) < 1e-6
    assert table["norm_magnitude"][3] == pytest.approx(0.1, rel=1e-3)
    assert table["thd"] == pytest.approx(10.0, rel=1e-3)
    assert len(table["phase"]) == len(table["norm_phase"]) == 10
    # A run with no `.four` has no table.
    plain = ps.Testbench(dut)
    plain.with_backend("ngspice")
    plain.V(name="1", positive="in", negative="0", value=1.0)
    assert plain.transient(step_time=1e-6, end_time=1e-4).fourier == []


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


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_p3_044_api_subcircuit_k_takes_names(backend):
    """`K` couples two inductors of the subcircuit, each named by its card
    ("L1"; P8-017 dropped the short form "1"), and refuses any other name. Oracle (closed
    form): an unloaded transformer, |v(out)| = k*sqrt(L2/L1) * |v(a)| with
    |v(a)| = wL1/sqrt(R^2 + (wL1)^2): 0.5 * 2 * 0.98757049 at 1 kHz for
    1 Ohm, 1 mH, 4 mH. ngspice-45 prints 9.875705e-01 for `K1 L1 L2 0.5` and
    "coupling to non-existent inductor ll1" for `K1 LL1 LL2 0.5`
    (research/impl/evidence/P3-044)."""
    import spicerack as ps

    def transformer(first, second):
        d = ps.Subcircuit("xf", ["in", "out"])
        d.R("1", "in", "a", 1.0)
        d.L("1", "a", "0", 1e-3)
        d.L("2", "out", "0", 4e-3)
        d.K("1", first, second, coupling=0.5)
        return d

    w = 2 * math.pi * 1e3 * 1e-3
    expected = 0.5 * 2.0 * w / math.hypot(1.0, w)
    for first, second in (("L1", "L2"), ("l1", "L2")):
        tb = ps.Testbench(transformer(first, second))
        tb.with_backend(backend)
        tb.V("in", "in", "0", 0.0, ac=1.0)
        for target in ("ngspice", "espice", "hspice", "xyce", "ltspice", "spectre", "vacask", "cdl"):
            assert "ll1" not in tb.netlist(target).lower(), (first, target)
        ac = tb.ac(variation="lin", number_of_points=1, start_frequency=1e3, stop_frequency=1e3)
        # AC is a linear solve: exact to rounding.
        assert ac.magnitude("out")[0] == pytest.approx(expected, rel=1e-9), (first, second)
    for first, second in (("1", "2"), ("zz", "L2"), ("L1", "L3"), ("L", "L2"), ("", "L2")):
        with pytest.raises(ValueError, match="names no inductor"):
            transformer(first, second)


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


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_p3_096_api_operatingpoint_has_no_nodes(backend):
    """ZERO-LEDGER P3-096: an OperatingPoint lists its nodes and branches and
    reads a vector by the simulator's spelling too. Oracle: closed form, 9 V
    over 2k + 1k: vout = 9 * 1k/3k = 3 V, the source's branch current
    -9/3k = -3 mA."""
    import spicerack as ps

    dut = ps.Subcircuit("divider", ["vin", "vout"])
    dut.R(name="top", positive="vin", negative="vout", value=2e3)
    dut.R(name="bot", positive="vout", negative="0", value=1e3)
    tb = ps.Testbench(dut)
    tb.with_backend(backend)
    tb.V(name="supply", positive="vin", negative="0", value=9.0)
    op = tb.operating_point()
    for key in ["v(vout)", "V(VOUT)", "vout"]:
        assert op[key] == pytest.approx(3.0, rel=1e-9), key
    for key in ["i(vsupply)", "I(Vsupply)", "vsupply#branch"]:
        assert op[key] == pytest.approx(-3e-3, rel=1e-9), key
    assert op.nodes == ["vin", "vout"]
    assert op.branches == ["i(vsupply)"]
    assert op.vout == pytest.approx(3.0, rel=1e-9)
    with pytest.raises(KeyError, match="No node 'v\\(nope\\)'"):
        op["v(nope)"]


@pytest.mark.needs("ngspice")
def test_p8_318_every_result_lists_its_names_in_one_order():
    """ZERO-LEDGER P8-318: `nodes`, `branches`, `measures` and `params` of a result are in name
    order, the same on every result object (they were a hash map's: another order per
    object). Oracle: the deck's six nodes and six measures, written out by hand in name
    order, and Python's own `sorted` for the lists that hold the sweep's vector too."""
    import spicerack as ps

    dut = ps.Subcircuit("ladder", ["a", "f"])
    for name, (p, n) in zip("123456", zip("abcdef", "bcdef0")):
        dut.R(name, p, n, 1e3)

    def bench():
        tb = ps.Testbench(dut)
        tb.with_backend("ngspice")
        tb.V("in", "a", "0", 1.0, ac=1.0)
        return tb

    def dc():
        return bench().dc(Vin=slice(0.0, 1.0, 0.5))

    def ac():
        return bench().ac(variation="dec", number_of_points=2, start_frequency=1.0, stop_frequency=100.0)

    def tran():
        tb = bench()
        for node in "fedcba":  # declared last name first
            tb.measure("tran", f"m_{node}", "max", f"v({node})")
        return tb.transient(step_time=1e-6, end_time=1e-5)

    for run in (dc, ac, tran, lambda: bench().operating_point()):
        one, two = run(), run()
        kind = type(one).__name__
        assert one.nodes == two.nodes == sorted(one.nodes), kind
        # The deck's nodes are the one-letter names; a sweep's own vector is a longer one.
        assert [n for n in one.nodes if len(n) == 1] == list("abcdef"), kind
        assert one.branches == two.branches == ["i(vin)"], kind
        assert list(one.measures) == list(two.measures) == sorted(one.measures), kind
        assert list(one.params) == sorted(one.params), kind
    assert list(tran().measures) == ["m_a", "m_b", "m_c", "m_d", "m_e", "m_f"]


def test_run_with_a_measure_runs_every_analysis_from_the_control_block():
    """A `.meas` moves ngspice's analyses into a `.control` block, which runs
    them in card order; the results still land on their own analyses."""
    tb = _queued_bench("ngspice")
    tb.measure("tran", "vend", "find", "v(out)", "at=1m")
    tran, op, ac = tb.run()
    _check_queued(tran, op, ac)
    assert tran.measures["vend"] == pytest.approx(1.0 - math.exp(-2.0), abs=2e-3)


def test_p8_102_a_failed_measure_reads_none():
    """1k into 1 uF, a 0 -> 1 V step, a transient to 5 ms and an AC sweep.
    Oracle: ngspice-45's own printout of the written deck (research/impl/
    evidence/P8-102/bench.stdout, bench.stderr): values for vend, t50 and
    gain1k; none for tnever, tbad and fnever, which its stderr names ("out of
    interval ... failed!"). The three values also have closed forms."""
    import spicerack as ps

    dut = ps.Subcircuit("rc", ["in", "out"])
    dut.R(name="1", positive="in", negative="out", value=1e3)
    dut.C(name="1", positive="out", negative="0", value=1e-6)
    tb = ps.Testbench(dut)
    tb.with_backend("ngspice")
    tb.PulseVoltageSource(name="in", positive="in", negative="0", initial_value=0.0, pulsed_value=1.0,
                          delay=0.0, rise_time=1e-9, fall_time=1e-9, pulse_width=1.0, period=2.0, ac=1.0)
    tb.add_transient(step_time=1e-5, end_time=5e-3, max_time=1e-5)
    tb.add_ac(variation="dec", number_of_points=10, start_frequency=1.0, stop_frequency=1e5)
    tb.measure("tran", "vend", "find", "v(out)", "at=5m")
    tb.measure("tran", "t50", "when", "v(out)=0.5")
    tb.measure("tran", "tnever", "when", "v(out)=2")
    tb.measure("tran", "tbad", "trig", "v(out)", "val=0.1", "rise=1", "targ", "v(out)", "val=5", "rise=1")
    tb.measure("ac", "gain1k", "find", "vdb(out)", "at=1k")
    tb.measure("ac", "fnever", "when", "vdb(out)=10")
    want = {
        "vend": 1.0 - math.exp(-5.0),                             # ngspice: 9.932623e-01
        "t50": 1e-3 * math.log(2.0),                              # ngspice: 6.93154e-04
        "gain1k": -10.0 * math.log10(1.0 + (2.0 * math.pi) ** 2),  # ngspice: -1.607224e+01
    }
    results = tb.run()
    assert len(results) == 2
    for res in results:
        m = res.measures
        assert set(m) == {"vend", "t50", "tnever", "tbad", "gain1k", "fnever"}
        assert {n for n, v in m.items() if v is None} == {"tnever", "tbad", "fnever"}
        for name, value in want.items():
            assert m[name] == pytest.approx(value, rel=2e-3), name


def test_p8_320_only_a_measure_the_deck_names_is_one():
    """ZERO-LEDGER P8-320: 1k into 1 uF, a 0 -> 1 V step. Oracle: the deck. With no measure a result
    has none (the line `Stack = 0 bytes.` of ngspice's footer was read as one:
    research/impl/evidence/P8-320/probe-b13.out); with one it has that name alone, and its value
    is the closed form."""
    import spicerack as ps

    def bench():
        dut = ps.Subcircuit("rc", ["in", "out"])
        dut.R(name="1", positive="in", negative="out", value=1e3)
        dut.C(name="1", positive="out", negative="0", value=1e-6)
        tb = ps.Testbench(dut)
        tb.with_backend("ngspice")
        tb.PulseVoltageSource(name="in", positive="in", negative="0", initial_value=0.0, pulsed_value=1.0,
                              delay=0.0, rise_time=1e-9, fall_time=1e-9, pulse_width=1.0, period=2.0)
        return tb

    assert bench().operating_point().measures == {}
    assert bench().transient(step_time=1e-5, end_time=5e-3).measures == {}
    tb = bench()
    tb.measure("tran", "vend", "find", "v(out)", "at=5m")
    measures = tb.transient(step_time=1e-5, end_time=5e-3).measures
    assert list(measures) == ["vend"]
    assert measures["vend"] == pytest.approx(1.0 - math.exp(-5.0), rel=2e-3)


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_p3_094_api_two_sweep_dc_has(backend):
    """ZERO-LEDGER P3-094: a two-source dc returns both sweeps: the inner one
    is the axis (`dc.sweep`), the outer one the vector `sweep1`, on every
    backend. Oracle: the simulator's own node voltages (`a` is V1, `b` is
    V2) and the closed form of the resistor star, out = (V1 + V2) / 3."""
    import spicerack as ps

    dut = ps.Subcircuit("star", ["a", "b", "out"])
    dut.R(name="1", positive="a", negative="out", value=1e3)
    dut.R(name="2", positive="b", negative="out", value=1e3)
    dut.R(name="3", positive="out", negative="0", value=1e3)
    tb = ps.Testbench(dut)
    tb.with_backend(backend)
    tb.V(name="1", positive="a", negative="0", value=0.0)
    tb.V(name="2", positive="b", negative="0", value=0.0)
    dc = tb.dc(V1=slice(0.0, 1.0, 0.5), V2=slice(0.0, 2.0, 1.0))
    assert dc.sweep == pytest.approx([0.0, 0.5, 1.0] * 3)
    assert dc.sweep == pytest.approx(dc["a"])
    assert dc["sweep1"] == pytest.approx([0.0] * 3 + [1.0] * 3 + [2.0] * 3)
    assert dc["sweep1"] == pytest.approx(dc["b"])
    assert dc["out"] == pytest.approx([(x + y) / 3 for x, y in zip(dc.sweep, dc["sweep1"])])


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


def _p8_lines_nearest(time, v, t):
    """The sample of `v` nearest to time `t`."""
    return v[min(range(len(time)), key=lambda i: abs(time[i] - t))]


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
def test_p8_049_a_lossy_line_at_dc_is_its_series_resistance(backend):
    """ngspice-45 manual §6.2: an LTRA model's `r` is ohms per unit length
    and `len` the line's length, so at DC the line is r*len in series:
    10 Ohm/m over 5 m under a 50 Ohm load and 1 V is 50/(50+50) V (closed
    form; ngspice-45 and ESPice 0.3.0 give 0.5,
    research/impl/evidence/P8-049/t21.ng.out, t21.es.out)."""
    import spicerack as ps

    d = ps.Subcircuit("ltra", ["in", "out"])
    d.model("omod", "ltra", r=10, l=250e-9, c=100e-12, len=5)
    d.O("1", "in", d.gnd, "out", d.gnd, model="omod")
    d.R("l", "out", d.gnd, 50)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.V("in", "in", "0", 1.0)
    # A linear DC solve: exact to rounding.
    assert tb.operating_point()["out"] == pytest.approx(50 / (50 + 10 * 5), rel=1e-9)


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
@pytest.mark.parametrize("lumps", [None, 6])
def test_p8_053_a_uniform_rc_line_at_dc_is_its_series_resistance(backend, lumps):
    """ngspice-45 manual §6.3: a URC line is `rperl` ohms per metre over
    `l` metres, in any number of lumps: 1 MOhm/m over 1 mm is 1 kOhm, and
    under a 1 kOhm load and 1 V the output is 1/2 V (closed form)."""
    import spicerack as ps

    d = ps.Subcircuit("urc", ["in", "out"])
    d.model("umod", "urc", rperl=1e6, cperl=1e-12, fmax=1e9)
    d.U("1", "in", "out", d.gnd, model="umod", length=1e-3, lumps=lumps)
    d.R("l", "out", d.gnd, 1e3)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.V("in", "in", "0", 1.0)
    # A linear DC solve over the lumps: exact to rounding.
    assert tb.operating_point()["out"] == pytest.approx(1e3 / (1e3 + 1e6 * 1e-3), rel=1e-9)


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
@pytest.mark.parametrize("length", [None, 2])
def test_p8_050_a_txl_line_delays_a_step_by_its_length(backend, length):
    """A matched line: Z0 = sqrt(l/c) = 50 Ohm between a 50 Ohm source and
    a 50 Ohm load, 5 ns per metre (sqrt(l*c)). A 1 V step reaches the load
    as 1/2 V after the line's delay and not before; `length=` on the card
    (ngspice-45 manual §6.4.1, `<LEN=LENGTH>`) replaces the model's 1 m.
    The closed form is the lossless line's: `r` is 0.01 Ohm/m (ESPice
    0.3.0 refuses a `txl` model with r=0), which moves the level by at
    most 0.5 * 0.02/100 = 1e-4 V."""
    import spicerack as ps

    d = ps.Subcircuit("txl", ["s", "out"])
    d.model("ymod", "txl", r=0.01, l=250e-9, g=0, c=100e-12, length=1)
    d.R("s", "s", "in", 50)
    d.Y("1", "in", d.gnd, "out", d.gnd, model="ymod", length=length)
    d.R("l", "out", d.gnd, 50)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.PulseVoltageSource("s", "s", "0", initial_value=0.0, pulsed_value=1.0, delay=0.0, rise_time=1e-10,
                          fall_time=1e-10, pulse_width=1e-7, period=2e-7)
    tran = tb.transient(step_time=1e-10, end_time=3e-8, max_time=1e-10)
    time, v = list(tran.time), list(tran["out"])
    delay = (length or 1) * math.sqrt(250e-9 * 100e-12)
    assert delay == pytest.approx((length or 1) * 5e-9)
    # Half a nanosecond either side of the arrival (the edge is 0.1 ns).
    assert _p8_lines_nearest(time, v, delay - 0.5e-9) == pytest.approx(0.0, abs=1e-6)
    assert _p8_lines_nearest(time, v, delay + 0.5e-9) == pytest.approx(0.5, abs=5e-4)
    assert v[-1] == pytest.approx(0.5, abs=5e-4)


# A symmetric pair, per metre (the L11, L12 and C11, C12 of the ngspice-45
# manual's §6.4.2 example): each conductor's own value, then the mutual one.
_P8_051_L = (3.83572e-7, 8.26253e-8)  # H/m
_P8_051_C = (9.143579e-11, -9.78265e-12)  # F/m, Maxwell form


def _p8_051_cpl_step(backend, length):
    """Two coupled lines, 50 Ohm at all four ends, a 1 V step into the
    first: time and the far-end voltages of the driven line and its
    neighbour."""
    import spicerack as ps

    (l11, l12), (c11, c12) = _P8_051_L, _P8_051_C
    d = ps.Subcircuit("cpl", ["s", "b1", "b2"])
    d.model("pmod", "cpl", r="0 0 0", l=f"{l11} {l12} {l11}", g="0 0 0", c=f"{c11} {c12} {c11}", length=1)
    d.R("s1", "s", "a1", 50)
    d.R("s2", "a2", d.gnd, 50)
    d.P("1", ["a1", "a2"], d.gnd, ["b1", "b2"], d.gnd, model="pmod", length=length)
    d.R("l1", "b1", d.gnd, 50)
    d.R("l2", "b2", d.gnd, 50)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.PulseVoltageSource("s", "s", "0", initial_value=0.0, pulsed_value=1.0, delay=0.0, rise_time=1e-10,
                          fall_time=1e-10, pulse_width=1.0, period=2.0)
    tran = tb.transient(step_time=1e-10, end_time=1.2e-8 * (length or 1), max_time=1e-10)
    return list(tran.time), list(tran["b1"]), list(tran["b2"])


def _p8_051_modes():
    """The pair's even and odd modes, each as (seconds per metre, the
    step's share at a 50 Ohm far end from a 50 Ohm source). A symmetric
    lossless pair carries a step on line 1 as half even (both lines
    alike) and half odd (opposite): mode m has L = l11 +- l12 and
    C = c11 +- c12, travels at sqrt(L*C) per metre in Z = sqrt(L/C), is
    launched as 1/2 * Z/(50+Z) and arrives as that times 2*50/(50+Z)."""
    (l11, l12), (c11, c12) = _P8_051_L, _P8_051_C
    out = []
    for sign in (1, -1):
        l, c = l11 + sign * l12, c11 + sign * c12
        z = math.sqrt(l / c)
        out.append((math.sqrt(l * c), 50 * z / (50 + z) ** 2))
    return out


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
def test_p8_051_coupled_lines_carry_a_step_as_two_modes(backend):
    """The odd mode is the faster (5.519 ns/m against 6.170 ns/m), so the
    far ends read 0 until it arrives, then +-0.2495310 V (line 1, line 2),
    then 0.4891703 V and -0.0098917 V once the even mode is in, until the
    first reflection (three odd delays). ngspice-45 and ESPice 0.3.0 give
    these to 7 digits (research/impl/evidence/P8-049/api/t25.out). A card with
    its nodes in another order gives none of the three plateaus."""
    time, b1, b2 = _p8_051_cpl_step(backend, None)
    (t_even, even), (t_odd, odd) = _p8_051_modes()
    assert t_odd < t_even < 3 * t_odd
    for t, want1, want2 in [
        (t_odd - 0.3e-9, 0.0, 0.0),
        ((t_odd + t_even) / 2, odd, -odd),
        (t_even + 1e-9, even + odd, even - odd),
    ]:
        assert _p8_lines_nearest(time, b1, t) == pytest.approx(want1, abs=1e-5), t
        assert _p8_lines_nearest(time, b2, t) == pytest.approx(want2, abs=1e-5), t


@pytest.mark.parametrize(
    "backend",
    [
        "ngspice",
        # ESPICE-CPL-LEN: ESPice 0.3.0 reads a P card's `LEN=` and keeps the
        # model's length (research/impl/evidence/P8-049/api/t25.out). The marker
        # goes when ESPice takes the card's: until then a pass fails.
        pytest.param("espice", marks=pytest.mark.xfail(strict=True, reason="ESPICE-CPL-LEN")),
    ],
)
def test_p8_051_a_coupled_line_takes_its_length_from_the_card(backend):
    """ngspice-45 manual §6.4.2: "len will override the model parameter
    length for the specific instance only". Ten times the model's metre is
    ten times each delay: nothing at the far end 0.3 ns before ten odd
    delays, the odd plateau between the two arrivals."""
    time, b1, b2 = _p8_051_cpl_step(backend, 10)
    (t_even, _), (t_odd, odd) = _p8_051_modes()
    assert _p8_lines_nearest(time, b1, 10 * t_odd - 0.3e-9) == pytest.approx(0.0, abs=1e-5)
    assert _p8_lines_nearest(time, b1, 5 * (t_odd + t_even)) == pytest.approx(odd, abs=1e-5)
    assert _p8_lines_nearest(time, b2, 5 * (t_odd + t_even)) == pytest.approx(-odd, abs=1e-5)


def _p8_060_am_with_phases(backend):
    """An AM source with both phases across 1 kOhm: its time and voltage
    samples over 100 us."""
    import spicerack as ps

    d = ps.Subcircuit("am", ["in"])
    d.R("1", "in", d.gnd, 1e3)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.AmplitudeModulatedVoltageSource("in", "in", "0", offset=0.5, amplitude=1.0, modulating_frequency=1e3,
                                       carrier_frequency=1e4, signal_phase=45.0, carrier_phase=90.0)
    tran = tb.transient(step_time=1e-6, end_time=1e-4, max_time=1e-6)
    return list(tran.time), list(tran["in"])


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
def test_p8_060_an_am_source_with_phases_follows_its_closed_form(backend):
    """ngspice-45 manual §4.1.6, eq. 4.4: V(t) = VO + (VMO + VMA sin(2 pi FM
    t + PHASEM)) sin(2 pi FC t + PHASEC) for t > TD, the phases in degrees.
    The source drives the node, so every sample after t = 0 is the closed
    form to rounding: ngspice-45 exactly, ESPice 0.3.0 to 2e-16
    (research/impl/evidence/P8-060/t18.out)."""
    import math

    time, v = _p8_060_am_with_phases(backend)
    assert len(time) > 100 and time[-1] == pytest.approx(1e-4)
    for t, got in zip(time[1:], v[1:]):
        want = (0.5 + math.sin(2 * math.pi * 1e3 * t + math.radians(45))) * math.sin(2 * math.pi * 1e4 * t + math.radians(90))
        assert got == pytest.approx(want, abs=1e-12), t


@pytest.mark.parametrize(
    "backend",
    [
        "ngspice",
        # ESPICE-AM-T0: ESPice 0.3.0 gives the waveform's value at t = 0
        # (1.2071 V here; research/impl/evidence/P8-060/t18.out). The marker
        # goes when ESPice gives 0 there: until then a pass fails.
        pytest.param("espice", marks=pytest.mark.xfail(strict=True, reason="ESPICE-AM-T0")),
    ],
)
def test_p8_060_an_am_source_is_zero_until_its_delay(backend):
    """ngspice-45 manual §4.1.6: "with t > TD, else V(t) = 0"
    (`vsrcload.c:307-309`), the operating point included."""
    time, v = _p8_060_am_with_phases(backend)
    assert time[0] == 0.0 and v[0] == 0.0


@pytest.mark.parametrize(
    "backend",
    [
        "ngspice",
        # ESPICE-SWITCH-ON: ESPice 0.3.0 reads the word and leaves the
        # switch off (research/impl/evidence/P8-045/es/e_ON.raw). The marker
        # goes when ESPice holds the state: until then a pass fails.
        pytest.param("espice", marks=pytest.mark.xfail(strict=True, reason="ESPICE-SWITCH-ON")),
    ],
)
def test_p8_045_a_switch_inside_its_hysteresis_band_starts_in_its_initial_state(backend):
    """ngspice-45 manual §3.3.15: "ON or OFF determine the initial state of
    the switch" when the control starts inside the hysteresis loop. The
    control is 0.5 V on vt=0.5, vh=0.2; the switch (1 Ohm on, 1 MOhm off)
    is the lower leg of a divider under 1 kOhm and 1 V: 1/1001 V on,
    1e6/1.001e6 V off (closed form). ngspice-45 prints 9.990010e-04 and
    9.990010e-01 on the hand decks (research/impl/evidence/P8-045/ng/)."""
    import spicerack as ps

    def out(state):
        d = ps.Subcircuit("sw", ["s", "c", "out"])
        d.model("sw", "sw", vt=0.5, vh=0.2, ron=1, roff=1e6)
        d.R("1", "s", "out", 1e3)
        d.S("1", "out", d.gnd, control_positive="c", control_negative=d.gnd, model="sw", initial_state=state)
        tb = ps.Testbench(d)
        tb.with_backend(backend)
        tb.V("s", "s", "0", 1.0)
        tb.V("c", "c", "0", 0.5)
        return tb.operating_point()["out"]

    # A linear DC solve once the state is chosen: exact to rounding.
    assert out("off") == pytest.approx(1e6 / 1.001e6, rel=1e-9)
    assert out(None) == pytest.approx(1e6 / 1.001e6, rel=1e-9)
    assert out("on") == pytest.approx(1 / 1001, rel=1e-9)


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_p8_014_each_instance_holds_the_branch_its_parameter_selects(backend):
    """ngspice-45 manual §2.13: the block of the first true condition is
    the one in the circuit. `cell` is r in series, then 5 kOhm to ground
    when r > 1500, 6 kOhm when r > 500, else 7 kOhm; under 1 V its three
    instances (r = 2k, 1k, 100) divide to 5/7, 6/7 and 7/7.1 V (closed
    form). ngspice-45, ESPice 0.3.0 and VACASK print those on the hand
    decks (research/impl/evidence/P8-014/ng/sub.log, x/)."""
    import spicerack as ps

    def load(ohms):
        b = ps.Block()
        b.R("9", "out", b.gnd, ohms)
        return b

    cell = ps.Subcircuit("cell", ["in", "out"], r="1k")
    cell.R("1", "in", "out", "{r}")
    cell.conditional([("r > 1500", load(5e3)), ("r > 500", load(6e3))], otherwise=load(7e3))
    d = ps.Subcircuit("three", ["in", "a", "b", "c"])
    for node, r in (("a", "2k"), ("b", "1k"), ("c", "100")):
        d.instance(cell, node, "in", node, r=r)
    tb = ps.Testbench(d)
    tb.add_subcircuit(cell)
    tb.with_backend(backend)
    tb.V("in", "in", "0", 1.0)
    op = tb.operating_point()
    for node, want in (("a", 5 / 7), ("b", 6 / 7), ("c", 7 / 7.1)):
        # A linear DC solve: exact to rounding.
        assert op[node] == pytest.approx(want, rel=1e-9), node


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
def test_p3_093_api_ngspice_integrated_noise_plot(backend):
    """ZERO-LEDGER P3-093: a noise result keeps the simulator's "Integrated
    Noise" plot: `onoise_total` is a one-point vector beside the spectra.
    Oracle: closed form. The output noise of R into C over [f1, f2] is
    sqrt(4kTR*fc*(atan(f2/fc) - atan(f1/fc))), fc = 1/(2*pi*R*C): 6.4245e-8 V
    over [1, 1e8] Hz, 3.8254e-8 V over [1, 100] Hz at 300.15 K. ngspice-45
    and ESPice 0.3.0 integrate their 10 points/decade grid 0.11 % low
    (6.4174e-8, 3.8227e-8), hence rel=5e-3."""
    import spicerack as ps

    dut = ps.Subcircuit("rc", ["in", "out"])
    dut.R(name="1", positive="in", negative="out", value=1e3)
    dut.C(name="1", positive="out", negative="0", value=1e-6)
    tb = ps.Testbench(dut)
    tb.with_backend(backend)
    tb.V(name="in", positive="in", negative="0", value=0.0, ac=1.0)
    band = dict(output_node="out", ref_node="0", src="Vin", variation="dec", points=10, start_frequency=1.0)
    fc = 1 / (2 * math.pi * 1e3 * 1e-6)

    def total(stop):
        return math.sqrt(4 * 1.380649e-23 * 300.15 * 1e3 * fc * (math.atan(stop / fc) - math.atan(1 / fc)))

    alone = tb.noise(stop_frequency=1e8, **band)
    assert alone["onoise_total"] == pytest.approx([total(1e8)], rel=5e-3)
    assert len(alone["onoise_spectrum"]) == len(alone.frequency) == 81
    tb.add_operating_point()
    tb.add_noise(stop_frequency=1e8, **band)
    tb.add_noise(stop_frequency=100.0, **band)
    _, wide, narrow = tb.run()
    assert wide["onoise_total"] == pytest.approx([total(1e8)], rel=5e-3)
    assert narrow["onoise_total"] == pytest.approx([total(100.0)], rel=5e-3)
    assert narrow.frequency[-1] == pytest.approx(100.0)


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


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
def test_p8_012_a_subcircuit_function_reads_the_instances_parameter(backend):
    """ngspice-45 manual §2.12 (`.func`), §2.11.3 (subcircuit parameters): the
    cell's resistor is scaled(500) = 500*k ohms, k = 6 on the instance, so
    3 kOhm under 1 kOhm and 1 V: 3/(1+3) = 0.75 V (closed form). With the
    cell's default k = 2 it would be 0.5 V. ngspice-45 and ESPice 0.3.0 on the
    hand deck: 7.5e-01 (research/impl/evidence/P8-012)."""
    import spicerack as ps

    cell = ps.Subcircuit("cell", ["a", "b"], k="2")
    cell.add_function("scaled", ["x"], "x*k")
    cell.R("1", "a", "b", "{scaled(500)}")
    d = ps.Subcircuit("div", ["in", "out"])
    d.R("1", "in", "out", 1e3)
    d.instance(cell, "1", "out", "0", k=6)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.add_subcircuit(cell)
    tb.V("in", "in", "0", 1.0)
    # A linear DC solve: exact to rounding.
    assert tb.operating_point()["out"] == pytest.approx(0.75, rel=1e-9)


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
def test_p8_010_nonlinear_sources_run_to_the_closed_form(backend):
    """ngspice-45 manual §5.2.2, §5.3.2: v(x) = 2*v(out) = 2 V; the G source
    draws 1 mA out of node y through 1 kOhm to ground, so v(y) = -1 V (the
    current flows from `positive` through the source to `negative`, as a B
    source's). v(out) = 1 V: no DC current in the RC. ngspice-45 and ESPice
    0.3.0 print 2, -1 for the hand decks (research/impl/evidence/P8-010)."""
    import spicerack as ps

    d = ps.Subcircuit("rc", ["in", "out"])
    d.R("1", "in", "out", 1e3)
    d.C("1", "out", d.gnd, 1e-9)
    d.NonLinearVoltageSource("x", "x", d.gnd, expression="V(out)*2")
    d.NonLinearCurrentSource("y", "y", d.gnd, expression="V(out)*1m")
    d.R("x", "x", d.gnd, 1e3)
    d.R("y", "y", d.gnd, 1e3)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.V("in", "in", "0", 1.0)
    op = tb.operating_point()
    # A DC solve of a linear network: exact to rounding.
    assert op["x"] == pytest.approx(2.0, rel=1e-9)
    assert op["y"] == pytest.approx(-1.0, rel=1e-9)


@pytest.mark.parametrize("backend", ["ngspice", "espice"])
def test_p8_009_a_lib_call_selects_the_section_a_circuit_wrote(backend, tmp_path):
    """ngspice-45 manual §2.10: `.lib file tt` reads the lines between
    `.lib tt` and `.endl` of `file`. The divider's lower resistor is the
    section's `rsh`: 3 kOhm in `tt`, 1 kOhm in `ff`, under 1 kOhm and 1 V:
    0.75 V and 0.5 V (closed form). ngspice-45 and ESPice 0.3.0 on the hand
    decks print 7.5e-01 and 5e-01 (research/impl/evidence/P8-009)."""
    import spicerack as ps

    lib = ps.Circuit("corners")
    for name, rsh in (("tt", "3k"), ("ff", "1k")):
        s = ps.Section(name)
        s.parameter("rsh", rsh)
        lib.section(s)
    path = tmp_path / "corners.lib"
    path.write_text(str(lib))
    for name, want in (("tt", 0.75), ("ff", 0.5)):
        d = ps.Subcircuit("div", ["in", "out"])
        d.lib(str(path), name)
        d.R("1", "in", "out", 1e3)
        d.R("2", "out", d.gnd, "{rsh}")
        tb = ps.Testbench(d)
        tb.with_backend(backend)
        tb.V("in", "in", "0", 1.0)
        # A linear DC solve: exact to rounding.
        assert tb.operating_point()["out"] == pytest.approx(want, rel=1e-9), name


@pytest.mark.needs("vacask")
@pytest.mark.parametrize("title", ["div", "analysis of a divider", "sweep of a divider", "mc of a divider"])
def test_p8_319_a_vacask_decks_title_is_never_read_as_a_statement(title):
    """ZERO-LEDGER P8-319: the built-in reads a VACASK run by the deck's `analysis`, `sweep` and
    `mc` lines; line 1 is the title, which VACASK runs whatever it says. Read as a statement,
    `analysis of ...` failed the run ("vacask wrote no of.raw") and `sweep of ...` moved the
    time vector behind the nodes with no error. Oracle: closed form, 1 V over 1k + 3k is
    0.75 V at every point, and a transient to 1 us starts at 0 s and ends at 1 us."""
    import spicerack as ps

    def bench():
        c = ps.Circuit(title)
        c.V("in", "in", "0", 1.0)
        c.R("1", "in", "out", 1e3)
        c.C("1", "out", "0", 1e-9)
        c.R("2", "out", "0", 3e3)
        tb = ps.Testbench(c)
        tb.with_backend("vacask")
        return tb

    assert bench().operating_point()["out"] == pytest.approx(0.75, rel=1e-9)
    tran = bench().transient(step_time=1e-7, end_time=1e-6)
    assert (tran.time[0], tran.time[-1]) == (0.0, pytest.approx(1e-6, rel=1e-9))
    assert all(t0 < t1 for t0, t1 in zip(tran.time, tran.time[1:]))
    assert tran["out"] == pytest.approx([0.75] * len(tran.time), rel=1e-6)


def _p3_035_circuit():
    """1 V over a child's 1k (tc1 = 0.01/K) and the top's 3k, at 50 C."""
    import spicerack as ps

    buf = ps.Subcircuit("buf", ["a", "b"])
    buf.R("1", "a", "b", 1e3, tc1=0.01)
    c = ps.Circuit("top")
    c.V("in", "in", c.gnd, 1.0)
    c.subcircuit(buf)
    c.X("1", "buf", "in", "out")
    c.R("2", "out", c.gnd, 3e3)
    c.options(reltol=1e-4)
    c.temp(50)
    return ps, c


@pytest.mark.needs("ngspice")
def test_p3_035_a_testbench_of_a_circuit_keeps_its_definitions_options_and_temperature():
    """ZERO-LEDGER P3-035: `Testbench(circuit)` took the circuit's top alone: `X1 in out buf` with no
    `.subckt buf` (ngspice-45: "unknown subckt") and no `.options` or `.temp`. Oracle: closed form on
    ngspice-45, the child's 1k at 50 C with tc1 = 0.01/K and TNOM 27 C is 1k * (1 + 0.01 * 23) = 1230,
    so v(out) = 3000 / 4230 = 0.70922 (0.75 with the `.temp` lost; research/impl/evidence/P3-035)."""
    ps, c = _p3_035_circuit()
    tb = ps.Testbench(c)
    tb.with_backend("ngspice")
    tb.add_operating_point()
    words = [l.lower().split() for l in tb.netlist("ngspice").splitlines()]
    assert [".subckt", "buf", "a", "b"] in words
    assert any(w[0].startswith(".option") and any(x.startswith("reltol=") for x in w) for w in words if w)
    assert [".temp", "50"] in words
    # A linear DC solve: exact to rounding.
    assert tb.operating_point()["out"] == pytest.approx(3000 / 4230, rel=1e-9)
    # A `Subcircuit` DUT is its top alone, as before.
    assert ps.Testbench(ps.Subcircuit("s", ["a"])).netlist("ngspice").splitlines()[-1] == ".end"


def test_p3_035_a_circuit_is_netlisted_and_stored_as_its_whole_deck(tmp_path):
    """ZERO-LEDGER P3-035: `Circuit.netlist(target)` is the deck `Testbench(circuit)` writes, in any
    dialect, and `Circuit.to_source()` / `save(path)` store that deck. Oracle: a round trip, the
    stored text read back (`Testbench.from_source`, `Testbench.load`) writes the same netlists."""
    ps, c = _p3_035_circuit()
    text = c.to_source()
    assert text.startswith("* spicerack-source ngspice\n")
    assert ".subckt buf a b" in text.splitlines()
    for target in ("ngspice", "xyce"):
        assert c.netlist(target) == ps.Testbench(c).netlist(target), target
        assert ps.Testbench.from_source(text).netlist(target) == c.netlist(target), target
    path = str(tmp_path / "top.sp")
    c.save(path)
    assert open(path).read() == text
    assert ps.Testbench.load(path).netlist("ngspice") == c.netlist("ngspice")


@pytest.mark.needs("ngspice")
def test_p3_036_save_currents_is_written_by_netlist_and_keeps_the_saves():
    """ZERO-LEDGER P3-036: `save_currents` was applied at a run only, where it replaced the `save()`
    list with `all @all[i]` (no ngspice vector: "unrecognized variable"). It is ngspice's `.options
    savecurrents` (manual 11.7.3), in the deck `netlist()` writes too, beside the saves. Oracle:
    ngspice-45 on that deck saves v(out) and each resistor's current (research/impl/evidence/P3-036:
    `i(@r1[i])` = 0.25 mA for 1 V over 1k + 3k)."""
    import spicerack as ps

    d = ps.Subcircuit("div", ["in", "out"])
    d.R("1", "in", "out", 1e3)
    d.R("2", "out", d.gnd, 3e3)
    tb = ps.Testbench(d)
    tb.V("in", "in", "0", 1.0)
    tb.with_backend("ngspice")
    tb.save("v(out)")
    assert tb.save_currents is False
    tb.save_currents = True
    assert tb.save_currents is True
    lines = [l.lower() for l in tb.netlist("ngspice").splitlines()]
    assert ".options savecurrents" in lines
    assert ".save v(out)" in lines
    op = tb.operating_point()
    assert op["out"] == pytest.approx(0.75, rel=1e-9)
    # 1 V over 1k + 3k: 0.25 mA through each resistor (ngspice names the vector `i(@r1[i])`).
    assert op.branches == ["i(@r1[i])", "i(@r2[i])"]
    assert op["i(@r1[i])"] == pytest.approx(2.5e-4, rel=1e-9)
    # Off again: the option goes, the saves stay.
    tb.save_currents = False
    lines = [l.lower() for l in tb.netlist("ngspice").splitlines()]
    assert ".save v(out)" in lines and not any("savecurrents" in l for l in lines)
    # A target with no such option leaves it out with a note (W0024).
    tb.save_currents = True
    with pytest.warns(UserWarning, match="W0024"):
        assert "savecurrents" not in tb.netlist("ltspice").lower()


@pytest.mark.needs("vacask")
def test_p3_040_a_dc_sweep_of_a_resistor_sweeps_its_resistance():
    """ZERO-LEDGER P3-040: `tb.dc(Rl=slice(...))` sweeps resistor Rl (ngspice's `.dc` takes a
    source, a resistor or `temp`). Spectre got `dc param=Rl`, a netlist parameter nothing
    declares, and VACASK `instance="rl" parameter="dc"`, which a resistor does not have (the run
    wrote no dc1.raw). Oracle: closed form, 1 V over 1k + Rl is Rl / (1k + Rl): 0.5, 2/3, 0.75,
    which ngspice-45, Xyce 7.10 and ESPice 0.3.0 give (research/impl/evidence/P3-040); the
    Spectre line is Spectre Reference 19.1 p.408 (`dev`, `param`)."""
    import spicerack as ps

    def bench(backend):
        c = ps.Circuit("div")
        c.V("in", "in", "0", 1.0)
        c.R("1", "in", "out", 1e3)
        c.R("l", "out", "0", 3e3)
        tb = ps.Testbench(c)
        tb.with_backend(backend)
        return tb

    dc = bench("vacask").dc(Rl=slice(1e3, 3e3, 1e3))
    assert dc.sweep == pytest.approx([1e3, 2e3, 3e3], rel=1e-12)
    assert dc["out"] == pytest.approx([0.5, 2 / 3, 0.75], rel=1e-9)
    tb = bench("spectre")
    tb.add_dc(Rl=slice(1e3, 3e3, 1e3))
    assert "dc1 dc dev=rl param=r start=1k stop=3k step=1k" in tb.netlist("spectre").splitlines()


@pytest.mark.needs("vacask")
def test_p3_043_save_all_reaches_hspice_xyce_and_vacask():
    """ZERO-LEDGER P3-043: `tb.save("all")` asks for every node, and the HSPICE, Xyce and VACASK
    decks lost it. HSPICE writes no waveform without `.option post` (cmdref E-2010.12 p.596:
    "POST=0: Does not output simulation results", the default); Xyce prints nothing without a
    `.print` ("No print specified"), and `V(*)` is every node voltage (RG 7.10, 2.1.31.11) on the
    print of the one kind the deck runs, and a deck of two kinds is refused (W0321); VACASK
    saves every node with no `save` line: no line, no note. Oracle: the two manuals' lines, and
    VACASK's run of the built deck: 1 V over 1k, 1k and 2k is 0.75 V and 0.5 V."""
    import spicerack as ps

    def bench(*saves):
        c = ps.Circuit("div")
        c.V("in", "in", "0", 1.0)
        c.R("1", "in", "mid", 1e3)
        c.R("2", "mid", "out", 1e3)
        c.R("l", "out", "0", 2e3)
        tb = ps.Testbench(c)
        tb.save(*saves)
        return tb

    def written(tb, target):
        text, notes = tb.netlist_with_notes(target)
        return text.splitlines(), [d.code for d in notes]

    op = bench("all")
    op.add_operating_point()
    assert ".option post" in written(op, "hspice")[0]
    lines, codes = written(op, "xyce")
    assert (".print dc V(*)" in lines, codes) == (True, [])
    lines, codes = written(op, "vacask")
    assert ([l for l in lines if l.split()[:1] == ["save"]], codes) == ([], [])
    tran = bench("all")
    tran.add_transient(step_time=1e-6, end_time=2e-6)
    assert ".print tran V(*)" in written(tran, "xyce")[0]
    # Two kinds: Xyce runs one card alone, so the deck is refused (W0321, P1-066).
    both = bench("all")
    both.add_transient(step_time=1e-6, end_time=2e-6)
    both.add_ac(variation="dec", number_of_points=1, start_frequency=1e3, stop_frequency=1e4)
    with pytest.raises(RuntimeError, match="W0321"):
        both.netlist("xyce")
    # A list is left out and noted, as before.
    one = bench("V(out)")
    one.add_operating_point()
    for target in ("xyce", "vacask"):
        lines, codes = written(one, target)
        assert ([l for l in lines if l.startswith(".print")], codes) == ([], ["W0023"]), target
    run = bench("all")
    run.with_backend("vacask")
    got = run.operating_point()
    assert got.nodes == ["in", "mid", "out"]
    assert [got[n] for n in got.nodes] == pytest.approx([1.0, 0.75, 0.5], rel=1e-9)


def _p3_021_fm(backend, offset=0.0):
    """An FM source across 1 kOhm: a 10 kHz carrier modulated at 200 Hz, index 4."""
    import spicerack as ps

    d = ps.Subcircuit("fm", ["in"])
    d.R("1", "in", d.gnd, 1e3)
    tb = ps.Testbench(d)
    tb.with_backend(backend)
    tb.SingleFrequencyFMVoltageSource("in", "in", "0", offset=offset, amplitude=1.0, carrier_frequency=1e4,
                                      modulation_index=4.0, signal_frequency=200.0)
    return tb


def _p3_021_closed(t, offset=0.0):
    """offset + sin(2 pi FC t + MDI sin(2 pi FS t)), by the arguments' names (ngspice-45 manual
    §4.1.5, eq. 4.3, FS its FM; HSPICE UG E-2010.12 p.256, eq. 9)."""
    return offset + math.sin(2 * math.pi * 1e4 * t + 4 * math.sin(2 * math.pi * 200 * t))


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_p3_021_an_fm_source_follows_its_closed_form(backend):
    """ZERO-LEDGER P3-021 (P8-315). ngspice-45's card is `SFFM(VO VA FM MDI FC)`: the builder put
    the carrier third, so ngspice and ESPice ran a 200 Hz carrier modulated at 10 kHz (0.279 V at
    213.3 us for the closed form's 0.948 V). The source drives the node, so every sample is the
    closed form to rounding: ngspice-45 exactly, ESPice 0.3.0 to 2.5e-15, VACASK to 3e-17
    (research/impl/evidence/P3-021/py/fixed.out, today.out). VACASK held before as well: its writer
    copied the slots back the same way."""
    tran = _p3_021_fm(backend).transient(step_time=1e-6, end_time=1e-3, max_time=1e-6)
    time, v = list(tran.time), list(tran["in"])
    assert len(time) > 1000 and time[-1] == pytest.approx(1e-3)
    for t, got in zip(time, v):
        assert got == pytest.approx(_p3_021_closed(t), abs=1e-12), t


def test_p3_021_an_fm_source_with_an_offset_is_ngspices_alone():
    """ngspice-45 `vsrcload.c:272`: SFFM is 0 up to time zero, the operating point included,
    whatever its offset, so the card's default DC value is 0 (`dc=` is "the waveform at t=0").
    SPICE3's form (Xyce RG 7.10 Table 2-59; HSPICE UG p.256; Spectre's and VACASK's FM sine) is
    the offset there: Xyce 7.10 and VACASK ran 0.5 V at time zero
    (research/impl/evidence/P3-021/run.txt), so each such target refuses the source (W0108)."""
    import spicerack as ps

    tb = _p3_021_fm("ngspice", offset=0.5)
    assert "Vin in 0 0 SFFM(0.5 1 200 4 10000)" in tb.netlist_with_notes("ngspice")[0].splitlines()
    assert tb.operating_point()["in"] == 0.0
    tran = tb.transient(step_time=1e-6, end_time=1e-4, max_time=1e-6)
    time, v = list(tran.time), list(tran["in"])
    assert (time[0], v[0]) == (0.0, 0.0)
    for t, got in zip(time[1:], v[1:]):
        assert got == pytest.approx(_p3_021_closed(t, 0.5), abs=1e-12), t
    for target in ("xyce", "hspice", "ltspice", "spectre", "vacask"):
        with pytest.raises(ps.Error, match="W0108"):
            tb.netlist_with_notes(target)
