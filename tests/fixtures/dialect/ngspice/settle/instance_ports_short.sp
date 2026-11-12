is: an instance with fewer nodes than its subcircuit's ports
* settle (R61, A36): -b: 'Too few parameters for subcircuit type "sub" (instance: xx1)', 'Simulation interrupted due to error!', exit 1: ngspice rejects the deck
.subckt sub a b
R1 a b 1k
.ends
V1 n1 0 1
R2 n1 0 1k
X1 n1 sub
.control
op
print v(n1)
.endc
.end
