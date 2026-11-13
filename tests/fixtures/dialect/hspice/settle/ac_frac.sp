ac_frac
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): exit 1, InvalidAnalysisArguments: a fractional count is refused
v1 1 0 dc 0 ac 1
r1 1 2 1k
c1 2 0 1u
.ac dec 2.5 1k 100k
.end
