dup_func_inc_after
* settle -b: 'v(n) = 2.000000e+00' (the deck's f, x*2, after the include): the last .func in the expanded text wins
.include dup_func.inc
.func f(x) {x*2}
v1 n 0 {f(1)}
r1 n 0 1
.control
op
print v(n)
.endc
.end
