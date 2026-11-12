mosfet_jfet_mesfet
* oracle: ngspice-45 inp2m.c (3 to 7 nodes, no unlabeled number), inp2j.c, inp2z.c (model 4th), mos1.c (temp is the instance temperature: run mm, MOS1 temp=50 runs; on BSIM4, b4.c, it is unknown parameter, a fact of the bound model), jfet.c, mes.c
M1 d g s b nmos w=1u l=100n m=2 off ic=1,2,3 temp=30
M2 d g s nvd
J1 d g s jmod 2 off ic=1,2
Z1 d g s zmod area=2
