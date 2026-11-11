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
import json
import os
import sys
from pathlib import Path

import pytest

import schema_check
import spicerack as ps
from spicerack.def_ import DialectDef, Simulator as BuiltIn

FAKE = Path(__file__).parent / "fixtures" / "sims" / "fake" / "fake.py"
FAKE_TOML = FAKE.with_name("fake.toml")
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


def test_p8_205_a_descriptor_file_is_the_same_simulator(monkeypatch):
    before = ps.simulators()
    sim = ps.load_simulator(FAKE_TOML)
    assert sim == ps.Simulator(
        "fake",
        dialect="ngspice",
        command=["python3", "-m", "fake", "{deck}", "{raw}", "{dir}/fake.log"],
        output="ngspice-ascii",
        analyses=["op", "dc"],
        features=[],
    )
    assert ps.load_simulator(str(FAKE_TOML)) == sim
    # Loaded and checked, not registered.
    assert ps.simulators() == before

    # It routes: the file's command runs `fake.py` as a module.
    assert ps.register_simulator(sim) == "fake"
    monkeypatch.setenv("PYTHONPATH", str(FAKE.parent))
    run = divider().simulator(simulator="fake")
    run.save("V(out)")
    assert run.operating_point()["out"] == expected("out")


def test_p8_205_a_bad_descriptor_file_names_its_key(tmp_path):
    good = FAKE_TOML.read_text()

    def load(text):
        path = tmp_path / "sim.toml"
        path.write_text(text)
        return ps.load_simulator(path)

    assert load(good) == ps.load_simulator(FAKE_TOML)
    cases = [
        ("schema", good.replace('schema = "spicerack.simulator/1"\n', "")),
        ("schema", good.replace('"spicerack.simulator/1"', '"spicerack.simulator/2"')),
        ("schema", good.replace('"spicerack.simulator/1"', "1")),
        ("shell", good + "shell = true\n"),  # an unknown key: a typo drops no declaration
        ("analyses", good.replace('["op", "dc"]', '["op", "transient"]')),
        ("command", good.replace('"{deck}", ', "")),
        ("verilog", good + 'verilog = "yes"\n'),
        ("name", good.replace('name = "fake"\n', "")),
        ("output", good.replace('output = "ngspice-ascii"\n', "")),
    ]
    for key, text in cases:
        assert text != good, key
        with pytest.raises(ValueError) as e:
            load(text)
        assert str(e.value).startswith(R0040 + key + ": "), (key, str(e.value))
    with pytest.raises(OSError):
        ps.load_simulator(tmp_path / "missing.toml")


def test_p8_207_a_dialect_runs_on_a_registered_simulator():
    ps.register_simulator(fake("p8-207-fake"))

    def on(base, sim):
        class Plugin(base):
            def define(self, d):
                return super().define(d).comment_tail("//").runs_on(sim)

        return Plugin

    # By name: the deck is written in the plugin's dialect and run on the simulator it names.
    lang = ps.register_dialect(on(ps.dialects.NGSPICE, "p8-207-fake")("p8-207-mine"))
    assert lang.runs_on() == "p8-207-fake"
    sim = divider().simulator(simulator="p8-207-mine")
    sim.save("V(out)")
    assert sim.operating_point()["out"] == expected("out")
    # ... and is held to what that simulator declares.
    with pytest.raises(RuntimeError, match="^W0180 ") as e:
        sim.transient(step_time=1e-6, end_time=1e-3)
    assert e.value.diagnostic["missing"] == {"analyses": ["tran"], "features": []}

    # A `Simulator` (by its name) and a built-in are taken too.
    by_object = ps.register_dialect(on(ps.dialects.NGSPICE, fake("p8-207-fake"))("p8-207-by-object"))
    assert by_object.runs_on() == "p8-207-fake"
    built_in = ps.register_dialect(on(ps.dialects.NGSPICE, BuiltIn.Ngspice)("p8-207-built-in"))
    assert built_in.runs_on() == "ngspice"

    # A dialect that does not extend the simulator's language is refused, the simulator named.
    refusal = "'p8-207-wrong': runs_on('p8-207-fake'), but it does not extend 'ngspice', the language simulator 'p8-207-fake' reads"
    with pytest.raises(ValueError) as e:
        ps.register_dialect(on(ps.dialects.SPECTRE, "p8-207-fake")("p8-207-wrong"))
    assert refusal in str(e.value)
    # The simulator is registered first.
    with pytest.raises(ValueError, match="no simulator is registered as 'p8-207-later'"):
        DialectDef("p8-207-early").runs_on("p8-207-later")
    with pytest.raises(TypeError, match="runs_on takes"):
        DialectDef("p8-207-early").runs_on(3)


MEASURES = {
    "schema": "spicerack.measures/1",
    "spicerack": "0",
    "backend": None,
    "raw": "out.raw",
    "analyses": [{"index": 0, "kind": "op", "plot": "Operating Point", "steps": None}],
    "measures": [
        {"name": "vout", "analysis": 0, "step": None, "value": 0.75, "status": "ok"},
        {"name": "never", "analysis": 0, "step": None, "value": None, "status": "failed"},
    ],
}
# A hook simulator's command: the fake writes where the hook then reads.
HOOKED = dict(command=[sys.executable, str(FAKE), "{deck}", "{dir}/out.raw"], output=None)


