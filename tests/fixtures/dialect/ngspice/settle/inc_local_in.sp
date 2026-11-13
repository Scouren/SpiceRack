ili: a .include inside a .subckt body, its subcircuit used inside
* settle (R71 case 2): -b: i(v1) = -7.50000e-04 (X1's 2k and R2 = pl = 4k): an include inside a .subckt body is read there
.subckt wrap a
.include inc_local.inc
X1 a sub
R2 a 0 {pl}
.ends
V1 n1 0 1
Xw n1 wrap
.control
op
print i(v1)
.endc
.end
