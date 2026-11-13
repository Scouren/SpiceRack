ac_lin_two: F32 (R97): an RC low-pass, .ac lin 2 from 1 kHz to 2 kHz: ngspice-45 runs 1 kHz alone (acan.c: a linear sweep of two points has no step); the VACASK twin of vacask-ngspice/ac_lin_two
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.ac lin 2 1k 2k
.end
