tss_mid_uic: ngspice-45 `.tran 0.3m 1m 0 10u uic`: 138 points (the uic breakpoint at 0.3m)
v1 in 0 pulse(0 1 50u 10u 10u 200u 500u)
r1 in out 1k
c1 out 0 100n
d1 out 0 dmod
.model dmod d is=1e-14
.tran 0.3m 1m 0 10u uic
.end
