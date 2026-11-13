ac_dec_grid
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 34 points, 10, 12.5902, ..., 20000: ngspice's floor-stretched grid (F33)
v1 1 0 dc 0 ac 1
r1 1 2 1k
c1 2 0 1u
.ac dec 10 10 20k
.end
