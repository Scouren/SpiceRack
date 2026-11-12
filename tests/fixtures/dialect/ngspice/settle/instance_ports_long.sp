il: an instance with more nodes than its subcircuit's ports
* settle (R61, A36): -b: 'Too many parameters for subcircuit type "sub" (instance: xx1)', 'Simulation interrupted due to error!', exit 1; the neighbour with X1 n1 sub prints i(v1) = -1.00000e-03
.subckt sub a
R1 a 0 1k
.ends
V1 n1 0 1
R2 n2 0 1k
X1 n1 n2 sub
.control
op
print v(n1)
.endc
.end
