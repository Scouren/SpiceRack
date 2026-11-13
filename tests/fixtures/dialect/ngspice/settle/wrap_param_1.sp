wparam1: a .param value: {a}+{b}
* settle (audit row 4): -b: 'Netlist line no. 4:', ' ; sign expected.', 'ERROR: fatal error in ngspice, exit(1)' (invalid in source)
.param a=1 b=2
.param p={a}+{b}
V1 n 0 1
R1 n 0 {p}
.control
set numdgt=17
op
print i(v1)
.endc
.end
