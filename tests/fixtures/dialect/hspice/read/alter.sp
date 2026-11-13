alter
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): two plots: -1e-3, then (alter=1) -5e-4: the .alter block's .param reruns the analysis
.param rr=1k
v1 1 0 1
r1 1 0 'rr'
.dc v1 1 1 1
.alter
.param rr=2k
.end
