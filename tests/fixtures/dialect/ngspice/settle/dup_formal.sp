dup_formal
* settle -b: 'i(v1) = -3.33333e-01' (x1: the body .param w=3 replaces the default 1), 'i(v2) = -2.00000e-01' (x2's w=5 still wins): a .param naming its subckt's formal changes the default only, R1002
.subckt cell 1 2 w=1
.param w=3
r1 1 2 {w}
.ends
x1 n 0 cell
x2 m 0 cell w=5
v1 n 0 1
v2 m 0 1
.control
op
print i(v1) i(v2)
.endc
.end
