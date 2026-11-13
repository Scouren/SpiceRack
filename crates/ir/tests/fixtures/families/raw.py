"""raw.py FILE EXPR: the last point of a VACASK ascii raw file, EXPR over its variables:
`i:v1` the current of source v1 (`v1:flow(br)`), `v:a` a node voltage, `v:a-b` a difference;
complex values print as magnitude."""
import sys
lines = open(sys.argv[1]).read().split('\n')
names, k, n = [], 0, 0
while not lines[k].startswith('Variables:'):
    if lines[k].startswith('No. Variables:'):
        n = int(lines[k].split(':')[1])
    k += 1
for j in range(n):
    names.append(lines[k + 1 + j].split()[1])
k = next(i for i in range(len(lines)) if lines[i].startswith('Values:')) + 1
vals, pts = [], []
for l in lines[k:]:
    if not l.strip():
        continue
    parts = l.split()
    if len(vals) == 0 or len(vals) == n:
        if len(vals) == n:
            pts.append(vals)
        vals = [parts[-1]]
    else:
        vals.append(parts[-1])
if len(vals) == n:
    pts.append(vals)
last = dict(zip(names, pts[-1]))
def num(s):
    if ',' in s:
        re_, im = map(float, s.split(','))
        return (re_ * re_ + im * im) ** 0.5
    return float(s)
e = sys.argv[2]
kind, what = e.split(':', 1)
if kind == 'i':
    print(repr(num(last[what + ':flow(br)'])))
else:
    a, _, b = what.partition('-')
    print(repr(num(last[a]) - (num(last[b]) if b else 0.0)))
