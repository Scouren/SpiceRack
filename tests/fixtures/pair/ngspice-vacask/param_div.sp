param_div: parameter expressions with a division of two whole numbers (ngspice: real, 1/4 = 0.25; VACASK: integer unless div_to_div_typed types it), as a resistor value
.param num=1 den=4 rbase=4k
v1 in 0 dc 1
r1 in out {rbase*num/den}
r2 out 0 1k
.op
.end
