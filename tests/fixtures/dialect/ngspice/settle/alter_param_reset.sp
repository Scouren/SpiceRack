ap: alterparam then op, reset
* settle (R70, A43; F7): -b, numdgt=8: v(n) = 1.00000000e+00, then 'Reset re-loads circuit …' and v(n) = 2.00000000e+00: reset applies the alterparam (R49)
.param a=1
V1 n 0 {a}
R1 n 0 1k
.control
set numdgt=8
op
print v(n)
alterparam a=2
reset
op
print v(n)
.endc
.end
