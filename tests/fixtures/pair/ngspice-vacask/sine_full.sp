sine_full: a SIN with every argument (offset, amplitude, frequency, delay, damping, phase) into an RC low-pass, tau 10 ns
v1 in 0 dc 0 sin(0.2 1 50meg 20n 1e7 45)
r1 in out 1k
c1 out 0 10p
.tran 0.1n 200n
.end
