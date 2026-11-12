us: a subcircuit body reads a parameter nothing defines
* settle (R63, A38): -b: 'Netlist line no. 4:', 'Undefined parameter [u]', 'Cannot compute substitute', 'ERROR: fatal error in ngspice, exit(1)': ngspice rejects the complete deck, though no instance scope defines u either
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
