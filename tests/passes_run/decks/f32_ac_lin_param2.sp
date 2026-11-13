f32_ac_lin_param2: ngspice-45 `.ac lin {n} 1k 2k` with n = 2 runs one frequency, 1k, as the literal does
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.param n=2
.ac lin {n} 1k 2k
.end
