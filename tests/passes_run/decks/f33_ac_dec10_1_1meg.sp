f33_ac_dec10_1_1meg: ngspice-45 runs 61 frequencies 1 ... 1e6 on the 10^(1/10) grid (stop on the grid: the same list as VACASK)
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.ac dec 10 1 1e6
.end
