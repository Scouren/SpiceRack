f33_ac_dec2_1k_5k: ngspice-45 runs 1000, 3162.28: under one decade the exact 10^(1/2) grid from start, stop not reached
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.ac dec 2 1k 5k
.end
