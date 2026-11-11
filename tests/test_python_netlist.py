"""
Comprehensive netlist generation tests.

Covers: every component type, waveform sources, directives, edge cases,
unit integration, element access, subcircuit instances, ground aliasing.

Oracle: the expected netlist lines, written by hand from each card's syntax
(ngspice manual §2, §3).

Run: maturin develop && pytest tests/test_python_netlist.py -v
"""
import pytest


def ps():
    import spicerack  # unsupported fails, never skips: an unbuilt extension is an error
    return spicerack


def unit():
    from spicerack import unit
    return unit


# ======================================================================
# Circuit creation & basic properties
# ======================================================================

class TestCircuitBasics:
    def test_empty_circuit_has_title_and_end(self):
        c = ps().Circuit("empty")
        netlist = str(c)
        assert ".title empty" in netlist
        assert ".end" in netlist

    def test_repr(self):
        c = ps().Circuit("repr_test")
        assert repr(c) == "Circuit('repr_test')"

    def test_gnd_is_zero(self):
        c = ps().Circuit("gnd")
        assert c.gnd == "0"

    def test_special_chars_in_title(self):
        c = ps().Circuit("Test: special & chars!")
        assert "Test: special & chars!" in str(c)

    def test_multiple_circuits_independent(self):
        c1 = ps().Circuit("c1")
        c2 = ps().Circuit("c2")
        c1.R(
            name="1",
            positive="a",
            negative="b",
            value=1000.0,
        )
        assert "R1" in str(c1)
        assert "R1" not in str(c2)


# ======================================================================
# Passive components
# ======================================================================

class TestPassives:
    def test_resistor_numeric(self):
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative="b",
            value=4700.0,
        )
        assert "R1 a b 4.7k" in str(c)

    def test_resistor_with_unit(self):
        u = unit()
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative="b",
            value=4.7 @ u.u_kOhm,
        )
        assert "R1 a b 4.7k" in str(c)

    def test_resistor_raw_spice(self):
        c = ps().Circuit("t")
        c.R(
            name="load",
            positive="out",
            negative=c.gnd,
            value=0.0,
            raw_spice="1Meg",
        )
        # Read as a number (ngspice manual §2.1.3: scale factors are case
        # blind), written in SPICE's own spelling.
        assert "Rload out 0 1meg" in str(c)

    def test_capacitor_picofarad(self):
        c = ps().Circuit("t")
        c.C(
            name="1",
            positive="a",
            negative="b",
            value=100e-12,
        )
        assert "C1 a b 100p" in str(c)

    def test_capacitor_with_unit(self):
        u = unit()
        c = ps().Circuit("t")
        c.C(
            name="1",
            positive="a",
            negative="b",
            value=100 @ u.u_pF,
        )
        assert "C1 a b 100p" in str(c)

    def test_inductor_microhenry(self):
        c = ps().Circuit("t")
        c.L(
            name="1",
            positive="a",
            negative="b",
            value=2.2e-6,
        )
        assert "L1 a b 2.2u" in str(c)

    def test_mutual_inductance(self):
        c = ps().Circuit("t")
        c.L(
            name="1",
            positive="a",
            negative="b",
            value=1e-6,
        )
        c.L(
            name="2",
            positive="c",
            negative="d",
            value=1e-6,
        )
        c.K("1", "L1", "L2", coupling=0.95)
        netlist = str(c)
        assert "K1 L1 L2 0.95" in netlist

    def test_very_small_value(self):
        c = ps().Circuit("t")
        c.C(
            name="1",
            positive="a",
            negative="b",
            value=1e-15,
        )
        assert "C1 a b 1f" in str(c)

    def test_very_large_value(self):
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative="b",
            value=1e6,
        )
        netlist = str(c)
        assert "R1 a b" in netlist


# ======================================================================
# Sources
# ======================================================================

class TestSources:
    def test_dc_voltage(self):
        c = ps().Circuit("t")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=5.0,
        )
        assert "Vdd vdd 0 5" in str(c)

    def test_dc_voltage_with_unit(self):
        u = unit()
        c = ps().Circuit("t")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3 @ u.u_V,
        )
        assert "Vdd vdd 0 3.3" in str(c)

    def test_dc_current(self):
        c = ps().Circuit("t")
        c.I(
            name="ref",
            positive=c.gnd,
            negative="drain",
            value=100e-6,
        )
        assert "Iref 0 drain 100u" in str(c)

    def test_behavioral_voltage(self):
        c = ps().Circuit("t")
        c.BV(
            name="1",
            positive="out",
            negative=c.gnd,
            expression="V(in)*2 + 0.5",
        )
        netlist = str(c)
        assert "B1 out 0 V=V(in)*2 + 0.5" in netlist

    def test_behavioral_current(self):
        c = ps().Circuit("t")
        c.BI(
            name="1",
            positive="out",
            negative=c.gnd,
            expression="V(ctrl)/1k",
        )
        assert "B1 out 0 I=V(ctrl)/1k" in str(c)


# ======================================================================
# Controlled sources
# ======================================================================

class TestControlledSources:
    def test_vcvs(self):
        c = ps().Circuit("t")
        c.E(
            name="1",
            positive="op",
            negative="om",
            control_positive="ip",
            control_negative="im",
            voltage_gain=100.0,
        )
        assert "E1 op om ip im 100" in str(c)

    def test_vccs(self):
        c = ps().Circuit("t")
        c.G(
            name="m1",
            positive="op",
            negative="om",
            control_positive="ip",
            control_negative="im",
            transconductance=0.01,
        )
        assert "Gm1 op om ip im 0.01" in str(c)

    def test_cccs(self):
        c = ps().Circuit("t")
        c.F(
            name="1",
            positive="op",
            negative="om",
            vsense="Vsense",
            current_gain=50.0,
        )
        assert "F1 op om Vsense 50" in str(c)

    def test_ccvs(self):
        c = ps().Circuit("t")
        c.H(
            name="1",
            positive="op",
            negative="om",
            vsense="Vsense",
            transresistance=500.0,
        )
        assert "H1 op om Vsense 500" in str(c)


# ======================================================================
# Semiconductors
# ======================================================================

