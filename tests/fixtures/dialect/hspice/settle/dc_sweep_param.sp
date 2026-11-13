dc_sweep_param
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): two plots, rr=1000: -1e-3, rr=2000: -5e-4: '.dc … sweep rr 1k 2k 1k'
.param rr=1k
v1 1 0 1
r1 1 0 'rr'
.dc v1 1 1 1 sweep rr 1k 2k 1k
.end
