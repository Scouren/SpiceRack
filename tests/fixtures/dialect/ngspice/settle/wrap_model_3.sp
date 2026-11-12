wmodel3: a model parameter: 'a+{b}'
* settle (audit row 4): -b: 'Netlist line no. 6:', 'Syntax error: letter [{]', 'Expression err: a+{b}}', 'Cannot compute substitute', 'ERROR: fatal error in ngspice, exit(1)': a brace inside the quotes is an error here (invalid in source)
.param a=1 b=2
V1 n 0 1
R1 n 0 rm l=1 w=1
.model rm r rsh='a+{b}'
.control
set numdgt=17
op
print i(v1)
.endc
.end