class TestSemiconductors:
    def test_diode(self):
        c = ps().Circuit("t")
        c.D(
            name="1",
            anode="anode",
            cathode="cathode",
            model="1N4148",
        )
        assert "D1 anode cathode 1N4148" in str(c)

    def test_bjt_npn(self):
        c = ps().Circuit("t")
        c.Q(
            name="1",
            collector="c",
            base="b",
            emitter="e",
            model="2N2222",
        )
        assert "Q1 c b e 2N2222" in str(c)

    def test_bjt_alias(self):
        c = ps().Circuit("t")
        c.BJT(
            name="1",
            collector="c",
            base="b",
            emitter="e",
            model="BC547",
        )
        assert "Q1 c b e BC547" in str(c)

    def test_mosfet(self):
        c = ps().Circuit("t")
        c.M(
            name="1",
            drain="d",
            gate="g",
            source="s",
            bulk="b",
            model="nmos3p3",
        )
        assert "M1 d g s b nmos3p3" in str(c)

    def test_mosfet_alias(self):
        c = ps().Circuit("t")
        c.MOSFET(
            name="1",
            drain="d",
            gate="g",
            source="s",
            bulk="b",
            model="pmos",
        )
        assert "M1 d g s b pmos" in str(c)

    def test_jfet(self):
        c = ps().Circuit("t")
        c.J(
            name="1",
            drain="d",
            gate="g",
            source="s",
            model="njf_mod",
        )
        assert "J1 d g s njf_mod" in str(c)

    def test_mesfet(self):
        c = ps().Circuit("t")
        c.Z(
            name="1",
            drain="d",
            gate="g",
            source="s",
            model="mes_mod",
        )
        assert "Z1 d g s mes_mod" in str(c)


# ======================================================================
# Switches and transmission lines
# ======================================================================

class TestSwitchesAndTLines:
    def test_voltage_switch(self):
        c = ps().Circuit("t")
        c.S(
            name="1",
            positive="out",
            negative=c.gnd,
            control_positive="cp",
            control_negative="cm",
            model="sw1",
        )
        assert "S1 out 0 cp cm sw1" in str(c)

    def test_current_switch(self):
        c = ps().Circuit("t")
        c.W(
            name="1",
            positive="out",
            negative=c.gnd,
            vcontrol="Vctrl",
            model="csw1",
        )
        assert "W1 out 0 Vctrl csw1" in str(c)

    def test_transmission_line(self):
        c = ps().Circuit("t")
        c.T(
            name="1",
            input_positive="ip",
            input_negative="im",
            output_positive="op",
            output_negative="om",
            Z0=50.0,
            TD=1e-9,
        )
        netlist = str(c)
        assert "T1 ip im op om" in netlist
        assert "Z0=50" in netlist
        assert "TD=" in netlist


# ======================================================================
# Waveform sources
# ======================================================================

