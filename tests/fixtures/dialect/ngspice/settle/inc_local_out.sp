ilo: a .include inside a .subckt body, its subcircuit used at the top
* settle (R71 case 2): -b: 'Error: unknown subckt: x2 n1 sub', 'Simulation interrupted due to error!', exit 1: an include inside a .subckt body is local to it
.subckt wrap a
.include inc_local.inc
.ends
V1 n1 0 1
Xw n1 wrap
X2 n1 sub
.control
op
print i(v1)
.endc
.end
