numparam_round
* settle -b (set numdgt=17): v(a) = v(b) = 1.04533851412886003e+00 (quoted, braced: numparam prints its result '% 23.15e', xpressn.c:1176, and the card reads that text), v(c) = 1.04533851412886047e+00 (bare), v(d) = 1.04533851412886003e+00; spice-bench tests/regression/parser/xpressn-1.sp v1061_g
v1 a 0 '1.0453385141288605'
v2 b 0 {1.0453385141288605}
v3 c 0 1.0453385141288605
v4 d 0 1.04533851412886
r1 a 0 1
r2 b 0 1
r3 c 0 1
r4 d 0 1
.control
set numdgt=17
op
print v(a) v(b) v(c) v(d)
.endc
.end
