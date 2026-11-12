lines
* oracle: ngspice-45 inp2t.c, tra.c, trasetup.c (z0 required; nl 0.25, f 1e9 when not given), inp2o.c, inp2u.c, urc.c (l, n), inp2y.c (len), inp2p.c (len, the conductor count)
T1 a 0 b 0 z0=50 td=1n
T2 a 0 b 0 zo=75 f=1g nl=0.5
T3 a 0 b 0 z0=50
O1 a 0 b 0 lmod
U1 a b 0 umod l=1m n=5
Y1 a 0 b 0 ymod len=2
P1 a1 a2 0 b1 b2 0 pmod len=1
