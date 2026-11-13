f33_sp_dec2_1k_5k: ngspice-45 .sp runs 1000, 3162.28, as .ac does
v1 in 0 dc 0 ac 1 portnum 1 z0 50
v2 out 0 dc 0 ac 0 portnum 2 z0 50
r1 in out 1k
.sp dec 2 1k 5k
.end
