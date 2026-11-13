res_params: a resistor's instance parameters move its current at 75 C: tc1=2e-3 raises 1k to 1.1k, m=2 halves it (i(v1) = -1/550 A); the same-dialect cell's planted fault drops tc1 here
.temp 75
v1 in 0 dc 1
r1 in 0 1k tc1=2e-3 m=2
.op
.end
