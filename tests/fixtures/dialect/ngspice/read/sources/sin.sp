sin
* oracle: vsrcload.c SINE (before TD: VO + VA*sin(PHASE), so HoldDelayValue; freq zero or omitted is 1/TSTOP); run `sin(0.5 1 1k 1m 0 30)` op 1.0; run `sin(0 1 0)` peaks at tstop/4; extra values ignored (run)
V1 a 0 sin(0.5 1 1k 1m 0 30)
V2 b 0 SINE(0 1 0)
V3 c 0 sin(0 1 1k 0 0 0 7 8)
.control
op
print v(a) v(b) v(c)
tran 1u 10u
print v(b)[10] v(c)[10]
.endc
.end
