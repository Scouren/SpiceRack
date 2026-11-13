f33_frac_ac_dec2p4: ngspice-45 `.ac dec 2.4 1k 100k` runs 2 per decade (1000, 3162.28, ..., 100000: 5 points): the count rounds
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac dec 2.4 1k 100k
.end
