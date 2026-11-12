um: a model parameter reads a parameter nothing defines
* settle (R63, A38): -b: 'Netlist line no. 5:', 'Undefined parameter [u]', 'Cannot compute substitute', 'ERROR: fatal error in ngspice, exit(1)': ngspice rejects the complete deck
V1 a 0 1
D1 a 0 dmod
.model dmod d is={u}
.control
op
print i(v1)
.endc
.end
