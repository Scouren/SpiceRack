ac_phase: an AC source with a magnitude and a phase (2 V at 45 degrees) into an RC low-pass, and a VCCS driven from its output
v1 in 0 dc 0 ac 2 45
r1 in out 1k
c1 out 0 1n
g1 0 g out 0 1m
rg g 0 1k
.ac dec 5 1k 10meg
.end
