noise
* oracle: vsrcpar.c TRNOISE, TRRANDOM (type (int) of the value; td 0, p1 1, p2 0 when omitted), 1-f-code.c:239 (before the first draw the value is p2); runs: `trrandom(1 1m 0 1 0.5)` op 0.5, `1 trnoise(1 1u 0 0 1 1u 1u)` op 1, `trnoise(1 1u)` op 0
V1 a 0 trrandom(1 1m 0 1 0.5)
V2 b 0 1 trnoise(1 1u 0 0 1 1u 1u)
V3 c 0 trnoise(1 1u)
V4 d 0 trrandom(2.7 1u)
.control
op
print v(a) v(b) v(c) v(d)
.endc
.end
