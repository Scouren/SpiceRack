dup_func
* settle -b: 'v(n) = 5.000000e+00' (the second f, x*5), no warning: ngspice keeps the last .func of a name, R1001
.func f(x) {x*2}
.func f(x) {x*5}
v1 n 0 {f(1)}
r1 n 0 1
.control
op
print v(n)
.endc
.end
