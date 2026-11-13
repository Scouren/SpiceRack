tss_span100: ngspice-45 `.tran 10u 1m 0 10u` (stop/100): 136 points, other doubles (the first step 10u/10 against (1m/100)/10)
v1 in 0 pulse(0 1 50u 10u 10u 200u 500u)
r1 in out 1k
c1 out 0 100n
d1 out 0 dmod
.model dmod d is=1e-14
.tran 10u 1m 0 10u
.end
