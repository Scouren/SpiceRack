"""The diagnostics file (ZERO D14, ZERO-LEDGER P8-107): `spicerack.diagnostics.netlist` and `run`
leave `<variant>.diag.json` (`spicerack.diagnostics/1`) for a deck, whatever happened to it.

Oracles: the schema file (`tests/fixtures/schemas/`), checked by `schema_check`;
`test_python_conformance.EXPECTED`, the matrix of refusal codes written by hand; the lines of
`test_python_errors.noted`'s and `am`'s decks, counted by hand; note 12 §4's `simulator` object
and codes, copied by hand; the fake simulator's closed form (`test_simulators.expected`); and
commands whose exit status and output are written here.
"""
import json
import re
import sys
from pathlib import Path

import pytest

import schema_check
import spicerack as ps
import test_python_conformance as conf
from spicerack import diagnostics
from test_python_errors import NO_STATEMENT, am, noted
from test_simulators import expected, fake

SCHEMA = json.loads((Path(__file__).parent / "fixtures/schemas/spicerack.diagnostics-1.schema.json").read_text())
# A simulator that prints, names an error and exits 3.
FAILS = "import sys; print('starting'); print('Error: no such model'); print('fatal', file=sys.stderr); sys.exit(3)"


def read(out, variant):
    """`variant`'s file, held to the schema; the files it names are beside it."""
    doc = json.loads((out / f"{variant}.diag.json").read_text())
    assert schema_check.errors(doc, SCHEMA) == []
    assert (doc["schema"], doc["variant"]) == ("spicerack.diagnostics/1", variant)
    assert re.match(r"\d+\.\d+\.\d+", doc["spicerack"])
    named = {doc["source"], (doc["simulator"] or {}).get("log")} - {None}
    assert named <= {f"{variant}.source.cir", f"{variant}.log"}
    assert all((out / file).is_file() for file in named)
    return doc


def spans(source, statement, line):
    """Whether `statement` stands in `source` on lines that include `line` (from 1)."""
    at = source.find(statement)
    while at >= 0:
        first = source.count("\n", 0, at) + 1
        if first <= line <= first + statement.count("\n"):
            return True
        at = source.find(statement, at + 1)
    return False


def bench(backend):
    """A 2 V source on a 1k/3k divider, `out` and `in` saved, to run on `backend`."""
    div = ps.Subcircuit("div", ["in", "out"])
    div.R("1", "in", "out", 1e3)
    div.R("2", "out", div.gnd, 3e3)
    tb = ps.Testbench(div)
    tb.V("in", "in", "0", 2.0)
    tb.save("v(out)", "v(in)")
    tb.with_backend(backend)
    return tb


def test_p8_107_every_cell_of_the_matrix_leaves_a_file_with_its_codes(tmp_path):
    """Oracles: the schema, on every file; `EXPECTED`: a written cell's file holds notes only,
    a refused cell's exactly the cell's codes; and the source beside the file: every located
    diagnostic's statement stands in it at the diagnostic's line."""
    wrong = []
    for name, build in conf.FEATURES.items():
        for target, cell in zip(conf.TARGETS, conf.EXPECTED[name]):
            variant = f"{name}-{target}"
            want = ("written", []) if cell == conf.OK else ("refused", cell.split("+"))
            status, text = diagnostics.netlist(build(ps), target, tmp_path, variant)
            doc = read(tmp_path, variant)
            found = doc["diagnostics"]
            codes = {d["code"] for d in found if status == "refused" or d["kind"] != "note"}
            if (status, sorted(codes)) != want:
                wrong.append(f"{name} on {target}: {(status, sorted(codes))} (expected {want})")
            head = (doc["stage"], doc["dialect"], doc["status"], doc["simulator"], text is None)
            if head != ("netlist", target, status, None, status == "refused"):
                wrong.append(f"{name} on {target}: {head}")
            source = (tmp_path / doc["source"]).read_text() if doc["source"] else None
            if (source is None) != (codes <= NO_STATEMENT and status == "refused"):
                wrong.append(f"{name} on {target}: source {doc['source']} with {sorted(codes)}")
            for d in found:
                if d["line"] is None:
                    here = (d["code"] in NO_STATEMENT, d["file"], d["column"], d["statement"], d["element"])
                    ok = here == (True, None, None, None, None)
                else:
                    ok = d["file"] == doc["source"] and spans(source, d["statement"], d["line"])
                if not ok:
                    wrong.append(f"{name} on {target}: {d['code']} at {d['file']}:{d['line']}")
    assert wrong == []
    assert len(list(tmp_path.glob("*.diag.json"))) == len(conf.FEATURES) * len(conf.TARGETS)


