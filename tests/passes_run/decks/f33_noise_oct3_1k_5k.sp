f33_noise_oct3_1k_5k: ngspice-45 .noise runs the same 7 frequencies as .ac oct 3 1k 5k
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.noise v(out) v1 oct 3 1k 5k
.end
