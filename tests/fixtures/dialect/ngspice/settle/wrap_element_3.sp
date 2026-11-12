welement3: an element value: 'a+{b}'
* settle (audit row 4): -b: i(v1) = -3.3333333333333326e-01 (R1 = 3): a brace inside the quotes is a parenthesis of the one expression
.param a=1 b=2
V1 n 0 1
R1 n 0 'a+{b}'
.control
set numdgt=17
op
print i(v1)
.endc
.end