class TestWaveformSources:
    def test_sinusoidal_voltage_default(self):
        c = ps().Circuit("t")
        c.SinusoidalVoltageSource(
            name="1",
            positive="in",
            negative=c.gnd,
        )
        netlist = str(c)
        assert "V1 in 0" in netlist
        assert "SIN(" in netlist

    def test_sinusoidal_voltage_custom(self):
        c = ps().Circuit("t")
        c.SinusoidalVoltageSource(
            name="1",
            positive="in",
            negative=c.gnd,
            dc_offset=1.0,
            offset=0.5,
            amplitude=2.0,
            frequency=5000.0,
        )
        netlist = str(c)
        assert "SIN(" in netlist

    def test_pulse_voltage(self):
        c = ps().Circuit("t")
        c.PulseVoltageSource(
            name="clk",
            positive="clk",
            negative=c.gnd,
            initial_value=0.0,
            pulsed_value=1.8,
            pulse_width=5e-9,
            period=10e-9,
            rise_time=0.1e-9,
            fall_time=0.1e-9,
        )
        netlist = str(c)
        assert "Vclk clk 0" in netlist
        assert "PULSE(" in netlist

    def test_pulse_delay_and_dc_value(self):
        # The delay reaches the card, and the DC value is the pulse's t=0
        # value, as a SPICE card with no DC value means: 1.2 V, not 0 V.
        c = ps().Circuit("t")
        c.PulseVoltageSource(name="clk", positive="clk", negative=c.gnd,
                             initial_value=1.2, pulsed_value=0.0, delay=3e-9)
        assert "Vclk clk 0 1.2 PULSE(1.2 0 0.000000003 " in str(c)

    def test_waveform_takes_expressions(self):
        # A waveform parameter may be a .param expression; with no DC value
        # the source starts at the (expression) t=0 value.
        c = ps().Circuit("t")
        c.PulseVoltageSource(name="clk", positive="clk", negative=c.gnd,
                             initial_value="vlo", pulsed_value="{vdd}", rise_time="tr*2")
        c.PieceWiseLinearVoltageSource("w", "w", c.gnd, values=[(0, 0), ("tr", "vdd")])
        n = str(c)
        assert "Vclk clk 0 {vlo} PULSE({vlo} {vdd} 0 {tr*2} " in n
        assert "PWL(0 0 {tr} {vdd})" in n

    def test_device_params_and_bjt_terminals(self):
        # D/Q/J/Z take instance parameters as M does; Q takes a substrate
        # and a thermal node after the emitter.
        c = ps().Circuit("t")
        c.D(name="1", anode="a", cathode=c.gnd, model="dm", area=2)
        c.Q(name="1", collector="c", base="b", emitter="e", model="qv", substrate="s", thermal="dt", m=3)
        c.J(name="1", drain="d", gate="g", source="s", model="jm", area=1.5)
        netlist = str(c)
        assert "D1 a 0 dm area=2" in netlist
        assert "Q1 c b e s dt qv m=3" in netlist
        assert "J1 d g s jm area=1.5" in netlist

    def test_waveform_sources_take_dc_ac_and_positional_nodes(self):
        # PySpice-style positional name/nodes; dc= and ac= beside a waveform;
        # the sine's delay/damping/phase; EXP/SFFM/AM; PWL current on a Subcircuit.
        c = ps().Circuit("t")
        c.PulseVoltageSource("p", "a", c.gnd, initial_value=0.0, pulsed_value=1.0, dc=0.5, ac=1.0)
        c.SinusoidalVoltageSource("s", "b", c.gnd, offset=0.2, amplitude=1.0, frequency=1e3, delay=1e-3, damping_factor=10.0, phase=90.0)
        c.ExponentialCurrentSource("e", "d", c.gnd, initial_value=0.0, pulsed_value=1e-3, rise_time_constant=1e-6, fall_delay_time=5e-6, fall_time_constant=1e-6)
        c.SingleFrequencyFMVoltageSource("f", "f", c.gnd, offset=0.0, amplitude=1.0, carrier_frequency=1e6, modulation_index=0.5, signal_frequency=1e3)
        c.AmplitudeModulatedVoltageSource("m", "m", c.gnd, offset=1.0, amplitude=0.5, modulating_frequency=1e3, carrier_frequency=1e6)
        c.PieceWiseLinearCurrentSource("w", "w", c.gnd, values=[(0, 0), (1e-6, 1e-3)])
        n = str(c)
        assert "Vp a 0 500m AC 1 PULSE(" in n
        assert "Vs b 0 200m SIN(0.2 1 1000 0.001 10 90)" in n  # DC defaults to the offset
        assert "Ie d 0 0 EXP(0 0.001 0 0.000001 0.000005 0.000001)" in n
        assert "SFFM(0 1 1000000 0.5 1000)" in n and "AM(0 0.5 0.5 1000 1000000)" in n
        assert "Iw w 0 0 PWL(" in n
        with pytest.raises(TypeError):
            c.PulseVoltageSource("x", "x", c.gnd, dc=1.0, dc_offset=1.0)

    def test_testbench_builds_tf_sens_pz_disto(self):
        dut = ps().Subcircuit("d")
        dut.R("1", "in", "out", 1e3)
        tb = ps().Testbench(dut)
        tb.V("in", "in", "0", 1.0, ac=1.0)
        tb.add_tf("v(out)", "Vin")
        tb.add_dc_sensitivity("v(out)")
        tb.add_polezero("in", "0", "out", "0", "vol", "pz")
        tb.add_distortion("dec", 10, 100.0, 1e6)
        n = tb.netlist("ngspice")
        for card in (".tf v(out) Vin", ".sens v(out)", ".pz in 0 out 0 vol pz", ".disto dec 10 100 1meg"):
            assert card in n, n
        # Keywords are parsed at the call: any case reads, a wrong one raises.
        tb.add_polezero("in", "0", "out", "0", "CUR", "Zer")
        assert ".pz in 0 out 0 cur zer" in tb.netlist("ngspice")
        with pytest.raises(ValueError, match="unknown transfer type 'volt'"):
            tb.add_polezero("in", "0", "out", "0", "volt", "pz")
        with pytest.raises(ValueError, match="unknown Spectre analysis 'op'"):
            tb.add_spectre_sweep("r", 1.0, 2.0, 1.0, "op1", "op")
        with pytest.raises(ValueError, match="unknown sweep 'decade'"):
            ps().Circuit("t").simulator().step_sweep("r", 1.0, 10.0, 2.0, "decade")

    def test_pwl_repeat_and_delay_reach_the_card(self):
        # PySpice's repeat_time/delay_time are ngspice's r=/td= (manual 4.1.4),
        # on the builders and on a Testbench; LTspice has no such card.
        c = ps().Circuit("t")
        c.PieceWiseLinearVoltageSource("w", "w", c.gnd, values=[(0, 0), (1e-3, 1), (2e-3, 0)],
                                       repeat_time=0, delay_time=5e-4)
        assert "Vw w 0 0 PWL(0 0 0.001 1 0.002 0) r=0 td=0.0005" in str(c)
        dut = ps().Subcircuit("d")
        dut.R("1", "w", "0", 1e3)
        tb = ps().Testbench(dut)
        tb.PieceWiseLinearVoltageSource("w", "w", "0", values=[(0, 0), (1e-3, 1)], repeat_time="tr")
        assert "PWL(0 0 0.001 1) r={tr}" in tb.netlist("ngspice")
        with pytest.raises(Exception, match="PWL"):
            tb.netlist("ltspice")

    def test_rf_ports_and_s_param_on_ngspice(self):
        # ngspice manual 4.1.11: `portnum n z0 x`. A matched T pad for K = 2 in
        # 50 ohm (arms 50/3, shunt 200/3) has |S11| = 0 and |S21| = 1/K = 0.5.
        c = ps().Circuit("pad")
        c.V("1", "in", c.gnd, 0.0, ac=1.0, portnum=1)
        c.V("2", "out", c.gnd, 0.0, ac=1.0, portnum=2, z0=50.0)
        c.R("1", "in", "mid", 50 / 3)
        c.R("2", "mid", "out", 50 / 3)
        c.R("3", "mid", c.gnd, 200 / 3)
        assert "V1 in 0 0 AC 1 portnum 1 z0 50" in str(c)
        sp = c.simulator(simulator="ngspice").s_param("lin", 3, 1e3, 3e3)
        assert all(abs(m) < 1e-9 for m in sp.magnitude("S_1_1"))
        assert all(abs(m - 0.5) < 1e-9 for m in sp.magnitude("S_2_1"))
        dut = ps().Subcircuit("d")
        dut.R("1", "a", "0", 50.0)
        tb = ps().Testbench(dut)
        tb.V("p", "a", "0", 0.0, ac=1.0, portnum=1, z0=75.0)
        assert "Vp a 0 0 AC 1 portnum 1 z0 75" in tb.netlist("ngspice")
        with pytest.raises(Exception, match="RF port"):
            tb.netlist("ltspice")

    def test_trnoise_and_trrandom_sources(self):
        # ngspice manual 4.1.7/4.1.8. Both add to the DC value (ngspice 45
        # holds `3 TRRANDOM(.. 3)` at 6 V), so the default DC is 0. ngspice only.
        c = ps().Circuit("t")
        c.TransientNoiseVoltageSource("n", "n", c.gnd, noise_rms=0.1, time_step=1e-6)
        c.TransientNoiseCurrentSource("rts", "r", c.gnd, noise_rms=1e-3, time_step=1e-6, flicker_exponent=1.0,
                                      flicker_rms=1e-4, rts_amplitude=15e-3, rts_capture_time=22e-6, rts_emission_time=50e-6)
        c.RandomVoltageSource("u", "u", c.gnd, random_type="uniform", duration=1e-6, parameter1=2, parameter2=3)
        c.RandomCurrentSource("g", "g", c.gnd, random_type="Gaussian", duration="ts")
        n = str(c)
        assert "Vn n 0 0 TRNOISE(0.1 0.000001 0 0)\n" in n
        assert "Irts r 0 0 TRNOISE(0.001 0.000001 1 0.0001 0.015 0.000022 0.00005)" in n
        assert "Vu u 0 0 TRRANDOM(1 0.000001 0 2 3)" in n
        assert "Ig g 0 0 TRRANDOM(2 {ts} 0 1 0)" in n
        with pytest.raises(ValueError, match="random type 'normal'"):
            c.RandomVoltageSource("x", "x", c.gnd, random_type="normal", duration=1e-6)
        dut = ps().Subcircuit("d")
        dut.R("1", "n", "0", 1e3)
        tb = ps().Testbench(dut)
        tb.TransientNoiseVoltageSource("n", "n", "0", noise_rms=0.1, time_step=1e-6)
        assert "TRNOISE(" in tb.netlist("ngspice")
        with pytest.raises(Exception, match="TRNOISE"):
            tb.netlist("espice")

    def test_pwl_voltage(self):
        c = ps().Circuit("t")
        c.PieceWiseLinearVoltageSource(
            name="1",
            positive="in",
            negative=c.gnd,
            values=[(0, 0), (1e-6, 1.0), (2e-6, 0.5), (3e-6, 0)],
        )
        netlist = str(c)
        assert "PWL(" in netlist

    def test_sinusoidal_current(self):
        c = ps().Circuit("t")
        c.SinusoidalCurrentSource(
            name="1",
            positive="in",
            negative=c.gnd,
            dc_offset=0.0,
            offset=0.0,
            amplitude=1e-3,
            frequency=10e3,
        )
        netlist = str(c)
        assert "I1 in 0" in netlist
        assert "SIN(" in netlist

    def test_pulse_current(self):
        c = ps().Circuit("t")
        c.PulseCurrentSource(
            name="1",
            positive="in",
            negative=c.gnd,
            initial_value=0.0,
            pulsed_value=1e-3,
            pulse_width=1e-6,
            period=2e-6,
        )
        netlist = str(c)
        assert "I1 in 0" in netlist
        assert "PULSE(" in netlist


