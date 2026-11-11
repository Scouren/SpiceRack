"""Failures and notes as data (ZERO D14, ZERO-LEDGER P8-105): every SpiceRack failure is a
`spicerack.Error` (a `RuntimeError`) with a `status` and its `diagnostics`; a written deck's
notes come back from `netlist_with_notes`.

Oracles: `test_python_conformance.EXPECTED`, the matrix of refusal codes written by hand; the
lines, columns and statements of small decks, counted by hand from the cards the calls write;
the catalogue's fixed text for the codes they meet, copied by hand (`src/syntax/diag/rows_*.rs`);
and a plugin's own declaration.
"""
import warnings

import pytest

import spicerack as ps
import test_python_conformance as conf
from spicerack.def_ import Construct, Kind
from test_simulators import divider, fake

# The codes raised before the deck has a statement for what they refuse: an analysis argument
# the target has no word for (`src/python/deck.rs::unsupported`). One diagnostic, at no statement.
NO_STATEMENT = {"W0040"}


def noted():
    """An RC whose deck holds, after its title: `R1`, `C1`, `Vin`, `.save v(vout)` (line 5),
    `.meas tran vmax max v(vout)` (line 6), `.tran`."""
    rc = ps.Subcircuit("rc", ["vin", "vout"])
    rc.R("1", "vin", "vout", 1e3)
    rc.C("1", "vout", rc.gnd, 1e-9)
    tb = ps.Testbench(rc)
    tb.V("in", "vin", "0", 1.0)
    tb.save("v(vout)")
    tb.measure("tran", "vmax", "max", "v(vout)")
    tb.add_transient(step_time=1e-8, end_time=5e-6)
    return tb


def am():
    """A deck whose line 3 is the source `Vin a 0 ... AM(...)`, after its title and `Rload`."""
    top = ps.Subcircuit("top", ["a"])
    top.R("load", "a", top.gnd, 50)
    tb = ps.Testbench(top)
    tb.AmplitudeModulatedVoltageSource(
        "in", "a", "0", offset=0, amplitude=1, modulating_frequency=1e4, carrier_frequency=1e6)
    tb.add_operating_point()
    return tb


def placed(d):
    return d.code, d.kind, d.line, d.column, d.statement, d.element


def test_p8_105_every_refusal_is_an_error_with_its_cells_codes():
    """Oracle: `EXPECTED`. A refused cell raises `spicerack.Error`, status `refused`, whose
    diagnostics hold exactly the cell's codes and whose messages are the text's lines; a
    written cell returns its deck with notes only. The codes a refusal is read by as text
    (`cell`) and as data cannot drift."""
    wrong = []
    for name, build in conf.FEATURES.items():
        for target, cell in zip(conf.TARGETS, conf.EXPECTED[name]):
            want = ("written", []) if cell == conf.OK else ("refused", cell.split("+"))
            try:
                _, found = build(ps).netlist_with_notes(target)
                got = ("written", sorted({d.code for d in found if d.kind != "note"}))
            except ps.Error as e:
                found = e.diagnostics
                got = (e.status, sorted({d.code for d in found}))
                if "\n".join(d.message for d in found) != str(e):
                    wrong.append(f"{name} on {target}: the messages are not the text's lines")
                if not any(d.kind == "refusal" for d in found):
                    wrong.append(f"{name} on {target}: refused by no refusal")
            if got != want:
                wrong.append(f"{name} on {target}: {got} (expected {want})")
            for d in found:
                place = (d.line, d.column, d.statement, d.element)
                nowhere = place == (None,) * 4
                if nowhere != (d.code in NO_STATEMENT) or d.file is not None:
                    wrong.append(f"{name} on {target}: {d.code} at {place}")
                elif not nowhere and (d.line < 1 or d.column < 1 or not d.statement):
                    wrong.append(f"{name} on {target}: {d.code} at {place}")
    assert wrong == []


def test_p8_105_a_diagnostic_names_its_statement():
    """Oracle: the decks' cards counted by hand (`noted`, `am`), and the catalogue's text
    for W0023, W0109 and W0108, copied by hand."""
    with pytest.raises(ps.Error) as exc:
        noted().netlist("vacask")
    e = exc.value
    assert isinstance(e, RuntimeError) and e.status == "refused"
    assert str(e) == ("rc:5:1: W0023 an output request not written\n"
                      "rc:6:1: W0109 a measurement the target has no statement for")
    assert [placed(d) for d in e.diagnostics] == [
        ("W0023", "note", 5, 1, ".save v(vout)", None),
        ("W0109", "refusal", 6, 1, ".meas tran vmax max v(vout)", None),
    ]
    assert [d.message for d in e.diagnostics] == str(e).splitlines()
    assert [d.title for d in e.diagnostics] == [
        "an output request not written", "a measurement the target has no statement for"]
    assert all(d.cite and d.file is None for d in e.diagnostics)
    assert repr(e.diagnostics[1]) == 'Diagnostic("rc:6:1: W0109 a measurement the target has no statement for")'

    # An element's statement names the element.
    with pytest.raises(ps.Error) as exc:
        am().netlist("xyce")
    [d] = exc.value.diagnostics
    assert (d.code, d.kind, d.line, d.column, d.element) == ("W0108", "refusal", 3, 1, "Vin")
    assert d.statement.split()[:3] == ["Vin", "a", "0"] and "AM(" in d.statement
    assert d.title == "a source waveform the target cannot run"

    # The same refusal twice is the same data; another is not.
    with pytest.raises(ps.Error) as again:
        am().netlist("xyce")
    assert again.value.diagnostics == [d] and e.diagnostics[0] != d


