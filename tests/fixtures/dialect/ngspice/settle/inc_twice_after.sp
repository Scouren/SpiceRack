ita: a subcircuit and a parameter defined in the deck after the .include that defines them
* settle (R71 case 4): -b: 'Warning: redefinition of .subckt sub, ignored', i(v1) = -1.50000e-03 (the include's sub 2k and the deck's p 1k): the first .subckt and the last .param of the expanded deck
.include inc_twice.inc
.subckt sub a
R1 a 0 1k
.ends
.param p=1k
V1 n1 0 1
X1 n1 sub
R2 n1 0 {p}
.control
op
print i(v1)
.endc
.end