# ======================================================================
# Directives
# ======================================================================

class TestDirectives:
    def test_model_with_params(self):
        c = ps().Circuit("t")
        c.model("nmos1", "NMOS", LEVEL=1, VTO=0.7, KP=110e-6)
        netlist = str(c)
        assert ".model nmos1 NMOS" in netlist
        assert "LEVEL=1" in netlist
        assert "VTO=0.7" in netlist

    def test_model_no_params(self):
        c = ps().Circuit("t")
        c.model("simple_d", "D")
        assert ".model simple_d D" in str(c)

    def test_include(self):
        c = ps().Circuit("t")
        c.include("/pdk/sky130/models.spice")
        assert ".include /pdk/sky130/models.spice" in str(c)

    def test_lib(self):
        c = ps().Circuit("t")
        c.lib("/pdk/models.lib", "tt")
        assert ".lib /pdk/models.lib tt" in str(c)

    def test_parameter(self):
        c = ps().Circuit("t")
        c.parameter("width", "1u")
        assert ".param width=1u" in str(c)

    def test_multiple_parameters(self):
        c = ps().Circuit("t")
        c.parameter("vdd", "3.3")
        c.parameter("length", "100n")
        netlist = str(c)
        assert ".param vdd=3.3" in netlist
        assert ".param length=100n" in netlist

    def test_subcircuit_instance(self):
        c = ps().Circuit("t")
        c.X("1", "NAND2", "a", "b", "out", "vdd", "gnd")
        netlist = str(c)
        assert "X1" in netlist
        assert "NAND2" in netlist

    def test_subcircuit_gnd_conversion(self):
        """Nodes named 'gnd' should become '0'."""
        c = ps().Circuit("t")
        c.X("1", "Buf", "in", "out", "gnd")
        assert "0" in str(c)


# ======================================================================
# Element access
# ======================================================================

class TestElementAccess:
    def test_getitem_exists(self):
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative="b",
            value=1e3,
        )
        result = c["1"]
        assert "R1" in result

    def test_getitem_not_found_raises(self):
        c = ps().Circuit("t")
        with pytest.raises(KeyError):
            c["nonexistent"]

    def test_element_method(self):
        c = ps().Circuit("t")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        result = c.element("dd")
        assert "Vdd" in result

    def test_access_after_multiple_adds(self):
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative="b",
            value=1e3,
        )
        c.C(
            name="2",
            positive="b",
            negative=c.gnd,
            value=1e-12,
        )
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=5.0,
        )
        assert "R1" in c["1"]
        assert "C2" in c["2"]
        assert "Vdd" in c["dd"]


# ======================================================================
# Initial conditions: an expression value, and inside a subcircuit (P8-006)
# ======================================================================

class TestInitialConditions:
    def test_p8_006_a_value_is_a_number_or_an_expression(self):
        mod = ps()
        d = mod.Subcircuit("rc", ["in", "out"], r="1k")
        d.R("1", "in", "out", "{r}")
        d.C("1", "out", d.gnd, 1e-9)
        tb = mod.Testbench(d)
        tb.V("in", "in", "0", 1.0)
        tb.initial_condition(out="{r/2000}")
        tb.node_set(out=0.25)
        tb.add_transient(1e-6, 1e-5)
        lines = tb.netlist("ngspice").splitlines()
        # The cards ngspice-45 ran (evidence/P8-006/ng/topexpr.cir, top.cir).
        assert ".ic V(out)={r/2000}" in lines
        assert ".nodeset V(out)=0.25" in lines
        # Spectre's statement (Spectre Reference, `ic`): the bare expression.
        assert "ic out=r/2000" in tb.netlist("spectre").splitlines()

    def test_p8_006_a_subcircuit_holds_its_own_initial_conditions(self):
        c = ps().Subcircuit("buf", ["a", "b"])
        c.R("1", "a", "n", 1e3)
        c.C("1", "n", c.gnd, 1e-9)
        c.initial_condition(n=0.5)
        c.node_set(n="vdd/2")
        lines = c.to_source().splitlines()
        # Inside the `.subckt`, where ngspice-45 applies them to each
        # instance (evidence/P8-006/ng/sub.cir).
        assert lines.index(".subckt buf a b") < lines.index(".ic V(n)=0.5") < lines.index(".ends buf")
        assert ".nodeset V(n)={vdd/2}" in lines

    def test_p8_006_a_value_that_is_no_number_or_string_adds_nothing(self):
        c = ps().Subcircuit("buf", ["a", "b"])
        with pytest.raises(TypeError):
            c.initial_condition(n=0.5, m=[1.0])
        assert ".ic" not in c.to_source()


# ======================================================================
# Ground node aliasing
# ======================================================================

class TestGroundAliasing:
    def test_gnd_string_becomes_zero(self):
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative="gnd",
            value=1e3,
        )
        assert "R1 a 0" in str(c)

    def test_zero_stays_zero(self):
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative="0",
            value=1e3,
        )
        assert "R1 a 0" in str(c)

    def test_testbench_stimulus_maps_gnd_like_circuit(self):
        # A Testbench is a top level, so `gnd` is the global ground there
        # too, on every stimulus method and both nodes.
        dut = ps().Subcircuit("d")
        dut.R("1", "a", "0", 1e3)
        tb = ps().Testbench(dut)
        tb.V("1", "a", "gnd", 1.0)
        tb.I("1", "GND", "a", 1e-3)
        tb.R("2", "a", "Gnd", 1e3)
        tb.C("2", "gnd", "a", 1e-12)
        tb.PulseVoltageSource("p", "b", "gnd")
        n = tb.netlist("ngspice")
        for card in ("V1 a 0 1", "I1 0 a 1m", "R2 a 0 1k", "C2 0 a 1p", "Vp b 0 0 PULSE("):
            assert card in n, n
        assert "gnd" not in n.lower(), n

    def test_circuit_gnd_property(self):
        c = ps().Circuit("t")
        c.R(
            name="1",
            positive="a",
            negative=c.gnd,
            value=1e3,
        )
        assert "R1 a 0" in str(c)


# ======================================================================
# Complex realistic circuits
# ======================================================================

