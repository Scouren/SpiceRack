f32_ac_lin0: ngspice-45 `.ac lin 0 1k 2k` warns "ngspice assumes default parameter(s)" and runs ten frequencies, 1k to 2k
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.ac lin 0 1k 2k
.end
