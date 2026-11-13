f33_ac_oct3_1k_5k: ngspice-45 runs 7 frequencies, 1000, 1259.92, 1587.4, 2000, 2519.84, 3174.8, 4000: the exact 2^(1/3) grid, stop not reached
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.ac oct 3 1k 5k
.end
