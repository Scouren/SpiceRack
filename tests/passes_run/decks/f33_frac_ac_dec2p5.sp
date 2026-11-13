f33_frac_ac_dec2p5: ngspice-45 `.ac dec 2.5 1k 100k` runs 3 per decade (1000, 2154.43, ..., 100000: 7 points): the count rounds half up, no message
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac dec 2.5 1k 100k
.end
