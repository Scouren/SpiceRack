ia: an X card after the .include that defines its subcircuit
* settle (R71 case 1): -b: i(v1) = -5.00000e-04 (the included sub's 2k): a subcircuit of a file included before the card binds
.include include_subckt.inc
V1 n1 0 1
X1 n1 sub
.control
op
print i(v1)
.endc
.end
