ac_dec_short
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 1000, 3162.28 (as ngspice)
v1 1 0 dc 0 ac 1
r1 1 2 1k
c1 2 0 1u
.ac dec 2 1k 5k
.end
