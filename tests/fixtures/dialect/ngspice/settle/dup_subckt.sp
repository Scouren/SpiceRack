dup_subckt
* settle -b: 'Warning: redefinition of .subckt cell, ignored', 'i(v1) = -1.00000e-03' (the first cell, 1k): ngspice keeps the first .subckt of a name in a scope (inpcom.c), R1001
.subckt cell 1 2
r1 1 2 1k
.ends
.subckt cell 1 2
r1 1 2 4k
.ends
x1 n 0 cell
v1 n 0 1
.control
op
print i(v1)
.endc
.end
