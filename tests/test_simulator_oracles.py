"""The simulator registry's oracles (D38 note 12 §6): what O1 (a fake simulator) and O3
(determinism) still lacked after `test_simulators.py`, and O2, parity.

O1, O3: the fake simulator (`tests/fixtures/sims/fake/fake.py`) run as a real process. It
writes the arguments it was started with to `$FAKE_ARGV` and one closed-form value per saved
node, so the paths a command is given, the simulator a route picked and the order of the
registry are read from outside SpiceRack.

O2: the built-in itself. `TWINS` registers ngspice, ESPice and VACASK again through the public
call (`ngspice2`, ...), each as the command its built-in runs, with the analyses and features
its built-in holds (`src/backend/kind.rs`'s routes and `src/backend/mod.rs`'s `*_CAPS`, copied
by hand: a wrong copy is a refusal on one side only). Every conformance deck
(`test_python_conformance.FEATURES`), the two closed-form benches and a deck that names a file
by a relative path run by name on both, and
the two runs must end alike: the same refusal (its type, its `diagnostic`, its text when it
carries a code; the twin's name read as the built-in's), or the same results, compared as the
bytes `spicerack.export` writes from them (every plot, vector, step and measure; no tolerance).
What a built-in's run does beyond its command is the limit of a registered simulator (note 12
§8): each cell it shows in is a line of `PARITY_SIM_GAP` with its cause, compared as a name
list, so a new difference fails and so does a listed one that is gone.
"""
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

import spicerack as ps
import test_python_conformance as conf
from test_simulators import BUILT_INS, FAKE, divider, expected, fake


def test_p8_210_a_command_gets_absolute_paths_and_no_shell(tmp_path, monkeypatch):
    """Oracle: the arguments the fake was started with. Each placeholder is an absolute path
    inside one scratch directory (made under `$TMPDIR`, here a directory with a space in its
    name), a placeholder inside a longer word is joined to it, and a word a shell would split
    or expand is one argument, as written. The value read back is the fake's closed form."""
    scratch = tmp_path / "scratch dir"
    scratch.mkdir()
    monkeypatch.setenv("TMPDIR", str(scratch))
    monkeypatch.setenv("FAKE_ARGV", str(tmp_path / "argv.json"))
    words = [sys.executable, str(FAKE), "{deck}", "{raw}", "{dir}/fake.log", "--out={raw}", "two words", "$HOME;*"]
    ps.register_simulator(fake("p8-210-argv", command=words))
    sim = divider().simulator(simulator="p8-210-argv")
    sim.save("V(out)")
    assert sim.operating_point()["out"] == expected("out")

    deck, raw, log, option, *literal = json.loads((tmp_path / "argv.json").read_text())
    run = os.path.dirname(deck)
    assert os.path.dirname(run) == str(scratch)
    assert (deck, raw, log, option) == (f"{run}/deck.cir", f"{run}/deck.raw", f"{run}/fake.log", f"--out={run}/deck.raw")
    assert literal == ["two words", "$HOME;*"]
    # The scratch directory is the run's: gone once the results are read.
    assert not os.path.exists(run)


# Run in a fresh interpreter whose PATH holds `which` alone: no built-in is installed there
# (this process has asked already, once: `backend::available`), and its registry starts with
# the five built-ins (this one's never shrinks). argv: the fake's path, the `$FAKE_ARGV` file.
ALONE = r"""
import json, sys
import spicerack as ps

fake_py, argv_file = sys.argv[1:]

def fake(name, analyses):
    # The command's last word is the simulator's name: `$FAKE_ARGV` tells which one ran.
    command = [sys.executable, fake_py, "{deck}", "{raw}", "{dir}/fake.log", name]
    return ps.Simulator(name, dialect="ngspice", command=command, output="ngspice-ascii", analyses=analyses)

def run(analysis):
    c = ps.Circuit("divider")
    c.V("in", "in", c.gnd, 2.0)
    c.R("1", "in", "out", 1e3)
    c.R("2", "out", c.gnd, 3e3)
    sim = c.simulator()
    sim.save("V(out)")
    try:
        got = sim.operating_point()["out"] if analysis == "op" else list(sim.dc(Vin=slice(0, 2, 1))["out"])
    except RuntimeError as e:
        return str(e)
    return [got, json.load(open(argv_file))[-1]]

out = {"installed": ps.CircuitSimulator.available_backends(), "none": run("op")}
ps.register_simulator(fake("first", ["op"]))
ps.register_simulator(fake("second", ["op", "dc"]))
out["op"], out["dc"] = run("op"), run("dc")
n = 0
while len(ps.simulators()) < 32:
    ps.register_simulator(fake(f"filler-{n}", ["op"]))
    n += 1
out["names"] = ps.simulators()
try:
    ps.register_simulator(fake("one-too-many", ["op"]))
except ValueError as e:
    out["full"] = str(e)
out["held"] = ps.register_simulator(fake("first", ["op"]))
out["after"] = ps.simulators()
print(json.dumps(out))
"""


