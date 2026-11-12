wsource1: a source field: {a}+{b}
* settle (audit row 4): -b: 'Error on line 4 or its substitute:', 'v1 n 0    1.000000000000000e+00    +    2.000000000000000e+00', 'unknown parameter (+)', 'Simulation interrupted due to error!' (invalid in source)
.param a=1 b=2
V1 n 0 {a}+{b}
R1 n 0 1
.control
set numdgt=17
op
print v(n)
.endc
.end
