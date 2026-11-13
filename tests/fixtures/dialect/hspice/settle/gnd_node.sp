gnd_node
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1e-3: gnd is ground
v1 1 0 1
r1 1 gnd 1k
r2 gnd 0 1k
.dc v1 1 1 1
.end
