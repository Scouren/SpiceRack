"""rawc.py RAW NAME: the last point of a VACASK ascii raw file's complex variable NAME: re im."""
import sys
L = open(sys.argv[1]).read().split('\n')
n = int(next(l for l in L if l.startswith('No. Variables:')).split(':')[1])
k = next(i for i, l in enumerate(L) if l.startswith('Variables:'))
names = [L[k + 1 + j].split()[1] for j in range(n)]
k = next(i for i, l in enumerate(L) if l.startswith('Values:')) + 1
flat = [l.split()[-1] for l in L[k:] if l.strip()]
v = flat[-n + names.index(sys.argv[2])]
print(*v.split(','))
