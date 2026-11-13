ac_oct
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 1000, 1259.92, 1587.40, 2000, ... (as ngspice)
v1 1 0 dc 0 ac 1
r1 1 2 1k
c1 2 0 1u
.ac oct 3 1k 5k
.end
