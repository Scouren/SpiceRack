"""The perf gate (TESTING.md, "Performance"): runs a tier's workloads under
`perf stat` and compares their counts with `benches/perf.pins`.

    python3 benches/perf.py pre-commit|merge|nightly [--pin]

Gated: `instructions:u` (+3% fails) and allocations (+2% fails), both
independent of machine load; wall time is printed, never gated. A count
below its pin prints the new number; `--pin` writes every measured count
into the pins file (run it on the parent commit first, then on the change:
a perf commit cites both). Run it in `nix develop .#testing` (the corpora)
with `IHP_PDK` set; the merge tier's Python workloads need `maturin
develop` in the active venv. `$PERF` names the `perf` binary (default
`perf`).
"""

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PINS = ROOT / "benches" / "perf.pins"
TARGETS = "ngspice espice hspice xyce ltspice spectre vacask cdl verilog".split()
LIMIT = {"instr": 1.03, "allocs": 1.02}

# name -> argv; "BENCH" is the perfbench binary, "PY" the venv's Python,
# "BIN"/"ASCII" the generated raw files.
TIERS = {
    "pre-commit": {
        "render:small": ["BENCH", "render", "fixtures,hspice,vacask,pdk-vacask"],
    },
    "merge": {
        "tokenize:all": ["BENCH", "tokenize", "-"],
        "parse:all": ["BENCH", "parse", "-"],
        "render:xyce": ["BENCH", "render", "xyce"],
        "render:pdk-sky130A": ["BENCH", "render", "pdk-sky130A"],
        "raw:bin-200x100k": ["BENCH", "raw", "BIN"],
        "raw:ascii-50x20k": ["BENCH", "raw", "ASCII"],
        "py:build": ["PY", "benches/pybench.py", "build"],
        **{f"py:netlist-{t}": ["PY", "benches/pybench.py", "netlist", t]
           for t in ("ngspice", "spectre", "xyce", "vacask")},
        "py:from_source": ["PY", "benches/pybench.py", "from_source"],
        "py:read_raw": ["PY", "benches/pybench.py", "read_raw", "BIN"],
        "py:columns": ["PY", "benches/pybench.py", "columns", "BIN"],
    },
    "nightly": {
        f"render:spice-bench-{t}": ["BENCH", "render", "spice-bench", t] for t in TARGETS
    },
}


def build():
    """Builds perfbench (release, no Python) and returns its path."""
    out = subprocess.run(
        ["cargo", "build", "--release", "--no-default-features", "--bench", "perfbench",
         "--message-format=json-render-diagnostics"],
        cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True).stdout
    exes = [m["executable"] for m in map(json.loads, out.splitlines())
            if m.get("reason") == "compiler-artifact" and m.get("executable")
            and m["target"]["name"] == "perfbench"]
    assert len(exes) == 1, exes
    return exes[0]


def measure(argv, tmp):
    """Runs argv under `perf stat`, counting only its measured region."""
    ctl, ack, csv = (os.path.join(tmp, n) for n in ("ctl", "ack", "stat.csv"))
    for f in (ctl, ack):
        if not os.path.exists(f):
            os.mkfifo(f)
    env = dict(os.environ, PERFBENCH_CTL=ctl, PERFBENCH_ACK=ack)
    run = subprocess.run(
        [os.environ.get("PERF", "perf"), "stat", "--no-scale", "-x,", "-e", "instructions:u", "-D", "-1",
         "--control", f"fifo:{ctl},{ack}", "-o", csv, "--", *argv],
        cwd=ROOT, env=env, stdout=subprocess.PIPE, text=True)
    if run.returncode != 0:
        sys.exit(f"perf.py: {' '.join(argv)} exited {run.returncode}")
    line = [l for l in run.stdout.splitlines() if l.startswith("bench ")][-1]
    got = dict(re.findall(r"(\w+)=(\S+)", line))
    # A hybrid CPU reports one row per core type (cpu_core, cpu_atom), each
    # counting while the process runs on that type: their unscaled sum.
    rows = [l.split(",") for l in Path(csv).read_text().splitlines() if "instructions" in l]
    got["instr"] = sum(int(r[0]) for r in rows if r[0].isdigit())
    return got


def main(argv):
    if not argv or argv[0] not in TIERS or argv[1:] not in ([], ["--pin"]):
        sys.exit(__doc__)
    tier, pin = argv[0], argv[1:] == ["--pin"]
    pins = {}
    if PINS.exists():
        for line in PINS.read_text().splitlines():
            if line and not line.startswith("#"):
                name, *kv = line.split()
                pins[name] = {k: int(v) for k, v in (p.split("=") for p in kv)}
    bench, failed = build(), []
    with tempfile.TemporaryDirectory() as tmp:
        files = {"BIN": os.path.join(tmp, "bin.raw"), "ASCII": os.path.join(tmp, "ascii.raw")}
        if tier == "merge":
            subprocess.run([bench, "rawgen", files["BIN"], "bin", "200", "100000"], check=True)
            subprocess.run([bench, "rawgen", files["ASCII"], "ascii", "50", "20000"], check=True)
        subst = {"BENCH": bench, "PY": sys.executable, **files}
        for name, argv in TIERS[tier].items():
            got = measure([subst.get(a, a) for a in argv], tmp)
            now = {k: int(got[k]) for k in ("instr", "allocs") if k in got}
            was = pins.get(name, {})
            verdict = []
            for k, v in now.items():
                if k not in was:
                    verdict.append(f"{k} unpinned")
                elif v > was[k] * LIMIT[k]:
                    verdict.append(f"{k} +{v / was[k] - 1:.1%} FAIL")
                    failed.append(name)
                elif v != was[k]:
                    verdict.append(f"{k} {v / was[k] - 1:+.1%}")
            print(f"{name:28s} " + " ".join(f"{k}={v}" for k, v in now.items())
                  + f" secs={got.get('secs', '?')} mb_per_s={got.get('mb_per_s', '?')}  " + ", ".join(verdict), flush=True)
            pins[name] = now
    if pin:
        PINS.write_text("# benches/perf.py --pin: workload instr=<instructions:u> allocs=<n>\n"
                        + "".join(f"{n} " + " ".join(f"{k}={v}" for k, v in sorted(c.items()))
                                  + "\n" for n, c in sorted(pins.items())))
    if failed and not pin:
        sys.exit(f"perf.py: regressed: {' '.join(failed)}")


if __name__ == "__main__":
    main(sys.argv[1:])
