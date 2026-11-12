wmodel1: a model parameter: {a}+{b}
* settle (audit row 4): -b: 'Warning: Model issue on line 6 :', '.model rm r rsh=    1.000000000000000e+00    +    2.000000000000000e+00  ...', 'unrecognized parameter (+) - ignored', then i(v1) = -1.0000000000000000e+00: the deck runs with rsh = 1, the first brace only (a wrong number with a warning)
.param a=1 b=2
V1 n 0 1
R1 n 0 rm l=1 w=1
.model rm r rsh={a}+{b}
.control
set numdgt=17
op
print i(v1)
.endc
.end
