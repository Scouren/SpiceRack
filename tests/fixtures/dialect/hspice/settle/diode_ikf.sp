diode_ikf
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -5.39777284e-03 (ngspice twin: the same; the ikf law is ngspice's)
.option tnom=27
.temp 27
v1 1 0 0.75
d1 1 0 dm
.model dm d is=1e-14 ikf=1m
.dc v1 0.75 0.75 1
.end