def test_p8_210_with_no_built_in_installed_the_registered_simulators_route_in_their_order(tmp_path):
    """Oracle: the fake's closed form and the name each fake is started with (`ALONE`); the
    capacity's arithmetic, 32 slots less the 5 built-ins: `first`, `second` and 25 fillers
    register, the 28th is refused (R0042, note 12 §4's title), and an equal `first` is still
    the one held."""
    which = shutil.which("which")
    assert which, "`which` is how a simulator's program is looked for (`backend::is_on_path`)"
    (tmp_path / "bin").mkdir()
    (tmp_path / "bin" / "which").symlink_to(which)
    argv = tmp_path / "argv.json"
    env = {k: v for k, v in os.environ.items() if k not in ("SPICERACK_BACKEND", "SPICERACK_LTSPICE")}
    env.update(PATH=str(tmp_path / "bin"), FAKE_ARGV=str(argv))
    done = subprocess.run([sys.executable, "-c", ALONE, str(FAKE), str(argv)], env=env, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    out = json.loads(done.stdout)

    # No built-in, nothing registered: nothing runs the deck.
    assert out["installed"] == []
    assert out["none"].startswith("No simulator backend found"), out["none"]
    # Auto-routing lands on a registered simulator: the first registered that declares the
    # analysis (`first` and `second` both run op; `second` alone runs dc).
    assert out["op"] == [expected("out"), "first"]
    assert out["dc"] == [[expected("out") * x for x in (0.0, 1.0, 2.0)], "second"]
    # The registry's order is the order of registration, the built-ins first.
    assert out["names"] == BUILT_INS + ["first", "second"] + [f"filler-{i}" for i in range(25)]
    assert out["full"] == "R0042 the simulator registry is full: the simulator registry is full (32 simulators)"
    assert (out["held"], out["after"]) == ("first", out["names"])


# ── O2: parity ──

TWINS = {
    "ngspice": dict(
        command=["ngspice", "-b", "-n", "-r", "{raw}", "{deck}"],
        output="ngspice-raw",
        analyses=["op", "dc", "ac", "tran", "noise", "tf", "sens", "sens_ac", "pz", "disto", "pss", "sp", "trannoise"],
        features=["xspice", "osdi", "measures", "control", "laplace", "verilog"],
    ),
    "espice": dict(
        command=["espice", "{deck}", "--rawfile", "{raw}", "--format=binary"],
        output="ngspice-raw",
        analyses=["op", "dc", "ac", "tran", "noise", "tf", "sens"],
        features=["osdi", "measures", "laplace"],
    ),
    "vacask": dict(
        command=["vacask", "-se", "-sp", "{deck}"],
        output="raw-dir",
        analyses=["op", "dc", "ac", "tran", "noise", "tf", "hb"],
        features=["osdi", "step"],
    ),
}


def bench(mod):
    """The deck of `test_written_decks_meet_the_closed_form`: four analyses of the divider."""
    tb = mod.Testbench(conf.divider(mod))
    tb.V("in", "in", "0", 1.0, ac=1.0)
    tb.add_operating_point()
    tb.add_dc(Vin=slice(0.0, 1.0, 0.5))
    tb.add_ac("dec", 2, 1e3, 1e6)
    tb.add_transient(1e-8, 5e-6, None, 1e-8)
    return tb


def noise_bench(mod):
    """Its noise deck."""
    tb = mod.Testbench(conf.divider(mod))
    tb.V("in", "in", "0", 1.0, ac=1.0)
    tb.add_noise("out", "0", "Vin", "dec", 1, 1e3, 1e5)
    return tb


def relative_include(mod):
    """The DUT includes `rel.inc` by a path relative to the working directory (the test makes
    both): the deck class of the built-in VACASK's path step."""
    return conf.elem(lambda d: d.include("rel.inc"))(mod)


DECKS = {**conf.FEATURES, "closed_form": bench, "closed_form_noise": noise_bench, "relative_include": relative_include}

# (twin, deck): how the twin departs from its built-in. "fails": it raises where the built-in
# runs; "other results": both run and the exported files differ. Each cause is a step of the
# built-in's run that is not in its command. (LTspice's is finding the program under Wine; it
# is not in the test shell, so it has no twin here.)
PARITY_SIM_GAP = {
    # `backend/ngspice.rs::append_write_block`. SpiceRack's ngspice deck runs a `.measure`'s or
    # a Fourier's analysis from a `.control` block; given such a deck and `-r`, ngspice-45
    # exits 0 and leaves no raw file. The built-in drops `-r` and appends a `write`.
    ("ngspice2", "fourier"): "fails",
    ("ngspice2", "fourier_count"): "fails",
    ("ngspice2", "measure"): "fails",
    # `backend/vacask.rs::absolutize_paths`: the built-in resolves a relative `load` or
    # `include` path against the working directory before the deck moves to the scratch
    # directory, where a `raw-dir` command runs.
    ("vacask2", "relative_include"): "fails",
}

# Cells the built-in itself does not repeat from one run to the next (ngspice-45): compared
# by each plot's name and vectors.
UNREPEATABLE = {
    ("ngspice2", "trnoise"): "a TRNOISE source is random per run",
    ("ngspice2", "trrandom"): "a TRRANDOM source is random per run, and with it the number of time points",
    ("ngspice2", "ac_sensitivity"): "the `frequency` vector of ngspice's AC sensitivity plot has an uninitialised imaginary part",
}


def outcome(build, name, base, stem):
    """How `build`'s deck ends, run by name on `name` (`base` or its twin): `("refused", ...)`,
    `("ran, no plot", ...)` for a deck with no analysis that writes one, or `("ran", raw,
    sidecar)`, the two files `export` wrote from the results at `stem`."""
    tb = build(ps)
    tb.with_backend(name)
    try:
        results = tb.run()
    except Exception as e:
        text = str(e).replace(name, base)
        told = json.dumps(getattr(e, "diagnostic", None), sort_keys=True).replace(name, base)
        # An uncoded failure is the simulator's own: its text holds the run's scratch paths.
        coded = re.search(r"\b[LPRW]\d{4}\b", text)
        return "refused", type(e).__name__, told, text if coded else "no code"
    if all(r is None for r in results):
        return "ran, no plot", len(results)
    stem.parent.mkdir(parents=True, exist_ok=True)
    try:
        raw, sidecar = ps.export(results, stem)
    except ValueError as e:
        return "unexported", str(e)
    return "ran", Path(raw), Path(sidecar)


def shape(ran):
    """What a run that does not repeat still repeats: each plot's name and vectors."""
    return [(p.plot_name, p.is_complex, p.variable_names) for p in ps.read_raw(str(ran[1]))]


def gap(a, b, by_shape):
    """How the twin's outcome `b` departs from the built-in's `a`; `None` when it does not."""
    if a[0] != "ran" or b[0] != "ran":
        if a == b:
            return None
        return "fails" if a[0] == "ran" else "runs" if b[0] == "ran" else "another refusal"
    read = shape if by_shape else (lambda ran: (ran[1].read_bytes(), ran[2].read_text()))
    return None if read(a) == read(b) else "other results"


@pytest.mark.parametrize("base", sorted(TWINS))
def test_p8_210_a_built_in_registered_again_as_a_command_runs_the_same(base, tmp_path, monkeypatch):
    if shutil.which(base) is None:
        pytest.fail(f"{base} not on PATH: run the tests in `nix develop .#testing`")
    # `relative_include`'s file, in the working directory.
    (tmp_path / "rel.inc").write_text("")
    monkeypatch.chdir(tmp_path)
    twin = ps.register_simulator(ps.Simulator(base + "2", dialect=base, **TWINS[base]))
    assert twin == base + "2"

    built_in, gaps = {}, {}
    for deck, build in DECKS.items():
        a = built_in[deck] = outcome(build, base, base, tmp_path / base / deck)
        b = outcome(build, twin, base, tmp_path / twin / deck)
        how = gap(a, b, (twin, deck) in UNREPEATABLE)
        if how:
            gaps[(twin, deck)] = how

    # Every run's results are files, so "the same" was read from bytes.
    assert [deck for deck, a in built_in.items() if a[0] == "unexported"] == []
    # The comparison is not empty: the benches run (their numbers are the conformance test's).
    assert [built_in[deck][0] for deck in ("closed_form", "closed_form_noise")] == ["ran", "ran"]
    assert gaps == {cell: how for cell, how in PARITY_SIM_GAP.items() if cell[0] == twin}
