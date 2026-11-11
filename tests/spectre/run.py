#!/usr/bin/env python3
"""T-S1 to T-S3 and T-M1 to T-M4 (the tests in tests/spectre/expected.json), and the
pending-real-run decks `p_*.scs` (tests/spectre/pending.json, written by
tests/test_pending_real_run.rs: one deck per construct row the Spectre writer
spells, no Spectre run yet), on a real Spectre.

Each deck here is what SpiceRack writes for a SPICE source (regenerated and
checked by tests/test_monte_carlo.rs). This script runs one deck, reads its
operating points (the nominal `op1` and, under `montecarlo`, every `dcmc`
iteration) and a transient's last values (`tran1`), and checks them against expected.json: the closed forms and the
ngspice reference, at the tolerances recorded there.

    python3 run.py t_s2_agauss.scs          # run spectre, then check
    python3 run.py t_s2_agauss.scs --raw DIR  # check an existing raw directory

Prints one PASS/FAIL line per check and a JSON summary to send back. Needs
`spectre` on PATH (it is called with `-format psfascii`, on a copy of the deck
with `savefamilyplots=yes` added to its `montecarlo`, so every iteration is kept:
`mc1-001_dcmc.dc`, ...). Checked on Spectre 21.1.
"""

import json
import math
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
# psfascii value rows: `"out" "V" 5.0e-01` (Spectre 21.1), or `"name" 5.0e-01`.
PAIR = re.compile(r'^\s*"([^"]+)"\s+(?:"[^"]*"\s+)?([-+0-9.eE]+)\s*$')
# One Monte Carlo iteration's results under savefamilyplots=yes: `mc1-001_dcmc.dc`.
ITERATION = re.compile(r"^[^-_]+-\d+_")


def values(path):
    """Every `"name" number` row of a psfascii file, in order."""
    out = {}
    with open(path, errors="replace") as f:
        for line in f:
            m = PAIR.match(line)
            if m:
                try:
                    out.setdefault(m.group(1), []).append(float(m.group(2)))
                except ValueError:
                    pass
    return out


def collect(raw):
    """(nominal values, per-iteration values, last transient values) from a Spectre raw directory."""
    nominal, runs, final = {}, {}, {}
    for root, _, files in os.walk(raw):
        for name in sorted(files):
            path = os.path.join(root, name)
            v = values(path)
            if ITERATION.match(name):
                for k, xs in v.items():
                    runs.setdefault(k, []).extend(xs)
            elif name.startswith("op1"):
                nominal.update({k: xs[0] for k, xs in v.items()})
            elif name.startswith("tran1"):
                final.update({k: xs[-1] for k, xs in v.items()})
    return nominal, runs, final


def stats(xs):
    n = len(xs)
    m = sum(xs) / n
    s = math.sqrt(sum((x - m) ** 2 for x in xs) / (n - 1)) if n > 1 else 0.0
    return m, s


def rho(a, b):
    (ma, sa), (mb, sb) = stats(a), stats(b)
    n = min(len(a), len(b))
    return sum((a[i] - ma) * (b[i] - mb) for i in range(n)) / (n - 1) / (sa * sb)


def pending(deck, want, raw):
    """A pending.json deck: each `"<analysis>:<vector>"` value against the last
    point of that analysis's psfascii file (`op1`, `tran1`, `dc1`), or a
    `measure:<name>` against the `.measure` output Spectre writes."""
    if want["status"].startswith("refused"):
        print(f"SKIP {deck}: {want['status']}")
        return True
    if "--raw" not in sys.argv:
        subprocess.run(["spectre", "-format", "psfascii", "-raw", raw, os.path.join(HERE, deck)], check=True)
    last = {}
    for root, _, files in os.walk(raw):
        for name in sorted(files):
            for k, xs in values(os.path.join(root, name)).items():
                prefix = "measure" if name.endswith((".measure", ".mt0")) else name.split(".")[0]
                last[f"{prefix}:{k}"] = xs[-1]
    # Spectre writes `.measure`/`.mt0` into the working directory, not the
    # raw one (Spectre 25.1, settle-remote 2026-11-07).
    stem = os.path.splitext(os.path.basename(deck))[0]
    for ext in (".measure", ".mt0"):
        if os.path.exists(stem + ext):
            for k, xs in values(stem + ext).items():
                last[f"measure:{k}"] = xs[-1]
    ok = True
    for key, value in want["values"].items():
        got = last.get(key)
        good = got is not None and abs(got - value) <= want["rel_tol"] * abs(value)
        ok &= good
        print(f"{'PASS' if good else 'FAIL'} {deck} {key}: {got} vs {value}")
    return ok


