noise
* oracle: ngspice-45 inp2dot.c dot_noise (v(out[,ref]) src DEC|OCT|LIN np fstart fstop [ptspersum]); manual 15.3.4 (input noise to a unit source)
.noise v(out) v1 dec 10 1 1meg
.noise v(a,b) vin lin 5 1 10 2
