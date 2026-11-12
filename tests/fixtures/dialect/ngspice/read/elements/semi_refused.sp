semi_refused
* oracle: ngspice-45 inp2m.c (no unlabeled parameter permitted on mosfet), inp2z.c (the unlabeled number depends on the model type); runs mm: area and mult on an M, initial on a D are unknown parameter (no ngspice device has the key); runs jj: a J card with fewer than three nodes before its model is Unable to find definition of model (the mutation law's shrunk inputs)
M1 d g s b nmos 2
Z1 d g s zmod 2
M2 d g s b nmos area=2
M3 d g s b nmos mult=2
D1 a k dmod initial=1
J1 njf beta=1e-4 vto=-2
J2
