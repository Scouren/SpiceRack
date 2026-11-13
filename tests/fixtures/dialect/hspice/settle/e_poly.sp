e_poly
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): v(a) = 17: poly(1) x 0 1 2 3 at v(x) = 2
vx x 0 2
e1 a 0 poly(1) x 0 1 2 3
ra a 0 1k
.dc vx 2 2 1
.end
