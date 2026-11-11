"""The public API against a checked-in snapshot (D13, D24): a documented name that is removed,
renamed or given another signature fails here, and changes only with a 2.0.

Oracle: `tests/fixtures/api_snapshot.txt`, written by hand from the docs (`docs/simulate.md`
"Your own simulator", `docs/dialects.md`): one line per name, `<dotted name><signature>`,
compared as a list. `ROOTS` are the objects snapshotted so far, the custom simulators' (D38);
a row that freezes more adds its roots and its lines. The provisional names (`PROVISIONAL`;
the package lists its own in `spicerack.provisional`) are outside the freeze: never in the
snapshot, whether this build has them or not. The descriptor's words are frozen with it
(note 12 §2, §8): each is still taken.
"""
import inspect
from pathlib import Path

import spicerack as ps

SNAPSHOT = Path(__file__).parent / "fixtures" / "api_snapshot.txt"
# A class (itself and each public attribute), a function, or one method of a class whose
# other names are not snapshotted yet.
ROOTS = [
    "spicerack.Simulator",
    "spicerack.register_simulator",
    "spicerack.simulators",
    "spicerack.load_simulator",
    "spicerack.def_.DialectDef.runs_on",
    "spicerack.def_.Lang.runs_on",
]
# Note 12 §5: the hooks and the view they are handed, as `spicerack.provisional` names them.
PROVISIONAL = {"Simulator.read", "Dialect.emit", "View"}

# Note 12 §2 and §4, and `docs/simulate.md`, copied by hand, in the order a descriptor reads
# them back.
OUTPUTS = ["ngspice-raw", "ngspice-ascii", "raw-dir", "ltspice-raw", "nutascii"]
ANALYSES = ["op", "dc", "ac", "tran", "noise", "tf", "sens", "sens_ac", "pz", "disto", "pss", "hb", "sp", "stb",
            "trannoise", "pac", "pnoise", "pxf", "pstb"]
FEATURES = ["xspice", "osdi", "measures", "step", "control", "laplace", "verilog"]


def signature(ob):
    """`ob`'s signature as `inspect` prints it; empty for what has none (a property)."""
    try:
        return str(inspect.signature(ob))
    except (TypeError, ValueError):
        return ""


def lines(root):
    """`root`'s line and, of a class, one per public attribute, sorted."""
    ob = ps
    for part in root.split(".")[1:]:
        ob = getattr(ob, part)
    out = [root + signature(ob)]
    if inspect.isclass(ob):
        out += [f"{root}.{n}{signature(getattr(ob, n))}" for n in sorted(dir(ob)) if not n.startswith("_")]
    return out


def test_public_api_matches_the_snapshot():
    # Whatever the package calls provisional is excluded here, by name.
    assert set(getattr(ps, "provisional", ())) <= PROVISIONAL
    got = [line for root in ROOTS for line in lines(root)]
    got = [line for line in got if line.split("(")[0].removeprefix("spicerack.") not in PROVISIONAL]
    assert got == SNAPSHOT.read_text().splitlines()


def test_p8_210_a_simulator_descriptor_takes_every_frozen_word():
    def sim(**over):
        fields = dict(dialect="ngspice", command=["x", "{deck}", "{raw}", "{dir}/x.log"], output="ngspice-raw", analyses=["op"])
        return ps.Simulator("p8-210-word", **{**fields, **over})

    assert [sim(output=o).output for o in OUTPUTS] == OUTPUTS
    assert sim(analyses=ANALYSES).analyses == ANALYSES
    assert sim(features=FEATURES).features == FEATURES
    assert sim(verilog=True, verilog_a=True).features == ["osdi", "verilog"]
    assert sim().command == ["x", "{deck}", "{raw}", "{dir}/x.log"]