class TestRealisticCircuits:
    def test_voltage_divider(self):
        c = ps().Circuit("vdiv")
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
        netlist = str(c)
        assert "Vdd vdd 0 3.3" in netlist
        assert "R1 vdd out 10k" in netlist
        assert "R2 out 0 10k" in netlist

    def test_diff_pair(self):
        c = ps().Circuit("dp")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=3.3,
        )
        c.R(
            name="d1",
            positive="vdd",
            negative="vo_m",
            value=5e3,
        )
        c.R(
            name="d2",
            positive="vdd",
            negative="vo_p",
            value=5e3,
        )
        c.MOSFET(
            name="1",
            drain="vo_m",
            gate="vi_m",
            source="tail",
            bulk=c.gnd,
            model="nmos",
        )
        c.MOSFET(
            name="2",
            drain="vo_p",
            gate="vi_p",
            source="tail",
            bulk=c.gnd,
            model="nmos",
        )
        c.I(
            name="ss",
            positive=c.gnd,
            negative="tail",
            value=200e-6,
        )
        netlist = str(c)
        assert "M1" in netlist
        assert "M2" in netlist
        assert "Iss" in netlist

    def test_bridge_rectifier(self):
        c = ps().Circuit("br")
        c.model("D1N", "D", IS=2.52e-9)
        c.SinusoidalVoltageSource(
            name="in",
            positive="ac_p",
            negative="ac_m",
            amplitude=12.0,
            frequency=60.0,
        )
        c.D(
            name="1",
            anode="ac_p",
            cathode="out_p",
            model="D1N",
        )
        c.D(
            name="2",
            anode="out_m",
            cathode="ac_p",
            model="D1N",
        )
        c.D(
            name="3",
            anode="ac_m",
            cathode="out_p",
            model="D1N",
        )
        c.D(
            name="4",
            anode="out_m",
            cathode="ac_m",
            model="D1N",
        )
        c.C(
            name="1",
            positive="out_p",
            negative="out_m",
            value=100e-6,
        )
        c.R(
            name="load",
            positive="out_p",
            negative="out_m",
            value=1e3,
        )
        c.V(
            name="ref",
            positive="out_m",
            negative=c.gnd,
            value=0.0,
        )
        netlist = str(c)
        assert "D1" in netlist
        assert "D2" in netlist
        assert "D3" in netlist
        assert "D4" in netlist
        assert ".model D1N D" in netlist

    def test_ring_oscillator_netlist(self):
        c = ps().Circuit("ro")
        c.V(
            name="dd",
            positive="vdd",
            negative=c.gnd,
            value=1.8,
        )
        c.model("nm", "NMOS", LEVEL=1, VTO=0.4)
        c.model("pm", "PMOS", LEVEL=1, VTO=-0.4)
        for i in range(1, 4):
            prev = f"n{(i-2)%3+1}" if i > 1 else "n3"
            out = f"n{i}"
            c.MOSFET(
                name=f"{i}n",
                drain=out,
                gate=prev,
                source=c.gnd,
                bulk=c.gnd,
                model="nm",
            )
            c.MOSFET(
                name=f"{i}p",
                drain=out,
                gate=prev,
                source="vdd",
                bulk="vdd",
                model="pm",
            )
        netlist = str(c)
        for i in range(1, 4):
            assert f"M{i}n" in netlist
            assert f"M{i}p" in netlist

    def test_many_components(self):
        """Stress test: 100 resistors in series."""
        c = ps().Circuit("stress")
        c.V(
            name="in",
            positive="n0",
            negative=c.gnd,
            value=10.0,
        )
        for i in range(100):
            c.R(
                name=str(i+1),
                positive=f"n{i}",
                negative=f"n{i+1}",
                value=100.0,
            )
        c.R(
            name="load",
            positive="n100",
            negative=c.gnd,
            value=1e3,
        )
        netlist = str(c)
        assert "R1 n0 n1 100" in netlist
        assert "R100 n99 n100 100" in netlist
        assert "Rload n100 0" in netlist


# ======================================================================
# A definition nested in a subcircuit (P8-013; ngspice-45 manual §2.6.1)
# ======================================================================

def wrapped(name, ohms):
    """`leaf` (one resistor of `ohms`) defined inside subcircuit `name`."""
    leaf = ps().Subcircuit("leaf", ["p", "q"])
    leaf.R("1", "p", "q", ohms)
    wrap = ps().Subcircuit(name, ["a", "b"])
    wrap.subcircuit(leaf)
    wrap.X("1", "leaf", "a", "b")
    return wrap


class TestNestedSubcircuit:
    def test_p8_013_a_nested_definition_is_written_under_its_parents_name(self):
        c = ps().Circuit("n")
        c.subcircuit(wrapped("wrap", 1e3))
        c.X("1", "wrap", "in", c.gnd)
        lines = str(c).splitlines()
        # `render/read/blocks.rs::close`: hoisted as `<parent>__<name>`, the
        # parent's instances renamed (golden record: model-scope-5.sp,
        # `.subckt sub2__sub3 4 5`, `X31 41a 41b sub2__sub3`).
        assert ".subckt wrap__leaf p q" in lines
        assert "X1 a b wrap__leaf" in lines
        assert ".subckt leaf p q" not in lines

    def test_p8_013_two_parents_each_keep_their_own(self):
        # Strictly local (manual §2.6.1): the same name in two parents is
        # two definitions.
        c = ps().Circuit("n")
        c.subcircuit(wrapped("one", 1e3))
        c.subcircuit(wrapped("two", 2e3))
        lines = str(c).splitlines()
        at = {n: lines.index(f".subckt {n}__leaf p q") for n in ("one", "two")}
        assert lines[at["one"] + 1] == "R1 p q 1k"
        assert lines[at["two"] + 1] == "R1 p q 2k"

    def test_p8_013_the_store_keeps_a_nested_definition(self):
        wrap = wrapped("wrap", 1e3)
        text = wrap.to_source()
        assert ".subckt leaf p q\nR1 p q 1k\n.ends leaf\n.ends wrap\n" in text
        again = ps().Subcircuit.from_source(text)
        assert again.to_source() == text

    def test_p8_013_a_childs_files_move_up_and_a_verilog_block_is_refused(self):
        leaf = ps().Subcircuit("leaf", ["p"])
        leaf.include("/m/x.inc")
        wrap = ps().Subcircuit("wrap", ["a"])
        wrap.subcircuit(leaf)
        text = wrap.to_source()
        assert text.count(".include /m/x.inc") == 1
        assert text.index(".include /m/x.inc") < text.index(".subckt leaf p")
        bad = ps().Subcircuit("bad", ["p"])
        bad.include("/m/y.inc")
        bad.verilog(source="module inv(input a, output y); assign y = ~a; endmodule",
                    instance_name="u1", connections={"a": "p", "y": "q"})
        with pytest.raises(ValueError, match="verilog"):
            wrap.subcircuit(bad)
        assert wrap.to_source() == text  # nothing of `bad` was kept


# ======================================================================
# A capacitor given by its charge (P8-015; ngspice manual §3.3.9)
# ======================================================================

