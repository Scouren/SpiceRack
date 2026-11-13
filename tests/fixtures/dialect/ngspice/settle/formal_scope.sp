formal_scope
* settle -b: 'i(v1) = -5.00000e-01' (a's r1 = 2: its own formal w=1), 'i(v2) = -8.33333e-02' (b's r1 = 12: its w=4 at x2), 'v(k) = 4.500000e+01' (t reads the top w=9): a formal binds in its own subckt only (F34)
.param w=9
.subckt a 1 2 w=1
.param x={w*2}
r1 1 2 {x}
.ends
.subckt b 1 2 w=1
.param y={w*3}
r1 1 2 {y}
.ends
.param t={w*5}
x1 n 0 a
x2 m 0 b w=4
v1 n 0 1
v2 m 0 1
vt k 0 {t}
.control
op
print i(v1) i(v2) v(k)
.endc
.end
