f33_frac_ac_lin_param2p5: ngspice-45 `.ac lin {n} 1k 2k` with `.param n=2.5` runs 1000, 1500, 2000, as the literal does
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.param n=2.5
.ac lin {n} 1k 2k
.end