class TestChargeCapacitor:
    def test_p8_015_charge_is_the_q_card(self):
        c = ps().Circuit("q")
        c.C("1", "a", c.gnd, charge="1n*v(a)")
        # The manual's form, as the ngspice writer spells a charge capacitor
        # (`render/spice/cards.rs::charge_card`: `Q='…'`).
        assert "C1 a 0 Q='1n*v(a)'" in str(c).splitlines()

    def test_p8_015_one_pair_of_braces_is_dropped(self):
        c = ps().Circuit("q")
        c.C("1", "a", c.gnd, charge="{1n*v(a)}")
        assert "C1 a 0 Q='1n*v(a)'" in str(c).splitlines()

    def test_p8_015_value_and_charge_exclude_each_other(self):
        c = ps().Circuit("q")
        with pytest.raises(ValueError, match="exactly one"):
            c.C("1", "a", c.gnd, 1e-9, charge="1n*v(a)")
        with pytest.raises(ValueError, match="exactly one"):
            c.C("1", "a", c.gnd)
        assert "C1" not in str(c)


@pytest.mark.needs("ngspice")
def test_p3_041_api_python_values_written_text():
    """`True` is SPICE's bare flag, `False` and `None` write nothing, and
    `R(..., value, raw_spice="tc=1e-3")` keeps its value. Oracles: the cards
    of the ngspice-45 manual (7.1 `D n+ n- mname <off>`, 11.1.1 `.options
    klu`, 3.3.1 `R n+ n- value <tc1=..>`), and ngspice's run of the deck: a
    1k/1k divider under 1 V is 0.5 V (closed form). ngspice-45 runs today's
    `R2 out 0 tc=1e-3` as 1 mOhm, 1e-6 V (research/impl/evidence/P3-041)."""
    import spicerack as ps

    dut = ps.Subcircuit("d", ["a", "out"])
    dut.model("dn", "D", IS=1e-14)
    dut.D("1", "0", "a", model="dn", off=True, area=None)
    dut.D("2", "0", "a", model="dn", off=False)
    dut.R("1", "a", "out", 1e3)
    dut.R("2", "out", "0", 1e3, raw_spice="tc=1e-3")
    dut.R("3", "a", "0", 5.0, raw_spice="1meg tc=1e-3")
    dut.R("4", "a", "0", 5.0, raw_spice="r=1meg")
    dut.R("5", "a", "0", 1e6, raw_spice="")
    tb = ps.Testbench(dut)
    tb.with_backend("ngspice")
    tb.V("1", "a", "0", 1.0)
    tb.options(KLU=True, RELTOL=1e-4)
    lines = tb.netlist("ngspice").splitlines()
    for card in ("D1 0 a dn off", "D2 0 a dn", "R2 out 0 1k tc=1e-3", "R3 a 0 1meg tc=1e-3", "R5 a 0 1meg",
                 ".options KLU RELTOL=0.0001"):
        assert card in lines, (card, lines)
    # raw_spice that holds its own value (a number, or `r=`) still replaces `value`.
    assert not any(l.startswith(("R3 a 0 5", "R4 a 0 5")) for l in lines), lines
    assert not any("True" in l or "False" in l or "None" in l for l in lines), lines
    # 1e-6: gmin and the reverse diodes shift the divider by less than 1e-9.
    assert tb.operating_point()["out"] == pytest.approx(0.5, rel=1e-6)


# ======================================================================
# Portable options (P8-004, D17)
# ======================================================================

# Oracle of the option table: the headwords of ngspice-45 manual §11.1.1-
# §11.1.6 (`.options`), lowercased, one line per section; `rseries` and
# `cshunt` are §11.1.2.1's. Copied from the manual, not from the code.
NGSPICE_OPTIONS = """
sparse klu acct noacct noinit list nomod nopage node norefvalue opts seed seedinfo temp tnom warn maxwarns savecurrents
abstol gmin gminsteps itl1 itl2 keepopinfo noopiter pivrel pivtol reltol rshunt vntol rseries cshunt
noopac
autostop chgtol convstep convabsstep interp itl3 itl4 itl5 itl6 maxevtiter maxopalter maxord method noopalter ramptime srcsteps trtol xmu
diode_cj0 diode_rser badmos3 defad defas defl defw scale
trytocompact
""".split()

# One simulator's own option each (api-parity Q3-Q5): a Xyce package (RG
# `.OPTIONS TIMEINT`), VACASK's `tran_method`, LTspice's `cshuntintern`
# (its `.OPTIONS` help page), HSPICE's `POST`, Spectre's `vabstol` (Spectre
# Reference 19.1 `options` #3); then the names SpiceRack uses that are no
# `.options` name of ngspice, and a misspelling.
NOT_PORTABLE = [
    "timeint", "tran_method", "cshuntintern", "post", "vabstol",
    "max_iterations", "ngbehavior", "nfreqs", "reltoll",
]


def option_bench():
    dut = ps().Subcircuit("test")
    dut.R(name="1", positive="a", negative="b", value=100.0)
    return ps().Testbench(dut)


def option_lines(tb):
    return [l for l in tb.netlist("ngspice").splitlines() if l.lower().startswith(".option")]


class TestPortableOptions:
    def test_p8_004_true_is_a_bare_flag(self):
        tb = option_bench()
        tb.options(reltol="1e-6", klu=True)
        assert option_lines(tb) == [".options reltol=1e-6 klu"]

    def test_p8_004_every_ngspice_option_is_portable_in_any_case(self):
        tb = option_bench()
        for key in NGSPICE_OPTIONS:
            tb.options(**{key: 1})
            tb.options(**{key.upper(): 1})

    @pytest.mark.parametrize("key", NOT_PORTABLE)
    def test_p8_004_another_simulators_option_raises(self, key):
        tb = option_bench()
        with pytest.raises(ValueError, match="not a portable option"):
            tb.options(reltol=1e-4, **{key: 1})
        # Fallible work before the commit: `reltol` was not kept either.
        assert option_lines(tb) == []

    @pytest.mark.parametrize("value", [None, False])
    def test_p8_004_none_and_false_set_nothing(self, value):
        tb = option_bench()
        with pytest.raises(ValueError, match="klu"):
            tb.options(klu=value)
        assert option_lines(tb) == []

    def test_p8_004_circuit_and_its_simulator_share_the_rule(self):
        c = ps().Circuit("shared")
        c.R(name="1", positive="a", negative=c.gnd, value=100.0)
        c.options(noacct=True)
        assert ".options noacct" in str(c).splitlines()
        with pytest.raises(ValueError, match="not a portable option"):
            c.options(tran_method="trap")
        with pytest.raises(ValueError, match="not a portable option"):
            c.simulator().options(tran_method="trap")


# ======================================================================
# K names its inductors in full (P8-017; ngspice manual §3.3.12)
# ======================================================================

