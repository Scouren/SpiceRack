uo: the legal neighbour of undef_*.sp: u defined at the top
* settle (R63, A38): -b: i(v1) = -5.00000e-04 (R1 = u = 2k, read in the subcircuit body from the top)
.param u=2k
.subckt sub a
R1 a 0 {u}
.ends
V1 a 0 1
X1 a sub
.control
op
print i(v1)
.endc
.end
