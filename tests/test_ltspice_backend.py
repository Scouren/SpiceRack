"""LTspice from Python: a stepped run is one result per step (ZERO D42), and
a bench of several analyses runs each in a deck of its own (P1-007).

Needs `ltspice` on PATH (`nix develop .#testing-ltspice`) and fails without
it. Under CI's LTSPICE_CI_PENDING_MIRROR marker, LTspice is absent and each
test checks that instead, failing once LTspice runs there.

Run: maturin develop && pytest tests/test_ltspice_backend.py -v
"""
import os
import shutil

import pytest


def ps():
    import spicerack  # unsupported fails, never skips: an unbuilt extension is an error
    return spicerack


def pending_mirror():
    """CI has no LTspice MSI yet: there, LTspice must be absent."""
    if os.environ.get("LTSPICE_CI_PENDING_MIRROR"):
        assert shutil.which("ltspice") is None, "LTspice runs: remove LTSPICE_CI_PENDING_MIRROR"
        return True
    assert shutil.which("ltspice"), "ltspice not on PATH: run in `nix develop .#testing-ltspice`"
    return False


def divider(mod):
    """1 V (AC 1) over R1 = {r} and 1k: V(out) = 1k / (r + 1k)."""
    sc = mod.Subcircuit("div", ["vdd", "out"], r="1k")
    sc.R(name="top", positive="vdd", negative="out", value="{r}")
    sc.R(name="bot", positive="out", negative=sc.gnd, value=1e3)
    tb = mod.Testbench(sc)
    tb.V(name="dd", positive="vdd", negative="0", value=1.0, ac=1.0)
    tb.with_backend("ltspice")
    return tb


def test_a_stepped_operating_point_is_one_result_per_step():
    if pending_mirror():
        return
    tb = divider(ps())
    tb.step("r", 1e3, 3e3, 1e3)
    ops = tb.operating_point()
    assert [op.params for op in ops] == [{"r": 1e3}, {"r": 2e3}, {"r": 3e3}]
    for op, gain in zip(ops, [0.5, 1 / 3, 0.25]):
        assert op["out"] == pytest.approx(gain, rel=1e-6)


def test_a_stepped_transient_takes_its_steps_from_the_log():
    if pending_mirror():
        return
    tb = divider(ps())
    tb.step("r", 1e3, 3e3, 1e3)
    runs = tb.transient(step_time=1e-6, end_time=1e-5)
    assert [t.params for t in runs] == [{"r": 1e3}, {"r": 2e3}, {"r": 3e3}]
    for t, gain in zip(runs, [0.5, 1 / 3, 0.25]):
        assert t.time[0] == 0.0
        assert t["out"] == pytest.approx([gain] * len(t.time), rel=1e-6)


def test_p8_102_a_stepped_measure_is_each_steps_own():
    """Oracle: the divider's closed form, 1k / (r + 1k), at each step, and
    LTspice 26.1.1's own log of this deck (research/impl/evidence/P8-102/
    lt-e2e.log): a `Measurement: vmax` table with one row a step, and
    `Measurement "tnever" FAIL'ed`."""
    if pending_mirror():
        return
    tb = divider(ps())
    tb.step("r", 1e3, 3e3, 1e3)
    tb.measure("tran", "vmax", "max", "v(out)")
    tb.measure("tran", "tnever", "when", "v(out)=2")
    runs = tb.transient(step_time=1e-6, end_time=1e-5)
    assert [t.params for t in runs] == [{"r": 1e3}, {"r": 2e3}, {"r": 3e3}]
    for t, gain in zip(runs, [0.5, 1 / 3, 0.25]):
        assert set(t.measures) == {"vmax", "tnever"}
        assert t.measures["vmax"] == pytest.approx(gain, rel=1e-6)
        assert t.measures["tnever"] is None


def test_an_unstepped_result_has_no_params():
    if pending_mirror():
        return
    op = divider(ps()).operating_point()
    assert op.params == {}
    assert op["out"] == pytest.approx(0.5, rel=1e-6)


def test_several_analyses_each_run_alone():
    if pending_mirror():
        return
    tb = divider(ps())
    tb.add_operating_point()
    tb.add_ac(variation="dec", number_of_points=2, start_frequency=1e3, stop_frequency=1e4)
    tb.add_transient(step_time=1e-6, end_time=1e-5)
    op, ac, tran = tb.run()
    assert op["out"] == pytest.approx(0.5, rel=1e-6)
    assert ac.magnitude("out") == pytest.approx([0.5] * len(ac.frequency), rel=1e-6)
    assert tran["out"] == pytest.approx([0.5] * len(tran.time), rel=1e-6)