def coupled():
    c = ps().Circuit("k")
    c.L("1", "a", "b", 1e-6)
    c.L("2", "c", "d", 1e-6)
    c.R("1", "b", "d", 1.0)
    return c


class TestCouplingNames:
    def test_p8_017_k_takes_the_inductors_full_names(self):
        c = coupled()
        c.K("1", "L1", "L2", coupling=0.95)
        assert "K1 L1 L2 0.95" in str(c).splitlines()

    def test_p8_008_k_couples_three_or_more_inductors(self):
        c = coupled()
        c.L("3", "e", "f", 1e-6)
        c.K("1", "L1", "L2", "L3", coupling=0.5)
        # One card for every pair: ngspice-45 gives it the numbers of the
        # three pairwise K (evidence/P8-008/ng/k3one.log, k3pairs.log).
        assert "K1 L1 L2 L3 0.5" in c.to_source().splitlines()

    @pytest.mark.parametrize("coupling", ["k", "{k}"])
    def test_p8_008_k_coupling_is_an_expression(self, coupling):
        c = coupled()
        c.parameter("k", "0.5")
        c.K("1", "L1", "L2", coupling=coupling)
        # One pair of braces, whether the caller wrote them or not
        # (evidence/P8-008/ng/kx.cir is the card ngspice-45 ran).
        assert "K1 L1 L2 {k}" in c.to_source().splitlines()

    @pytest.mark.parametrize("names", [
        ("1", "2"),          # the old spelling: the names without their letter
        ("L1", "L9"),        # no such inductor
        ("L1", "R1"),        # an element, but no inductor
        ("L1",),             # one
        ("L1", "L2", "L1"),  # one inductor named twice
    ])
    def test_p8_017_k_refuses_what_is_no_pair_of_its_inductors(self, names):
        c = coupled()
        with pytest.raises(ValueError, match="^K1: "):
            c.K("1", *names, coupling=0.5)
        assert not any(l.startswith("K") for l in str(c).splitlines())


# ======================================================================
# A function inside a subcircuit (P8-012; ngspice-45 manual §2.12)
# ======================================================================

def scaling_cell():
    cell = ps().Subcircuit("cell", ["a", "b"], k="2")
    cell.add_function("scaled", ["x"], "x*k")
    cell.R("1", "a", "b", "{scaled(500)}")
    return cell


class TestSubcircuitFunction:
    def test_p8_012_a_function_over_a_subcircuit_parameter_stays_in_it(self):
        c = ps().Circuit("f")
        c.subcircuit(scaling_cell())
        c.X("1", "cell", "in", c.gnd)
        lines = str(c).splitlines()
        # The ngspice writer's definition: head, functions, cards (golden
        # record xyce/PARAMS2/Params_A8.cir: `.subckt ResSub 1 2 PARAMS:
        # RES=5k`, `.func frobnitz(x) {5*x}`, `R1 1 2 {frobnitz(RES)}`).
        at = lines.index(".subckt cell a b PARAMS: k=2")
        assert lines[at + 1:at + 4] == [".func scaled(x) {x*k}", "R1 a b {scaled(500)}", ".ends cell"]
        assert sum(l.startswith(".func") for l in lines) == 1

    def test_p8_012_several_arguments(self):
        cell = ps().Subcircuit("cell", ["a"])
        cell.add_function("par", ["x", "y"], "x*y/(x+y)")
        assert ".func par(x, y) {x*y/(x+y)}" in cell.to_source().splitlines()

    def test_p8_012_the_store_keeps_the_function(self):
        text = scaling_cell().to_source()
        assert ps().Subcircuit.from_source(text).to_source() == text


# ======================================================================
# E/G sources given by an expression (P8-010; ngspice-45 manual §5.2.2, §5.3.2)
# ======================================================================

class TestNonLinearSources:
    def test_p8_010_the_cards_are_e_and_g_value(self):
        sub = ps().Subcircuit("s", ["a"])
        sub.NonLinearVoltageSource("sq", "x", sub.gnd, expression="V(a)*V(a)")
        sub.NonLinearCurrentSource("lk", "y", sub.gnd, expression="{V(a)*1m}")  # braces given
        lines = sub.to_source().splitlines()
        assert "Esq x 0 value={V(a)*V(a)}" in lines
        assert "Glk y 0 value={V(a)*1m}" in lines

    def test_p8_010_ngspice_gets_the_behavioural_source(self):
        c = ps().Circuit("nl")
        c.V("in", "in", c.gnd, 1.0)
        c.NonLinearVoltageSource("sq", "x", c.gnd, expression="V(in)*V(in)")
        c.NonLinearCurrentSource("lk", "y", c.gnd, expression="V(in)*1m")
        lines = str(c).splitlines()
        # `render/spice/cards.rs::behavioural_card`: `B<name> n+ n- V=expr` /
        # `I=expr` (golden record xyce/POWER/power_polarity_b_e_f_g_h_trap.cir:
        # `EV1 … value=` is written `BV1 ev1_1 0 V=5V*SQRT(V(bv1_1,0))`).
        assert "Bsq x 0 V=V(in)*V(in)" in lines
        assert "Blk y 0 I=V(in)*1m" in lines

    def test_p8_010_expression_is_a_keyword(self):
        sub = ps().Subcircuit("s", ["a"])
        with pytest.raises(TypeError):
            sub.NonLinearVoltageSource("1", "a", sub.gnd, "V(a)")
        with pytest.raises(TypeError):
            sub.NonLinearCurrentSource("1", "a", sub.gnd)


# ======================================================================
# The analyses only Spectre has, one door (P8-025; Spectre Reference 19.1 ch.3)
# ======================================================================

def rc_bench():
    dut = ps().Subcircuit("rc", ["in", "out"])
    dut.R("1", "in", "out", 1e3)
    dut.C("1", "out", dut.gnd, 1e-9)
    tb = ps().Testbench(dut)
    tb.V("in", "in", "0", 1.0)
    return tb