def main():
    deck = sys.argv[1]
    if os.path.basename(deck).startswith("p_"):
        want = json.load(open(os.path.join(HERE, "pending.json")))[os.path.basename(deck)]
        raw = sys.argv[sys.argv.index("--raw") + 1] if "--raw" in sys.argv else os.path.splitext(deck)[0] + ".raw"
        sys.exit(0 if pending(deck, want, raw) else 1)
    want = json.load(open(os.path.join(HERE, "expected.json")))[os.path.basename(deck)]
    raw = sys.argv[sys.argv.index("--raw") + 1] if "--raw" in sys.argv else os.path.splitext(deck)[0] + ".raw"
    if "--raw" not in sys.argv:
        # Spectre keeps only the last iteration unless asked for every one (a run option,
        # not part of the deck SpiceRack writes).
        text = open(os.path.join(HERE, deck)).read()
        text = re.sub(r"(\bmontecarlo\b[^{\n]*)\{", r"\1 savefamilyplots=yes {", text)
        run_deck = os.path.splitext(raw)[0] + ".run.scs"
        open(run_deck, "w").write(text)
        subprocess.run(["spectre", "-format", "psfascii", "-raw", raw, run_deck], check=True)
    nominal, runs, final = collect(raw)
    summary = {"deck": deck, "nominal": nominal, "final": final, "runs": {k: len(v) for k, v in runs.items()}}
    ok = True

    def check(name, good, detail):
        nonlocal ok
        ok &= good
        print(f"{'PASS' if good else 'FAIL'} {deck} {name}: {detail}")

    if "nominal" in want:
        for k, v in want["nominal"].items():
            got = nominal.get(k)
            check(f"nominal {k}", got is not None and abs(got - v) <= want["rel_tol"] * abs(v), f"{got} vs {v}")
    if "final" in want:
        for k, v in want["final"].items():
            got = final.get(k)
            check(f"final {k}", got is not None and abs(got - v) <= want["rel_tol"] * abs(v), f"{got} vs {v}")
    if "probe" in want:
        xs = runs.get(want["probe"], [])
        if not xs:
            check("iterations", False, f"no {want['probe']} values; files: {sorted(os.listdir(raw))}")
        else:
            m, s = stats(xs)
            summary["stats"] = {"n": len(xs), "mean": m, "std": s}
            check("mean", abs(m - want["mean"]) < want["mean_tol"], f"{m:.6e} vs {want['mean']:.6e} (ngspice {want['ngspice']['mean']:.6e})")
            check("std", abs(s - want["std"]) < want["std_tol"], f"{s:.6e} vs {want['std']:.6e} (ngspice {want['ngspice']['std']:.6e})")
            if "values" in want:
                off = [x for x in xs if min(abs(x - v) for v in want["values"]) > want["value_tol"]]
                check("two values", not off, f"{len(off)} values off {want['values']}")
    if "pairs" in want:
        summary["rho"] = {}
        for a, b in want["pairs"]:
            xa, xb = runs.get(a, []), runs.get(b, [])
            if not xa or not xb:
                check(f"rho {a},{b}", False, f"no values; files: {sorted(os.listdir(raw))}")
                continue
            r = rho(xa, xb)
            summary["rho"][f"{a},{b}"] = r
            check(f"rho {a},{b}", abs(r) < want["abs_rho_below"], f"{r:+.4f}")
        for k, s0 in want["std"].items():
            xs = runs.get(k, [])
            if xs:
                s = stats(xs)[1]
                check(f"std {k}", abs(s - s0) < want["std_tol"], f"{s:.6e} vs {s0:.6e}")
    print(json.dumps(summary, indent=1))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
