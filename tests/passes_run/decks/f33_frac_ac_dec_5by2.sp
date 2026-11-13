f33_frac_ac_dec_5by2: ngspice-45 `.ac dec {5/2} 1k 100k` runs 3 per decade (7 points): 5/2 is 2.5 there, then rounded half up
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
* inline expression
.ac dec {5/2} 1k 100k
.end
