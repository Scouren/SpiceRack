f32_noise_lin2: ngspice-45 `.noise v(out) v1 lin 2 1k 2k` runs two frequencies, 1k and 2k (noise does not share F32)
v1 in 0 dc 0 ac 1
r1 in out 1k
c1 out 0 1u
.noise v(out) v1 lin 2 1k 2k
.end
