end_stops
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1e-3: a card after .end is not read
v1 1 0 1
r1 1 0 1k
.dc v1 1 1 1
.end
r2 1 0 1k
.end
