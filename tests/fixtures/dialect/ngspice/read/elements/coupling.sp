coupling
* oracle: ngspice-45 inp2k.c (K L1 L2 k), ind.c (k, coefficient), inpcom.c inp_compat (three or more inductors: every pair); INV-semantics 6.18
L1 a 0 1u
L2 b 0 1u
L3 c 0 1u
K1 L1 L2 0.5
K2 L1 L2 L3 k=0.9
