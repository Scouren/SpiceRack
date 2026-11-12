bsource_refused
* oracle: ngspice-45 asrcload.c:41-52 (tc from 300.15 K; m multiplies the value: run f5 B2 d 0 V=1 m=2 is 2 V), asrc.c (reciproctc)
B1 a 0 V=1 m=2
B2 b 0 I=1 tc1=1m
B3 c 0 I=1 reciproctc=1
