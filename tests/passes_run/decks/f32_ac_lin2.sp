f32_ac_lin2: ngspice-45 `.ac lin 2 1k 2k` runs one frequency, 1k (acan.c: delta 0 unless n - 1 > 1; F32, R97)
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac lin 2 1k 2k
.end
