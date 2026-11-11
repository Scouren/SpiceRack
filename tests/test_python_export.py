"""`spicerack.export`: a run's results as `<stem>.raw` and `<stem>.meas.json`.

Oracles, none of them the code's own arithmetic:
- the round trip: `read_raw` of the written file gives every column bit for bit;
- ngspice's own `load` of the written file, printed at 17 digits (every f64
  reads back the same), as an external reader;
- ngspice's own `.meas` printout of the same deck, parsed here by a regex
  (not by `measure_parse`), and the RC's closed forms;
- the schema file (`tests/fixtures/schemas/`), checked by `schema_check`;
- a metamorphic relation: exporting what `read_raw` reads of an exported
  file gives the same raw bytes; the same results give the same bytes.

Run: maturin develop && pytest tests/test_python_export.py -v
"""
import copy
import json
import math
import re
import subprocess
from pathlib import Path

import pytest

import spicerack as ps
from schema_check import errors

SCHEMA = json.loads((Path(__file__).parent / "fixtures/schemas/spicerack.measures-1.schema.json").read_text())
pytestmark = pytest.mark.needs("ngspice")


def bench():
    """1k into 1 uF (tau = 1 ms), a 0 -> 1 V step at t = 0, AC 1; a
    transient to 5 ms and an AC sweep; three tran measures (one that
    fails) and an AC one."""
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
    tb.measure("ac", "gain1k", "find", "vdb(out)", "at=1k")
    return tb


@pytest.fixture(scope="module")
def run():
    """The bench and its two results, run once for the module."""
    tb = bench()
    tran, ac = tb.run()
    return tb, tran, ac


def ngspice_prints(folder, plot, *exprs):
    """ngspice's own `load` of `bench.raw` in `folder`: what it prints for
    `exprs` of its plot `plot` (`tran1`, `ac1`), one list per expression.
    17 digits: every f64 reads back the same. ngspice exits 1 on a deck
    that only loads, so the exit status says nothing."""
    (folder / "load.sp").write_text(
        f"* load\n.control\nset numdgt=17\nload bench.raw\nsetplot {plot}\nprint {' '.join(exprs)}\n.endc\n.end\n")
    out = subprocess.run(["ngspice", "-b", "-n", "load.sp"], cwd=folder, capture_output=True, text=True).stdout
    rows = [line.split("\t")[1:1 + len(exprs)] for line in out.splitlines() if re.match(r"\d+\t", line)]
    assert rows, out
    return [[float(r[i]) for r in rows] for i in range(len(exprs))]


def test_p8_100_the_raw_file_round_trips_and_ngspice_loads_it(run, tmp_path):
    _, tran, ac = run
    raw_path, meas_path = ps.export([tran, ac], tmp_path / "bench")
    assert (raw_path, meas_path) == (str(tmp_path / "bench.raw"), str(tmp_path / "bench.meas.json"))
    plots = ps.read_raw(raw_path)
    # One plot per analysis, in the order given (ngspice ran the AC sweep's
    # plot first or not: the file's order is the caller's); canonical names.
    assert [p.plot_name for p in plots] == ["Transient Analysis", "AC Analysis"]
    assert plots[0].variable_names == ["time", "in", "out", "i(vin)"]
    assert plots[1].variable_names == ["frequency", "in", "out", "i(vin)"]
    # The round trip, bit for bit.
    assert len(tran.time) > 500
    assert plots[0]["time"] == tran.time
    assert plots[0]["out"] == tran["out"]
    assert plots[0]["i(vin)"] == tran["i(vin)"]
    assert plots[1]["frequency"] == ac.frequency
    assert [v.real for v in plots[1].complex("out")] == ac["out"]
    assert [abs(v) for v in plots[1].complex("out")] == pytest.approx(ac.magnitude("out"), rel=1e-15)
    # ngspice's own load of the file: the same numbers, and `v(out)` finds `out`.
    time, out, current = ngspice_prints(tmp_path, "tran1", "time", "v(out)", "i(vin)")
    assert (time, out, current) == (tran.time, tran["out"], tran["i(vin)"])
    real, imag = ngspice_prints(tmp_path, "ac1", "real(v(out))", "imag(v(out))")
    assert [complex(a, b) for a, b in zip(real, imag)] == plots[1].complex("out")


