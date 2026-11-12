bsource
* oracle: ngspice-45 inp2b.c, asrc.c (v i temp dtemp tc1 tc2 m), inpcom.c inp_bsource_compat (the B-source parser reads it), asrcload.c:41 (tc from 300.15 K: Fixed27), :49-52 (m times the value: run f5 B2 d 0 V=1 m=2 is 2 V, read as a product, R31); run f5 (B1 c 0 V=1 + 2 tc1=0 is 3 V)
B1 a 0 V=1 + 2
B2 b 0 I={v(a)*2} m=2
B3 c 0 v=v(a)*3 temp=50 dtemp=1
B4 d 0 V=1 m=2
B5 e 0 I=1 tc1=1m
