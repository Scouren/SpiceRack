exp
* oracle: vsrcload.c EXP (TD1, TAU1, TAU2 zero or omitted are TSTEP, TD2 is TD1 + TSTEP); run `exp(0 1 0 0 0 0)` under .tran 1u: 0.632 at 2u
V1 a 0 exp(0 1 0 0 0 0)
V2 b 0 exp(1 2)
V3 c 0 exp(0 1 1u 1u 3u 1u)
.control
op
print v(a) v(b) v(c)
tran 1u 6u
print v(a)[20] v(b)[20] v(c)[20]
.endc
.end
