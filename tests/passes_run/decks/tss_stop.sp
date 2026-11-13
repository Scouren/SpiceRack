tss_stop: ngspice-45 `.tran 1m 1m 0 10u` (the step at the stop): 136 points, identical to 0.3m and 2m
v1 in 0 pulse(0 1 50u 10u 10u 200u 500u)
r1 in out 1k
c1 out 0 100n
d1 out 0 dmod
.model dmod d is=1e-14
.tran 1m 1m 0 10u
.end
