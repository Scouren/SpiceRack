f33_frac_ac_dec_param2p4: ngspice-45 `.ac dec {n} 1k 100k` with `.param n=2.4` runs 2 per decade (5 points), as the literal does
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.param n=2.4
.ac dec {n} 1k 100k
.end
