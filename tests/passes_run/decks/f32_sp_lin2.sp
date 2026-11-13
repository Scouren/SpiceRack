f32_sp_lin2: ngspice-45 `.sp lin 2 1k 2k` runs one frequency (No. Points: 1; `lin 3` runs 3)
v1 in 0 dc 0 ac 1 portnum 1 z0 50
v2 out 0 dc 0 ac 0 portnum 2 z0 50
r1 in out 1k
.sp lin 2 1k 2k
.end
