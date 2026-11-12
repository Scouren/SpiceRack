wsource3: a source field: 'a+{b}'
* settle (audit row 4): -b: v(n) = 3.00000000000000044e+00: a brace inside the quotes is a parenthesis of the one expression
.param a=1 b=2
V1 n 0 'a+{b}'
R1 n 0 1
.control
set numdgt=17
op
print v(n)
.endc
.end
