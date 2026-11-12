sffm
* oracle: vsrcload.c SFFM (VO VA FM MDI FC TD PHM PHC; 0 for t - TD <= 0); isrcload.c SFFM (no delay; 6th value PHM, 7th PHC); R36 (dc is the wave at t = 0); runs: V op 0 for td 0, 0.5 for td -1m; I `sffm(0.5 1 1k 2 10k 30)` op -1.34147 into 1 ohm
V1 a 0 sffm(0.5 1 1k 2 10k)
V2 b 0 sffm(0.5 1 1k 2 10k -1m)
I3 c 0 sffm(0.5 1 1k 2 10k 30)
R3 c 0 1
V4 d 0 sffm(0 1)
.control
op
print v(a) v(b) v(c) v(d)
tran 1u 100u
print v(a)[30] v(b)[30] v(c)[30]
.endc
.end
