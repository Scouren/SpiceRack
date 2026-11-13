iso: an X card before the .include that defines its subcircuit, as many nodes as ports
* settle (the later-file subcircuit binding; R61, A36): -b: i(v1) = -5.00000e-04 (R1 = 2k of the included sub): ngspice binds an X card to a .subckt of a file included after it
V1 n1 0 1
X1 n1 sub
.include include_subckt.inc
.control
op
print i(v1)
.endc
.end