def test_p8_100_the_sidecar_is_ngspices_own_printout(run, tmp_path):
    tb, tran, ac = run
    _, meas_path = ps.export([tran, ac], tmp_path / "bench")
    doc = json.loads(Path(meas_path).read_text())
    assert errors(doc, SCHEMA) == []
    assert (doc["schema"], doc["backend"], doc["raw"]) == ("spicerack.measures/1", "ngspice", "bench.raw")
    assert re.match(r"\d+\.\d+\.\d+", doc["spicerack"])
    assert doc["analyses"] == [
        {"index": 0, "kind": "tran", "plot": "Transient Analysis", "steps": None},
        {"index": 1, "kind": "ac", "plot": "AC Analysis", "steps": None},
    ]
    # ngspice's own run of the deck SpiceRack writes: `name = value` per
    # measure with a value, nothing on stdout for one that failed.
    (tmp_path / "deck.cir").write_text(tb.netlist("ngspice"))
    told = subprocess.run(["ngspice", "-b", "-n", "deck.cir"], cwd=tmp_path, capture_output=True, text=True)
    found = re.finditer(r"^(vend|t50|tnever|gain1k)\s+=\s+(\S+)", told.stdout, re.M)
    printed = {m.group(1): float(m.group(2)) for m in found}
    assert "tnever" not in printed and re.search(r"measure\s+tnever\b.*out of interval", told.stderr)
    # By analysis, then the deck's order; each measure once, under its own analysis.
    assert [(m["name"], m["analysis"], m["step"], m["status"]) for m in doc["measures"]] == [
        ("vend", 0, None, "ok"), ("t50", 0, None, "ok"), ("tnever", 0, None, "failed"), ("gain1k", 1, None, "ok")]
    closed = {
        "vend": 1.0 - math.exp(-5.0),
        "t50": 1e-3 * math.log(2.0),
        "gain1k": -10.0 * math.log10(1.0 + (2.0 * math.pi) ** 2),
    }
    for m in doc["measures"]:
        if m["status"] == "ok":
            assert m["value"] == pytest.approx(printed[m["name"]], rel=1e-6), m["name"]
            assert m["value"] == pytest.approx(closed[m["name"]], rel=2e-3), m["name"]
        else:
            assert m["value"] is None and "reason" not in m


def test_p8_100_a_measure_of_an_analysis_not_exported_is_listed_once(run, tmp_path):
    """Every plot of a run carries the run's measures: `gain1k` (an AC
    measure) is the transient's too when the AC result is left out."""
    _, tran, ac = run
    _, meas_path = ps.export([None, tran], tmp_path / "bench")
    doc = json.loads(Path(meas_path).read_text())
    assert errors(doc, SCHEMA) == []
    # The `None` (a Fourier's place in `run()`'s list) is no analysis.
    assert doc["analyses"] == [{"index": 0, "kind": "tran", "plot": "Transient Analysis", "steps": None}]
    assert [(m["name"], m["analysis"]) for m in doc["measures"]] == [
        ("vend", 0), ("t50", 0), ("tnever", 0), ("gain1k", 0)]


def test_p8_100_the_same_results_give_the_same_bytes(run, tmp_path):
    _, tran, ac = run
    (tmp_path / "a").mkdir()
    (tmp_path / "b").mkdir()
    first = ps.export([tran, ac], tmp_path / "a" / "bench")
    again = ps.export((tran, ac), tmp_path / "b" / "bench", raw_format="binary")
    for one, two in zip(first, again):
        assert Path(one).read_bytes() == Path(two).read_bytes()
    # What `read_raw` reads of an exported file exports to the same raw
    # bytes; a plot read from a file names no analysis kind and no backend.
    raw_path, meas_path = ps.export(ps.read_raw(first[0]), tmp_path / "plots")
    assert Path(raw_path).read_bytes() == Path(first[0]).read_bytes()
    doc = json.loads(Path(meas_path).read_text())
    assert errors(doc, SCHEMA) == []
    assert doc["backend"] is None and doc["measures"] == []
    assert doc["analyses"] == [
        {"index": 0, "kind": None, "plot": "Transient Analysis", "steps": None},
        {"index": 1, "kind": None, "plot": "AC Analysis", "steps": None},
    ]
    # ASCII: text, and the same numbers bit for bit.
    ascii_path, _ = ps.export([tran, ac], tmp_path / "text", raw_format="ascii")
    assert b"Values:" in Path(ascii_path).read_bytes() and b"Binary:" not in Path(ascii_path).read_bytes()
    text = ps.read_raw(ascii_path)
    assert text[0]["out"] == tran["out"] and text[1].complex("out") == ps.read_raw(first[0])[1].complex("out")


def test_p8_100_what_is_no_export_is_refused_and_nothing_is_written(run, tmp_path):
    _, tran, _ = run
    stem = tmp_path / "bad"
    with pytest.raises(ValueError, match="raw_format"):
        ps.export([tran], stem, raw_format="hex")
    with pytest.raises(ValueError, match="no result holds a plot"):
        ps.export([], stem)
    with pytest.raises(ValueError, match="no result holds a plot"):
        ps.export([None], stem)
    with pytest.raises(TypeError, match="one result"):
        ps.export(tran, stem)
    with pytest.raises(TypeError, match="is not a result"):
        ps.export([tran, 1.5], stem)
    assert list(tmp_path.iterdir()) == []


def test_p8_100_the_schema_checker_reports_each_break(run, tmp_path):
    """The checker is the schema test's oracle: it is trusted only on
    documents it is seen to refuse."""
    _, tran, ac = run
    _, meas_path = ps.export([tran, ac], tmp_path / "bench")
    good = json.loads(Path(meas_path).read_text())
    breaks = [
        lambda d: d.pop("raw"),
        lambda d: d.update(extra=1),
        lambda d: d.update(schema="spicerack.measures/2"),
        lambda d: d["analyses"][0].update(index=-1),
        lambda d: d["analyses"][0].update(steps=[{"r": "1k"}]),
        lambda d: d["measures"][0].update(value=None),
        lambda d: d["measures"][2].update(value=1.0),
        lambda d: d["measures"][0].update(status="maybe"),
        lambda d: d["measures"][0].update(step=1.5),
    ]
    for k, damage in enumerate(breaks):
        bad = copy.deepcopy(good)
        damage(bad)
        assert errors(bad, SCHEMA), f"break {k} passes the schema"
    with pytest.raises(AssertionError, match="not one schema_check knows"):
        errors(good, {"pattern": "x"})