def test_p8_105_a_plugins_refusal_is_located_and_a_hook_that_raises_fails():
    """Oracle: the plugin's own declaration (its code's id and message) and the card's line in
    the text given; the exception the hook raises, written out by hand."""
    class Loud(ps.dialects.NGSPICE):
        def define(self, d):
            return (super().define(d)
                    .code("LOUD1", Kind.Refusal, "loud has no Laplace sources")
                    .refuse_code(Construct.Laplace, "LOUD1")
                    .hook(".noise", self.boom))

        def boom(self, cards):
            raise KeyError("boom")

    loud = ps.register_dialect(Loud("p8-105-loud")).name()
    laplace = ps.Testbench.from_source(
        "* t\nV1 in 0 1\nB1 out 0 V=Laplace(v(in), 1/(1+s*1e-3))\nR1 out 0 1k\n.op\n.end\n",
        dialect="ngspice")
    with pytest.raises(ps.Error) as exc:
        laplace.netlist(loud)
    [d] = exc.value.diagnostics
    assert (exc.value.status, d.code, d.kind, d.line, d.column, d.element) == (
        "refused", "W0201", "refusal", 3, 1, "B1")
    assert d.message == str(exc.value)
    assert d.message.endswith(":3:1: W0201 p8-105-loud:LOUD1 loud has no Laplace sources")
    assert d.statement.startswith("B1 out 0 ")

    # The hook ran and raised: a failure, not a refusal, and no code.
    noise = ps.Testbench.from_source(
        "* t\nV1 in 0 1 AC 1\nR1 in out 1k\nR2 out 0 1k\n.noise v(out) V1 dec 10 1 1k\n.end\n",
        dialect="ngspice")
    with pytest.raises(ps.Error) as exc:
        noise.netlist(loud)
    assert (exc.value.status, exc.value.diagnostics) == ("failed", [])
    assert str(exc.value) == "hook '.noise': KeyError: 'boom'"


def test_p8_105_a_written_decks_notes_come_back_as_data():
    """Oracle: line 5 of `noted`'s deck, counted by hand; and `netlist()` itself, whose text
    and warnings (P1-028) the twin must agree with."""
    with warnings.catch_warnings():
        warnings.simplefilter("error")  # the twin returns its notes and warns of none
        text, notes = noted().netlist_with_notes("xyce")
        assert noted().netlist_with_notes("ngspice") == (noted().netlist("ngspice"), [])
    assert [placed(d) for d in notes] == [("W0023", "note", 5, 1, ".save v(vout)", None)]
    with pytest.warns(UserWarning) as seen:
        assert noted().netlist("xyce") == text
    assert [str(w.message) for w in seen] == ["spicerack rc:5:1: W0023 an output request not written"]
    assert [str(w.message) for w in seen] == ["spicerack " + d.message for d in notes]

    # A bad argument stays a ValueError, on both.
    for call in (noted().netlist, noted().netlist_with_notes):
        with pytest.raises(ValueError, match="unknown backend 'nope'") as bad:
            call("nope")
        assert not isinstance(bad.value, ps.Error)


def test_p8_105_a_run_that_is_refused_or_fails_is_an_error(tmp_path):
    """Oracle: note 12 §4's table (code, kind, title), copied by hand, for a simulator that
    declares too little and one whose program is missing; `false`, which fails with no code."""
    ps.register_simulator(fake("p8-105-fake"))
    ps.register_simulator(fake("p8-105-ghost", command=["p8-105-no-such-program", "{deck}", "{raw}"]))
    ps.register_simulator(fake("p8-105-false", command=["false", "{deck}", "{raw}"]))

    def error(run):
        with pytest.raises(ps.Error) as exc:
            run()
        return exc.value

    # Routed nowhere: nothing ran.
    sim = divider().simulator(simulator="p8-105-fake")
    e = error(lambda: sim.transient(step_time=1e-6, end_time=1e-3))
    [d] = e.diagnostics
    assert (e.status, d.code, d.kind, d.title) == (
        "refused", "W0180", "refusal", "an analysis the chosen simulator does not declare")
    assert (d.file, d.line, d.column, d.statement, d.element) == (None,) * 5
    assert d.message == str(e) and str(e).startswith("W0180 ")
    assert e.diagnostic["stage"] == "route"  # P8-206's attribute stays

    # Routed, and its program is not there.
    e = error(divider().simulator(simulator="p8-105-ghost").operating_point)
    assert (e.status, [(d.code, d.kind) for d in e.diagnostics]) == ("failed", [("R0041", "note")])
    assert e.diagnostic["stage"] == "run"

    # A simulator that ran and failed has no code.
    e = error(divider().simulator(simulator="p8-105-false").operating_point)
    assert (e.status, e.diagnostics) == ("failed", []) and not hasattr(e, "diagnostic")

    # A tool that fails outside a run.
    e = error(lambda: ps.compile_veriloga(str(tmp_path / "missing.va")))
    assert (e.status, e.diagnostics) == ("failed", [])
    assert str(e).startswith("Verilog-A file not found: ")


def test_p8_105_an_argument_no_target_word_holds_is_refused_at_no_statement():
    """Oracle: the catalogue's W0040 row, copied by hand; the argument is refused before the
    deck has a statement for its analysis, so the diagnostic names none."""
    tb = conf.FEATURES["ac_log"](ps)
    for call in (tb.netlist, tb.netlist_with_notes):
        with pytest.raises(ps.Error) as exc:
            call("ngspice")
        e = exc.value
        [d] = e.diagnostics
        assert (e.status, d.code, d.kind, d.title) == (
            "refused", "W0040", "refusal", "an analysis argument the target has no word for")
        assert (d.file, d.line, d.column, d.statement, d.element) == (None,) * 5
        assert d.message == str(e) == (
            "W0040 an analysis argument the target has no word for: "
            "frequency sweep 'log' (N points in total) on ngspice")
