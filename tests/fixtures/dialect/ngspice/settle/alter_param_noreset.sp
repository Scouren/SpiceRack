ap: alterparam then op, noreset
* settle (R70, A43; F7): -b, numdgt=8: v(n) = 1.00000000e+00 then, after 'alterparam a=2' and op with no reset, v(n) = 1.00000000e+00 again, no message, exit 0: the alter is silently not applied
.param a=1
V1 n 0 {a}
R1 n 0 1k
.control
set numdgt=8
op
print v(n)
alterparam a=2
op
print v(n)
.endc
.end
