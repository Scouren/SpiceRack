"""A fake simulator for the simulator registry's oracles (D38 note 12 §6, O1).

    python3 fake.py <deck> <raw> [<log>]

Reads an ngspice-family deck and writes an ngspice ASCII raw file (manual
§17.6) at <raw>: one plot per `.op` or `.dc` line, one variable per `.save`d
node. A node's value is a closed form of its name (`value`), times the swept
value in a `.dc` plot, so a test computes what it must read back without a
simulator. Every other line is ignored: the fake declares op and dc, and the
router keeps the rest away from it.

`$FAKE_ARGV`, when set, names a file that receives this run's arguments as
JSON. <log>, when given, receives one line.
"""
import hashlib
import json
import os
import re
import sys


def value(node):
    """sha256(name)'s first 8 bytes as a fraction of 2**64: in [0, 1)."""
    return int.from_bytes(hashlib.sha256(node.encode()).digest()[:8], "big") / 2**64


def plot(title, name, nodes, sweep):
    """One ASCII plot; `sweep` is None (one point) or the swept values (variable 0)."""
    names = ([] if sweep is None else ["v-sweep"]) + [f"v({n})" for n in nodes]
    points = [None] if sweep is None else sweep
    out = [f"Title: {title}", "Date: fake", f"Plotname: {name}", "Flags: real"]
    out += [f"No. Variables: {len(names)}", f"No. Points: {len(points)}", "Variables:"]
    out += [f"\t{i}\t{n}\tvoltage" for i, n in enumerate(names)]
    out.append("Values:")
    for i, x in enumerate(points):
        row = [value(n) if x is None else value(n) * x for n in nodes]
        row = row if x is None else [x] + row
        out += [f"{i}\t{row[0]!r}"] + [f"\t{v!r}" for v in row[1:]]
    return "\n".join(out) + "\n"


def main(argv):
    deck, raw = argv[1], argv[2]
    if os.environ.get("FAKE_ARGV"):
        with open(os.environ["FAKE_ARGV"], "w") as f:
            json.dump(argv[1:], f)
    lines = open(deck).read().splitlines()
    saved = [m for l in lines if l.lower().startswith(".save") for m in re.findall(r"(?i)\bv\(([^)]+)\)", l)]
    nodes = [n.lower() for n in saved]
    plots = []
    for l in lines[1:]:
        words = l.lower().split()
        if words[:1] == [".op"]:
            plots.append(plot(lines[0], "Operating Point", nodes, None))
        elif words[:1] == [".dc"]:
            start, stop, step = (float(w) for w in words[2:5])
            sweep = [start + i * step for i in range(round((stop - start) / step) + 1)]
            plots.append(plot(lines[0], "DC transfer characteristic", nodes, sweep))
    with open(raw, "w") as f:
        f.write("".join(plots))
    if len(argv) > 3:
        with open(argv[3], "w") as f:
            f.write("fake ran\n")


if __name__ == "__main__":
    main(sys.argv)
