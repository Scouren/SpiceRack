f33_noise_dec2_1k_5k: ngspice-45 .noise runs 1000, 3162.28, as .ac does
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.noise v(out) v1 dec 2 1k 5k
.end
