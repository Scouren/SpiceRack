dup_func_inc_before
* settle -b: 'v(n) = 5.000000e+00' (the included f, x*5, defined after the deck's): the last .func in the expanded text wins, through an include too (visible.rs)
.func f(x) {x*2}
.include dup_func.inc
v1 n 0 {f(1)}
r1 n 0 1
.control
op
print v(n)
.endc
.end
