amd: an alter of a device no statement defines
* settle (R70, A43; F7): -b, numdgt=8: 'Error: no such device or model name __topinst__', 'Error: no such device or model name nope', v(n) = 1.00000000e+00 after each op, exit 0: ngspice reports and runs on at the old values
.param a=1
V1 n 0 {a}
R1 n 0 1k
.control
set numdgt=8
op
print v(n)
alter @__topinst__[a]=2
op
print v(n)
alter nope 2
op
print v(n)
.endc
.end
