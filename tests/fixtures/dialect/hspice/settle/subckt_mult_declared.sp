subckt_mult_declared
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1e-3: with m declared by the .subckt as well
.subckt cell a b m=1
r1 a b 1k
.ends
v1 1 0 1
x1 1 0 cell m=5
.dc v1 1 1 1
.end
