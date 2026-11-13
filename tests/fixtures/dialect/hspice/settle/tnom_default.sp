tnom_default
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -6.79912802e-03: TNOM and TEMP default to 25 C (with 27: tnom_27)
v1 1 0 0.7
d1 1 0 dm
.model dm d is=1e-14
.dc v1 0.7 0.7 1
.end
