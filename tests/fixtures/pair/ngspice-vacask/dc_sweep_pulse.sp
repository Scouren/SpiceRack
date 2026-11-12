dc_sweep_pulse: a DC sweep of a source that has a PULSE (ngspice sweeps its DC value; VACASK ignores the dc of a waveform source, so R64a: the writer must refuse it, never sweep it)
v1 in 0 dc 0 pulse(0 1 1n 1n 1n 5n 20n)
r1 in out 1k
r2 out 0 3k
.dc v1 0 5 0.25
.end
