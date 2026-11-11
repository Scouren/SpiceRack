"""Custom simulators (D38): `Simulator`, `register_simulator`, `simulators()`, a backend chosen by its name.

Oracle: a fake simulator (`tests/fixtures/sims/fake/fake.py`) run as a real
process. It writes one value per saved node, a closed form of the node's
name; `expected` below computes that value on its own (never imported from
the fake), so a wrong argv, a wrong file read back or a wrong route is a
wrong number or a refusal, compared with no tolerance. The codes, their
titles and what each refusal names are note 12 §4's table, copied by hand.

The registry is one per process and never shrinks: every test registers
under its own `p8-<row>-` names and asserts by name, never by count.
"""
import hashlib
import sys
from pathlib import Path

import pytest

import spicerack as ps

FAKE = Path(__file__).parent / "fixtures" / "sims" / "fake" / "fake.py"
BUILT_INS = ["ngspice", "ltspice", "vacask", "spectre", "espice"]
R0040 = "R0040 a simulator descriptor that does not validate: "


def expected(node):
    """The fake's value for `node`: sha256(name)'s first 8 bytes over 2**64."""
    return int.from_bytes(hashlib.sha256(node.encode()).digest()[:8], "big") / 2**64


def fake(name, **over):
    """The fake's descriptor under `name` (op and dc, no feature), `over` replacing fields."""
    fields = dict(
        dialect="ngspice",
        command=[sys.executable, str(FAKE), "{deck}", "{raw}"],
        output="ngspice-ascii",
        analyses=["op", "dc"],
    )
    fields.update(over)
    return ps.Simulator(name, **fields)


def divider():
    c = ps.Circuit("divider")
    c.V("in", "in", c.gnd, 2.0)
    c.R("1", "in", "out", 1e3)
    c.R("2", "out", c.gnd, 3e3)
    return c


def test_p8_204_a_registered_simulator_runs_by_name():
    assert ps.register_simulator(fake("p8-204-fake")) == "p8-204-fake"
    assert ps.simulators()[:5] == BUILT_INS
    assert "p8-204-fake" in ps.simulators()[5:]

    sim = divider().simulator(simulator="p8-204-fake")
    sim.save("V(out)", "V(in)")
    op = sim.operating_point()
    assert (op["out"], op["in"]) == (expected("out"), expected("in"))
    dc = sim.dc(Vin=slice(0, 2, 1))
    assert list(dc.sweep) == [0.0, 1.0, 2.0]
    assert list(dc["out"]) == [expected("out") * x for x in (0.0, 1.0, 2.0)]
    # The real simulator gives 1.5 here: these numbers can only be the fake's.
    assert op["out"] != 1.5


def test_p8_204_a_descriptor_is_checked_once_when_it_is_built():
    ok = fake("p8-204-edge")
    assert (ok.name, ok.dialect, ok.output) == ("p8-204-edge", "ngspice", "ngspice-ascii")
    assert ok.command == [sys.executable, str(FAKE), "{deck}", "{raw}"]
    assert (ok.analyses, ok.features, ok.verilog, ok.verilog_a) == (["op", "dc"], [], False, False)
    more = fake("p8-204-edge", analyses={"tran", "op"}, features=("measures",), verilog=True, verilog_a=True)
    assert more.analyses == ["op", "tran"]
    assert more.features == ["osdi", "measures", "verilog"]
    assert (more.verilog, more.verilog_a) == (True, True)
    assert fake("p8-204-edge") == ok and more != ok
    # Built, not registered.
    assert "p8-204-edge" not in ps.simulators()

    refused = [
        ("name", lambda: fake("")),
        ("name", lambda: fake(None)),
        ("name", lambda: fake("ngspice-shared")),  # a built-in's alias
        ("dialect", lambda: fake("x", dialect="nosuch")),
        ("dialect", lambda: fake("x", dialect=None)),
        ("command", lambda: fake("x", command="fake {deck}")),  # a shell line, not argv
        ("command", lambda: fake("x", command=[])),
        ("command", lambda: fake("x", command=["fake", "{raw}"])),  # never given the deck
        ("command", lambda: fake("x", command=["fake", "{deck}", "{out}"])),
        ("command", lambda: fake("x", command=["{dir}/fake", "{deck}"])),
        ("output", lambda: fake("x", output="csv")),
        ("output", lambda: fake("x", output=None)),
        ("analyses", lambda: fake("x", analyses=["op", "transient"])),
        ("analyses", lambda: fake("x", analyses=[])),
        ("analyses", lambda: fake("x", analyses="op")),
        ("features", lambda: fake("x", features=["meas"])),
        ("verilog", lambda: fake("x", verilog="yes")),
        ("verilog_a", lambda: fake("x", verilog_a=1)),
        ("shell", lambda: fake("x", shell=True)),  # no such field
    ]
    for field, build in refused:
        with pytest.raises(ValueError) as e:
            build()
        assert str(e.value).startswith(R0040 + field + ": "), (field, str(e.value))


