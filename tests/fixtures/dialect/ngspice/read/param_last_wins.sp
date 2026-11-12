param_last_wins
* oracle: INV-semantics 3.01 (ngspice keeps the last .param; a read between the two reads the last; run XY G4), ngspice-45 inpcom.c inp_sort_params
.param a=1
.param b={a*2}
.param a=3 c=4
