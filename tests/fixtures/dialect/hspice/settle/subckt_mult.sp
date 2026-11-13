subckt_mult
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1e-3: m=5 on the X line is ignored, one copy (ngspice twin subckt_mult.ng.cir: -5e-3; F67)
.subckt cell a b
r1 a b 1k
.ends
v1 1 0 1
x1 1 0 cell m=5
.dc v1 1 1 1
.end
