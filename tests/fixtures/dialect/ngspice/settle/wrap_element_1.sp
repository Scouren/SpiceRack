welement1: an element value: {a}+{b}
* settle (audit row 4): -b: 'Error on line 5 or its substitute:', 'r1 n 0    1.000000000000000e+00    +    2.000000000000000e+00', 'unknown parameter (+)', 'Simulation interrupted due to error!': numparam replaces each {…} on its own, and the card reads 1 then a stray + (invalid in source)
.param a=1 b=2
V1 n 0 1
R1 n 0 {a}+{b}
.control
set numdgt=17
op
print i(v1)
.endc
.end