def test_p8_204_registering_twice_is_once_and_a_name_is_one_simulator():
    first = ps.register_simulator(fake("p8-204-twice"))
    before = ps.simulators()
    assert ps.register_simulator(fake("p8-204-twice")) == first
    assert ps.simulators() == before
    for name, other in [("p8-204-twice", dict(analyses=["op"])), ("NGSPICE", {}), ("P8-204-TWICE", {})]:
        with pytest.raises(ValueError) as e:
            ps.register_simulator(fake(name, **other))
        assert str(e.value) == R0040 + f"name: '{name}' is held by a different simulator"
    assert ps.simulators() == before

    # A backend name is a registered simulator's, a built-in's, or refused.
    divider().simulator(simulator="p8-204-twice")
    with pytest.raises(ValueError, match='unknown backend "p8-204-nobody".*p8-204-twice'):
        divider().simulator(simulator="p8-204-nobody")


def test_p8_206_a_refusal_carries_its_code_and_what_is_missing(monkeypatch):
    ps.register_simulator(fake("p8-206-fake"))
    ps.register_simulator(fake("p8-206-ghost", command=["p8-206-no-such-program", "{deck}", "{raw}"]))

    def refusal(run, code, title):
        with pytest.raises(RuntimeError) as e:
            run()
        assert str(e.value).startswith(f"{code} {title}: "), str(e.value)
        return e.value.diagnostic

    # W0180: the fake declares op and dc.
    sim = divider().simulator(simulator="p8-206-fake")
    got = refusal(lambda: sim.transient(step_time=1e-6, end_time=1e-3), "W0180", "an analysis the chosen simulator does not declare")
    assert got == {"code": "W0180", "stage": "route", "missing": {"analyses": ["tran"], "features": []}}

    # W0181: it declares no `measures`.
    sim = divider().simulator(simulator="p8-206-fake")
    sim.measure("op", "vout", "find", "v(out)", "at=0")
    got = refusal(sim.operating_point, "W0181", "a feature the chosen simulator does not declare")
    assert got == {"code": "W0181", "stage": "route", "missing": {"analyses": [], "features": ["measures"]}}

    # W0182: pz runs on ngspice only, hb on VACASK and Spectre only; no name changes that.
    dut = ps.Subcircuit("half", ["a", "y"])
    dut.R("1", "a", "y", 1e3)
    dut.R("2", "y", dut.gnd, 1e3)
    for name in (None, "p8-206-fake", "ngspice"):
        tb = ps.Testbench(dut)
        tb.V("in", "a", "0", 1.0)
        tb.add_polezero("a", "0", "y", "0", "vol", "pz")
        tb.add_harmonic_balance([1e6])
        if name:
            tb.with_backend(name)
        got = refusal(tb.run, "W0182", "no simulator runs every analysis of the deck")
        assert got == {"code": "W0182", "stage": "route", "missing": {"analyses": ["hb"], "features": []}}

    # R0041: routed by name, refused when its program is asked to run.
    sim = divider().simulator(simulator="p8-206-ghost")
    got = refusal(sim.operating_point, "R0041", "the simulator's command is not installed")
    assert got == {"code": "R0041", "stage": "run", "missing": None}

    # No code, no `diagnostic`: a name that is no simulator's.
    monkeypatch.setenv("SPICERACK_BACKEND", "p8-206-nobody")
    with pytest.raises(RuntimeError, match="^Simulation failed: Unknown backend: p8-206-nobody$") as e:
        divider().simulator().operating_point()
    assert not hasattr(e.value, "diagnostic")
