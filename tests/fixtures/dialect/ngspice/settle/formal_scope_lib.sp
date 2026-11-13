formal_scope_lib
* settle -b: 'i(v1) = -5.00000e-01' (a's r1 = 2*x = 2: its own formal w=1), 'i(v2) = -8.33333e-02' (b's r1 = 3*y = 12: its w=4 at x2), 'v(k) = 9.000000e+00' (t reads the top w): a formal binds in its own subckt, also inside a .lib section (F34)
.param w=9
.lib formal_scope.lib sec
.param t={w}
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