def test_p8_107_a_refusal_is_located_in_the_source_beside_its_file(tmp_path):
    """Oracle: `noted`'s deck, its cards counted by hand (`.save` is line 5, `.meas` line 6),
    and the catalogue's titles for W0023 and W0109, copied by hand."""
    assert diagnostics.netlist(noted(), "vacask", tmp_path, "rc") == ("refused", None)
    doc = read(tmp_path, "rc")
    assert all(d.pop("cite") for d in doc["diagnostics"])
    assert doc == {
        "schema": "spicerack.diagnostics/1",
        "spicerack": doc["spicerack"],
        "variant": "rc",
        "stage": "netlist",
        "dialect": "vacask",
        "status": "refused",
        "source": "rc.source.cir",
        "diagnostics": [
            {"code": "W0023", "kind": "note", "title": "an output request not written",
             "message": "rc:5:1: W0023 an output request not written",
             "file": "rc.source.cir", "line": 5, "column": 1, "statement": ".save v(vout)", "element": None},
            {"code": "W0109", "kind": "refusal", "title": "a measurement the target has no statement for",
             "message": "rc:6:1: W0109 a measurement the target has no statement for",
             "file": "rc.source.cir", "line": 6, "column": 1,
             "statement": ".meas tran vmax max v(vout)", "element": None},
        ],
        "simulator": None,
    }
    lines = (tmp_path / "rc.source.cir").read_text().splitlines()
    assert (lines[0], lines[4], lines[5], lines[-1]) == ("rc", ".save v(vout)", ".meas tran vmax max v(vout)", ".end")
    assert sorted(p.name for p in tmp_path.iterdir()) == ["rc.diag.json", "rc.source.cir"]

    # A written deck is returned, and its file holds its notes: the file always exists.
    status, text = diagnostics.netlist(noted(), "xyce", tmp_path, "rc-xyce")
    assert (status, text) == ("written", noted().netlist_with_notes("xyce")[0])
    doc = read(tmp_path, "rc-xyce")
    assert (doc["status"], doc["source"], doc["simulator"]) == ("written", "rc-xyce.source.cir", None)
    assert [(d["code"], d["kind"], d["file"], d["line"]) for d in doc["diagnostics"]] == [
        ("W0023", "note", "rc-xyce.source.cir", 5)]
    assert diagnostics.netlist(noted(), "ngspice", tmp_path, "rc-ngspice")[0] == "written"
    assert read(tmp_path, "rc-ngspice")["diagnostics"] == []

    # The same deck gives the same bytes.
    again = tmp_path / "again"
    again.mkdir()
    diagnostics.netlist(noted(), "vacask", again, "rc")
    for file in ("rc.diag.json", "rc.source.cir"):
        assert (again / file).read_bytes() == (tmp_path / file).read_bytes()

    # A bad argument is a ValueError before any file is written.
    for call in (lambda: diagnostics.netlist(noted(), "nope", again, "bad"),
                 lambda: diagnostics.netlist(noted(), "vacask", again, "a/b"),
                 lambda: diagnostics.run(noted(), again, "")):
        with pytest.raises(ValueError) as bad:
            call()
        assert not isinstance(bad.value, ps.Error)
    assert sorted(p.name for p in again.iterdir()) == ["rc.diag.json", "rc.source.cir"]


def test_p8_107_a_hook_that_raises_is_a_failed_netlist_with_its_message(tmp_path):
    """Oracle: the exception the hook raises, written out by hand."""
    class Loud(ps.dialects.NGSPICE):
        def define(self, d):
            return super().define(d).hook(".noise", self.boom)

        def boom(self, cards):
            raise KeyError("boom")

    loud = ps.register_dialect(Loud("p8-107-loud")).name()
    noise = ps.Testbench.from_source(
        "* t\nV1 in 0 1 AC 1\nR1 in out 1k\nR2 out 0 1k\n.noise v(out) V1 dec 10 1 1k\n.end\n",
        dialect="ngspice")
    assert diagnostics.netlist(noise, loud, tmp_path, "t") == ("failed", None)
    doc = read(tmp_path, "t")
    assert (doc["stage"], doc["dialect"], doc["status"], doc["diagnostics"]) == ("netlist", loud, "failed", [])
    assert doc["simulator"] == {"backend": None, "dialect": loud, "exit_status": None, "log": None,
                                "message": "hook '.noise': KeyError: 'boom'", "missing": None}


