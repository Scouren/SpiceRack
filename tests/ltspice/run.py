#!/usr/bin/env python3
"""The pending-real-run decks (tests/ltspice/pending.json) on a real LTspice.

Each `p_*.cir` here is what SpiceRack writes for LTspice from a SPICE source in
tests/test_pending_real_run.rs; pending.json holds the values a run must give
(`"op1:out"`: the operating point, `"tran1:q"`/`"dc1:out"`: the last point of
that analysis, `"measure:vmax"`: a `.meas` result in the log), the closed forms
ngspice-45 meets on the source. A deck whose status is `refused ...` was not
written. No LTspice has run these yet: send back the PASS/FAIL lines.

    python3 run.py p_elements.cir            # LTspice -b, then check
    python3 run.py p_elements.cir --raw F    # check an existing raw file (and F's .log)

Needs `LTspice` on PATH (or $LTSPICE), and the spicerack Python package for
read_raw (LTspice's UTF-16 raw format).
"""

import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    deck = sys.argv[1]
    want = json.load(open(os.path.join(HERE, "pending.json")))[os.path.basename(deck)]
    if want["status"] != "pending-real-run":
        print(f"SKIP {deck}: {want['status']}")
        return
    if "--raw" in sys.argv:
        raw = sys.argv[sys.argv.index("--raw") + 1]
    else:
        subprocess.run([os.environ.get("LTSPICE", "LTspice"), "-b", os.path.join(HERE, deck)], check=True)
        raw = os.path.splitext(os.path.join(HERE, deck))[0] + ".raw"
    from spicerack import read_raw

    plots = read_raw(raw)
    log = open(os.path.splitext(raw)[0] + ".log", errors="replace").read() if "measure:" in str(want) else ""
    ok = True
    for key, value in want["values"].items():
        analysis, vector = key.split(":")
        if analysis == "measure":
            m = re.search(rf"^{vector}\s*[:=].*?=?\s*([-+0-9.eE]+)", log, re.M | re.I)
            got = float(m.group(1)) if m else None
        else:
            got = None
            for p in plots:
                for name in (f"V({vector})", vector):
                    try:
                        got = list(p[name])[-1]
                        break
                    except (KeyError, IndexError):
                        pass
        good = got is not None and abs(got - value) <= want["rel_tol"] * abs(value)
        ok &= good
        print(f"{'PASS' if good else 'FAIL'} {deck} {key}: {got} vs {value}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
