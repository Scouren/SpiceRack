capacitor_inductor
* oracle: ngspice-45 inp2c.c, inp2l.c (value, model either side: run f2), cap.c, ind.c (keys); INV-semantics 8.21 (device ic only under uic)
C1 a 0 1p
C2 a 0 2p ic=0.5 m=3 temp=40 dtemp=1 tc1=1m tc2=1u scale=2
C3 a 0 cmod 3p w=1u l=2u
L1 a 0 1u
L2 a 0 2u ic=1m m=2 nt=10
L3 a 0 lmod 3u
