"""The Python API's conformance matrix: every public feature, netlisted for
every dialect the writer has, is written or refused with one exact code.

Oracle: the table below, reviewed cell by cell against the target's manual
(a refusal's cause is the target lacking the construct, class G in
CONFORMANCE-GAPS.md; the citations are the codes' catalogue rows,
`src/syntax/diag/rows_*.rs`). It is compared as name lists, never counts: a cell
that changes, a feature added without a row, or a refusal with no code
fails. Where a simulator is installed (`nix develop .#testing`: ngspice,
ESPice, VACASK), the written decks run and their numbers meet a closed form.
"""
import math
import re
import shutil
import tempfile
from pathlib import Path

import pytest

TARGETS = ("ngspice", "espice", "hspice", "xyce", "ltspice", "spectre", "vacask", "cdl", "verilog")


# A copy: `veriloga()` compiles next to the source, which stays clean.
VA = Path(tempfile.mkdtemp()) / "varactor.va"
shutil.copy(Path(__file__).parent / "fixtures" / "verilog_a" / "va" / "varactor.va", VA)
INV = "module inv(input a, output y); assign y = ~a; endmodule"


def ps():
    import spicerack  # unsupported fails, never skips: an unbuilt extension is an error
    return spicerack


def dut(mod):
    """An RC low-pass with a resistor parameter `r`: in -R- out -C- 0."""
    d = mod.Subcircuit("rc", ["in", "out"], r="1k")
    d.R("1", "in", "out", "{r}")
    d.C("1", "out", d.gnd, 1e-9)
    return d


def bench(mod, d=None):
    tb = mod.Testbench(d or dut(mod))
    tb.V("in", "in", "0", 1.0, ac=1.0)
    return tb


# Each feature: what it adds to a bench (the DUT's elements go through
# `elem`, which gets a fresh DUT).
def elem(build):
    def f(mod):
        d = dut(mod)
        build(d)
        tb = bench(mod, d)
        tb.add_operating_point()
        return tb
    return f


def analysis(build):
    def f(mod):
        tb = bench(mod)
        build(tb)
        return tb
    return f


def oscillator(build):
    """A van der Pol tank (L 1 uH, C 1 nF, cell i = -1m v + (1m/3) v^3): an
    autonomous circuit, the only kind ngspice's and ESPice's `.pss` take
    (ngspice manual §11.3.12); closed-form limit cycle 2 V at 5.03292 MHz."""
    def f(mod):
        d = mod.Subcircuit("vdp", ["x"])
        d.L("1", "x", d.gnd, 1e-6)
        d.C("1", "x", d.gnd, 1e-9)
        d.BI("1", "x", d.gnd, expression="-1m*v(x)+(1m/3)*v(x)*v(x)*v(x)")
        tb = mod.Testbench(d)
        build(tb)
        return tb
    return f


def setting(build):
    def f(mod):
        tb = bench(mod)
        build(tb)
        tb.add_transient(1e-6, 1e-5)
        return tb
    return f


def source(build):
    def f(mod):
        tb = mod.Testbench(dut(mod))
        build(tb)
        tb.add_transient(1e-6, 1e-5)
        return tb
    return f


def _with_ic(mod):
    """A child whose internal node `n` holds an `.ic` and a `.nodeset`
    inside its own `.subckt` (ngspice applies them to every instance)."""
    c = mod.Subcircuit("buf", ["a", "b"])
    c.R("1", "a", "n", 1e3)
    c.R("2", "n", "b", 1e3)
    c.C("1", "n", c.gnd, 1e-9)
    c.initial_condition(n=0.5)
    c.node_set(n=0.25)
    d = dut(mod)
    d.instance(c, "1", "out", "0")
    tb = bench(mod, d)
    tb.add_subcircuit(c)
    tb.add_transient(1e-6, 1e-5)
    return tb


def _child(mod):
    c = mod.Subcircuit("buf", ["a", "b"])
    c.R("1", "a", "b", 1e3)
    return c


def _with_child(mod):
    d = dut(mod)
    child = _child(mod)
    d.instance(child, "1", "out", "0")
    tb = bench(mod, d)
    tb.add_subcircuit(child)
    tb.add_operating_point()
    return tb


def _with_section(mod):
    """A library file: a `Circuit` holding one section, netlisted through
    the class that has `netlist` (`Testbench(circuit)` takes its top)."""
    s = mod.Section("tt")
    s.parameter("rsh", "3k")
    lib = mod.Circuit("corners")
    lib.section(s)
    return mod.Testbench(lib)


