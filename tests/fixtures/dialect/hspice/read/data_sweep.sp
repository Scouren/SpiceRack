data_sweep
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): two plots, rr=1000: -1e-3, rr=4000: -2.5e-4: '.dc … sweep data=tab'
.param rr=1k
v1 1 0 1
r1 1 0 'rr'
.data tab rr
1k
4k
.enddata
.dc v1 1 1 1 sweep data=tab
.end
