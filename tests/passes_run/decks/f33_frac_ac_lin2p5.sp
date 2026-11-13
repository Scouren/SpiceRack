f33_frac_ac_lin2p5: ngspice-45 `.ac lin 2.5 1k 2k` runs 1000, 1500, 2000: the count rounds half up to 3, no message
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac lin 2.5 1k 2k
.end