def _with_local_function(mod):
    """`cell` holds a `.func` over its own parameter `k`: the subcircuit's
    (`Construct::LocalFunc`), not a top-level function."""
    cell = mod.Subcircuit("cell", ["a", "b"], k="2")
    cell.add_function("scaled", ["x"], "x*k")
    cell.R("1", "a", "b", "{scaled(500)}")
    d = dut(mod)
    d.instance(cell, "1", "out", "0")
    tb = bench(mod, d)
    tb.add_subcircuit(cell)
    tb.add_operating_point()
    return tb


def _with_nested(mod):
    """`buf` defined inside `wrap` (a nested `.subckt`), `wrap` placed in the DUT."""
    wrap = mod.Subcircuit("wrap", ["a", "b"])
    wrap.subcircuit(_child(mod))
    wrap.X("1", "buf", "a", "b")
    d = dut(mod)
    d.instance(wrap, "1", "out", "0")
    tb = bench(mod, d)
    tb.add_subcircuit(wrap)
    tb.add_operating_point()
    return tb


FEATURES = {
    # Analyses.
    "op": analysis(lambda tb: tb.add_operating_point()),
    "dc": analysis(lambda tb: tb.add_dc(Vin=slice(0, 1, 0.5))),
    "dc_temp": analysis(lambda tb: tb.add_dc(temp=slice(0, 50, 25))),
    "ac": analysis(lambda tb: tb.add_ac("dec", 10, 1.0, 1e6)),
    "ac_oct": analysis(lambda tb: tb.add_ac("oct", 10, 1.0, 1e6)),
    "ac_log": analysis(lambda tb: tb.add_ac("log", 10, 1.0, 1e6)),
    "tran": analysis(lambda tb: tb.add_transient(1e-6, 1e-3)),
    "tran_full": analysis(lambda tb: tb.add_transient(1e-6, 1e-3, 1e-4, 1e-6, True)),
    "noise": analysis(lambda tb: tb.add_noise("out", "0", "Vin")),
    "noise_summary": analysis(lambda tb: tb.add_noise("out", "0", "Vin", points_per_summary=5)),
    "tf": analysis(lambda tb: tb.add_tf("V(out)", "Vin")),
    "dc_sensitivity": analysis(lambda tb: tb.add_dc_sensitivity("V(out)")),
    "ac_sensitivity": analysis(lambda tb: tb.add_ac_sensitivity("V(out)")),
    "polezero": analysis(lambda tb: tb.add_polezero("in", "0", "out", "0", "vol", "pz")),
    "distortion": analysis(lambda tb: tb.add_distortion()),
    "pss": oscillator(lambda tb: tb.add_pss(5e6, 1e-5, "x")),
    "pss_controls": oscillator(lambda tb: tb.add_pss(5e6, 1e-5, "x", sc_iter=20)),
    "s_param": analysis(lambda tb: tb.add_s_param()),
    "harmonic_balance": analysis(lambda tb: tb.add_harmonic_balance([1e6])),
    "stability": analysis(lambda tb: tb.add_stability("Vin")),
    "transient_noise": analysis(lambda tb: tb.add_transient_noise(1e-9, 1e-6)),
    "fourier": analysis(lambda tb: (tb.add_transient(1e-6, 1e-3), tb.add_fourier(1e3, ["V(out)"]))),
    "fourier_count": analysis(lambda tb: (tb.add_transient(1e-6, 1e-3), tb.add_fourier(1e3, ["V(out)"], num_harmonics=5))),
    "network_params": analysis(lambda tb: tb.add_network_params("I(R1)", "Vin")),
    "spectre_sweep": analysis(lambda tb: (tb.add_ac("dec", 10, 1.0, 1e6), tb.add_spectre_sweep("r", 1e3, 2e3, 1e3, "ac1", "ac"))),
    "spectre_monte_carlo": analysis(lambda tb: (tb.add_ac("dec", 10, 1.0, 1e6), tb.add_spectre_monte_carlo(3, "ac1", "ac", seed=1))),
    "spectre_pac": analysis(lambda tb: tb.add_spectre_pac(1e6, 1e-5)),
    "spectre_pnoise": analysis(lambda tb: tb.add_spectre_pnoise(1e6, 1e-5, "out", "0")),
    "spectre_pxf": analysis(lambda tb: tb.add_spectre_pxf(1e6, 1e-5, "out", "Vin")),
    "spectre_pstb": analysis(lambda tb: tb.add_spectre_pstb(1e6, 1e-5, "Vin")),
    "spectre_analysis": analysis(lambda tb: tb.add_spectre_analysis("dcmatch", "dcmm3", ("out", "0"), mth=1e-3, where="rawfile", stats="yes")),
    # Settings.
    "options": setting(lambda tb: tb.options(RELTOL=1e-4, noacct=True)),
    "temperature": setting(lambda tb: (setattr(tb, "temperature", 85.0), setattr(tb, "nominal_temperature", 27.0))),
    "initial_condition": setting(lambda tb: tb.initial_condition(out=0.0)),
    "node_set": setting(lambda tb: tb.node_set(out=0.0)),
    "ic_expression": setting(lambda tb: (tb.initial_condition(out="{r/2000}"), tb.node_set(out="{r/2000}"))),
    "ic_in_subcircuit": _with_ic,
    "save": setting(lambda tb: tb.save("V(out)")),
    "measure": setting(lambda tb: tb.measure("tran", "vmax", "max", "v(out)")),
    "print": setting(lambda tb: tb.add_print("tran", "v(out)")),
    "step": setting(lambda tb: tb.step("r", 1e3, 3e3, 1e3)),
    "step_dec": setting(lambda tb: tb.step_sweep("r", 1e3, 1e5, 2, "dec")),
    "step_log": setting(lambda tb: tb.step_sweep("r", 1e3, 1e5, 5, "log")),
    "ngbehavior": setting(lambda tb: setattr(tb, "ngbehavior", "hsa")),
    "function": setting(lambda tb: tb.add_function("f", ["x"], "x*2")),
    "function_local": _with_local_function,
    "global_nodes": setting(lambda tb: tb.global_nodes("vdd")),
    "extra_line": setting(lambda tb: tb.extra_line(".options noacct")),
    "extra_line_spectre": setting(lambda tb: tb.extra_line("parameters k=2", dialect="spectre")),
    "use_pdk": setting(lambda tb: tb.use_pdk(ps().ModelLibrary("/ng/m.lib", corner="tt", spectre="/sp/m.scs"))),
    "use_pdk_include": setting(lambda tb: tb.use_pdk(ps().ModelLibrary("/ng/m.inc"))),
    "subcircuit": _with_child,
    "subcircuit_nested": _with_nested,
    # Sources.
    "sin": source(lambda tb: tb.SinusoidalVoltageSource("in", "in", "0", amplitude=1.0, frequency=1e3)),
    "distortion_inputs": source(lambda tb: tb.V("in", "in", "0", 0.0, distof1=(1, 0), distof2=(0.5, 90))),
    "pulse": source(lambda tb: tb.PulseVoltageSource("in", "in", "0")),
    "pwl": source(lambda tb: tb.PieceWiseLinearVoltageSource("in", "in", "0", values=[(0, 0), (1e-6, 1)])),
    "pwl_repeat": source(lambda tb: tb.PieceWiseLinearVoltageSource("in", "in", "0", values=[(0, 0), (1e-6, 1)], repeat_time=0.0)),
    "exp": source(lambda tb: tb.ExponentialVoltageSource("in", "in", "0", initial_value=0, pulsed_value=1, rise_time_constant=1e-6, fall_delay_time=5e-6, fall_time_constant=1e-6)),
    "sffm": source(lambda tb: tb.SingleFrequencyFMVoltageSource("in", "in", "0", offset=0, amplitude=1, carrier_frequency=1e6, modulation_index=2, signal_frequency=1e4)),
    "am": source(lambda tb: tb.AmplitudeModulatedVoltageSource("in", "in", "0", offset=0, amplitude=1, modulating_frequency=1e4, carrier_frequency=1e6)),
    "trnoise": source(lambda tb: tb.TransientNoiseVoltageSource("in", "in", "0", noise_rms=1e-3, time_step=1e-9)),
    "trrandom": source(lambda tb: tb.RandomVoltageSource("in", "in", "0", random_type="uniform", duration=1e-9)),
    "current_source": source(lambda tb: tb.I("in", "0", "in", 1e-3)),
    "rf_port": source(lambda tb: tb.V("in", "in", "0", 0.0, portnum=1)),
    # Elements and directives.
    "L_K": elem(lambda d: (d.L("1", "in", "0", 1e-6), d.L("2", "out", "0", 1e-6), d.K("1", "L1", "L2", coupling=0.5))),
    "K_three": elem(lambda d: (d.L("1", "in", "a", 1e-6), d.L("2", "out", "b", 1e-6), d.L("3", "out", "c", 1e-6), d.K("1", "L1", "L2", "L3", coupling=0.5))),
    "K_expr": elem(lambda d: (d.parameter("k", "0.5"), d.L("1", "in", "a", 1e-6), d.L("2", "out", "b", 1e-6), d.K("1", "L1", "L2", coupling="{k}"))),
    "C_charge": elem(lambda d: d.C("2", "out", d.gnd, charge="1n*v(out)")),
    "D": elem(lambda d: (d.model("dn", "d"), d.D("1", "out", "0", model="dn"))),
    "D_off": elem(lambda d: (d.model("dn", "d"), d.D("1", "out", "0", model="dn", off=True))),
    "Q": elem(lambda d: (d.model("qn", "npn"), d.Q("1", "out", "in", "0", model="qn"))),
    "M": elem(lambda d: (d.model("nm", "nmos"), d.M("1", "out", "in", "0", "0", model="nm", W=1e-6, L=1e-6))),
    "M_three_node": elem(lambda d: (d.model("vd", "vdmos"), d.M("1", "out", "in", "0", model="vd"))),
    "J": elem(lambda d: (d.model("jn", "njf"), d.J("1", "out", "in", "0", model="jn"))),
    "Z": elem(lambda d: (d.model("zn", "nmf"), d.Z("1", "out", "in", "0", model="zn"))),
    "E_G": elem(lambda d: (d.E("1", "x", "0", control_positive="out", control_negative="0", voltage_gain=2), d.G("1", "y", "0", control_positive="out", control_negative="0", transconductance=1e-3), d.R("x", "x", "0", 1e3), d.R("y", "y", "0", 1e3))),
    "F_H": elem(lambda d: (d.V("s", "out", "z", 0), d.R("z", "z", "0", 1e3), d.F("1", "x", "0", vsense="Vs", current_gain=2), d.H("1", "y", "0", vsense="Vs", transresistance=10), d.R("x", "x", "0", 1e3), d.R("y", "y", "0", 1e3))),
    "BV_BI": elem(lambda d: (d.BV("1", "x", "0", expression="V(out)*2"), d.BI("1", "y", "0", expression="V(out)*1m"), d.R("x", "x", "0", 1e3), d.R("y", "y", "0", 1e3))),
    "NonLinear": elem(lambda d: (d.NonLinearVoltageSource("x", "x", "0", expression="V(out)*2"), d.NonLinearCurrentSource("y", "y", "0", expression="V(out)*1m"), d.R("x", "x", "0", 1e3), d.R("y", "y", "0", 1e3))),
    "POLY": elem(lambda d: (d.POLY("1", "E", "x", "0", ["out", "0"], [0, 1, 2]), d.R("x", "x", "0", 1e3))),
    "S": elem(lambda d: (d.model("sw", "sw"), d.S("1", "out", "0", control_positive="in", control_negative="0", model="sw"))),
    "W": elem(lambda d: (d.V("s", "in", "z", 0), d.R("z", "z", "0", 1e3), d.model("cs", "csw"), d.W("1", "out", "0", vcontrol="Vs", model="cs"))),
    "T": elem(lambda d: (d.T("1", "out", "0", "x", "0", Z0=50, TD=1e-9), d.R("x", "x", "0", 50))),
    "A": elem(lambda d: (d.model("amp", "gain", gain=2.0), d.A(name="1", connections=["out", "x"], model="amp"), d.R("x", "x", "0", 1e3))),
    "parameter": elem(lambda d: d.parameter("k", "2")),
    "include": elem(lambda d: d.include("/m/x.inc")),
    "lib": elem(lambda d: d.lib("/m/x.lib", "tt")),
    "section": _with_section,
    "raw_spice": elem(lambda d: d.raw_spice("Rr out 0 1meg")),
    "osdi": elem(lambda d: d.osdi("/m/x.osdi")),
    "veriloga": elem(lambda d: d.veriloga(str(VA))),
    "verilog": elem(lambda d: d.verilog(source=INV, instance_name="u1", connections={"a": "in", "y": "x"})),
    "X": elem(lambda d: (d.raw_spice(".subckt buf a b\nR1 a b 1k\n.ends buf"), d.X("1", "buf", "out", "0"))),
    "I": elem(lambda d: d.I("1", "out", "0", 1e-6)),
}

