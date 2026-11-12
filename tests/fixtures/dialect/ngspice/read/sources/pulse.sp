pulse
* oracle: vsrcload.c PULSE (positional V1 V2 TD TR TF PW PER NP; a zero or omitted TR, TF is TSTEP, PW, PER TSTOP, evaluated after numparam: run z=0, rise 1u under .tran 1u); A17 (a field holds what the card holds)
.param z=0 tr=1u
V1 a 0 pulse(0 1 0 1n 1n 5u 10u)
V2 b 0 PULSE 0,1,0,{z},{z},5u,10u
V3 c 0 pulse(0 1)
V4 d 0 pulse(0 1 0 tr tr 1u 2u 3)
V5 e 0 dc 0.5 pulse(1 2 -1u 0 0 0 0)
.control
tran 1u 20u
print v(a)[3] v(b)[3] v(c)[3] v(d)[3] v(e)[3] v(a)[10] v(b)[10] v(c)[10] v(d)[10] v(e)[10] v(d)[40]
op
print v(a) v(e)
.endc
.end
