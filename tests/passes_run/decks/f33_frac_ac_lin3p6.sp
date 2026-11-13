f33_frac_ac_lin3p6: ngspice-45 `.ac lin 3.6 1k 2k` runs 4 points: the count rounds
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac lin 3.6 1k 2k
.end
