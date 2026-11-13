"""cmp.py FAMILYDIR ESPICE_OUT: each deck's ESPice reading against runs.out's ngspice and VACASK
readings: which it matches (rel 1e-6), or neither."""
import re, sys
runs = open(sys.argv[1] + "/runs.out").read()
ref = {}
for m in re.finditer(r"(?m)^(\S+)\s+ngspice=(\S+)\s+vacask=(\S+)", runs):
    ref[m.group(1)] = (m.group(2), m.group(3))
for line in open(sys.argv[2]):
    m = re.match(r"(\S+)\s+espice=(\S+)", line)
    if not m:
        if line.strip() and not line.startswith("#"): print(line.rstrip())
        continue
    n, e = m.group(1), float(m.group(2))
    if n not in ref: print(f"{n:22} espice={e:.6e} (no reference)"); continue
    def close(x):
        try: x = float(x)
        except ValueError: return False
        return abs(abs(x) - abs(e)) <= 1e-6 * max(abs(x), abs(e), 1e-300) + 1e-14
    a, b = close(ref[n][0]), close(ref[n][1])
    tag = "both" if a and b else "ngspice" if a else "vacask" if b else "NEITHER"
    print(f"{n:22} {tag:8} espice={e:.6e} ngspice={ref[n][0]} vacask={ref[n][1]}")
