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
