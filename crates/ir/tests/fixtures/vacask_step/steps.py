"""steps.py RAW: the time points of a VACASK ascii transient raw file: count, first step, largest
step, the median step (s)."""
import sys
lines = open(sys.argv[1]).read().split('\n')
n = int(next(l for l in lines if l.startswith('No. Variables:')).split(':')[1])
k = next(i for i, l in enumerate(lines) if l.startswith('Variables:'))
names = [lines[k + 1 + j].split()[1] for j in range(n)]
k = next(i for i, l in enumerate(lines) if l.startswith('Values:')) + 1
flat = [l.split()[-1] for l in lines[k:] if l.strip()]
t = [float(flat[i + names.index('time')]) for i in range(0, len(flat) - n + 1, n)]
h = [b - a for a, b in zip(t, t[1:])]
s = sorted(h)
print(f"points={len(t)} first={h[0]:.4g} max={max(h):.4g} median={s[len(s)//2]:.4g}")
