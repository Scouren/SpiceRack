pss
* oracle: ngspice-45 inp2dot.c dot_pss (fguess stabtime oscnode points harmonics sc_iter steady_coeff [uic], the node against ground), A16; run pss.cir
.pss 1k 1m b 64 10 50 1e-3
.pss 10meg 5u out 128 5 20 1e-4 uic
