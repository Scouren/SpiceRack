itb: a subcircuit and a parameter defined in the deck before the .include that defines them again
* settle (R71 case 4): -b: 'Warning: redefinition of .subckt sub, ignored', i(v1) = -1.25000e-03 (the deck's sub 1k and the include's p 4k): in the expanded deck the first .subckt and the last .param win
.subckt sub a
R1 a 0 1k
.ends
.param p=1k
.include inc_twice.inc
V1 n1 0 1
X1 n1 sub
R2 n1 0 {p}
.control
op
print i(v1)
.endc
.end
