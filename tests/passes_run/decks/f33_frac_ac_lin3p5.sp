f33_frac_ac_lin3p5: ngspice-45 `.ac lin 3.5 1k 2k` runs 1000, 1333.33, 1666.67, 2000: the count rounds to 4
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac lin 3.5 1k 2k
.end
