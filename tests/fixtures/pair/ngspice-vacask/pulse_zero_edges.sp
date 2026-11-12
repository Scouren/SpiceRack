pulse_zero_edges: a PULSE with rise and fall given as zero (ngspice takes TSTEP for each: given_zero_to_as_given) into an RC low-pass, tau 1 ns
v1 in 0 dc 0 pulse(0 1 2n 0 0 5n 20n)
r1 in out 1k
c1 out 0 1p
.tran 0.1n 40n
.end
