"""The perf suite's Python workloads (TESTING.md, "Performance"): the
builders (measured only, S3's AST builders replace them) and reading a raw
file into numpy.

    pybench.py build|netlist|from_source [target]
    pybench.py read_raw|columns <raw file>

Each run prints one line, `bench <key>=<value> ...`. With `PERFBENCH_CTL`
and `PERFBENCH_ACK` set (`benches/perf.py`), `perf stat` counts only the
measured region, so the interpreter's start and the import are left out.
"""

import os
import sys
import time

import numpy as np
import spicerack as ps

N = 10_000


class Region:
    """The measured region: `perf stat` counts between enter and exit."""

    def __enter__(self):
        ctl, ack = os.environ.get("PERFBENCH_CTL"), os.environ.get("PERFBENCH_ACK")
        self.fifos = (open(ctl, "w"), open(ack)) if ctl and ack else None
        self.send("enable")
        self.t0 = time.perf_counter()
        return self

    def __exit__(self, *exc):
        self.secs = time.perf_counter() - self.t0
        self.send("disable")

    def send(self, cmd):
        if self.fifos:
            self.fifos[0].write(cmd + "\n")
            self.fifos[0].flush()
            assert self.fifos[1].readline().strip("\0\n ") == "ack", cmd


def build():
    d = ps.Subcircuit("big", ["a", "b"])
    for i in range(N):
        d.R(str(i), f"n{i}", f"n{i + 1}", 1e3)
    return d


def main(argv):
    work, arg = argv[0], argv[1] if len(argv) > 1 else "ngspice"
    if work == "build":
        with Region() as r:
            build()
    elif work == "netlist":
        tb = ps.Testbench(build())
        tb.V("s", "a", "0", 1.0)
        with Region() as r:
            tb.netlist(arg)
    elif work == "from_source":
        src = build().to_source()
        with Region() as r:
            ps.Subcircuit.from_source(src)
    elif work == "read_raw":
        with Region() as r:
            ps.read_raw(arg)
    elif work == "columns":
        p = ps.read_raw(arg)[0]  # perf.py's file: v(n1) .. v(n199)
        with Region() as r:
            for i in range(1, 200):
                np.asarray(p[f"v(n{i})"], float)
    else:
        sys.exit(f"usage: {__doc__}")
    print(f"bench py {work} arg={arg} n={N} secs={r.secs:.4f}")


if __name__ == "__main__":
    main(sys.argv[1:])
