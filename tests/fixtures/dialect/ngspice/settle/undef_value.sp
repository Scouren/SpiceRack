uv: an element value reads a parameter nothing defines
* settle (R63, A38): -b: 'Netlist line no. 4:', 'Undefined parameter [u]', 'Cannot compute substitute', 'ERROR: fatal error in ngspice, exit(1)': ngspice rejects the complete deck (the neighbour undef_ok.sp runs)
V1 a 0 1
R1 a 0 {u}
.control
op
print i(v1)
.endc
.end
