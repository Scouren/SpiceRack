ac_dec_offgrid: F33: an RC low-pass, .ac dec 2 1k 5k (stop off the grid): ngspice-45 runs 1000 and 3162.28, VACASK on the deck written for it stretches the grid to end on 5 kHz (1000, 2236.07, 5000)
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.ac dec 2 1k 5k
.end