def test_p8_100_a_stepped_run_exports_a_plot_and_a_point_per_step(tmp_path):
    """Oracle: the divider's closed form, 1k / (r + 1k), at each step, read
    back from the exported file, and what LTspice 26.1.1's own log of this
    deck says (research/impl/evidence/P8-102/lt-e2e.log): three steps, r =
    1000, 2000, 3000; a `Measurement: vmax` row a step; `tnever` FAIL'ed."""
    if pending_mirror():
        return
    import json

    mod = ps()
    tb = divider(mod)
    tb.step("r", 1e3, 3e3, 1e3)
    tb.measure("tran", "vmax", "max", "v(out)")
    tb.measure("tran", "tnever", "when", "v(out)=2")
    runs = tb.transient(step_time=1e-6, end_time=1e-5)
    raw_path, meas_path = mod.export([runs], tmp_path / "steps")
    with open(meas_path) as f:
        doc = json.load(f)
    gains = [0.5, 1 / 3, 0.25]
    # One analysis, stepped: its three plots follow each other in the file.
    assert doc["backend"] == "ltspice"
    assert doc["analyses"] == [{"index": 0, "kind": "tran", "plot": "Transient Analysis",
                                "steps": [{"r": 1e3}, {"r": 2e3}, {"r": 3e3}]}]
    plots = mod.read_raw(raw_path)
    assert len(plots) == 3
    for plot, gain in zip(plots, gains):
        assert plot["out"] == pytest.approx([gain] * len(plot["time"]), rel=1e-6)
    # By step, then the deck's order.
    assert [(m["name"], m["analysis"], m["step"], m["status"]) for m in doc["measures"]] == [
        ("vmax", 0, 0, "ok"), ("tnever", 0, 0, "failed"),
        ("vmax", 0, 1, "ok"), ("tnever", 0, 1, "failed"),
        ("vmax", 0, 2, "ok"), ("tnever", 0, 2, "failed")]
    assert [m["value"] for m in doc["measures"][0::2]] == pytest.approx(gains, rel=1e-6)
    assert [m["value"] for m in doc["measures"][1::2]] == [None, None, None]


def test_p3_037_a_stepped_source_steps_its_own_dc_value():
    """ZERO-LEDGER P3-037 / P3-134. Oracle: the divider's closed form, V(out) = Vdd / 2 at r = 1k, at
    Vdd = 1, 2, 3 V (LTspice 26.1.1 by hand on `.step Vin 1 3 1`: research/impl/evidence/P3-134/run.txt).
    `.step param Vdd` stepped a parameter no card reads: 0.5 V at every step."""
    if pending_mirror():
        return
    tb = divider(ps())
    tb.step("Vdd", 1.0, 3.0, 1.0)
    assert ".step Vdd 1 3 1" in tb.netlist("ltspice").splitlines()
    ops = tb.operating_point()
    assert [op["out"] for op in ops] == pytest.approx([0.5, 1.0, 1.5], rel=1e-6)


def test_p3_115_a_dc_sweep_of_a_resistor_is_refused_on_ltspice():
    """ZERO-LEDGER P3-115. LTspice's `.dc` sweeps an independent source or `temp` (help `.DC`): on
    `.dc Rl 1k 3k 1k` LTspice 26.1.1 stops, "DC sweep source must be an independent source."
    (research/impl/evidence/P3-115/run.txt), which the backend reported as a failed simulation. The
    card is refused when the deck is written. Oracle for the neighbour: the divider's closed form,
    V(out) = Vin * 3k / (1k + 3k) at Vin = 1, 2, 3 V."""
    if pending_mirror():
        return
    mod = ps()

    def bench():
        c = mod.Circuit("div")
        c.V("in", "in", "0", 1.0)
        c.R("1", "in", "out", 1e3)
        c.R("l", "out", "0", 3e3)
        tb = mod.Testbench(c)
        tb.with_backend("ltspice")
        return tb

    tb = bench()
    tb.add_dc(Rl=slice(1e3, 3e3, 1e3))
    with pytest.raises(mod.Error, match="W0015"):
        tb.netlist("ltspice")
    dc = bench().dc(Vin=slice(1.0, 3.0, 1.0))
    assert dc["out"] == pytest.approx([0.75, 1.5, 2.25], rel=1e-6)