def test_p8_107_a_run_leaves_its_file_whatever_happened(tmp_path):
    """Oracles: the fake's closed form for the run that passes; note 12 §4's `simulator` object
    and its W0180 and R0041 rows for the runs that are not routed or not installed; the exit
    status and the output of the commands written here for the runs that fail."""
    ps.register_simulator(fake("p8-107-fake"))
    ps.register_simulator(fake("p8-107-ghost", command=["p8-107-no-such-program", "{deck}", "{raw}"]))
    ps.register_simulator(fake("p8-107-exits", command=[sys.executable, "-c", FAILS, "{deck}", "{raw}"]))
    ps.register_simulator(fake("p8-107-false", command=["false", "{deck}", "{raw}"]))
    ps.register_simulator(fake("p8-107-xyce", dialect="xyce"))

    def ran(tb, variant):
        status, text, results = diagnostics.run(tb, tmp_path, variant)
        doc = read(tmp_path, variant)
        assert doc["status"] == status
        return doc, text, results

    def op(backend):
        tb = bench(backend)
        tb.add_operating_point()
        return tb

    # It ran: the deck that ran and the results come back, and the file says ok.
    doc, text, results = ran(op("p8-107-fake"), "ok")
    [point] = results
    assert (point["out"], point["in"]) == (expected("out"), expected("in"))
    assert text == op("p8-107-fake").netlist("ngspice")
    assert (doc["stage"], doc["dialect"], doc["status"], doc["source"], doc["diagnostics"], doc["simulator"]) == (
        "run", "ngspice", "ok", "ok.source.cir", [], None)
    assert (tmp_path / "ok.source.cir").read_text().splitlines()[0] == "div"

    # Routed nowhere: the fake declares op and dc. Nothing is written but the file.
    tb = bench("p8-107-fake")
    tb.add_transient(step_time=1e-6, end_time=1e-3)
    doc, text, results = ran(tb, "route")
    assert (text, results) == (None, None)
    assert (doc["stage"], doc["dialect"], doc["status"], doc["source"]) == ("route", "ngspice", "refused", None)
    [d] = doc["diagnostics"]
    assert (d["code"], d["kind"], d["title"]) == ("W0180", "refusal", "an analysis the chosen simulator does not declare")
    assert (d["file"], d["line"], d["column"], d["statement"], d["element"]) == (None,) * 5
    assert doc["simulator"] == {"backend": "p8-107-fake", "dialect": "ngspice", "exit_status": None, "log": None,
                                "message": None, "missing": {"analyses": ["tran"], "features": []}}
    assert sorted(p.name for p in tmp_path.glob("route.*")) == ["route.diag.json"]

    # Routed, and the writer of its language refuses the deck: a netlist refusal, located.
    tb = am()
    tb.with_backend("p8-107-xyce")
    doc, text, results = ran(tb, "am")
    assert (text, results) == (None, None)
    assert (doc["stage"], doc["dialect"], doc["status"], doc["simulator"]) == ("netlist", "xyce", "refused", None)
    [d] = doc["diagnostics"]
    assert (d["code"], d["file"], d["line"], d["column"], d["element"]) == ("W0108", "am.source.cir", 3, 1, "Vin")

    # It ran and exited 3: its status, its first error line, and everything it printed.
    doc, text, results = ran(op("p8-107-exits"), "exits")
    assert (text, results) == (op("p8-107-exits").netlist("ngspice"), None)
    assert (doc["stage"], doc["dialect"], doc["status"], doc["diagnostics"]) == ("run", "ngspice", "failed", [])
    assert doc["simulator"] == {"backend": "p8-107-exits", "dialect": "ngspice", "exit_status": 3,
                                "log": "exits.log", "message": "Error: no such model", "missing": None}
    assert (tmp_path / "exits.log").read_text() == "starting\nError: no such model\nfatal\n"

    # It exited 1 and printed nothing: the message says so, and the log is empty.
    doc, _, _ = ran(op("p8-107-false"), "false")
    assert doc["simulator"] == {"backend": "p8-107-false", "dialect": "ngspice", "exit_status": 1,
                                "log": "false.log", "message": "false exited with exit status: 1", "missing": None}
    assert (tmp_path / "false.log").read_text() == ""

    # Its program is not there: R0041, at no statement, and no log.
    doc, _, results = ran(op("p8-107-ghost"), "ghost")
    assert (doc["stage"], doc["status"], results) == ("run", "failed", None)
    assert [(d["code"], d["line"]) for d in doc["diagnostics"]] == [("R0041", None)]
    sim = doc["simulator"]
    assert sim["message"].startswith("R0041 the simulator's command is not installed: ")
    assert {**sim, "message": None} == {"backend": "p8-107-ghost", "dialect": "ngspice", "exit_status": None,
                                        "log": None, "message": None, "missing": None}
    assert not (tmp_path / "ghost.log").exists()
