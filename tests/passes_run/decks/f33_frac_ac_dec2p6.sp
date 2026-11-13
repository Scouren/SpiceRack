f33_frac_ac_dec2p6: ngspice-45 `.ac dec 2.6 1k 100k` runs 3 per decade (7 points): the count rounds
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac dec 2.6 1k 100k
.end