# Written ("ok") or the one code its refusal names. Reviewed cell by cell;
# each refusal is the target lacking the construct (class G) unless a line
# below says otherwise, with the owner and the findings file entry.
OK = "ok"
# A `verilog(mode="simulate")` block is compiled by Icarus Verilog when the
# deck is written; without iverilog every cell reads this, and fails.
NEEDS_IVERILOG = "needs iverilog"
EXPECTED = {
    'op': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'dc': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'dc_temp': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'ac': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'ac_oct': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'ac_log': ('W0040', 'W0040', 'W0040', 'W0040', 'W0040', OK, 'W0040', OK, OK),
    'tran': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # VACASK's `tran` has `start=` and `icmode="uic"` (docs/cmd-analysis-tran.md; ZERO-LEDGER P4-006).
    'tran_full': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'noise': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # ngspice's points-per-summary: LTspice notes W0033 (writer B); Spectre, VACASK drop it with no note (writer C; FINDINGS-python.md).
    'noise_summary': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'tf': (OK, OK, OK, 'W0015', OK, OK, OK, OK, OK),
    'dc_sensitivity': (OK, OK, OK, 'W0015', 'W0015', OK, 'W0015', OK, OK),
    'ac_sensitivity': (OK, OK, 'W0015', 'W0015', 'W0015', OK, 'W0015', OK, OK),
    # HSPICE `.PZ v(out) src` where a source stands across the input (D's 8b52303).
    'polezero': (OK, OK, OK, 'W0015', 'W0015', OK, 'W0015', OK, OK),
    # G: ngspice-only `.disto`; W0015 would name the cause better than W0001 (writers B/C).
    'distortion': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    # The van der Pol bench's cell is a B source: VACASK's `i=` line since
    # ZERO-LEDGER P4-002, so only the analysis is left (W0001).
    'pss': (OK, OK, 'W0001', 'W0001', 'W0001', OK, 'W0001', OK, OK),
    'pss_controls': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0040', 'W0001', OK, OK),
    # Xyce: P ports and `.LIN` (D's a5f2c0a); HSPICE has `.LIN`: writer B (FINDINGS-python.md).
    's_param': (OK, OK, 'W0015', OK, 'W0015', OK, OK, OK, OK),
    # VACASK has `hb` (its test_hb1.sim), Xyce and HSPICE `.HB` (W0001, has but untranslated:
    # read::spectre_refusal): writers C and B (FINDINGS-python.md).
    'harmonic_balance': ('W0015', 'W0015', 'W0001', 'W0001', 'W0015', OK, 'W0001', OK, OK),
    # VACASK has `acstb` (36be560 docs/cmd-analysis-acstb.md; ZERO-LEDGER P4-106): W0001 until P4-008 writes it.
    'stability': ('W0015', 'W0015', 'W0015', 'W0015', 'W0015', OK, 'W0001', OK, OK),
    'transient_noise': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'fourier': (OK, OK, OK, OK, OK, OK, 'W0015', OK, OK),
    'fourier_count': (OK, OK, 'W0040', 'W0040', OK, OK, 'W0015', OK, OK),
    'network_params': ('W0001', 'W0001', 'W0001', 'W0001', OK, 'W0001', 'W0001', OK, OK),
    # LTspice and Xyce `.step` over the one analysis: stream A (Spectre analyses as Bench analyses).
    'spectre_sweep': ('W0015', 'W0015', 'W0001', 'W0001', 'W0001', OK, 'W0001', OK, OK),
    'spectre_monte_carlo': ('W0015', 'W0015', 'W0015', 'W0015', 'W0015', OK, OK, OK, OK),
    # VACASK 36be560 runs pac/pnoise untranslated (ZERO-LEDGER P4-001, VS
    # dossier M4): W0001, not "the target lacks it". pxf and pstb carry
    # their own pss alongside (`add_spectre_pxf`/`add_spectre_pstb`
    # `pss_fundamental`/`pss_stabilization`): the pss half is now W0001
    # too, the pxf/pstb half still W0015 (untouched by this row).
    'spectre_pac': ('W0015', 'W0015', 'W0015', 'W0015', 'W0015', OK, 'W0001', OK, OK),
    'spectre_pnoise': ('W0015', 'W0015', 'W0015', 'W0015', 'W0015', OK, 'W0001', OK, OK),
    'spectre_pxf': ('W0015', 'W0015', 'W0015', 'W0015', 'W0015', OK, 'W0001+W0015', OK, OK),
    'spectre_pstb': ('W0015', 'W0015', 'W0015', 'W0015', 'W0015', OK, 'W0001+W0015', OK, OK),
    # `dcmatch`, through the one door for the kinds only Spectre has: no other target
    # lists it (`Behaviours::spectre_analyses`), as `stability`'s `stb` above.
    'spectre_analysis': ('W0015', 'W0015', 'W0015', 'W0015', 'W0015', OK, 'W0015', OK, OK),
    'options': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'temperature': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'initial_condition': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'node_set': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'ic_expression': (OK,) * 9,
    # `.ic`/`.nodeset` inside a `.subckt` are kept as ngspice's cards: its own family writes them, W0001 elsewhere (P5-025 reads them).
    'ic_in_subcircuit': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    'save': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'measure': (OK, OK, OK, OK, OK, OK, 'W0109', OK, OK),
    'print': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'step': ('W0016', 'W0016', 'W0016', OK, OK, OK, OK, OK, OK),
    'step_dec': ('W0016', 'W0016', 'W0016', OK, OK, OK, OK, OK, OK),
    'step_log': ('W0016', 'W0016', 'W0016', 'W0016', 'W0016', OK, 'W0016', OK, OK),
    'ngbehavior': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'function': (OK, OK, OK, OK, OK, OK, 'W0106', OK, OK),
    # A function of a subcircuit (it reads the subcircuit's parameter): LTspice, Spectre
    # and VACASK define functions at the top only (`Construct::LocalFunc`: W0106).
    'function_local': (OK, OK, OK, OK, 'W0106', 'W0106', 'W0106', OK, OK),
    'global_nodes': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'extra_line': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    'extra_line_spectre': ('W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, 'W0001', OK, OK),
    'use_pdk': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'use_pdk_include': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'subcircuit': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # Every target writes the nested definition hoisted, as `wrap__buf`.
    'subcircuit_nested': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'sin': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # `distof1`/`distof2` are ngspice's words: the card is kept as its text, W0001 elsewhere (P4-071).
    'distortion_inputs': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    'pulse': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'pwl': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'pwl_repeat': (OK, OK, 'W0008', 'W0008', 'W0008', 'W0008', 'W0108', OK, OK),
    'exp': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'sffm': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # Xyce and LTspice have no AM source; VACASK's `type="am"` cannot hold a
    # zero carrier (`offset=0`: modindex VMA/VMO); stream D's AM order.
    'am': (OK, OK, OK, 'W0108', 'W0108', 'W0108', 'W0108', OK, OK),
    'trnoise': (OK, 'W0009', 'W0009', 'W0009', 'W0009', 'W0009', 'W0009', OK, OK),
    'trrandom': (OK, 'W0009', 'W0009', 'W0009', 'W0009', 'W0009', 'W0009', OK, OK),
    'current_source': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # Xyce's P port (D's a5f2c0a); HSPICE has a `P` port: writer B (FINDINGS-python.md).
    'rf_port': (OK, OK, 'W0007', OK, 'W0007', OK, OK, OK, OK),
    'L_K': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # A `K` over three inductors is kept as ngspice's card: its own family writes it, W0001 elsewhere (P5-074 reads it).
    'K_three': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    # A coupling given as an expression: kept as ngspice's card, as `K_three` (P5-030 reads it).
    'K_expr': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    # VACASK has no charge element: the source line `c2 (out 0) i=ddt(1e-9*v(out))` (P4-017).
    'C_charge': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'D': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # The `off` flag: Spectre's `region=off`; VACASK has no spelling for it (W0101).
    'D_off': (OK, OK, OK, OK, OK, OK, 'W0101', OK, OK),
    'Q': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'M': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'M_three_node': (OK, OK, OK, 'W0050+W0053', OK, 'W0104+W0105', OK, OK, OK),
    'J': (OK, OK, OK, OK, OK, 'W0104', OK, OK, OK),
    'Z': (OK, OK, OK, OK, OK, 'W0104', OK, OK, OK),
    'E_G': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # An E/G `value={expr}`: the behavioural source `BV_BI` is, under its own letter.
    'NonLinear': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'F_H': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'BV_BI': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'POLY': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'S': (OK, OK, OK, OK, OK, 'W0104', 'W0104+W0105', OK, OK),
    'W': (OK, OK, OK, OK, OK, 'W0104+W0105', 'W0104+W0105', OK, OK),
    'T': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'A': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    'parameter': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'include': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    'lib': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # A library section: LTspice reads a library as a whole file (`Construct::LibSection`: W0012).
    'section': (OK, OK, OK, OK, 'W0012', OK, OK, OK, OK),
    'raw_spice': (OK, OK, OK, OK, OK, OK, OK, OK, OK),
    # LTspice has no Verilog-A or OSDI loader (W0004, writer B's 53199ba);
    # Spectre's ahdl_include compiles Verilog-A source, no OSDI binary.
    'osdi': (OK, 'W0004', 'W0004', 'W0004', 'W0004', 'W0004', OK, OK, OK),
    'veriloga': (OK, OK, OK, 'W0004', 'W0004', OK, OK, OK, OK),
    # XSPICE d_cosim, an A element: ngspice and ESPice only, as `A` (stream F put iverilog in .#testing).
    'verilog': (OK, OK, 'W0001', 'W0001', 'W0001', 'W0001', 'W0001', OK, OK),
    'X': (OK,) * 9,
    'I': (OK,) * 9,
}


def cell(mod, name, target):
    try:
        FEATURES[name](mod).netlist(target)
        return OK
    except RuntimeError as e:
        if "iverilog not found" in str(e):
            return NEEDS_IVERILOG
        codes = sorted(set(re.findall(r"\b[LPRW]\d{4}\b", str(e))))
        return "+".join(codes) if codes else "uncoded: " + str(e)[:80]


def matrix(mod):
    return {n: dict(zip(TARGETS, (cell(mod, n, t) for t in TARGETS))) for n in FEATURES}


def test_every_feature_has_a_row():
    assert sorted(FEATURES) == sorted(EXPECTED)


def test_every_cell_is_written_or_refused_with_its_code():
    mod = ps()
    got = matrix(mod)
    want = {n: dict(zip(TARGETS, row)) for n, row in EXPECTED.items()}
    wrong = [f"{n} on {t}: {got[n][t]} (expected {want[n][t]})"
             for n in FEATURES for t in TARGETS if got[n][t] != want[n][t]]
    assert wrong == []


# Every public method of the builders a deck is made with: each is a
# feature above, or listed here as no deck feature (it reads, runs or
# stores a deck, or it is an alias of one that is).
NOT_A_FEATURE = {
    "netlist", "run", "to_source", "save_source", "from_source", "load", "save",
    "to_json", "from_json", "save_json", "load_json", "check_backend", "with_backend",
    "available_backends", "add_subcircuit", "name", "ports", "gnd", "instance",
}
COVERED = {
    "Testbench": {
        "add_spectre_analysis",
        "V", "I", "R", "C", "add_operating_point", "add_dc", "add_ac", "add_transient",
        "add_noise", "add_tf", "add_transfer_function", "add_dc_sensitivity",
        "add_ac_sensitivity", "add_polezero", "add_distortion", "add_pss", "add_s_param",
        "add_harmonic_balance", "add_stability", "add_transient_noise", "add_fourier",
        "add_network_params", "add_spectre_sweep", "add_spectre_monte_carlo",
        "add_spectre_pac", "add_spectre_pnoise", "add_spectre_pxf", "add_spectre_pstb",
        "options", "temperature", "nominal_temperature", "initial_condition", "node_set",
        "measure", "add_print", "step", "step_sweep", "ngbehavior", "add_function",
        "global_nodes", "extra_line", "use_pdk", "save_currents",
    },
    "Subcircuit": {
        "add_function",
        "subcircuit",
        "R", "C", "L", "K", "D", "Q", "BJT", "M", "MOSFET", "J", "Z", "E", "G", "F", "H",
        "initial_condition", "node_set",
        "BV", "BI", "POLY", "S", "W", "T", "X", "A", "V", "I", "model", "parameter",
        "include", "lib", "raw_spice", "osdi", "veriloga", "verilog",
    },
}


@pytest.mark.parametrize("cls", sorted(COVERED))
def test_every_public_builder_method_is_a_feature(cls):
    mod = ps()
    public = {n for n in dir(getattr(mod, cls)) if not n.startswith("_")}
    sources = {n for n in public if n.endswith(("VoltageSource", "CurrentSource"))}
    # The run twins of the queued analyses, and the deprecated alias.
    runs = {n for n in public if "add_" + n in public} | {"spectre_montecarlo", "tf"}
    left = public - COVERED[cls] - NOT_A_FEATURE - sources - runs
    assert sorted(left) == []


# ── The written decks, run where the simulator is installed ──

K_BOLTZMANN = 1.380649e-23  # J/K, exact (SI 2019)
T_NOM = 300.15  # 27 °C, every backend's default temperature


def divider(mod):
    """in -1k- out -3k- 0, 1 nF at out: H(0) = 0.75, R_th = 750 Ω."""
    d = mod.Subcircuit("div", ["in", "out"])
    d.R("1", "in", "out", 1e3)
    d.R("2", "out", d.gnd, 3e3)
    d.C("1", "out", d.gnd, 1e-9)
    return d


def h(f):
    """The divider's transfer at `f` (closed form)."""
    return 0.75 / complex(1.0, 2 * math.pi * f * 750.0 * 1e-9)


def test_p3_039_api_spectre_mos_instance_params():
    """Ledger P3-039: the API's `M(..., W=, L=)` on Spectre. Spectre names are case-sensitive (Spectre
    Reference 5.0 front matter) and a MOSFET's instance parameters are `w`, `l` (p.408). Spectre 25.1
    on `W=10u L=1u`: "SFE-102 Parameter `W' ... ignored" and the default size, Id 4.225e-5 for 4.225e-4
    (iccad4, 2026-11-07; research/settle-remote.md, recheck P3-039)."""
    text = FEATURES["M"](ps()).netlist("spectre")
    cards = [line.strip() for line in text.splitlines() if line.strip().startswith("m1 ")]
    assert cards == ["m1 (out in 0 0) nm w=1u l=1u"], text


def p3_042_bench():
    """The divider of P3-042's tests: R2 names the model `rm` (tc1) at 127 C."""
    mod = ps()
    d = mod.Subcircuit("div", ["in", "out"])
    d.model("rm", "r", tc1=1e-3)
    d.R("1", "in", "out", 1e3)
    d.R("2", "out", d.gnd, 1e3, model="rm")
    t = mod.Testbench(d)
    t.V("in", "in", "0", 1.0)
    t.temperature = 127.0
    t.add_operating_point()
    return t


def p3_042_cards(target, head):
    text = p3_042_bench().netlist(target)
    return [line.strip() for line in text.splitlines() if line.strip().startswith(head)]


def p3_042_runs(backend):
    if shutil.which(backend) is None:
        pytest.fail(f"{backend} not on PATH: run the tests in `nix develop .#testing`")
    t = p3_042_bench()
    t.with_backend(backend)
    (op,) = t.run()
    assert op["out"] == pytest.approx(11 / 21, rel=1e-9), backend


def test_p3_042_api_model_rm_resistor():
    """Ledger P3-042: `R(..., model="rm")` names a `.model` card. ngspice-45 manual §3.3.2: `RXXXXXXX n+
    n- <value> <mname> <l=length> ...`. Spectre Reference 5.0 p.628: `Name 1 2 ModelName parameter=value
    ...`, "with model: r1 (1 2) resmod l=8u w=1u"; R(T) = R(tnom)*[1 + tc1*(T - tnom)]. The model line's
    `l=10u w=10u r=1m ...` are ngspice's defaults where Spectre's differ, as the pinned
    tests/spectre/t_m1_resistors.scs (Spectre 25.1: PASS 6/6, research/settle-remote.md). The numbers:
    R2 = 1k*(1 + 1e-3*(127 - 27)) = 1.1k under R1 = 1k, so out = 11/21 (the closed form). `model=rm` as a
    parameter is ngspice-45's fatal "Undefined parameter [rm]" (research/impl/evidence/P3-042)."""
    assert p3_042_cards("ngspice", "R2 ") == ["R2 out 0 1k rm"]
    p3_042_runs("ngspice")


# Known failure: the reader keeps `R2 out 0 1k rm` as ngspice text (render/read/elements.rs::rcl reads a word
# after the value as a flag), so Spectre and VACASK refuse it W0001 (batch-9).
@pytest.mark.xfail(strict=True, raises=RuntimeError, reason="ZERO-LEDGER P3-126")
def test_p3_042_api_spectre_model_rm_resistor():
    """P3-042 on Spectre and VACASK: the card `r2 (out 0) rm r=1k`, the model's Spectre defaults, 11/21."""
    assert p3_042_cards("spectre", "r2 ") == ["r2 (out 0) rm r=1k"]
    assert p3_042_cards("spectre", "model rm ") == [
        "model rm resistor tc1=1m l=10u w=10u r=1m af=1 wdexp=0 ldexp=0 leexp=1 weexp=1"
    ]
    p3_042_runs("vacask")


@pytest.mark.parametrize("backend", ["ngspice", "espice", "vacask"])
def test_written_decks_meet_the_closed_form(backend):
    if shutil.which(backend) is None:
        pytest.fail(f"{backend} not on PATH: run the tests in `nix develop .#testing`")
    mod = ps()
    tb = mod.Testbench(divider(mod))
    tb.V("in", "in", "0", 1.0, ac=1.0)
    tb.with_backend(backend)
    tb.add_operating_point()
    tb.add_dc(Vin=slice(0.0, 1.0, 0.5))
    tb.add_ac("dec", 2, 1e3, 1e6)
    tb.add_transient(1e-8, 5e-6, None, 1e-8)
    op, dc, ac, tr = tb.run()
    assert op["out"] == pytest.approx(0.75, rel=1e-9)
    assert dc.sweep == pytest.approx([0.0, 0.5, 1.0])
    assert dc["out"] == pytest.approx([0.0, 0.375, 0.75], abs=1e-12)
    # AC is a linear solve: exact to rounding.
    assert ac.magnitude("out") == pytest.approx([abs(h(f)) for f in ac.frequency], rel=1e-9)
    # The source is 1 V from t = 0, so the cap settles to 0.75 V (tau 0.75 us).
    assert tr["out"][-1] == pytest.approx(0.75, rel=1e-4)
    # Thermal noise of R_th = 750 Ω through the RC: sqrt(4kTR)|H/H(0)|.
    nz_tb = mod.Testbench(divider(mod))
    nz_tb.V("in", "in", "0", 1.0, ac=1.0)
    nz_tb.with_backend(backend)
    nz_tb.add_noise("out", "0", "Vin", "dec", 1, 1e3, 1e5)
    (nz,) = nz_tb.run()
    want = [math.sqrt(4 * K_BOLTZMANN * T_NOM * 750.0) * abs(h(f)) / 0.75 for f in nz.frequency]
    assert nz["onoise_spectrum"] == pytest.approx(want, rel=1e-4)
    assert nz["inoise_spectrum"] == pytest.approx([w / abs(h(f)) for w, f in zip(want, nz.frequency)], rel=1e-4)


if __name__ == "__main__":
    for name, row in matrix(ps()).items():
        print(f"    {name!r}: {tuple(row.values())!r},")
