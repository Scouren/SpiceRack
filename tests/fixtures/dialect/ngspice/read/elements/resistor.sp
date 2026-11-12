resistor
* oracle: ngspice-45 inp2r.c (value, model, unlabeled number, tc=a b), res.c (keys), restemp.c:79 (no value: 1 mOhm); run f1 (R4 c 0 rmod 2k reads 2k, R2 l=1u is 1 mOhm)
R1 a b 1k
R2 a b r=2k tc1=1m tc2=2u m=2
R3 a b 3k rmod l=1u w=2u
R4 a b rmod 2k
R5 a b 4k tc=1m 2u temp=50 dtemp=2 ac=5k scale=2 noisy=0 tce=0.5
R6 a b l=1u
