f33_ac_dec10_10_20k: ngspice-45 runs 34 frequencies, 10, 12.5902, 15.8512 ... 15885.4, 20000: floor(10*log10(2000)) = 33 steps stretched to end on stop (acan.c DECADE)
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.ac dec 10 10 20k
.end
