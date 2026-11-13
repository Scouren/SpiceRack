e_max
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): v(a) = 7: vol='v(x)*5' max=7 clamps
vx x 0 2
e1 a 0 vol='v(x)*5' max=7
ra a 0 1k
.dc vx 2 2 1
.end
