options_probe: a pulse into an RC, a diode through an inductor to a load, a level-1 MOSFET with no W, L, AD or AS; op, dc, ac, tran
v1 in 0 dc 1 ac 1 pulse(0 1 1u 1u 1u 5u 10u)
r1 in mid 1k
c1 mid 0 1n
d1 mid out dmod
l1 out lo 10u
r2 lo 0 10k
vdd dd 0 dc 3
rd dd drn 10k
m1 drn mid 0 0 nmod
.model dmod d is=1e-14 cjo=1p
.model nmod nmos level=1 vto=0.5 kp=1e-4 cj=1e-4 cjsw=1e-10
.op
.dc v1 0 2 0.1
.ac dec 5 1k 1g
.tran 0.1u 20u
.four 100k v(mid)
*OPTION
.end
