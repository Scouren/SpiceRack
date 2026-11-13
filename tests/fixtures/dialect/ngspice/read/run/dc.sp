dc
* oracle: ngspice-45 inp2dot.c dot_dc (name start stop step, a second axis outer), dctrcurv.c:89-149 (a resistor, a V or I source, temp)
v1 a 0 1
R2 a 0 1k
i1 a 0 1m
.dc v1 0 5 0.1
.dc V1 -1 1 0.5 R2 1k 2k 1k
.dc temp -40 85 25
.dc i1 0 {imax} 1m
