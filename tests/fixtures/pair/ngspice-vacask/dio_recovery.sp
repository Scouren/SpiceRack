dio_recovery: F52 (R84a): the VACASK corpus's demo/spice/dio.sim written for ngspice, a diode's reverse recovery (tt=1u, cjo=50n) from 1 V to -1 V at 0.2 us; refined to span/2000 the two still differ (i(v1) at 0.5 us: ngspice-45 1.1157, VACASK 1.2138 A), so the harness's refinement does not class it
.model d d rs=1.0 is=1e-9 tt=1e-6 cjo=50n bv=5 ibv=1e-6
v1 1 0 pulse(1 -1 0.2u 1n 1n 1 2)
d1 1 0 d
.tran 0.1u 1u
.end
