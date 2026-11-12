unread
* oracle: rewrite/AMEND-A12.md A17 (Q4: pwr, freq, phase Unread), vsrcpar.c (TRRANDOM (int) type; VSRC_R below -0.5 is no repeat; portnum an integer, a port only above 0)
.param t=1 rr=0
V1 a 0 dc 1 portnum 1 pwr 1m freq 1g
V2 b 0 trrandom({t} 1u)
V3 c 0 trrandom(5 1u)
V4 d 0 pwl(0 0 1u 1 2u 0) r={rr}
V5 e 0 r=0 pwl(0 0 1u 1 2u 0)
V6 f 0 dc 3 portnum 0
.end