class TestSpectreAnalysis:
    def test_p8_025_the_statement_is_the_manuals_form(self):
        # Spectre Reference 19.1 p.73: `dcmm1 dcmatch mth=1e-3 oprobe=vd porti=1`,
        # `dcmm3 n1 n2 dcmatch mth=1e-3 where=rawfile stats=yes`; p.49: `Name
        # ([node1] [node2]) acmatch parameter=value ...`. Python writes 1e-3 as 0.001.
        tb = rc_bench()
        tb.add_spectre_analysis("dcmatch", "dcmm1", mth=1e-3, oprobe="vin", porti=1)
        tb.add_spectre_analysis("dcmatch", "dcmm3", ("out", "0"), mth=1e-3, where="rawfile", stats="yes")
        tb.add_spectre_analysis("acmatch", nodes=["out", "0"], start=1, stop="1G", dec=10)
        tb.add_spectre_analysis("tdr", nodes=())
        lines = tb.netlist("spectre").splitlines()
        for want in ("dcmm1 dcmatch mth=0.001 oprobe=vin porti=1",
                     "dcmm3 (out 0) dcmatch mth=0.001 where=rawfile stats=yes",
                     "acmatch1 (out 0) acmatch start=1 stop=1G dec=10",
                     "tdr1 tdr"):
            assert lines.count(want) == 1, want

    def test_p8_025_a_target_without_the_analysis_refuses_it(self):
        # `read::spectre_refusal`: no other target has a `dcmatch` (W0015);
        # CDL and Verilog write no analysis at all.
        for other in ("ngspice", "espice", "hspice", "xyce", "ltspice", "vacask"):
            tb = rc_bench()
            tb.add_spectre_analysis("dcmatch", mth=1e-3)
            with pytest.raises(RuntimeError, match="W0015"):
                tb.netlist(other)
        tb = rc_bench()
        tb.add_spectre_analysis("dcmatch", mth=1e-3)
        assert "dcmatch" not in tb.netlist("cdl")
        assert "dcmatch" not in tb.netlist("verilog")

    @pytest.mark.parametrize("call, message", [
        (lambda tb: tb.add_spectre_analysis("dcmach"), "'dcmach' is no Spectre analysis"),
        (lambda tb: tb.add_spectre_analysis("options", reltol=1e-3), "'options' is no Spectre analysis"),
        (lambda tb: tb.add_spectre_analysis("DCMATCH"), "'DCMATCH' is no Spectre analysis"),
        (lambda tb: tb.add_spectre_analysis("dcmatch", "my run"), "dcmatch: 'my run' is no statement name"),
        (lambda tb: tb.add_spectre_analysis("dcmatch", nodes=("out 0",)), "dcmatch: 'out 0' is no node name"),
        (lambda tb: tb.add_spectre_analysis("dcmatch", nodes=("(out", "0)")), r"dcmatch: '\(out' is no node name"),
        (lambda tb: tb.add_spectre_analysis("dcmatch", **{"m th": 1}), "dcmatch: 'm th' is no parameter name"),
        (lambda tb: tb.add_spectre_analysis("dcmatch", stats=True), "dcmatch: stats= takes a value on one line"),
        (lambda tb: tb.add_spectre_analysis("dcmatch", file="a\nb"), "dcmatch: file= takes a value on one line"),
    ])
    def test_p8_025_a_bad_statement_is_a_value_error_and_adds_nothing(self, call, message):
        tb = rc_bench()
        tb.add_transient(1e-6, 1e-3)
        before = tb.netlist("spectre")
        with pytest.raises(ValueError, match=message):
            call(tb)
        assert tb.netlist("spectre") == before

    def test_p8_025_nodes_are_a_sequence_of_names(self):
        tb = rc_bench()
        with pytest.raises(TypeError):
            tb.add_spectre_analysis("dcmatch", nodes="out")

    def test_p8_025_the_store_keeps_the_statement(self):
        tb = rc_bench()
        tb.add_spectre_analysis("dcmatch", "dcmm3", ("out", "0"), mth=1e-3)
        again = ps().Testbench.from_source(tb.to_source())
        assert again.netlist("spectre") == tb.netlist("spectre")
        assert "dcmm3 (out 0) dcmatch mth=0.001" in again.netlist("spectre").splitlines()


# ======================================================================
# Library sections (P8-009; ngspice-45 manual §2.10)
# ======================================================================

def corner(name, rsh):
    s = ps().Section(name)
    s.parameter("rsh", rsh)
    return s


class TestSection:
    def test_p8_009_a_circuit_of_sections_is_a_library_file(self):
        # The file ngspice-45 and ESPice 0.3.0 read a corner from
        # (research/impl/evidence/P8-009/corners.lib).
        lib = ps().Circuit("corners")
        lib.section(corner("tt", "3k"))
        lib.section(corner("ff", "1k"))
        lines = str(lib).splitlines()
        assert lines[0] == ".title corners"
        for name, rsh in (("tt", "3k"), ("ff", "1k")):
            at = lines.index(f".lib {name}")
            assert lines[at:at + 3] == [f".lib {name}", f".param rsh={rsh}", f".endl {name}"]

    def test_p8_009_a_section_holds_files_params_models_and_definitions(self):
        cell = ps().Subcircuit("cell", ["a", "b"])
        cell.R("1", "a", "b", "{rsh}")
        s = ps().Section("tt")
        assert (s.name, repr(s)) == ("tt", "Section('tt')")
        s.include("/m/common.inc")
        s.lib("/m/res.lib", "res_tt")
        s.parameter("rsh", "3k")
        s.model("dn", "d", **{"is": 1e-14})
        s.subcircuit(cell)
        lib = ps().Circuit("corners")
        lib.section(s)
        # The deck's own source: the section's lines in call order.
        src = ps().Testbench(lib).to_source().splitlines()
        at = src.index(".lib tt")
        assert src[at:at + 9] == [
            ".lib tt", ".include /m/common.inc", ".lib /m/res.lib res_tt", ".param rsh=3k",
            ".model dn d (is=1e-14)", ".subckt cell a b", "R1 a b {rsh}", ".ends cell", ".endl tt",
        ]

    def test_p8_009_the_library_in_other_dialects(self):
        # Spectre Reference 19.1 p.498 (`section` … `endsection`); LTspice
        # reads a library as a whole file (W0012).
        lib = ps().Circuit("corners")
        lib.section(corner("tt", "3k"))
        tb = ps().Testbench(lib)
        spectre = tb.netlist("spectre").splitlines()
        assert spectre.index("section tt") < spectre.index("endsection tt")
        assert any(l.startswith("section tt") for l in tb.netlist("vacask").splitlines())
        with pytest.raises(RuntimeError, match="W0012"):
            tb.netlist("ltspice")

    def test_p8_009_a_bad_section_is_refused_at_the_call(self):
        for name in ("", "t t", "tt\n"):
            with pytest.raises(ValueError, match="is no section name"):
                ps().Section(name)
        loaded = ps().Subcircuit("dev", ["p"])
        loaded.osdi("/m/dev.osdi")
        s = corner("tt", "3k")
        with pytest.raises(ValueError, match="dev: a section carries no model load"):
            s.subcircuit(loaded)
        lib = ps().Circuit("corners")
        lib.section(s)
        # The refused definition left nothing behind.
        assert "dev" not in str(lib)
        with pytest.raises(TypeError):
            lib.section("tt")

    def test_p8_009_the_store_keeps_a_section(self):
        lib = ps().Circuit("corners")
        lib.section(corner("tt", "3k"))
        tb = ps().Testbench(lib)
        again = ps().Testbench.from_source(tb.to_source())
        assert again.netlist("ngspice") == tb.netlist("ngspice")
        assert again.netlist("spectre") == tb.netlist("spectre")
