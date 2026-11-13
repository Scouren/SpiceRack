f33_frac_ac_dec_param2p5: ngspice-45 `.ac dec {n} 1k 100k` with `.param n=2.5` runs 3 per decade (7 points), as the literal 2.5 does: rounds half up, no message
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.param n=2.5
.ac dec {n} 1k 100k
.end
