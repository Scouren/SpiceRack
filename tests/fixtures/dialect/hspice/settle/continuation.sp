continuation
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1.5e-3: '+' continues a line; a trailing '\' joins the next one
v1 1 0 1
r1 1 0
+ 1k
r2 1 0 \
2k
.dc v1 1 1 1
.end