def test_p8_209_a_read_hook_reads_the_run_back():
    """Oracle: the fake's closed form, and the same deck on the fake read by `output`: the hook hands
    back the fake's own raw file, so the two runs agree bit for bit. The measures are `MEASURES`,
    written by hand (the fake measures nothing, and the deck holds no `.meas` card)."""
    answer = {}

    def read(outdir):
        answer["outdir"] = outdir
        return answer["raw"](os.path.join(outdir, "out.raw")), answer["measures"]

    hooked = fake("p8-209-hook", read=read, **HOOKED)
    assert hooked.read is read and hooked.output is None
    assert fake("p8-209-hook", read=read, **HOOKED) == hooked
    assert fake("p8-209-hook", read=lambda outdir: None, **HOOKED) != hooked
    assert fake("p8-209-hook") != hooked
    assert "Simulator.read" in ps.provisional
    assert ps.register_simulator(hooked) == "p8-209-hook"
    # The same function again is the same simulator; another function under the name is refused.
    assert ps.register_simulator(fake("p8-209-hook", read=read, **HOOKED)) == "p8-209-hook"
    with pytest.raises(ValueError, match="^R0040 .*name: 'p8-209-hook' is held by a different simulator$"):
        ps.register_simulator(fake("p8-209-hook", read=lambda outdir: None, **HOOKED))
    ps.register_simulator(fake("p8-209-plain"))

    def run(name):
        sim = divider().simulator(simulator=name)
        sim.save("V(out)", "V(in)")
        return sim.operating_point()

    plain = run("p8-209-plain")
    assert plain.measures == {}
    forms = {
        "a path": lambda path: path,
        "a path inside outdir": lambda path: "out.raw",
        "a pathlib.Path": Path,
        "a RawData": lambda path: ps.read_raw(path)[0],
        "a list of RawData": ps.read_raw,
        "a tuple of RawData": lambda path: tuple(ps.read_raw(path)),
    }
    for what, raw in forms.items():
        answer.update(raw=raw, measures=MEASURES)
        op = run("p8-209-hook")
        assert (op["out"], op["in"]) == (expected("out"), expected("in")), what
        assert (op["out"], op["in"]) == (plain["out"], plain["in"]), what
        assert op.measures == {"vout": 0.75, "never": None}, what
        assert os.path.isabs(answer["outdir"]) and not os.path.exists(answer["outdir"]), what
    answer.update(measures=None)
    assert run("p8-209-hook").measures == {}

    # `MEASURES` is the documented shape: the schema `export`'s sidecar is checked against.
    schema = Path(__file__).parent / "fixtures" / "schemas" / "spicerack.measures-1.schema.json"
    assert schema_check.errors(MEASURES, json.loads(schema.read_text())) == []


def test_p8_209_a_read_hook_that_fails_is_a_failed_run():
    """Oracle: each wrong answer written out with the part of it the refusal must name."""
    answer = {}
    ps.register_simulator(
        fake("p8-209-wrong", read=lambda outdir: answer["do"](os.path.join(outdir, "out.raw")), **HOOKED)
    )

    def boom(path):
        raise KeyError("boom")

    def row(**over):
        measures = [{**MEASURES["measures"][0], **over}]
        return lambda path: (path, {**MEASURES, "measures": measures})

    def without(key):
        doc = {k: v for k, v in MEASURES.items() if k != key}
        return lambda path: (path, doc)

    cases = [
        (boom, "KeyError: 'boom'"),
        (lambda path: path, "it returns a (raw, measures) tuple"),
        (lambda path: (path, None, None), "it returns a (raw, measures) tuple"),
        (lambda path: [path, None], "it returns a (raw, measures) tuple"),
        (lambda path: (3, None), "raw: a path, a RawData or a sequence of RawData"),
        (lambda path: ([path], None), "raw: a path, a RawData or a sequence of RawData"),
        (lambda path: (path + ".missing", None), "raw: " + os.sep),
        (lambda path: ([], None), "it returned no plot"),
        (lambda path: (path, []), "measures: a dict or None"),
        (lambda path: (path, {**MEASURES, "schema": "spicerack.measures/2"}), 'measures: schema: not "spicerack.measures/1"'),
        (without("schema"), "measures: schema: missing"),
        (without("analyses"), "measures: analyses: missing"),
        (without("measures"), "measures: measures: missing"),
        (lambda path: (path, {**MEASURES, "analyses": [{"kind": 3}]}), "measures: analyses: kind: a str or None"),
        (row(analysis=1), "measures: measures[0].analysis: a place in analyses"),
        (row(step=-1), "measures: measures[0].step: "),
        (row(value=None), "measures: measures[0].value: "),
        (row(value=float("inf")), "measures: measures[0].value: "),
        (row(value="0.75"), "measures: measures[0].value: "),
        (row(status="done"), "measures: measures[0].status: "),
        (row(name=3), "measures: measures[0].name: a str"),
    ]
    for do, why in cases:
        answer["do"] = do
        sim = divider().simulator(simulator="p8-209-wrong")
        sim.save("V(out)")
        with pytest.raises(RuntimeError) as e:
            sim.operating_point()
        assert str(e.value).startswith("Simulation failed: read hook: "), str(e.value)
        assert why in str(e.value), (why, str(e.value))
    # The simulator still runs once its hook answers.
    answer["do"] = lambda path: (path, None)
    sim = divider().simulator(simulator="p8-209-wrong")
    sim.save("V(out)")
    assert sim.operating_point()["out"] == expected("out")

    # The descriptor: `read` is a function, in `output`'s place.
    refused = [
        ("read", lambda: fake("x", read=3, **HOOKED)),
        ("output", lambda: fake("x", read=boom)),  # both
        ("output", lambda: fake("x", **HOOKED)),  # neither
        ("command", lambda: fake("x", read=boom, command=["fake", "{raw}"], output=None)),
    ]
    for field, build in refused:
        with pytest.raises(ValueError) as e:
            build()
        assert str(e.value).startswith(R0040 + field + ": "), (field, str(e.value))
