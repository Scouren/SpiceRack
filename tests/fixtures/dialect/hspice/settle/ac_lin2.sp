ac_lin2
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 1000, 2000
v1 1 0 dc 0 ac 1
r1 1 2 1k
c1 2 0 1u
.ac lin 2 1k 2k
.end
