tnom_27
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -5.67034677e-03: '.option tnom=27' and '.temp 27' give ngspice's 27 C value
.option tnom=27
.temp 27
v1 1 0 0.7
d1 1 0 dm
.model dm d is=1e-14
.dc v1 0.7 0.7 1
.end
