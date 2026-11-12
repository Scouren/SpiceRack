diode_bjt
* oracle: ngspice-45 inpcom.c get_number_terminals (which item is the model; run: D2 a k dmod 2 is could not find a valid modelname), inp2q.c (unlabeled number is area), dio.c, bjt.c (off ic area temp dtemp m); INV-semantics 6.26, 8.21
D1 a k dmod
D2 a k dmod 2
D3 a k t dmod
D4 a k dmod area=3 off ic=0.6 temp=50 dtemp=2 m=2 pj=1u
Q1 c b e qmod
Q2 c b e s qmod 1.5 off ic=0.7,0.3
Q3 c b e s t qmod
