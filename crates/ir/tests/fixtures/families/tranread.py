"""tranread.py RAW NODE T...: a VACASK ascii transient raw file's NODE voltage at each time T
(seconds, SPICE suffixes n p u m allowed), linearly interpolated between the two points around it."""
import sys
SUF = {'p': 1e-12, 'n': 1e-9, 'u': 1e-6, 'm': 1e-3}
lines = open(sys.argv[1]).read().split('\n')
n = int(next(l for l in lines if l.startswith('No. Variables:')).split(':')[1])
k = next(i for i, l in enumerate(lines) if l.startswith('Variables:'))
names = [lines[k + 1 + j].split()[1] for j in range(n)]
k = next(i for i, l in enumerate(lines) if l.startswith('Values:')) + 1
flat = [l.split()[-1] for l in lines[k:] if l.strip()]
pts = [list(map(float, flat[i:i + n])) for i in range(0, len(flat) - n + 1, n)]
ti, vi = names.index('time'), names.index(sys.argv[2])
for t in sys.argv[3:]:
    x = float(t[:-1]) * SUF[t[-1]] if t[-1] in SUF else float(t)
    for a, b in zip(pts, pts[1:]):
        if a[ti] <= x <= b[ti]:
            w = (x - a[ti]) / (b[ti] - a[ti]) if b[ti] > a[ti] else 0.0
            print(t, repr(a[vi] + w * (b[vi] - a[vi])))
            break
