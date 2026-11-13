ilp: a .include inside a .subckt body, its .param read at the top
* settle (R71 case 2): -b: 'Undefined parameter [pl]', 'Cannot compute substitute', fatal, exit 1: its .param is local too
.subckt wrap a
.include inc_local.inc
.ends
V1 n1 0 1
Xw n1 wrap
R3 n1 0 {pl}
.control
op
print i(v1)
.endc
.end
