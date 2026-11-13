ac_poi
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): exit 1, InvalidAnalysisArguments: '.ac poi 3 1k 3k 2k', a list not increasing, is refused
v1 1 0 dc 0 ac 1
r1 1 2 1k
c1 2 0 1u
.ac poi 3 1k 3k 2k
.end
