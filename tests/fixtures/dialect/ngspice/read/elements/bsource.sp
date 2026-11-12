bsource
* oracle: ngspice-45 inp2b.c, asrc.c (v i temp dtemp m), inpcom.c inp_bsource_compat (the B-source parser reads it); run f5 (B1 c 0 V=1 + 2 tc1=0 is 3 V)
B1 a 0 V=1 + 2
B2 b 0 I={v(a)*2} m=2
B3 c 0 v=v(a)*3 temp=50 dtemp=1
