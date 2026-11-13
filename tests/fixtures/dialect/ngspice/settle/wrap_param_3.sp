wparam3: a .param value: 'a+{b}'
* settle (audit row 4): -b: 'Netlist line no. 4:', 'Syntax error: letter [{]', 'Expression err: a+{b', ' Formula() error.', '|.param p={a+{b}}| : |p|=|a+{b|', then 'Undefined parameter [p]', fatal (invalid in source)
.param a=1 b=2
.param p='a+{b}'
V1 n 0 1
R1 n 0 {p}
.control
set numdgt=17
op
print i(v1)
.endc
.end
